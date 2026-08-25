//! Snapshot-consistent, privacy-bounded journal pagination.
//!
//! Page tokens are encrypted with the journal key.  They bind the complete
//! request, policy scope, event/storage schema versions, the ordering-contract
//! version, an ingest upper bound, and the last `(observed_at, ingest_seq,
//! event_id)` ordering key.  A token is
//! therefore a capability for one query shape and one logical snapshot, not a
//! caller-editable offset.

use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    crypto::{decrypt_payload, encrypt_payload, KeyProvider},
    error::GhostraceError,
    journal::StoredEvent,
    model::{EventKind, EventSource, PolicyProfileId, SnapshotDigest, EVENT_SCHEMA_VERSION},
    ordering::ORDERING_CONTRACT_VERSION,
    policy::PolicyProfile,
};

pub const QUERY_CONTRACT_VERSION: u32 = 1;
pub const DEFAULT_QUERY_PAGE_SIZE: usize = 50;
pub const MAX_QUERY_PAGE_SIZE: usize = 256;
pub const QUERY_TOKEN_TTL_SECONDS: i64 = 15 * 60;
const QUERY_TOKEN_AAD: &[u8] = b"ghostrace:query-token:v1";
const MAX_QUERY_TOKEN_BYTES: usize = 16 * 1024;
const QUERY_DIGEST_BYTES: usize = 71;

/// A complete, immutable query shape.  The policy identity and scope digest
/// are part of the shape so a token cannot be reused across profiles or scope
/// versions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryRequest {
    pub policy_profile_id: PolicyProfileId,
    pub policy_profile_version: u32,
    pub scope_digest: SnapshotDigest,
    pub source: Option<EventSource>,
    pub kind: Option<EventKind>,
    pub observed_from: Option<DateTime<Utc>>,
    pub observed_until: Option<DateTime<Utc>>,
    pub page_size: usize,
}

impl QueryRequest {
    /// Build a request from the exact policy choices that authorized ingestion.
    pub fn for_policy(policy: &PolicyProfile) -> Result<Self, GhostraceError> {
        let policy_profile_id = PolicyProfileId::try_from(policy.id.clone())
            .map_err(|_| GhostraceError::QueryInvalid)?;
        if policy.version == 0 {
            return Err(GhostraceError::QueryInvalid);
        }
        let scope_digest = policy
            .to_document()
            .and_then(|document| document.scope_digest())
            .map_err(|_| GhostraceError::QueryInvalid)?;
        Ok(Self {
            policy_profile_id,
            policy_profile_version: policy.version,
            scope_digest,
            source: None,
            kind: None,
            observed_from: None,
            observed_until: None,
            page_size: DEFAULT_QUERY_PAGE_SIZE,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), GhostraceError> {
        if self.policy_profile_version == 0
            || !(1..=MAX_QUERY_PAGE_SIZE).contains(&self.page_size)
            || self.observed_from.zip(self.observed_until).is_some_and(|(from, until)| from > until)
        {
            return Err(GhostraceError::QueryInvalid);
        }
        Ok(())
    }
}

/// One bounded page.  `snapshot_boundary` is an ingest-sequence upper bound,
/// not a promise that rows deleted by a later retention operation can return.
#[derive(Debug)]
pub struct QueryPage {
    pub events: Vec<StoredEvent>,
    pub next_page_token: Option<String>,
    pub snapshot_boundary: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct QueryTokenPayload {
    pub contract_version: u32,
    pub ordering_contract_version: u32,
    pub event_schema_version: u32,
    pub storage_schema_version: u32,
    pub policy_profile_id: PolicyProfileId,
    pub policy_profile_version: u32,
    pub scope_digest: SnapshotDigest,
    pub query_digest: String,
    pub snapshot_boundary: u64,
    pub issued_at: i64,
    pub expires_at: i64,
    pub last_order: Option<QueryOrderKey>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct QueryOrderKey {
    pub observed_at: String,
    pub ingest_seq: u64,
    pub event_id: Uuid,
}

pub(crate) fn query_digest(request: &QueryRequest) -> Result<String, GhostraceError> {
    request.validate()?;
    digest_string(&serde_json::to_vec(request).map_err(|_| GhostraceError::QueryInvalid)?)
}

pub(crate) fn encode_page_token(
    payload: &QueryTokenPayload,
    provider: &dyn KeyProvider,
) -> Result<String, GhostraceError> {
    validate_token_shape(payload)?;
    let bytes = serde_json::to_vec(payload).map_err(|_| GhostraceError::QueryTokenInvalid)?;
    let encrypted = encrypt_payload(provider, QUERY_TOKEN_AAD, &bytes)
        .map_err(|_| GhostraceError::QueryTokenInvalid)?;
    let encoded = hex_encode(&encrypted);
    if encoded.len() > MAX_QUERY_TOKEN_BYTES {
        return Err(GhostraceError::QueryTokenInvalid);
    }
    Ok(encoded)
}

pub(crate) fn decode_page_token(
    encoded: &str,
    provider: &dyn KeyProvider,
    now: i64,
) -> Result<QueryTokenPayload, GhostraceError> {
    if encoded.is_empty() || encoded.len() > MAX_QUERY_TOKEN_BYTES {
        return Err(GhostraceError::QueryTokenInvalid);
    }
    let encrypted = hex_decode(encoded).ok_or(GhostraceError::QueryTokenInvalid)?;
    let plaintext = decrypt_payload(provider, QUERY_TOKEN_AAD, &encrypted)
        .map_err(|_| GhostraceError::QueryTokenInvalid)?;
    let payload: QueryTokenPayload =
        serde_json::from_slice(&plaintext).map_err(|_| GhostraceError::QueryTokenInvalid)?;
    validate_token_shape(&payload)?;
    if payload.issued_at > now.saturating_add(60) {
        return Err(GhostraceError::QueryTokenInvalid);
    }
    if payload.expires_at <= now || payload.expires_at <= payload.issued_at {
        return Err(GhostraceError::QueryTokenExpired);
    }
    Ok(payload)
}

pub(crate) fn make_token(
    request: &QueryRequest,
    storage_schema_version: u32,
    snapshot_boundary: u64,
    last_order: Option<QueryOrderKey>,
    provider: &dyn KeyProvider,
    now: i64,
) -> Result<String, GhostraceError> {
    let expires_at =
        now.checked_add(QUERY_TOKEN_TTL_SECONDS).ok_or(GhostraceError::QueryTokenInvalid)?;
    let payload = QueryTokenPayload {
        contract_version: QUERY_CONTRACT_VERSION,
        ordering_contract_version: ORDERING_CONTRACT_VERSION,
        event_schema_version: EVENT_SCHEMA_VERSION,
        storage_schema_version,
        policy_profile_id: request.policy_profile_id.clone(),
        policy_profile_version: request.policy_profile_version,
        scope_digest: request.scope_digest.clone(),
        query_digest: query_digest(request)?,
        snapshot_boundary,
        issued_at: now,
        expires_at,
        last_order,
    };
    encode_page_token(&payload, provider)
}

pub(crate) fn validate_token_request(
    payload: &QueryTokenPayload,
    request: &QueryRequest,
    storage_schema_version: u32,
) -> Result<(), GhostraceError> {
    if payload.ordering_contract_version != ORDERING_CONTRACT_VERSION
        || payload.event_schema_version != EVENT_SCHEMA_VERSION
        || payload.storage_schema_version != storage_schema_version
    {
        return Err(GhostraceError::QuerySchemaChanged);
    }
    if payload.policy_profile_id != request.policy_profile_id
        || payload.policy_profile_version != request.policy_profile_version
        || payload.scope_digest != request.scope_digest
        || payload.query_digest != query_digest(request)?
    {
        return Err(GhostraceError::QueryTokenMismatch);
    }
    if payload.last_order.as_ref().is_some_and(|order| order.ingest_seq > payload.snapshot_boundary)
    {
        return Err(GhostraceError::QueryTokenInvalid);
    }
    Ok(())
}

fn validate_token_shape(payload: &QueryTokenPayload) -> Result<(), GhostraceError> {
    if payload.contract_version != QUERY_CONTRACT_VERSION
        || payload.ordering_contract_version != ORDERING_CONTRACT_VERSION
        || payload.event_schema_version != EVENT_SCHEMA_VERSION
        || payload.storage_schema_version == 0
        || payload.policy_profile_version == 0
        || payload.snapshot_boundary == 0 && payload.last_order.is_some()
        || !valid_digest(&payload.query_digest)
        || payload.last_order.as_ref().is_some_and(|order| {
            order.event_id.is_nil()
                || DateTime::parse_from_rfc3339(&order.observed_at).is_err()
                || order.ingest_seq == 0
        })
    {
        return Err(GhostraceError::QueryTokenInvalid);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == QUERY_DIGEST_BYTES
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn digest_string(bytes: &[u8]) -> Result<String, GhostraceError> {
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity(QUERY_DIGEST_BYTES);
    value.push_str("sha256:");
    for byte in digest {
        write!(&mut value, "{byte:02x}").map_err(|_| GhostraceError::QueryInvalid)?;
    }
    Ok(value)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing hex cannot fail");
    }
    value
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let raw = value.as_bytes();
    for pair in raw.chunks_exact(2) {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::DeterministicKeyProvider;

    #[test]
    fn expired_tokens_are_rejected_without_exposing_payload() {
        let policy = PolicyProfile::fixture_default();
        let request = QueryRequest::for_policy(&policy).expect("request");
        let now = 1_800_000_000;
        let payload = QueryTokenPayload {
            contract_version: QUERY_CONTRACT_VERSION,
            ordering_contract_version: ORDERING_CONTRACT_VERSION,
            event_schema_version: EVENT_SCHEMA_VERSION,
            storage_schema_version: 4,
            policy_profile_id: request.policy_profile_id.clone(),
            policy_profile_version: request.policy_profile_version,
            scope_digest: request.scope_digest.clone(),
            query_digest: query_digest(&request).expect("digest"),
            snapshot_boundary: 1,
            issued_at: now - QUERY_TOKEN_TTL_SECONDS - 1,
            expires_at: now - 1,
            last_order: None,
        };
        let provider = DeterministicKeyProvider::from_seed("query-expired-token");
        let encoded = encode_page_token(&payload, &provider).expect("token");
        assert!(matches!(
            decode_page_token(&encoded, &provider, now),
            Err(GhostraceError::QueryTokenExpired)
        ));
    }
}
