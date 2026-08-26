//! Signed local verification checkpoints and bounded, copy-only repair.
//!
//! A checkpoint is a local-key receipt, not remote attestation. It binds the
//! durable database identity to the authenticated journal anchor and the
//! integrity report observed at one verification time. Repair never mutates
//! the source journal: Journal copies and verifies the source before applying
//! bounded interval deletions and explicit repair gaps to the copy.

use std::{fmt::Write as _, path::Path};

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    authenticated::AuthenticatedState,
    error::GhostraceError,
    integrity::IntegrityReport,
    model::{EventSource, ReasonCode, SnapshotDigest},
};

/// Wire version for verification checkpoint receipts.
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;
/// Wire version for copy-only repair manifests.
pub const REPAIR_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum number of intervals accepted by one repair operation.
pub const MAX_REPAIR_INTERVALS: usize = 64;
/// Maximum number of existing events one interval may remove.
pub const MAX_REPAIR_INTERVAL_EVENTS: u64 = 4096;
const MAX_CHECKPOINT_TIMESTAMP_BYTES: usize = 64;
const MAX_SIGNATURE_BYTES: usize = 64;
const CHECKPOINT_DOMAIN: &[u8] = b"ghostrace:verification-checkpoint:v1";

/// A signed, path-free local verification receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheckpoint {
    pub schema_version: u32,
    pub database_identity: SnapshotDigest,
    pub journal_schema_version: u32,
    pub chain_epoch: u64,
    pub head_mac: String,
    pub event_count: u64,
    pub max_ingest_seq: u64,
    pub key_generation: u32,
    pub policy_digest: SnapshotDigest,
    pub integrity_digest: SnapshotDigest,
    pub verified_at: String,
    /// HMAC-SHA-256 over every field except this signature, encoded as lower
    /// case hexadecimal. The key is never serialized.
    pub signature: String,
}

impl VerificationCheckpoint {
    pub(crate) fn unsigned(
        database_identity: SnapshotDigest,
        journal_schema_version: u32,
        state: &AuthenticatedState,
        integrity_digest: SnapshotDigest,
        verified_at: String,
    ) -> Self {
        Self {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            database_identity,
            journal_schema_version,
            chain_epoch: state.chain_epoch,
            head_mac: state.head_mac.clone(),
            event_count: state.event_count,
            max_ingest_seq: state.max_ingest_seq,
            key_generation: state.key_generation,
            policy_digest: SnapshotDigest::try_from(state.policy_digest.clone())
                .expect("authenticated state validates its policy digest"),
            integrity_digest,
            verified_at,
            signature: String::new(),
        }
    }

    pub(crate) fn sign(&mut self, key: &[u8; 32]) -> Result<(), GhostraceError> {
        self.validate_unsigned()?;
        self.signature = hex_digest(&hmac_sha256(key, &canonical_checkpoint(self)));
        self.validate()
    }

    /// Verify the local-key signature without exposing key material.
    pub fn verify_signature(&self, key: &[u8; 32]) -> Result<(), GhostraceError> {
        self.validate()?;
        let expected = hex_digest(&hmac_sha256(key, &canonical_checkpoint(self)));
        if constant_time_equal(expected.as_bytes(), self.signature.as_bytes()) {
            Ok(())
        } else {
            Err(GhostraceError::CheckpointMismatch(
                "checkpoint signature does not verify with the configured key".to_owned(),
            ))
        }
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        self.validate_unsigned()?;
        if self.signature.len() != MAX_SIGNATURE_BYTES
            || !self
                .signature
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(GhostraceError::CheckpointInvalid(
                "checkpoint signature is not lower-case hexadecimal".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_unsigned(&self) -> Result<(), GhostraceError> {
        if self.schema_version != CHECKPOINT_SCHEMA_VERSION
            || self.journal_schema_version == 0
            || self.head_mac.len() != 64
            || !self
                .head_mac
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || self.verified_at.is_empty()
            || self.verified_at.len() > MAX_CHECKPOINT_TIMESTAMP_BYTES
            || self.verified_at.chars().any(char::is_control)
        {
            return Err(GhostraceError::CheckpointInvalid(
                "checkpoint fields do not satisfy the bounded contract".to_owned(),
            ));
        }
        DateTime::parse_from_rfc3339(&self.verified_at).map_err(|_| {
            GhostraceError::CheckpointInvalid("verified_at must be RFC3339".to_owned())
        })?;
        Ok(())
    }
}

/// One inclusive ingest-sequence interval to remove from a verified copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairInterval {
    pub source: EventSource,
    pub start_ingest_seq: u64,
    pub end_ingest_seq: u64,
    pub reason_code: ReasonCode,
}

impl RepairInterval {
    pub fn new(
        source: EventSource,
        start_ingest_seq: u64,
        end_ingest_seq: u64,
    ) -> Result<Self, GhostraceError> {
        Self::with_reason(source, start_ingest_seq, end_ingest_seq, "repair_gap")
    }

    pub fn with_reason(
        source: EventSource,
        start_ingest_seq: u64,
        end_ingest_seq: u64,
        reason_code: impl Into<String>,
    ) -> Result<Self, GhostraceError> {
        let interval = Self {
            source,
            start_ingest_seq,
            end_ingest_seq,
            reason_code: ReasonCode::try_from(reason_code.into())?,
        };
        interval.validate()?;
        Ok(interval)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.source == EventSource::Fixture {
            return Err(GhostraceError::RepairIntervalInvalid(
                "repair cannot assert fixture provenance".to_owned(),
            ));
        }
        if self.start_ingest_seq == 0
            || self.end_ingest_seq < self.start_ingest_seq
            || self.end_ingest_seq.saturating_sub(self.start_ingest_seq).saturating_add(1)
                > MAX_REPAIR_INTERVAL_EVENTS
        {
            return Err(GhostraceError::RepairIntervalInvalid(
                "interval is empty or exceeds the bounded event limit".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Counts returned by the transactional copy mutation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairApplication {
    pub dropped_event_count: u64,
    pub reconstructed_event_count: u64,
    pub gap_event_count: u64,
}

/// Path-free state receipt for one side of a repair operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairStateManifest {
    pub database_identity: SnapshotDigest,
    pub integrity_digest: SnapshotDigest,
    pub chain_epoch: u64,
    pub head_mac: String,
    pub event_count: u64,
    pub max_ingest_seq: u64,
    pub key_generation: u32,
    pub gap_event_count: u64,
}

impl RepairStateManifest {
    pub(crate) fn from_checkpoint(
        checkpoint: &VerificationCheckpoint,
        gap_event_count: u64,
    ) -> Self {
        Self {
            database_identity: checkpoint.database_identity.clone(),
            integrity_digest: checkpoint.integrity_digest.clone(),
            chain_epoch: checkpoint.chain_epoch,
            head_mac: checkpoint.head_mac.clone(),
            event_count: checkpoint.event_count,
            max_ingest_seq: checkpoint.max_ingest_seq,
            key_generation: checkpoint.key_generation,
            gap_event_count,
        }
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.head_mac.len() != 64
            || !self
                .head_mac
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(GhostraceError::RepairManifestInvalid(
                "state manifest head MAC is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Before/after receipt for one verified-copy repair. No source path, payload,
/// event identifier, or key material is included.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairManifest {
    pub schema_version: u32,
    pub verified_copy: bool,
    pub before: RepairStateManifest,
    pub after: RepairStateManifest,
    pub intervals: Vec<RepairInterval>,
    pub dropped_event_count: u64,
    pub reconstructed_event_count: u64,
    pub gap_event_count: u64,
    pub repaired_at: String,
}

impl RepairManifest {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != REPAIR_MANIFEST_SCHEMA_VERSION
            || !self.verified_copy
            || self.intervals.is_empty()
            || self.intervals.len() > MAX_REPAIR_INTERVALS
            || self.gap_event_count != u64::try_from(self.intervals.len()).unwrap_or(u64::MAX)
            || self.reconstructed_event_count != 0
            || self.before.validate().is_err()
            || self.after.validate().is_err()
            || self.repaired_at.is_empty()
            || self.repaired_at.len() > MAX_CHECKPOINT_TIMESTAMP_BYTES
        {
            return Err(GhostraceError::RepairManifestInvalid(
                "repair manifest shape is invalid".to_owned(),
            ));
        }
        for interval in &self.intervals {
            interval.validate()?;
        }
        DateTime::parse_from_rfc3339(&self.repaired_at).map_err(|_| {
            GhostraceError::RepairManifestInvalid("repaired_at must be RFC3339".to_owned())
        })?;
        let expected_after = self
            .before
            .event_count
            .checked_sub(self.dropped_event_count)
            .and_then(|count| count.checked_add(self.gap_event_count))
            .ok_or_else(|| {
                GhostraceError::RepairManifestInvalid("event count overflow".to_owned())
            })?;
        if expected_after != self.after.event_count {
            return Err(GhostraceError::RepairManifestInvalid(
                "before/after event counts do not reconcile".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Hash a path-free integrity/state tuple for in-memory journals, or the
/// checkpointed database bytes for file-backed journals.
pub(crate) fn database_identity(
    path: Option<&Path>,
    report: &IntegrityReport,
    state: &AuthenticatedState,
) -> Result<SnapshotDigest, GhostraceError> {
    let bytes = if let Some(path) = path {
        std::fs::read(path)
            .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?
    } else {
        serde_json::to_vec(&(report, state))?
    };
    digest_bytes(&bytes)
}

pub(crate) fn digest_json<T: Serialize>(value: &T) -> Result<SnapshotDigest, GhostraceError> {
    digest_bytes(&serde_json::to_vec(value)?)
}

pub(crate) fn digest_bytes(bytes: &[u8]) -> Result<SnapshotDigest, GhostraceError> {
    let digest = Sha256::digest(bytes);
    SnapshotDigest::try_from(format!("sha256:{}", hex_digest(&digest)))
        .map_err(|_| GhostraceError::CheckpointInvalid("digest encoding failed".to_owned()))
}

fn canonical_checkpoint(checkpoint: &VerificationCheckpoint) -> Vec<u8> {
    let schema = checkpoint.schema_version.to_string();
    let journal_schema = checkpoint.journal_schema_version.to_string();
    let epoch = checkpoint.chain_epoch.to_string();
    let events = checkpoint.event_count.to_string();
    let max_seq = checkpoint.max_ingest_seq.to_string();
    let generation = checkpoint.key_generation.to_string();
    let fields = [
        ("domain", CHECKPOINT_DOMAIN),
        ("schema", schema.as_bytes()),
        ("database", checkpoint.database_identity.as_str().as_bytes()),
        ("journal-schema", journal_schema.as_bytes()),
        ("epoch", epoch.as_bytes()),
        ("head", checkpoint.head_mac.as_bytes()),
        ("events", events.as_bytes()),
        ("max-seq", max_seq.as_bytes()),
        ("generation", generation.as_bytes()),
        ("policy", checkpoint.policy_digest.as_str().as_bytes()),
        ("integrity", checkpoint.integrity_digest.as_str().as_bytes()),
        ("verified-at", checkpoint.verified_at.as_bytes()),
    ];
    let mut bytes = Vec::new();
    for (label, value) in fields {
        bytes.extend_from_slice(&(label.len() as u32).to_le_bytes());
        bytes.extend_from_slice(label.as_bytes());
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes
}

fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut ipad = [0x36_u8; 64];
    let mut opad = [0x5c_u8; 64];
    for (index, byte) in key.iter().enumerate() {
        ipad[index] ^= byte;
        opad[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
