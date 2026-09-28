//! The native host's per-connection session: pairing, message
//! authentication, protocol rules, and URL minimization composed in order.
//!
//! For every frame body the session, in this order: parses it strictly;
//! for `hello`, admits the pairing and derives the session key; for any
//! later message, verifies its MAC over the canonical fields before any
//! sequence or content handling; applies the protocol session rules; and
//! reduces a navigation to a [`CanonicalNavigation`] or a counted refusal.
//! Nothing from an unauthenticated message reaches the caller.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    browser_origin::{CanonicalNavigation, NavigationRefusal, UrlShapePolicy},
    browser_pairing::{ClientHello, PairedSession, PairingError, PairingRecord},
    native_messaging::{
        parse_message, ExtensionMessage, NativeMessagingError, NavigationTransition,
        ProtocolSession, SessionEvent, NATIVE_MESSAGING_PROTOCOL_VERSION,
    },
};

/// Messages the host sends back to the extension.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    /// The pairing was admitted; the extension derives the session key from
    /// its secret, its client nonce, and `host_nonce`.
    Welcome { protocol_version: u32, host_nonce: String },
    /// The session ends; `reason` is a fixed code.
    Refused { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum NativeSessionError {
    #[error(transparent)]
    Protocol(#[from] NativeMessagingError),
    #[error(transparent)]
    Pairing(#[from] PairingError),
    #[error("a hex field is malformed")]
    MalformedHex,
    #[error("the message has no valid authentication code")]
    Unauthenticated,
}

impl NativeSessionError {
    /// The fixed code sent to the extension in a `refused` reply.
    pub fn code(self) -> &'static str {
        match self {
            Self::Pairing(PairingError::NotPaired) => "not_paired",
            Self::Pairing(PairingError::Revoked) => "revoked",
            Self::Pairing(PairingError::RePairingRequired) => "re_pairing_required",
            Self::Pairing(_) | Self::Unauthenticated => "unauthenticated",
            Self::MalformedHex | Self::Protocol(_) => "protocol_error",
        }
    }
}

/// What an accepted frame produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostOutput {
    /// Reply to send to the extension.
    Reply(HostMessage),
    /// An authenticated navigation, reduced to its canonical shape.
    /// `missing` counts sequence numbers that never arrived before it.
    Navigation { navigation: CanonicalNavigation, transition: NavigationTransition, missing: u64 },
    /// An authenticated navigation that is not retained; only the count is.
    NavigationRefused { refusal: NavigationRefusal, missing: u64 },
    /// An authenticated heartbeat.
    Heartbeat { missing: u64 },
    /// The extension ended the session.
    Closed,
}

pub struct NativeHostSession {
    protocol: ProtocolSession,
    paired: Option<PairedSession>,
    policy: UrlShapePolicy,
}

impl NativeHostSession {
    pub fn new(policy: UrlShapePolicy) -> Self {
        Self { protocol: ProtocolSession::new(), paired: None, policy }
    }

    /// Handle one frame body. `pairing` looks up an approval by its ID.
    pub fn receive<F>(
        &mut self,
        body: &[u8],
        now: Duration,
        wall_clock: DateTime<Utc>,
        pairing: F,
    ) -> Result<HostOutput, NativeSessionError>
    where
        F: FnOnce(Uuid) -> Option<PairingRecord>,
    {
        let message = parse_message(body)?;
        if let ExtensionMessage::Hello {
            pairing_id,
            extension_id,
            extension_key_digest,
            permissions_digest,
            client_nonce,
            ..
        } = &message
        {
            if self.paired.is_none() {
                let record = pairing(*pairing_id).ok_or(PairingError::NotPaired)?;
                let hello = ClientHello {
                    pairing_id: *pairing_id,
                    extension_id: extension_id.clone(),
                    extension_key_digest: extension_key_digest.clone(),
                    permissions_digest: permissions_digest.clone(),
                    client_nonce: decode_hex32(client_nonce)?,
                };
                let session = PairedSession::open(&record, &hello, wall_clock)?;
                self.protocol.receive(body, now)?;
                let host_nonce = encode_hex(&session.host_nonce);
                self.paired = Some(session);
                return Ok(HostOutput::Reply(HostMessage::Welcome {
                    protocol_version: NATIVE_MESSAGING_PROTOCOL_VERSION,
                    host_nonce,
                }));
            }
            // A second hello is handled (and refused) by the protocol rules.
            self.protocol.receive(body, now)?;
            return Err(NativeSessionError::Protocol(NativeMessagingError::DuplicateHello));
        }

        let Some(session) = &self.paired else {
            return Err(NativeSessionError::Protocol(NativeMessagingError::HelloRequired));
        };
        let mac = decode_hex32(message.mac().ok_or(NativeSessionError::Unauthenticated)?)?;
        let input = message.mac_input().ok_or(NativeSessionError::Unauthenticated)?;
        session
            .verify(message.seq(), &input, &mac)
            .map_err(|_| NativeSessionError::Unauthenticated)?;

        let (accepted, missing) = match self.protocol.receive(body, now)? {
            SessionEvent::Accepted(message) => (message, 0),
            SessionEvent::AcceptedAfterGap { message, missing } => (message, missing),
            SessionEvent::Closed => return Ok(HostOutput::Closed),
        };
        Ok(match accepted {
            ExtensionMessage::Navigation { url, private_context, transition, .. } => {
                match CanonicalNavigation::from_url(&url, private_context, self.policy) {
                    Ok(navigation) => HostOutput::Navigation { navigation, transition, missing },
                    Err(refusal) => HostOutput::NavigationRefused { refusal, missing },
                }
            }
            ExtensionMessage::Heartbeat { .. } => HostOutput::Heartbeat { missing },
            ExtensionMessage::Goodbye { .. } => HostOutput::Closed,
            ExtensionMessage::Hello { .. } => {
                return Err(NativeSessionError::Protocol(NativeMessagingError::DuplicateHello))
            }
        })
    }
}

/// Lowercase hex encoding used on the wire for nonces and MACs.
pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex32(text: &str) -> Result<[u8; 32], NativeSessionError> {
    if text.len() != 64 || !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(NativeSessionError::MalformedHex);
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| NativeSessionError::MalformedHex)?;
    }
    Ok(bytes)
}
