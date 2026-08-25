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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CryptoError;

/// Version of the authenticated ciphertext envelope.
pub const CIPHERTEXT_ENVELOPE_VERSION: u32 = 1;
const CIPHERTEXT_MAGIC: &[u8; 4] = b"GRCE";
const CIPHERTEXT_HEADER_BYTES: usize = 4 + 1 + 1 + 4 + 24;
const NONCE_BYTES: usize = 24;
/// A ciphertext envelope is bounded before allocation or decryption.
pub const MAX_CIPHERTEXT_BYTES: usize = 16 * 1024 * 1024;

/// Algorithms are named in the envelope so a future reader never guesses at
/// the cipher used to protect a payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAlgorithm {
    XChaCha20Poly1305,
}

impl KeyAlgorithm {
    const CODE: u8 = 1;
}

/// Authenticated payload metadata. It contains no key material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiphertextEnvelope {
    pub schema_version: u32,
    pub algorithm: KeyAlgorithm,
    pub key_generation: u32,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl CiphertextEnvelope {
    pub fn encrypt_with_key(
        key_generation: u32,
        key: [u8; 32],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Self, CryptoError> {
        validate_generation(key_generation)?;
        if plaintext.len() > MAX_CIPHERTEXT_BYTES {
            return Err(CryptoError::Encoding("plaintext exceeds the ciphertext bound".to_owned()));
        }
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
        let mut nonce = [0_u8; NONCE_BYTES];
        rand_core::OsRng.try_fill_bytes(&mut nonce).map_err(|_| CryptoError::Random)?;
        let ciphertext = cipher.encrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad: &envelope_associated_data(associated_data, key_generation),
            },
        )?;
        Ok(Self {
            schema_version: CIPHERTEXT_ENVELOPE_VERSION,
            algorithm: KeyAlgorithm::XChaCha20Poly1305,
            key_generation,
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub fn decrypt_with_key(
        &self,
        key: [u8; 32],
        associated_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        self.validate()?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
        Ok(cipher.decrypt(
            XNonce::from_slice(&self.nonce),
            chacha20poly1305::aead::Payload {
                msg: &self.ciphertext,
                aad: &envelope_associated_data(associated_data, self.key_generation),
            },
        )?)
    }

    pub fn encode(&self) -> Result<Vec<u8>, CryptoError> {
        self.validate()?;
        let mut encoded = Vec::with_capacity(CIPHERTEXT_HEADER_BYTES + self.ciphertext.len());
        encoded.extend_from_slice(CIPHERTEXT_MAGIC);
        encoded.push(u8::try_from(self.schema_version).map_err(|_| {
            CryptoError::Encoding("ciphertext schema version is out of range".to_owned())
        })?);
        encoded.push(self.algorithm.code());
        encoded.extend_from_slice(&self.key_generation.to_le_bytes());
        encoded.extend_from_slice(&self.nonce);
        encoded.extend_from_slice(&self.ciphertext);
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CryptoError> {
        if encoded.len() < CIPHERTEXT_HEADER_BYTES + 1 {
            return Err(CryptoError::Truncated);
        }
        if &encoded[..CIPHERTEXT_MAGIC.len()] != CIPHERTEXT_MAGIC {
            return Err(CryptoError::Encoding("ciphertext envelope magic is invalid".to_owned()));
        }
        let schema_version = u32::from(encoded[4]);
        let algorithm = match encoded[5] {
            KeyAlgorithm::CODE => KeyAlgorithm::XChaCha20Poly1305,
            _ => {
                return Err(CryptoError::Encoding("ciphertext algorithm is unsupported".to_owned()))
            }
        };
        let mut generation_bytes = [0_u8; 4];
        generation_bytes.copy_from_slice(&encoded[6..10]);
        let key_generation = u32::from_le_bytes(generation_bytes);
        let nonce = encoded[10..34].to_vec();
        let ciphertext_len = encoded.len() - CIPHERTEXT_HEADER_BYTES;
        if ciphertext_len == 0 || ciphertext_len > MAX_CIPHERTEXT_BYTES {
            return Err(CryptoError::Encoding("ciphertext has an invalid length".to_owned()));
        }
        let ciphertext = encoded[34..].to_vec();
        let envelope = Self { schema_version, algorithm, key_generation, nonce, ciphertext };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn metadata(&self) -> KeyMetadata {
        KeyMetadata {
            schema_version: self.schema_version,
            algorithm: self.algorithm,
            key_generation: self.key_generation,
        }
    }

    fn validate(&self) -> Result<(), CryptoError> {
        if self.schema_version != CIPHERTEXT_ENVELOPE_VERSION {
            return Err(CryptoError::Encoding(
                "ciphertext schema version is unsupported".to_owned(),
            ));
        }
        if self.algorithm != KeyAlgorithm::XChaCha20Poly1305 {
            return Err(CryptoError::Encoding("ciphertext algorithm is unsupported".to_owned()));
        }
        validate_generation(self.key_generation)?;
        if self.nonce.len() != NONCE_BYTES {
            return Err(CryptoError::Encoding("ciphertext nonce has an invalid length".to_owned()));
        }
        if self.ciphertext.is_empty() || self.ciphertext.len() > MAX_CIPHERTEXT_BYTES {
            return Err(CryptoError::Encoding("ciphertext has an invalid length".to_owned()));
        }
        Ok(())
    }
}

/// Metadata recorded with a payload; key material is deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyMetadata {
    pub schema_version: u32,
    pub algorithm: KeyAlgorithm,
    pub key_generation: u32,
}

impl KeyAlgorithm {
    fn code(self) -> u8 {
        match self {
            Self::XChaCha20Poly1305 => Self::CODE,
        }
    }
}

fn validate_generation(generation: u32) -> Result<(), CryptoError> {
    if generation == 0 {
        return Err(CryptoError::Encoding("key generation must be positive".to_owned()));
    }
    Ok(())
}

fn envelope_associated_data(associated_data: &[u8], key_generation: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity(32 + associated_data.len());
    result.extend_from_slice(b"ghostrace:ciphertext-envelope:v1:");
    result.extend_from_slice(&key_generation.to_le_bytes());
    result.extend_from_slice(associated_data);
    result
}

pub trait KeyProvider: Send + Sync {
    fn key(&self) -> Result<[u8; 32], CryptoError>;

    /// The generation returned by [`Self::key`]. Existing providers default to
    /// generation one, preserving the fixture contract.
    fn key_generation(&self) -> u32 {
        1
    }

    /// Resolve an older generation during a resumable rotation. Providers that
    /// retain only their active key fail closed for retired generations.
    fn key_for_generation(&self, generation: u32) -> Result<[u8; 32], CryptoError> {
        if generation == self.key_generation() {
            self.key()
        } else {
            Err(CryptoError::KeyProvider("key generation is unavailable".to_owned()))
        }
    }
}

impl<T> KeyProvider for Arc<T>
where
    T: KeyProvider + ?Sized,
{
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        self.as_ref().key()
    }

    fn key_generation(&self) -> u32 {
        self.as_ref().key_generation()
    }

    fn key_for_generation(&self, generation: u32) -> Result<[u8; 32], CryptoError> {
        self.as_ref().key_for_generation(generation)
    }
}

/// Deterministic key provider used by fixtures and tests.  There is deliberately
/// no environment-variable or OS-Keychain implementation in this vertical slice.
#[derive(Clone)]
pub struct DeterministicKeyProvider {
    key: [u8; 32],
    generation: u32,
}

impl DeterministicKeyProvider {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key, generation: 1 }
    }

    pub fn from_seed(seed: &str) -> Self {
        let digest = Sha256::digest(seed.as_bytes());
        let mut key = [0_u8; 32];
        key.copy_from_slice(&digest);
        Self { key, generation: 1 }
    }

    pub fn with_generation(key: [u8; 32], generation: u32) -> Result<Self, CryptoError> {
        validate_generation(generation)?;
        Ok(Self { key, generation })
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}

impl KeyProvider for DeterministicKeyProvider {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        Ok(self.key)
    }

    fn key_generation(&self) -> u32 {
        self.generation
    }
}

pub type SharedKeyProvider = Arc<dyn KeyProvider>;

/// Encrypts a payload into a self-describing envelope with a fresh 24-byte
/// XChaCha nonce.  The envelope metadata is not secret; authentication covers
/// the payload, generation, and caller-supplied event-associated data.
pub fn encrypt_payload(
    provider: &dyn KeyProvider,
    associated_data: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let generation = provider.key_generation();
    let key_bytes = provider.key_for_generation(generation)?;
    CiphertextEnvelope::encrypt_with_key(generation, key_bytes, associated_data, plaintext)?
        .encode()
}

pub fn decrypt_payload(
    provider: &dyn KeyProvider,
    associated_data: &[u8],
    encoded: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if encoded.starts_with(CIPHERTEXT_MAGIC) {
        let envelope = CiphertextEnvelope::decode(encoded)?;
        let key_bytes = provider.key_for_generation(envelope.key_generation)?;
        return envelope.decrypt_with_key(key_bytes, associated_data);
    }
    // Legacy v0 payloads had only nonce+ciphertext. They remain readable while
    // a verified rotation or migration moves them to the metadata envelope.
    if encoded.len() < NONCE_BYTES {
        return Err(CryptoError::Truncated);
    }
    let (nonce_bytes, ciphertext) = encoded.split_at(NONCE_BYTES);
    let key_bytes = provider.key()?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let nonce = XNonce::from_slice(nonce_bytes);
    Ok(cipher.decrypt(
        nonce,
        chacha20poly1305::aead::Payload { msg: ciphertext, aad: associated_data },
    )?)
}
