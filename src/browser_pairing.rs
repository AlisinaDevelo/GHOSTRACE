//! Explicit browser pairing and per-message replay protection.
//!
//! Installing the native-host manifest does not authorize anything: a browser
//! extension is trusted only after the user approves a pairing that names the
//! browser channel, profile class, extension ID, the digest of the extension's
//! public key, the requested event classes, the retained fields, and the
//! private-context policy. The approval yields a random pairing secret that is
//! handed to the extension once. Each session derives a fresh key from that
//! secret and two fresh nonces, and every message carries a MAC over its
//! sequence number and body, so a transcript from an earlier session never
//! verifies. A different extension ID is refused; a changed key or permission
//! set, an expired approval, or a revoked pairing requires re-pairing.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Utc};
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// How long an approval stays valid before the user must re-pair.
pub const PAIRING_LIFETIME_DAYS: i64 = 90;
const SESSION_KEY_DOMAIN: &[u8] = b"ghostrace-browser-session-v1\0";
const MESSAGE_MAC_DOMAIN: &[u8] = b"ghostrace-browser-message-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingError {
    #[error("the extension is not paired")]
    NotPaired,
    #[error("the pairing was revoked")]
    Revoked,
    #[error("the pairing must be approved again")]
    RePairingRequired,
    #[error("the message authentication code does not verify")]
    BadMac,
    #[error("the random source failed")]
    Random,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserEventClass {
    TopLevelNavigation,
    Bookmarks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileClass {
    Default,
    Named,
    Unknown,
}

/// Everything shown to the user before approval.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRequest {
    pub browser_channel: String,
    pub profile_class: ProfileClass,
    pub extension_id: String,
    /// SHA-256 of the extension's public key (the manifest `key`).
    pub extension_key_digest: String,
    /// SHA-256 of the extension's reviewed permission set.
    pub permissions_digest: String,
    pub event_classes: BTreeSet<BrowserEventClass>,
    pub retained_fields: Vec<String>,
    /// Always `refuse_private_context`; recorded so the user sees it.
    pub private_context_policy: String,
}

/// A stored approval. The secret never leaves the host except once, to the
/// extension, at approval time.
/// Persisted only inside the encrypted journal; `Debug` redacts the secret.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRecord {
    pub pairing_id: Uuid,
    pub request: PairingRequest,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked: bool,
    secret: [u8; 32],
}

impl std::fmt::Debug for PairingRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingRecord")
            .field("pairing_id", &self.pairing_id)
            .field("request", &self.request)
            .field("approved_at", &self.approved_at)
            .field("expires_at", &self.expires_at)
            .field("revoked", &self.revoked)
            .field("secret", &"<redacted>")
            .finish()
    }
}

impl PairingRecord {
    /// Approve `request` at `now`, generating a fresh pairing secret.
    pub fn approve(request: PairingRequest, now: DateTime<Utc>) -> Result<Self, PairingError> {
        let mut secret = [0u8; 32];
        rand_core::OsRng.try_fill_bytes(&mut secret).map_err(|_| PairingError::Random)?;
        Ok(Self {
            pairing_id: Uuid::new_v4(),
            request,
            approved_at: now,
            expires_at: now + Duration::days(PAIRING_LIFETIME_DAYS),
            revoked: false,
            secret,
        })
    }

    /// The secret delivered to the extension once, when pairing completes.
    pub fn secret_for_extension(&self) -> [u8; 32] {
        self.secret
    }

    pub fn revoke(&mut self) {
        self.revoked = true;
    }

    /// Check a connecting extension against this approval.
    pub fn admit(&self, hello: &ClientHello, now: DateTime<Utc>) -> Result<(), PairingError> {
        if hello.pairing_id != self.pairing_id || hello.extension_id != self.request.extension_id {
            // A copied manifest or another extension claiming this pairing.
            return Err(PairingError::NotPaired);
        }
        if self.revoked {
            return Err(PairingError::Revoked);
        }
        if now >= self.expires_at
            || hello.extension_key_digest != self.request.extension_key_digest
            || hello.permissions_digest != self.request.permissions_digest
        {
            // Stale approval, replaced extension, or broadened permissions.
            return Err(PairingError::RePairingRequired);
        }
        Ok(())
    }
}

/// The extension's opening message for a session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
    pub pairing_id: Uuid,
    pub extension_id: String,
    pub extension_key_digest: String,
    pub permissions_digest: String,
    pub client_nonce: [u8; 32],
}

/// One authenticated session. Its key exists only for this host start and
/// these two nonces.
pub struct PairedSession {
    key: [u8; 32],
    pub host_nonce: [u8; 32],
}

impl PairedSession {
    /// Admit `hello` against `record` and derive a session key with a fresh
    /// host nonce, which the host sends back to the extension.
    pub fn open(
        record: &PairingRecord,
        hello: &ClientHello,
        now: DateTime<Utc>,
    ) -> Result<Self, PairingError> {
        record.admit(hello, now)?;
        let mut host_nonce = [0u8; 32];
        rand_core::OsRng.try_fill_bytes(&mut host_nonce).map_err(|_| PairingError::Random)?;
        Ok(Self::derive(&record.secret, &host_nonce, &hello.client_nonce))
    }

    /// Derive the session key both sides compute from the shared secret and
    /// both nonces.
    pub fn derive(secret: &[u8; 32], host_nonce: &[u8; 32], client_nonce: &[u8; 32]) -> Self {
        let mut input = Vec::with_capacity(SESSION_KEY_DOMAIN.len() + 64);
        input.extend_from_slice(SESSION_KEY_DOMAIN);
        input.extend_from_slice(host_nonce);
        input.extend_from_slice(client_nonce);
        Self { key: hmac_sha256(secret, &input), host_nonce: *host_nonce }
    }

    /// MAC for message `seq` with frame body `body`.
    pub fn mac(&self, seq: u64, body: &[u8]) -> [u8; 32] {
        let mut input = Vec::with_capacity(MESSAGE_MAC_DOMAIN.len() + 8 + body.len());
        input.extend_from_slice(MESSAGE_MAC_DOMAIN);
        input.extend_from_slice(&seq.to_be_bytes());
        input.extend_from_slice(body);
        hmac_sha256(&self.key, &input)
    }

    /// Verify a received MAC in constant time.
    pub fn verify(&self, seq: u64, body: &[u8], mac: &[u8; 32]) -> Result<(), PairingError> {
        let expected = self.mac(seq, body);
        let difference = expected.iter().zip(mac).fold(0u8, |acc, (a, b)| acc | (a ^ b));
        if difference == 0 {
            Ok(())
        } else {
            Err(PairingError::BadMac)
        }
    }
}

/// HMAC-SHA256 (RFC 2104) over the crate's existing SHA-256.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block_key = [0u8; BLOCK];
    if key.len() > BLOCK {
        block_key[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block_key.map(|byte| byte ^ 0x36));
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(block_key.map(|byte| byte ^ 0x5c));
    outer.update(inner);
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&outer.finalize());
    mac
}

#[doc(hidden)]
pub fn hmac_sha256_for_test(key: &[u8], message: &[u8]) -> [u8; 32] {
    hmac_sha256(key, message)
}
