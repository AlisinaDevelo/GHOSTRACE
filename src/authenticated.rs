//! Keyed authentication for the journal's mutable metadata and ordering state.
//!
//! This is a local-integrity contract, not a remote attestation mechanism.  A
//! keyed SHA-256 chain binds the current event rows, cursor rows, policy
//! history, diagnostics, and explicit deletion boundaries.  The canonical
//! bytes and domain separator are deliberately versioned so a future format
//! can reject rather than reinterpret old state.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{crypto::KeyProvider, error::GhostraceError};

/// Wire version for the authenticated journal state contract.
pub const AUTHENTICATED_STATE_SCHEMA_VERSION: u32 = 1;
/// Public domain separator for every keyed digest in this module.
pub const AUTHENTICATED_STATE_DOMAIN: &str = "ghostrace:authenticated-journal-state:v1";

const EMPTY_DELETION_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const MAX_AUTH_FIELD_BYTES: usize = 16 * 1024 * 1024;
const MAX_ANOMALIES: usize = 16;

/// A bounded marker retained in the chain when the official retention command
/// removes rows.  Event identifiers are intentionally not retained here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedDeletionMarker {
    pub plan_digest: String,
    pub candidate_set_digest: String,
    pub snapshot_boundary: u64,
    pub requested_event_count: u64,
    pub deleted_event_count: u64,
}

impl AuthenticatedDeletionMarker {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        for (label, value) in
            [("plan digest", &self.plan_digest), ("candidate digest", &self.candidate_set_digest)]
        {
            if !value.starts_with("sha256:") || value.len() != 71 {
                return Err(GhostraceError::AuthenticatedStateInvalid(format!(
                    "{label} is not a canonical digest"
                )));
            }
        }
        if self.deleted_event_count > self.requested_event_count {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "deletion marker count is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// The durable keyed anchor.  It contains no key material, paths, payloads,
/// or event identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedState {
    pub state_key: String,
    pub schema_version: u32,
    pub chain_epoch: u64,
    pub chain_start_mac: String,
    pub head_mac: String,
    pub key_generation: u32,
    pub event_count: u64,
    pub max_ingest_seq: u64,
    pub event_order_digest: String,
    pub event_set_digest: String,
    pub event_content_digest: String,
    pub cursor_digest: String,
    pub policy_digest: String,
    pub diagnostic_digest: String,
    pub deletion_count: u64,
    pub deletion_digest: String,
    pub updated_at: String,
}

impl AuthenticatedState {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.state_key != "journal"
            || self.schema_version != AUTHENTICATED_STATE_SCHEMA_VERSION
            || self.chain_start_mac.len() != 64
            || self.head_mac.len() != 64
            || !self.chain_start_mac.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !self.head_mac.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !valid_sha256_digest(&self.event_order_digest)
            || !valid_sha256_digest(&self.event_set_digest)
            || !valid_sha256_digest(&self.event_content_digest)
            || !valid_sha256_digest(&self.cursor_digest)
            || !valid_sha256_digest(&self.policy_digest)
            || !valid_sha256_digest(&self.diagnostic_digest)
            || !valid_sha256_digest(&self.deletion_digest)
            || self.updated_at.is_empty()
        {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated state shape is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A path-free classification emitted by the verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticatedAnomaly {
    AnchorMissing,
    AnchorInvalid,
    EventInserted,
    EventDeleted,
    EventReordered,
    EventEdited,
    ChainTruncated,
    CursorRollback,
    PolicySubstitution,
    DiagnosticTampering,
    KeyUnavailable,
}

/// Bounded verifier output.  `local_key_only` explicitly prevents callers
/// from treating a successful result as origin authenticity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedStateReport {
    pub schema_version: u32,
    pub valid: bool,
    pub chain_epoch: u64,
    pub key_generation: u32,
    pub event_count: u64,
    pub stored_event_count: u64,
    pub max_ingest_seq: u64,
    pub stored_max_ingest_seq: u64,
    pub deletion_count: u64,
    pub anomalies: Vec<AuthenticatedAnomaly>,
    pub local_key_only: bool,
    pub message: String,
}

impl AuthenticatedStateReport {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != AUTHENTICATED_STATE_SCHEMA_VERSION
            || self.anomalies.len() > MAX_ANOMALIES
            || !self.local_key_only
            || self.message.is_empty()
            || self.message.len() > 256
        {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated report shape is invalid".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn origin_authenticity_limit(&self) -> &'static str {
        "validity is bounded to possession of the configured local journal key; it is not origin attestation"
    }
}

#[derive(Clone, Debug)]
struct CanonicalSnapshot {
    event_count: u64,
    max_ingest_seq: u64,
    event_order_digest: String,
    event_set_digest: String,
    event_content_digest: String,
    cursor_digest: String,
    policy_digest: String,
    diagnostic_digest: String,
}

/// Seed the anchor after migrations.  Existing pre-authentication journals are
/// explicitly bootstrapped at their first open; subsequent mutations are
/// authenticated transactionally.
pub(crate) fn ensure_anchor(
    connection: &mut Connection,
    provider: &dyn KeyProvider,
) -> Result<(), GhostraceError> {
    let exists: Option<String> = connection
        .query_row(
            "SELECT state_key FROM authenticated_state WHERE state_key = 'journal'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_some() {
        return Ok(());
    }
    let bootstrap: Option<String> = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata
             WHERE metadata_key = 'authenticated_state_bootstrap'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if bootstrap.as_deref() != Some("pending") {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated anchor is missing".to_owned(),
        ));
    }
    let snapshot = canonical_snapshot(connection)?;
    let state = new_state(provider, snapshot, None, None)?;
    let transaction = connection.transaction()?;
    insert_state(&transaction, &state)?;
    transaction.execute(
        "UPDATE journal_metadata SET metadata_value = 'complete'
         WHERE metadata_key = 'authenticated_state_bootstrap'",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Recompute and persist the keyed anchor within the caller's write
/// transaction.  Every event, cursor, policy, diagnostic, and retention
/// mutation calls this before commit, so a rollback leaves no authenticated
/// half-state.
pub(crate) fn refresh_transaction(
    transaction: &Transaction<'_>,
    provider: &dyn KeyProvider,
    deletion: Option<&AuthenticatedDeletionMarker>,
) -> Result<(), GhostraceError> {
    if let Some(marker) = deletion {
        marker.validate()?;
    }
    let snapshot = canonical_snapshot(transaction)?;
    let state = match load_state(transaction)? {
        Some(previous) => {
            previous.validate()?;
            next_state(provider, previous, snapshot, deletion)?
        }
        None => new_state(provider, snapshot, None, deletion)?,
    };
    insert_state(transaction, &state)
}

pub(crate) fn report(
    connection: &Connection,
    provider: &dyn KeyProvider,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    let snapshot = canonical_snapshot(connection)?;
    let Some(state) = load_state(connection)? else {
        return report_with_anomalies(None, &snapshot, vec![AuthenticatedAnomaly::AnchorMissing]);
    };
    if state.validate().is_err() {
        return report_with_anomalies(
            Some(&state),
            &snapshot,
            vec![AuthenticatedAnomaly::AnchorInvalid],
        );
    }

    let mut anomalies = Vec::new();
    if snapshot.event_count > state.event_count {
        anomalies.push(AuthenticatedAnomaly::EventInserted);
    } else if snapshot.event_count < state.event_count {
        anomalies.push(AuthenticatedAnomaly::EventDeleted);
    }
    if snapshot.max_ingest_seq < state.max_ingest_seq {
        anomalies.push(AuthenticatedAnomaly::ChainTruncated);
    }
    if snapshot.event_set_digest == state.event_set_digest
        && snapshot.event_order_digest != state.event_order_digest
    {
        anomalies.push(AuthenticatedAnomaly::EventReordered);
    }
    if snapshot.event_content_digest != state.event_content_digest
        && snapshot.event_set_digest == state.event_set_digest
        && snapshot.event_order_digest == state.event_order_digest
    {
        anomalies.push(AuthenticatedAnomaly::EventEdited);
    }
    if snapshot.cursor_digest != state.cursor_digest {
        anomalies.push(AuthenticatedAnomaly::CursorRollback);
    }
    if snapshot.policy_digest != state.policy_digest {
        anomalies.push(AuthenticatedAnomaly::PolicySubstitution);
    }
    if snapshot.diagnostic_digest != state.diagnostic_digest {
        anomalies.push(AuthenticatedAnomaly::DiagnosticTampering);
    }

    let key = match provider.key_for_generation(state.key_generation) {
        Ok(key) => key,
        Err(_) => {
            anomalies.push(AuthenticatedAnomaly::KeyUnavailable);
            return report_with_anomalies(Some(&state), &snapshot, anomalies);
        }
    };
    let expected = head_mac(&state, &key);
    if expected != state.head_mac {
        anomalies.push(AuthenticatedAnomaly::AnchorInvalid);
    }
    report_with_anomalies(Some(&state), &snapshot, anomalies)
}

pub(crate) fn require_valid(
    connection: &Connection,
    provider: &dyn KeyProvider,
) -> Result<(), GhostraceError> {
    let report = report(connection, provider)?;
    if report.valid {
        Ok(())
    } else {
        Err(GhostraceError::AuthenticatedStateInvalid(report.message))
    }
}

pub(crate) fn load_public(connection: &Connection) -> Result<AuthenticatedState, GhostraceError> {
    let state = load_state(connection)?.ok_or_else(|| {
        GhostraceError::AuthenticatedStateInvalid("authenticated anchor is missing".to_owned())
    })?;
    state.validate()?;
    Ok(state)
}

fn report_with_anomalies(
    state: Option<&AuthenticatedState>,
    snapshot: &CanonicalSnapshot,
    anomalies: Vec<AuthenticatedAnomaly>,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    let mut anomalies = anomalies;
    anomalies.sort_by_key(|anomaly| *anomaly as u8);
    anomalies.dedup();
    if anomalies.len() > MAX_ANOMALIES {
        anomalies.truncate(MAX_ANOMALIES);
    }
    let valid = anomalies.is_empty();
    let report = AuthenticatedStateReport {
        schema_version: AUTHENTICATED_STATE_SCHEMA_VERSION,
        valid,
        chain_epoch: state.map_or(0, |state| state.chain_epoch),
        key_generation: state.map_or(0, |state| state.key_generation),
        event_count: snapshot.event_count,
        stored_event_count: state.map_or(0, |state| state.event_count),
        max_ingest_seq: snapshot.max_ingest_seq,
        stored_max_ingest_seq: state.map_or(0, |state| state.max_ingest_seq),
        deletion_count: state.map_or(0, |state| state.deletion_count),
        anomalies,
        local_key_only: true,
        message: if state.is_none() {
            "authenticated anchor is missing".to_owned()
        } else if valid {
            "authenticated journal state is valid".to_owned()
        } else {
            "authenticated journal state failed verification".to_owned()
        },
    };
    report.validate()?;
    Ok(report)
}

fn new_state(
    provider: &dyn KeyProvider,
    snapshot: CanonicalSnapshot,
    previous: Option<&AuthenticatedState>,
    deletion: Option<&AuthenticatedDeletionMarker>,
) -> Result<AuthenticatedState, GhostraceError> {
    let generation = provider.key_generation();
    let key = provider.key_for_generation(generation)?;
    let chain_epoch = previous.map_or(0, |state| state.chain_epoch);
    let start_material = canonical_fields(&[
        ("kind", b"chain-start"),
        ("epoch", &chain_epoch.to_le_bytes()),
        ("generation", &generation.to_le_bytes()),
    ]);
    let chain_start_mac = keyed_hex(&key, &start_material);
    let deletion_digest = deletion.map_or_else(
        || EMPTY_DELETION_DIGEST.to_owned(),
        |marker| deletion_digest(EMPTY_DELETION_DIGEST, marker),
    );
    let state = AuthenticatedState {
        state_key: "journal".to_owned(),
        schema_version: AUTHENTICATED_STATE_SCHEMA_VERSION,
        chain_epoch,
        chain_start_mac,
        head_mac: String::new(),
        key_generation: generation,
        event_count: snapshot.event_count,
        max_ingest_seq: snapshot.max_ingest_seq,
        event_order_digest: snapshot.event_order_digest,
        event_set_digest: snapshot.event_set_digest,
        event_content_digest: snapshot.event_content_digest,
        cursor_digest: snapshot.cursor_digest,
        policy_digest: snapshot.policy_digest,
        diagnostic_digest: snapshot.diagnostic_digest,
        deletion_count: deletion.map_or(0, |_| 1),
        deletion_digest,
        updated_at: Utc::now().to_rfc3339(),
    };
    finish_state(state, &key)
}

fn next_state(
    provider: &dyn KeyProvider,
    previous: AuthenticatedState,
    snapshot: CanonicalSnapshot,
    deletion: Option<&AuthenticatedDeletionMarker>,
) -> Result<AuthenticatedState, GhostraceError> {
    let generation = provider.key_generation();
    let key = provider.key_for_generation(generation)?;
    let boundary_change = deletion.is_some() || generation != previous.key_generation;
    let chain_epoch = if boundary_change {
        previous.chain_epoch.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("chain epoch overflow".to_owned())
        })?
    } else {
        previous.chain_epoch
    };
    let chain_start_mac = if boundary_change {
        let marker =
            deletion.map(canonical_deletion).unwrap_or_else(|| b"key-generation-boundary".to_vec());
        let material = canonical_fields(&[
            ("kind", b"chain-boundary"),
            ("previous-head", previous.head_mac.as_bytes()),
            ("epoch", &chain_epoch.to_le_bytes()),
            ("generation", &generation.to_le_bytes()),
            ("marker", &marker),
        ]);
        keyed_hex(&key, &material)
    } else {
        previous.chain_start_mac.clone()
    };
    let deletion_count = if deletion.is_some() {
        previous.deletion_count.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("deletion count overflow".to_owned())
        })?
    } else {
        previous.deletion_count
    };
    let deletion_digest = deletion.map_or_else(
        || previous.deletion_digest.clone(),
        |marker| deletion_digest(&previous.deletion_digest, marker),
    );
    let state = AuthenticatedState {
        state_key: "journal".to_owned(),
        schema_version: AUTHENTICATED_STATE_SCHEMA_VERSION,
        chain_epoch,
        chain_start_mac,
        head_mac: String::new(),
        key_generation: generation,
        event_count: snapshot.event_count,
        max_ingest_seq: snapshot.max_ingest_seq,
        event_order_digest: snapshot.event_order_digest,
        event_set_digest: snapshot.event_set_digest,
        event_content_digest: snapshot.event_content_digest,
        cursor_digest: snapshot.cursor_digest,
        policy_digest: snapshot.policy_digest,
        diagnostic_digest: snapshot.diagnostic_digest,
        deletion_count,
        deletion_digest,
        updated_at: Utc::now().to_rfc3339(),
    };
    finish_state(state, &key)
}

fn finish_state(
    mut state: AuthenticatedState,
    key: &[u8; 32],
) -> Result<AuthenticatedState, GhostraceError> {
    state.head_mac = head_mac(&state, key);
    state.validate()?;
    Ok(state)
}

fn head_mac(state: &AuthenticatedState, key: &[u8; 32]) -> String {
    keyed_hex(key, &canonical_state(state))
}

fn canonical_state(state: &AuthenticatedState) -> Vec<u8> {
    canonical_fields(&[
        ("domain", AUTHENTICATED_STATE_DOMAIN.as_bytes()),
        ("schema", &state.schema_version.to_le_bytes()),
        ("epoch", &state.chain_epoch.to_le_bytes()),
        ("chain-start", state.chain_start_mac.as_bytes()),
        ("generation", &state.key_generation.to_le_bytes()),
        ("event-count", &state.event_count.to_le_bytes()),
        ("max-ingest-seq", &state.max_ingest_seq.to_le_bytes()),
        ("event-order", state.event_order_digest.as_bytes()),
        ("event-set", state.event_set_digest.as_bytes()),
        ("event-content", state.event_content_digest.as_bytes()),
        ("cursor", state.cursor_digest.as_bytes()),
        ("policy", state.policy_digest.as_bytes()),
        ("diagnostic", state.diagnostic_digest.as_bytes()),
        ("deletion-count", &state.deletion_count.to_le_bytes()),
        ("deletion", state.deletion_digest.as_bytes()),
    ])
}

fn canonical_snapshot(connection: &Connection) -> Result<CanonicalSnapshot, GhostraceError> {
    let mut event_order = Sha256::new();
    event_order.update(b"ghostrace:event-order:v1\0");
    let mut event_set = Vec::new();
    let mut event_content = Sha256::new();
    event_content.update(b"ghostrace:event-content:v1\0");
    let mut statement = connection.prepare(
        "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                kind, collector_instance, source_cursor, provenance_version,
                policy_profile_id, policy_profile_version, evidence, parent_event_id,
                payload_ciphertext
         FROM events ORDER BY ingest_seq ASC",
    )?;
    let mut rows = statement.query([])?;
    let mut event_count = 0_u64;
    let mut max_ingest_seq = 0_u64;
    while let Some(row) = rows.next()? {
        let ingest_seq = to_u64(row.get::<_, i64>(0)?, "event ingest sequence")?;
        let event_id: String = row.get(1)?;
        bounded_bytes("event ID", event_id.as_bytes())?;
        let mut order_bytes = Vec::new();
        put_field(&mut order_bytes, "seq", &ingest_seq.to_le_bytes());
        put_field(&mut order_bytes, "event", event_id.as_bytes());
        event_order.update(&order_bytes);
        event_set.push(event_id.clone());

        let mut content = Vec::new();
        put_field(&mut content, "seq", &ingest_seq.to_le_bytes());
        put_field(&mut content, "event", event_id.as_bytes());
        put_field(&mut content, "schema", &row.get::<_, i64>(2)?.to_le_bytes());
        put_field(&mut content, "observed", bounded_string(row.get(3)?)?.as_bytes());
        put_field(&mut content, "ingested", bounded_string(row.get(4)?)?.as_bytes());
        put_field(&mut content, "source", bounded_string(row.get(5)?)?.as_bytes());
        put_field(&mut content, "kind", bounded_string(row.get(6)?)?.as_bytes());
        put_field(&mut content, "collector", bounded_string(row.get(7)?)?.as_bytes());
        put_optional_field(&mut content, "cursor", row.get::<_, Option<String>>(8)?)?;
        put_field(&mut content, "provenance", bounded_string(row.get(9)?)?.as_bytes());
        put_field(&mut content, "policy-id", bounded_string(row.get(10)?)?.as_bytes());
        put_field(&mut content, "policy-version", &row.get::<_, i64>(11)?.to_le_bytes());
        put_field(&mut content, "evidence", bounded_string(row.get(12)?)?.as_bytes());
        put_optional_field(&mut content, "parent", row.get::<_, Option<String>>(13)?)?;
        let ciphertext: Vec<u8> = row.get(14)?;
        bounded_bytes("event ciphertext", &ciphertext)?;
        put_field(&mut content, "ciphertext", &ciphertext);
        event_content.update(canonical_fields(&[("row", &content)]));
        event_count = event_count.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("event count overflow".to_owned())
        })?;
        max_ingest_seq = max_ingest_seq.max(ingest_seq);
    }
    event_set.sort_unstable();
    let mut event_set_hasher = Sha256::new();
    event_set_hasher.update(b"ghostrace:event-set:v1\0");
    for event_id in event_set {
        put_field_hash(&mut event_set_hasher, "event", event_id.as_bytes());
    }

    let cursor_digest = digest_cursor_table(connection)?;
    let policy_digest = digest_policy_table(connection)?;
    let diagnostic_digest = digest_diagnostic_table(connection)?;
    Ok(CanonicalSnapshot {
        event_count,
        max_ingest_seq,
        event_order_digest: sha_digest(event_order.finalize().as_slice()),
        event_set_digest: sha_digest(event_set_hasher.finalize().as_slice()),
        event_content_digest: sha_digest(event_content.finalize().as_slice()),
        cursor_digest,
        policy_digest,
        diagnostic_digest,
    })
}

fn digest_cursor_table(connection: &Connection) -> Result<String, GhostraceError> {
    let mut digest = Sha256::new();
    digest.update(b"ghostrace:cursor-state:v1\0");
    let mut statement = connection.prepare(
        "SELECT source, collector_instance, source_cursor, updated_at, epoch, state,
                cursor_kind, policy_profile_id, policy_profile_version, last_event_id,
                boundary_json
         FROM cursors ORDER BY source, collector_instance",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut bytes = Vec::new();
        for index in 0..4 {
            put_field(&mut bytes, "text", bounded_string(row.get(index)?)?.as_bytes());
        }
        put_field(&mut bytes, "epoch", &row.get::<_, i64>(4)?.to_le_bytes());
        put_field(&mut bytes, "state", bounded_string(row.get(5)?)?.as_bytes());
        put_field(&mut bytes, "kind", bounded_string(row.get(6)?)?.as_bytes());
        put_optional_field(&mut bytes, "policy-id", row.get::<_, Option<String>>(7)?)?;
        put_optional_field(
            &mut bytes,
            "policy-version",
            row.get::<_, Option<i64>>(8)?.map(|v| v.to_string()),
        )?;
        put_optional_field(&mut bytes, "last-event", row.get::<_, Option<String>>(9)?)?;
        put_optional_field(&mut bytes, "boundary", row.get::<_, Option<String>>(10)?)?;
        digest.update(canonical_fields(&[("row", &bytes)]));
    }
    Ok(sha_digest(digest.finalize().as_slice()))
}

fn digest_policy_table(connection: &Connection) -> Result<String, GhostraceError> {
    let mut digest = Sha256::new();
    digest.update(b"ghostrace:policy-state:v1\0");
    let mut statement = connection.prepare(
        "SELECT profile_id, profile_version, profile_json, recorded_at
         FROM policy_metadata ORDER BY profile_id, profile_version",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut bytes = Vec::new();
        put_field(&mut bytes, "id", bounded_string(row.get(0)?)?.as_bytes());
        put_field(&mut bytes, "version", &row.get::<_, i64>(1)?.to_le_bytes());
        put_field(&mut bytes, "json", bounded_string(row.get(2)?)?.as_bytes());
        put_field(&mut bytes, "recorded", bounded_string(row.get(3)?)?.as_bytes());
        digest.update(canonical_fields(&[("row", &bytes)]));
    }
    Ok(sha_digest(digest.finalize().as_slice()))
}

fn digest_diagnostic_table(connection: &Connection) -> Result<String, GhostraceError> {
    let mut digest = Sha256::new();
    digest.update(b"ghostrace:diagnostic-state:v1\0");
    let mut statement = connection.prepare(
        "SELECT diagnostic_id, code, detail, created_at FROM diagnostics ORDER BY diagnostic_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut bytes = Vec::new();
        put_field(&mut bytes, "id", &row.get::<_, i64>(0)?.to_le_bytes());
        put_field(&mut bytes, "code", bounded_string(row.get(1)?)?.as_bytes());
        put_field(&mut bytes, "detail", bounded_string(row.get(2)?)?.as_bytes());
        put_field(&mut bytes, "created", bounded_string(row.get(3)?)?.as_bytes());
        digest.update(canonical_fields(&[("row", &bytes)]));
    }
    Ok(sha_digest(digest.finalize().as_slice()))
}

fn load_state(connection: &Connection) -> Result<Option<AuthenticatedState>, GhostraceError> {
    connection
        .query_row(
            "SELECT state_key, schema_version, chain_epoch, chain_start_mac, head_mac,
                    key_generation, event_count, max_ingest_seq, event_order_digest,
                    event_set_digest, event_content_digest, cursor_digest, policy_digest,
                    diagnostic_digest, deletion_count, deletion_digest, updated_at
             FROM authenticated_state WHERE state_key = 'journal'",
            [],
            |row| {
                Ok(AuthenticatedState {
                    state_key: row.get(0)?,
                    schema_version: to_u32_sql(row.get(1)?, "auth schema")?,
                    chain_epoch: to_u64_sql(row.get(2)?, "chain epoch")?,
                    chain_start_mac: row.get(3)?,
                    head_mac: row.get(4)?,
                    key_generation: to_u32_sql(row.get(5)?, "key generation")?,
                    event_count: to_u64_sql(row.get(6)?, "event count")?,
                    max_ingest_seq: to_u64_sql(row.get(7)?, "max ingest sequence")?,
                    event_order_digest: row.get(8)?,
                    event_set_digest: row.get(9)?,
                    event_content_digest: row.get(10)?,
                    cursor_digest: row.get(11)?,
                    policy_digest: row.get(12)?,
                    diagnostic_digest: row.get(13)?,
                    deletion_count: to_u64_sql(row.get(14)?, "deletion count")?,
                    deletion_digest: row.get(15)?,
                    updated_at: row.get(16)?,
                })
            },
        )
        .optional()
        .map_err(GhostraceError::from)
}

fn insert_state(
    transaction: &Transaction<'_>,
    state: &AuthenticatedState,
) -> Result<(), GhostraceError> {
    transaction.execute(
        "INSERT INTO authenticated_state(
            state_key, schema_version, chain_epoch, chain_start_mac, head_mac,
            key_generation, event_count, max_ingest_seq, event_order_digest,
            event_set_digest, event_content_digest, cursor_digest, policy_digest,
            diagnostic_digest, deletion_count, deletion_digest, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(state_key) DO UPDATE SET
            schema_version=excluded.schema_version, chain_epoch=excluded.chain_epoch,
            chain_start_mac=excluded.chain_start_mac, head_mac=excluded.head_mac,
            key_generation=excluded.key_generation, event_count=excluded.event_count,
            max_ingest_seq=excluded.max_ingest_seq, event_order_digest=excluded.event_order_digest,
            event_set_digest=excluded.event_set_digest, event_content_digest=excluded.event_content_digest,
            cursor_digest=excluded.cursor_digest, policy_digest=excluded.policy_digest,
            diagnostic_digest=excluded.diagnostic_digest, deletion_count=excluded.deletion_count,
            deletion_digest=excluded.deletion_digest, updated_at=excluded.updated_at",
        params![
            state.state_key,
            state.schema_version,
            state.chain_epoch as i64,
            state.chain_start_mac,
            state.head_mac,
            state.key_generation,
            state.event_count as i64,
            state.max_ingest_seq as i64,
            state.event_order_digest,
            state.event_set_digest,
            state.event_content_digest,
            state.cursor_digest,
            state.policy_digest,
            state.diagnostic_digest,
            state.deletion_count as i64,
            state.deletion_digest,
            state.updated_at,
        ],
    )?;
    Ok(())
}

fn deletion_digest(previous: &str, marker: &AuthenticatedDeletionMarker) -> String {
    let bytes = canonical_fields(&[
        ("domain", AUTHENTICATED_STATE_DOMAIN.as_bytes()),
        ("previous", previous.as_bytes()),
        ("marker", &canonical_deletion(marker)),
    ]);
    sha_digest(Sha256::digest(bytes).as_slice())
}

fn canonical_deletion(marker: &AuthenticatedDeletionMarker) -> Vec<u8> {
    canonical_fields(&[
        ("plan", marker.plan_digest.as_bytes()),
        ("candidate", marker.candidate_set_digest.as_bytes()),
        ("boundary", &marker.snapshot_boundary.to_le_bytes()),
        ("requested", &marker.requested_event_count.to_le_bytes()),
        ("deleted", &marker.deleted_event_count.to_le_bytes()),
    ])
}

fn canonical_fields(fields: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for (label, value) in fields {
        put_field(&mut bytes, label, value);
    }
    bytes
}

fn put_field(output: &mut Vec<u8>, label: &str, value: &[u8]) {
    output.extend_from_slice(&(label.len() as u32).to_le_bytes());
    output.extend_from_slice(label.as_bytes());
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}

fn put_field_hash(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(canonical_fields(&[(label, value)]));
}

fn put_optional_field(
    output: &mut Vec<u8>,
    label: &str,
    value: Option<String>,
) -> Result<(), GhostraceError> {
    match value {
        Some(value) => put_field(output, label, bounded_string(value)?.as_bytes()),
        None => put_field(output, label, b"<none>"),
    }
    Ok(())
}

fn bounded_string(value: String) -> Result<String, GhostraceError> {
    bounded_bytes("authenticated text", value.as_bytes())?;
    if value.chars().any(char::is_control) {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated text contains a control character".to_owned(),
        ));
    }
    Ok(value)
}

fn bounded_bytes(label: &str, value: &[u8]) -> Result<(), GhostraceError> {
    if value.len() > MAX_AUTH_FIELD_BYTES {
        return Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} exceeds the authenticated bound"
        )));
    }
    Ok(())
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(71);
    output.push_str("sha256:");
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn keyed_hex(key: &[u8; 32], bytes: &[u8]) -> String {
    let mut ipad = [0x36_u8; 64];
    let mut opad = [0x5c_u8; 64];
    for index in 0..32 {
        ipad[index] ^= key[index];
        opad[index] ^= key[index];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(bytes);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let digest = outer.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn to_u64(value: i64, label: &str) -> Result<u64, GhostraceError> {
    u64::try_from(value)
        .map_err(|_| GhostraceError::AuthenticatedStateInvalid(format!("{label} is negative")))
}

fn to_u32_sql(value: i64, label: &str) -> Result<u32, rusqlite::Error> {
    u32::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, label)),
        )
    })
}

fn to_u64_sql(value: i64, label: &str) -> Result<u64, rusqlite::Error> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, label)),
        )
    })
}
