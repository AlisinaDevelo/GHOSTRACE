//! Payload AEAD boundary.
//!
//! SQLite metadata (event ID, timestamps, source, kind, and ingest sequence) is
//! intentionally queryable for indexing.  The payload bytes are encrypted and
//! authenticated before insertion; callers must not treat metadata as secret.

use std::sync::Arc;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand_core::RngCore;
use sha2::{Digest, Sha256};

use crate::error::CryptoError;

pub trait KeyProvider: Send + Sync {
    fn key(&self) -> Result<[u8; 32], CryptoError>;
}

impl<T> KeyProvider for Arc<T>
where
    T: KeyProvider + ?Sized,
{
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        self.as_ref().key()
    }
}

/// Deterministic key provider used by fixtures and tests.  There is deliberately
/// no environment-variable or OS-Keychain implementation in this vertical slice.
#[derive(Clone)]
pub struct DeterministicKeyProvider {
    key: [u8; 32],
}

impl DeterministicKeyProvider {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn from_seed(seed: &str) -> Self {
        let digest = Sha256::digest(seed.as_bytes());
        let mut key = [0_u8; 32];
        key.copy_from_slice(&digest);
        Self { key }
    }
}

impl KeyProvider for DeterministicKeyProvider {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        Ok(self.key)
    }
}

pub type SharedKeyProvider = Arc<dyn KeyProvider>;

/// Encrypts a payload with a fresh 24-byte XChaCha nonce.  The nonce is stored
/// as the first bytes of the ciphertext and is not secret; authentication covers
/// both payload bytes and the caller-supplied event-associated data.
pub fn encrypt_payload(
    provider: &dyn KeyProvider,
    associated_data: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = provider.key()?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let mut nonce_bytes = [0_u8; 24];
    rand_core::OsRng.try_fill_bytes(&mut nonce_bytes).map_err(|_| CryptoError::Random)?;
    let nonce = XNonce::from_slice(&nonce_bytes);
    let mut ciphertext = cipher
        .encrypt(nonce, chacha20poly1305::aead::Payload { msg: plaintext, aad: associated_data })?;
    let mut encoded = nonce_bytes.to_vec();
    encoded.append(&mut ciphertext);
    Ok(encoded)
}

pub fn decrypt_payload(
    provider: &dyn KeyProvider,
    associated_data: &[u8],
    encoded: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if encoded.len() < 24 {
        return Err(CryptoError::Truncated);
    }
    let (nonce_bytes, ciphertext) = encoded.split_at(24);
    let key_bytes = provider.key()?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let nonce = XNonce::from_slice(nonce_bytes);
    Ok(cipher.decrypt(
        nonce,
        chacha20poly1305::aead::Payload { msg: ciphertext, aad: associated_data },
    )?)
}
