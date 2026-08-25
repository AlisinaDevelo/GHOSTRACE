//! Crash-safe key rotation and explicit destruction boundaries.
//!
//! The key ring deliberately keeps key bytes in memory only.  Checkpoints,
//! confirmations, receipts, `Debug`, and all serialized values contain
//! generations and outcomes, never secret material.  A rotation has two
//! phases: stage the next generation, verify every re-encrypted record, then
//! commit by retiring the prior generation.  Until that commit, old
//! ciphertext remains readable.

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::crypto::{CiphertextEnvelope, KeyAlgorithm, KeyMetadata, KeyProvider};
use crate::error::CryptoError;

/// Version for serialized key-lifecycle checkpoints and receipts.
pub const KEY_LIFECYCLE_SCHEMA_VERSION: u32 = 1;
/// Bound retained generations so a damaged checkpoint cannot cause unbounded
/// in-memory key material or recovery work.
pub const MAX_KEY_GENERATIONS: usize = 64;

const UNRECOVERABLE_GENERATION_EXPLANATION: &str =
    "ciphertext encrypted under the destroyed key generation is unrecoverable";
const UNRECOVERABLE_RESET_EXPLANATION: &str =
    "ciphertext encrypted under destroyed local key generations is unrecoverable; no cloud recovery secret exists";

#[derive(Debug, Error)]
pub enum KeyLifecycleError {
    #[error("key generation must be positive: {0}")]
    InvalidGeneration(u32),

    #[error("key material must contain at least one non-zero byte")]
    InvalidKeyMaterial,

    #[error("key generation {0} is already retained")]
    GenerationExists(u32),

    #[error("key generation {next} must be greater than current generation {current}")]
    GenerationOrder { next: u32, current: u32 },

    #[error("key ring cannot retain more than {MAX_KEY_GENERATIONS} generations")]
    TooManyGenerations,

    #[error("key generation {0} is not retained")]
    GenerationMissing(u32),

    #[error("the current key generation cannot be retired or destroyed")]
    CurrentGeneration,

    #[error("rotation checkpoint is invalid: {0}")]
    InvalidCheckpoint(String),

    #[error("rotation is already committed")]
    RotationCommitted,

    #[error("rotation has no remaining records to verify")]
    NoRecordsRemaining,

    #[error("rotation is incomplete: verified {verified} of {total} records")]
    RotationIncomplete { verified: u64, total: u64 },

    #[error("destruction requires explicit confirmation")]
    ConfirmationRequired,

    #[error("destruction confirmation scope does not match the requested operation")]
    ConfirmationScope,

    #[error("cryptographic operation failed: {0}")]
    Crypto(#[from] CryptoError),
}

/// The only cipher suite currently admitted by the envelope contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationPhase {
    Prepared,
    Reencrypting,
    Committed,
}

/// Durable, key-free progress for a rotation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationCheckpoint {
    pub schema_version: u32,
    pub from_generation: u32,
    pub to_generation: u32,
    pub total_records: u64,
    pub verified_records: u64,
    pub phase: RotationPhase,
}

/// A reason is retained in the receipt so recovery UX can distinguish a lost
/// key from a user-requested reset or a compromise response.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DestructionReason {
    LostKey,
    UserReset,
    Compromise,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "generation")]
pub enum DestructionScope {
    Generation(u32),
    All,
}

/// Confirmation is intentionally explicit and serializable, but never carries
/// a key or a recovery secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestructionConfirmation {
    pub schema_version: u32,
    pub scope: DestructionScope,
    pub reason: DestructionReason,
    pub confirmed: bool,
}

impl DestructionConfirmation {
    pub fn for_generation(generation: u32, reason: DestructionReason) -> Self {
        Self {
            schema_version: KEY_LIFECYCLE_SCHEMA_VERSION,
            scope: DestructionScope::Generation(generation),
            reason,
            confirmed: true,
        }
    }

    pub fn for_all(reason: DestructionReason) -> Self {
        Self {
            schema_version: KEY_LIFECYCLE_SCHEMA_VERSION,
            scope: DestructionScope::All,
            reason,
            confirmed: true,
        }
    }

    pub fn unconfirmed(scope: DestructionScope, reason: DestructionReason) -> Self {
        Self { schema_version: KEY_LIFECYCLE_SCHEMA_VERSION, scope, reason, confirmed: false }
    }

    fn validate(&self) -> Result<(), KeyLifecycleError> {
        if self.schema_version != KEY_LIFECYCLE_SCHEMA_VERSION {
            return Err(KeyLifecycleError::InvalidCheckpoint(
                "destruction confirmation schema version is unsupported".to_owned(),
            ));
        }
        if let DestructionScope::Generation(generation) = self.scope {
            validate_generation(generation)?;
        }
        if !self.confirmed {
            return Err(KeyLifecycleError::ConfirmationRequired);
        }
        Ok(())
    }
}

/// Public receipt describing exactly which local generations were destroyed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDestructionReceipt {
    pub schema_version: u32,
    pub scope: DestructionScope,
    pub reason: DestructionReason,
    pub destroyed_generations: Vec<u32>,
    pub data_unrecoverable: bool,
    pub explanation: String,
}

/// In-memory retained key generations.  The custom formatter exposes only
/// generation numbers, never key bytes.
#[derive(Clone)]
pub struct KeyRing {
    keys: BTreeMap<u32, [u8; 32]>,
    current_generation: u32,
    algorithm: KeyAlgorithm,
}

impl fmt::Debug for KeyRing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyRing")
            .field("algorithm", &self.algorithm)
            .field("current_generation", &self.current_generation)
            .field("retained_generations", &self.keys.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl KeyRing {
    pub fn new(generation: u32, key: [u8; 32]) -> Result<Self, KeyLifecycleError> {
        validate_generation(generation)?;
        validate_key_material(&key)?;
        let mut keys = BTreeMap::new();
        keys.insert(generation, key);
        Ok(Self {
            keys,
            current_generation: generation,
            algorithm: KeyAlgorithm::XChaCha20Poly1305,
        })
    }

    pub fn current_generation(&self) -> u32 {
        self.current_generation
    }

    pub fn current_metadata(&self) -> Result<KeyMetadata, KeyLifecycleError> {
        if self.current_generation == 0 {
            return Err(KeyLifecycleError::GenerationMissing(0));
        }
        if !self.keys.contains_key(&self.current_generation) {
            return Err(KeyLifecycleError::GenerationMissing(self.current_generation));
        }
        Ok(KeyMetadata {
            schema_version: crate::crypto::CIPHERTEXT_ENVELOPE_VERSION,
            algorithm: self.algorithm,
            key_generation: self.current_generation,
        })
    }

    pub fn metadata(&self, generation: u32) -> Option<KeyMetadata> {
        self.keys.contains_key(&generation).then_some(KeyMetadata {
            schema_version: crate::crypto::CIPHERTEXT_ENVELOPE_VERSION,
            algorithm: self.algorithm,
            key_generation: generation,
        })
    }

    pub fn contains_generation(&self, generation: u32) -> bool {
        self.keys.contains_key(&generation)
    }

    pub fn encrypt_current(
        &self,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<CiphertextEnvelope, CryptoError> {
        let key = self.keys.get(&self.current_generation).copied().ok_or_else(|| {
            CryptoError::KeyProvider("current key generation is unavailable".to_owned())
        })?;
        CiphertextEnvelope::encrypt_with_key(
            self.current_generation,
            key,
            associated_data,
            plaintext,
        )
    }

    pub fn decrypt(
        &self,
        envelope: &CiphertextEnvelope,
        associated_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        if envelope.algorithm != self.algorithm {
            return Err(CryptoError::Encoding("ciphertext algorithm is unsupported".to_owned()));
        }
        let key = self.keys.get(&envelope.key_generation).copied().ok_or_else(|| {
            CryptoError::KeyProvider(format!(
                "key generation {} is unavailable",
                envelope.key_generation
            ))
        })?;
        envelope.decrypt_with_key(key, associated_data)
    }

    pub fn stage_generation(
        &mut self,
        generation: u32,
        key: [u8; 32],
    ) -> Result<(), KeyLifecycleError> {
        validate_generation(generation)?;
        validate_key_material(&key)?;
        if generation <= self.current_generation {
            return Err(KeyLifecycleError::GenerationOrder {
                next: generation,
                current: self.current_generation,
            });
        }
        if self.keys.contains_key(&generation) {
            return Err(KeyLifecycleError::GenerationExists(generation));
        }
        if self.keys.len() >= MAX_KEY_GENERATIONS {
            return Err(KeyLifecycleError::TooManyGenerations);
        }
        self.keys.insert(generation, key);
        Ok(())
    }

    pub fn retire_generation(&mut self, generation: u32) -> Result<(), KeyLifecycleError> {
        validate_generation(generation)?;
        if generation == self.current_generation {
            return Err(KeyLifecycleError::CurrentGeneration);
        }
        self.keys
            .remove(&generation)
            .map(|_| ())
            .ok_or(KeyLifecycleError::GenerationMissing(generation))
    }

    pub fn destroy_generation(
        &mut self,
        generation: u32,
        confirmation: DestructionConfirmation,
    ) -> Result<KeyDestructionReceipt, KeyLifecycleError> {
        confirmation.validate()?;
        if confirmation.scope != DestructionScope::Generation(generation) {
            return Err(KeyLifecycleError::ConfirmationScope);
        }
        validate_generation(generation)?;
        if generation == self.current_generation {
            return Err(KeyLifecycleError::CurrentGeneration);
        }
        self.keys.remove(&generation).ok_or(KeyLifecycleError::GenerationMissing(generation))?;
        Ok(KeyDestructionReceipt {
            schema_version: KEY_LIFECYCLE_SCHEMA_VERSION,
            scope: confirmation.scope,
            reason: confirmation.reason,
            destroyed_generations: vec![generation],
            data_unrecoverable: true,
            explanation: UNRECOVERABLE_GENERATION_EXPLANATION.to_owned(),
        })
    }

    pub fn reset_all(
        &mut self,
        confirmation: DestructionConfirmation,
    ) -> Result<KeyDestructionReceipt, KeyLifecycleError> {
        confirmation.validate()?;
        if confirmation.scope != DestructionScope::All {
            return Err(KeyLifecycleError::ConfirmationScope);
        }
        let destroyed_generations = self.keys.keys().copied().collect::<Vec<_>>();
        self.keys.clear();
        self.current_generation = 0;
        Ok(KeyDestructionReceipt {
            schema_version: KEY_LIFECYCLE_SCHEMA_VERSION,
            scope: confirmation.scope,
            reason: confirmation.reason,
            data_unrecoverable: !destroyed_generations.is_empty(),
            destroyed_generations,
            explanation: UNRECOVERABLE_RESET_EXPLANATION.to_owned(),
        })
    }

    fn encrypt_generation(
        &self,
        generation: u32,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<CiphertextEnvelope, KeyLifecycleError> {
        let key = self
            .keys
            .get(&generation)
            .copied()
            .ok_or(KeyLifecycleError::GenerationMissing(generation))?;
        Ok(CiphertextEnvelope::encrypt_with_key(generation, key, associated_data, plaintext)?)
    }

    fn activate_generation(&mut self, generation: u32) -> Result<(), KeyLifecycleError> {
        if !self.keys.contains_key(&generation) {
            return Err(KeyLifecycleError::GenerationMissing(generation));
        }
        self.current_generation = generation;
        Ok(())
    }
}

impl KeyProvider for KeyRing {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        self.keys.get(&self.current_generation).copied().ok_or_else(|| {
            CryptoError::KeyProvider("current key generation is unavailable".to_owned())
        })
    }

    fn key_generation(&self) -> u32 {
        self.current_generation
    }

    fn key_for_generation(&self, generation: u32) -> Result<[u8; 32], CryptoError> {
        self.keys.get(&generation).copied().ok_or_else(|| {
            CryptoError::KeyProvider(format!("key generation {generation} is unavailable"))
        })
    }
}

/// A staged rotation that can be checkpointed between verified records.
#[derive(Clone, Debug)]
pub struct KeyRotation {
    key_ring: KeyRing,
    checkpoint: RotationCheckpoint,
}

impl KeyRotation {
    pub fn begin(
        mut key_ring: KeyRing,
        to_generation: u32,
        new_key: [u8; 32],
        total_records: u64,
    ) -> Result<Self, KeyLifecycleError> {
        let from_generation = key_ring.current_generation;
        key_ring.stage_generation(to_generation, new_key)?;
        Ok(Self {
            key_ring,
            checkpoint: RotationCheckpoint {
                schema_version: KEY_LIFECYCLE_SCHEMA_VERSION,
                from_generation,
                to_generation,
                total_records,
                verified_records: 0,
                phase: RotationPhase::Prepared,
            },
        })
    }

    pub fn resume(
        key_ring: KeyRing,
        checkpoint: RotationCheckpoint,
    ) -> Result<Self, KeyLifecycleError> {
        validate_checkpoint(&checkpoint)?;
        if checkpoint.phase == RotationPhase::Committed {
            return Err(KeyLifecycleError::RotationCommitted);
        }
        if key_ring.current_generation != checkpoint.from_generation {
            return Err(KeyLifecycleError::InvalidCheckpoint(
                "checkpoint source generation is not the active generation".to_owned(),
            ));
        }
        if !key_ring.contains_generation(checkpoint.to_generation) {
            return Err(KeyLifecycleError::InvalidCheckpoint(
                "checkpoint target generation is not retained".to_owned(),
            ));
        }
        Ok(Self { key_ring, checkpoint })
    }

    pub fn checkpoint(&self) -> &RotationCheckpoint {
        &self.checkpoint
    }

    pub fn key_ring(&self) -> &KeyRing {
        &self.key_ring
    }

    pub fn is_ready_to_commit(&self) -> bool {
        self.checkpoint.phase != RotationPhase::Committed
            && self.checkpoint.verified_records == self.checkpoint.total_records
    }

    pub fn reencrypt(
        &mut self,
        old_envelope: &CiphertextEnvelope,
        associated_data: &[u8],
    ) -> Result<CiphertextEnvelope, KeyLifecycleError> {
        if self.checkpoint.phase == RotationPhase::Committed {
            return Err(KeyLifecycleError::RotationCommitted);
        }
        if self.checkpoint.verified_records >= self.checkpoint.total_records {
            return Err(KeyLifecycleError::NoRecordsRemaining);
        }
        if old_envelope.key_generation != self.checkpoint.from_generation {
            return Err(KeyLifecycleError::InvalidCheckpoint(
                "ciphertext belongs to a generation outside this rotation".to_owned(),
            ));
        }
        let plaintext = self.key_ring.decrypt(old_envelope, associated_data)?;
        let replacement = self.key_ring.encrypt_generation(
            self.checkpoint.to_generation,
            associated_data,
            &plaintext,
        )?;
        let verified = self.key_ring.decrypt(&replacement, associated_data)?;
        if verified != plaintext {
            return Err(KeyLifecycleError::Crypto(CryptoError::AuthenticationFailed));
        }
        self.checkpoint.verified_records += 1;
        self.checkpoint.phase = RotationPhase::Reencrypting;
        Ok(replacement)
    }

    pub fn commit(mut self) -> Result<KeyRing, KeyLifecycleError> {
        if self.checkpoint.phase == RotationPhase::Committed {
            return Err(KeyLifecycleError::RotationCommitted);
        }
        if self.checkpoint.verified_records != self.checkpoint.total_records {
            return Err(KeyLifecycleError::RotationIncomplete {
                verified: self.checkpoint.verified_records,
                total: self.checkpoint.total_records,
            });
        }
        self.key_ring
            .keys
            .remove(&self.checkpoint.from_generation)
            .ok_or(KeyLifecycleError::GenerationMissing(self.checkpoint.from_generation))?;
        self.key_ring.activate_generation(self.checkpoint.to_generation)?;
        self.checkpoint.phase = RotationPhase::Committed;
        Ok(self.key_ring)
    }
}

fn validate_generation(generation: u32) -> Result<(), KeyLifecycleError> {
    if generation == 0 {
        Err(KeyLifecycleError::InvalidGeneration(generation))
    } else {
        Ok(())
    }
}

fn validate_key_material(key: &[u8; 32]) -> Result<(), KeyLifecycleError> {
    if key.iter().all(|byte| *byte == 0) {
        Err(KeyLifecycleError::InvalidKeyMaterial)
    } else {
        Ok(())
    }
}

fn validate_checkpoint(checkpoint: &RotationCheckpoint) -> Result<(), KeyLifecycleError> {
    if checkpoint.schema_version != KEY_LIFECYCLE_SCHEMA_VERSION {
        return Err(KeyLifecycleError::InvalidCheckpoint(
            "checkpoint schema version is unsupported".to_owned(),
        ));
    }
    validate_generation(checkpoint.from_generation)?;
    validate_generation(checkpoint.to_generation)?;
    if checkpoint.to_generation <= checkpoint.from_generation {
        return Err(KeyLifecycleError::InvalidCheckpoint(
            "target generation must be greater than source generation".to_owned(),
        ));
    }
    if checkpoint.verified_records > checkpoint.total_records {
        return Err(KeyLifecycleError::InvalidCheckpoint(
            "verified records exceed total records".to_owned(),
        ));
    }
    if checkpoint.phase == RotationPhase::Prepared && checkpoint.verified_records != 0 {
        return Err(KeyLifecycleError::InvalidCheckpoint(
            "prepared rotation cannot contain verified records".to_owned(),
        ));
    }
    Ok(())
}
