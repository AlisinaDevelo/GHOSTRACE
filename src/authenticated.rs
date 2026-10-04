//! Keyed authentication for the journal's mutable metadata and ordering state.
//!
//! This is a local-integrity contract, not a remote attestation mechanism.  A
//! keyed SHA-256 chain binds the current event rows, cursor rows, policy
//! history, diagnostics, and explicit deletion boundaries.  The canonical
//! bytes and domain separator are deliberately versioned so a future format
//! can reject rather than reinterpret old state.

use chrono::Utc;
use rusqlite::{
    params,
    types::{Type, ValueRef},
    Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::cell::Cell;
use std::{
    collections::{BTreeMap, HashSet},
    convert::TryInto,
};

use crate::{
    crypto::KeyProvider,
    error::GhostraceError,
    model::{MAX_CURSOR_BYTES, MAX_EVENT_PAYLOAD_BYTES, MAX_IDENTIFIER_BYTES},
};

/// Legacy wire version for the authenticated journal state contract.
pub const AUTHENTICATED_STATE_V1_SCHEMA_VERSION: u32 = 1;
/// Current wire version for the authenticated journal state contract.
pub const AUTHENTICATED_STATE_SCHEMA_VERSION: u32 = 2;
/// Public domain separator for the legacy keyed digest contract.
pub const AUTHENTICATED_STATE_DOMAIN: &str = "ghostrace:authenticated-journal-state:v1";
/// Public domain separator for the incremental keyed digest contract.
pub const AUTHENTICATED_STATE_V2_DOMAIN: &str = "ghostrace:authenticated-journal-state:v2";

const EMPTY_DELETION_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const MAX_ANOMALIES: usize = 16;
const EMPTY_INCREMENTAL_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

const MAX_AUTH_TRIGGER_SQL_BYTES: usize = 64 * 1024;
// These are authentication-record bounds, not generic SQLite limits.  Every
// byte/string helper below checks the raw SQLite value before allocating it.
const MAX_STATE_KEY_BYTES: usize = 32;
const MAX_MAC_HEX_BYTES: usize = 64;
const MAX_DIGEST_TEXT_BYTES: usize = 71;
const MAX_TIMESTAMP_BYTES: usize = 128;
const MAX_TABLE_NAME_BYTES: usize = 32;
const MAX_TRIGGER_NAME_BYTES: usize = 64;
const MAX_DATABASE_NAME_BYTES: usize = 64;
const MAX_LOGICAL_KEY_BYTES: usize = 512;
const MAX_OPERATION_KIND_BYTES: usize = 16;
const MAX_CONTEXT_BYTES: usize = 512;
const MAX_DIAGNOSTIC_CODE_BYTES: usize = 64;
const MAX_DIAGNOSTIC_DETAIL_BYTES: usize = 512;
const MAX_POLICY_JSON_BYTES: usize = 64 * 1024;
const MAX_EVENT_CIPHERTEXT_BYTES: usize = MAX_EVENT_PAYLOAD_BYTES + 64;
const MAX_EVENT_ID_BYTES: usize = 36;
const MAX_EVENT_LOCATOR_BYTES: usize = 20;
const MAX_BOUNDARY_JSON_BYTES: usize = 4 * 1024;

const AUTHENTICATED_TABLES: [&str; 9] = [
    "events",
    "cursors",
    "policy_metadata",
    "diagnostics",
    "authenticated_state",
    "authenticated_commitments",
    "authenticated_operations",
    "authenticated_operation_changes",
    "authenticated_pending_changes",
];

struct AuthenticationTrigger {
    name: &'static str,
    table_name: &'static str,
    sql: &'static str,
}

const AUTHENTICATION_TRIGGERS: [AuthenticationTrigger; 12] = [
    AuthenticationTrigger {
        name: "authenticated_events_ai",
        table_name: "events",
        sql: "CREATE TRIGGER authenticated_events_ai
         AFTER INSERT ON events
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('events', CAST(NEW.ingest_seq AS TEXT), 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_events_au",
        table_name: "events",
        sql: "CREATE TRIGGER authenticated_events_au
         AFTER UPDATE ON events
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('events', CAST(OLD.ingest_seq AS TEXT), 'delete');
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('events', CAST(NEW.ingest_seq AS TEXT), 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_events_ad",
        table_name: "events",
        sql: "CREATE TRIGGER authenticated_events_ad
         AFTER DELETE ON events
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('events', CAST(OLD.ingest_seq AS TEXT), 'delete');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_cursors_ai",
        table_name: "cursors",
        sql: "CREATE TRIGGER authenticated_cursors_ai
         AFTER INSERT ON cursors
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('cursors', NEW.source || char(0) || NEW.collector_instance, 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_cursors_au",
        table_name: "cursors",
        sql: "CREATE TRIGGER authenticated_cursors_au
         AFTER UPDATE ON cursors
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('cursors', OLD.source || char(0) || OLD.collector_instance, 'delete');
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('cursors', NEW.source || char(0) || NEW.collector_instance, 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_cursors_ad",
        table_name: "cursors",
        sql: "CREATE TRIGGER authenticated_cursors_ad
         AFTER DELETE ON cursors
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('cursors', OLD.source || char(0) || OLD.collector_instance, 'delete');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_policy_metadata_ai",
        table_name: "policy_metadata",
        sql: "CREATE TRIGGER authenticated_policy_metadata_ai
         AFTER INSERT ON policy_metadata
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('policy_metadata', NEW.profile_id || char(0) || NEW.profile_version, 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_policy_metadata_au",
        table_name: "policy_metadata",
        sql: "CREATE TRIGGER authenticated_policy_metadata_au
         AFTER UPDATE ON policy_metadata
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('policy_metadata', OLD.profile_id || char(0) || OLD.profile_version, 'delete');
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('policy_metadata', NEW.profile_id || char(0) || NEW.profile_version, 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_policy_metadata_ad",
        table_name: "policy_metadata",
        sql: "CREATE TRIGGER authenticated_policy_metadata_ad
         AFTER DELETE ON policy_metadata
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('policy_metadata', OLD.profile_id || char(0) || OLD.profile_version, 'delete');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_diagnostics_ai",
        table_name: "diagnostics",
        sql: "CREATE TRIGGER authenticated_diagnostics_ai
         AFTER INSERT ON diagnostics
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('diagnostics', CAST(NEW.diagnostic_id AS TEXT), 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_diagnostics_au",
        table_name: "diagnostics",
        sql: "CREATE TRIGGER authenticated_diagnostics_au
         AFTER UPDATE ON diagnostics
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('diagnostics', CAST(OLD.diagnostic_id AS TEXT), 'delete');
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('diagnostics', CAST(NEW.diagnostic_id AS TEXT), 'insert');
         END",
    },
    AuthenticationTrigger {
        name: "authenticated_diagnostics_ad",
        table_name: "diagnostics",
        sql: "CREATE TRIGGER authenticated_diagnostics_ad
         AFTER DELETE ON diagnostics
         BEGIN
             INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
             VALUES ('diagnostics', CAST(OLD.diagnostic_id AS TEXT), 'delete');
         END",
    },
];

#[cfg(test)]
thread_local! {
    static FULL_SNAPSHOT_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_full_snapshot_test_counter() {
    FULL_SNAPSHOT_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn full_snapshot_test_counter() -> usize {
    FULL_SNAPSHOT_CALLS.with(Cell::get)
}

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
    /// v2 operation position. These fields are deliberately skipped from the
    /// v1/v2 JSON wire contract; they are only used for same-connection
    /// revalidation and durable operation-chain verification.
    #[serde(skip)]
    pub(crate) operation_seq: u64,
    #[serde(skip)]
    pub(crate) pending_change_id: u64,
    #[serde(skip)]
    pub(crate) event_order_context: Vec<u8>,
    #[serde(skip)]
    pub(crate) event_content_context: Vec<u8>,
}

impl AuthenticatedState {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.state_key != "journal"
            || self.state_key.len() > MAX_STATE_KEY_BYTES
            || !matches!(
                self.schema_version,
                AUTHENTICATED_STATE_V1_SCHEMA_VERSION | AUTHENTICATED_STATE_SCHEMA_VERSION
            )
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
            || self.updated_at.len() > MAX_TIMESTAMP_BYTES
            || self.event_order_context.len() > MAX_CONTEXT_BYTES
            || self.event_content_context.len() > MAX_CONTEXT_BYTES
            || (self.schema_version == AUTHENTICATED_STATE_SCHEMA_VERSION
                && (self.operation_seq > i64::MAX as u64
                    || self.pending_change_id > i64::MAX as u64))
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
    EventReplayed,
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
        if !matches!(
            self.schema_version,
            AUTHENTICATED_STATE_V1_SCHEMA_VERSION | AUTHENTICATED_STATE_SCHEMA_VERSION
        ) || self.anomalies.len() > MAX_ANOMALIES
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedAnchorIdentity {
    pub(crate) schema_version: u32,
    pub(crate) chain_epoch: u64,
    pub(crate) chain_start_mac: String,
    pub(crate) key_generation: u32,
    pub(crate) operation_seq: u64,
    pub(crate) pending_change_id: u64,
    pub(crate) event_count: u64,
    pub(crate) max_ingest_seq: u64,
    pub(crate) head_mac: String,
    pub(crate) event_order_digest: String,
    pub(crate) event_set_digest: String,
    pub(crate) event_content_digest: String,
    pub(crate) cursor_digest: String,
    pub(crate) policy_digest: String,
    pub(crate) diagnostic_digest: String,
    pub(crate) deletion_count: u64,
    pub(crate) deletion_digest: String,
    pub(crate) updated_at: String,
    pub(crate) event_order_context: Vec<u8>,
    pub(crate) event_content_context: Vec<u8>,
}

#[derive(Clone, Debug)]
struct IncrementalSnapshot {
    event_count: u64,
    max_ingest_seq: u64,
    event_order_digest: String,
    event_set_digest: String,
    event_content_digest: String,
    cursor_digest: String,
    policy_digest: String,
    diagnostic_digest: String,
    commitments: Vec<CommitmentRow>,
}

#[derive(Clone, Debug)]
struct CommitmentRow {
    table_name: &'static str,
    row_key_digest: String,
    row_locator: String,
    row_digest: String,
}

#[derive(Clone, Debug)]
struct PendingChange {
    change_id: u64,
    table_name: String,
    row_key: String,
    operation: String,
}

/// One authenticated map mutation.  This is deliberately restricted to
/// domain-separated digests and bounded locators: raw event identifiers,
/// logical keys, and payloads stay out of the durable operation history.
#[derive(Clone, Debug, Eq, PartialEq)]
struct OperationChange {
    table_name: &'static str,
    row_key_digest: String,
    row_locator: String,
    kind: &'static str,
    row_digest: Option<String>,
}

#[derive(Clone, Debug)]
struct OperationRecord {
    sequence: u64,
    generation: u32,
    epoch: u64,
    previous_head: String,
    head: String,
    delta_digest: String,
    operation_mac: String,
    committed_at: String,
}

/// Seed the anchor after migrations.  Existing pre-authentication journals are
/// explicitly bootstrapped at their first open; subsequent mutations are
/// authenticated transactionally.
pub(crate) fn ensure_anchor(
    connection: &mut Connection,
    provider: &dyn KeyProvider,
) -> Result<(), GhostraceError> {
    // Legacy verification and promotion are one indivisible initialization
    // operation. A deferred transaction could verify a stale v1 snapshot and
    // then race a foreign commit before publishing the v2 commitments.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    ensure_anchor_in(&transaction, provider)?;
    transaction.commit()?;
    Ok(())
}

/// [`ensure_anchor`] inside a transaction the caller already holds.
pub(crate) fn ensure_anchor_in(
    connection: &Transaction<'_>,
    provider: &dyn KeyProvider,
) -> Result<(), GhostraceError> {
    if let Some(state) = load_state(connection)? {
        state.validate()?;
        if state.schema_version == AUTHENTICATED_STATE_SCHEMA_VERSION {
            ensure_incremental_tables_in(connection)?;
            complete_bootstrap_metadata(connection)?;
            return Ok(());
        }
        // A v1 journal is never silently reinterpreted. Verify its exact v1
        // canonical state once, then promote it to the explicit v2 contract.
        verify_v1_state(connection, provider, &state)?;
        promote_to_incremental_in(connection, provider, Some(state))?;
        complete_bootstrap_metadata(connection)?;
        return Ok(());
    }
    let bootstrap: Option<String> = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata
             WHERE metadata_key = 'authenticated_state_bootstrap'",
            [],
            |row| {
                bounded_text_ref(row, 0, "authenticated bootstrap metadata", MAX_STATE_KEY_BYTES)
                    .map_err(sql_conversion_error)
            },
        )
        .optional()?;
    if bootstrap.as_deref() != Some("pending") {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated anchor is missing".to_owned(),
        ));
    }
    let snapshot = canonical_snapshot(connection)?;
    let state = new_state(provider, snapshot, None, None)?;
    insert_state(connection, &state)?;
    promote_to_incremental_in(connection, provider, Some(state))?;
    complete_bootstrap_metadata(connection)?;
    Ok(())
}

/// Close the one-way bootstrap gate after a v1 promotion or first v2
/// bootstrap.  This is deliberately checked on every authenticated open: a
/// stale `pending` marker must never survive alongside a v2 anchor, because
/// deleting that anchor would otherwise make the journal eligible for a fresh
/// rebootstrap rather than failing closed.
fn complete_bootstrap_metadata(connection: &Transaction<'_>) -> Result<(), GhostraceError> {
    let bootstrap: Option<String> = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata
             WHERE metadata_key = 'authenticated_state_bootstrap'",
            [],
            |row| {
                bounded_text_ref(row, 0, "authenticated bootstrap metadata", MAX_STATE_KEY_BYTES)
                    .map_err(sql_conversion_error)
            },
        )
        .optional()?;
    match bootstrap.as_deref() {
        Some("complete") => Ok(()),
        Some("pending") => {
            let updated = connection.execute(
                "UPDATE journal_metadata SET metadata_value = 'complete'
                 WHERE metadata_key = 'authenticated_state_bootstrap'
                   AND metadata_value = 'pending'",
                [],
            )?;
            if updated == 1 {
                Ok(())
            } else {
                Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated bootstrap metadata changed during promotion".to_owned(),
                ))
            }
        }
        Some(value) => Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "authenticated bootstrap metadata has unsupported state {value:?}"
        ))),
        None => Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated bootstrap metadata is missing".to_owned(),
        )),
    }
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
    let previous = load_state(transaction)?;
    let state = match previous {
        Some(previous) if previous.schema_version == AUTHENTICATED_STATE_SCHEMA_VERSION => {
            refresh_incremental_transaction(transaction, provider, previous, deletion)?
        }
        Some(previous) => {
            previous.validate()?;
            verify_v1_state(transaction, provider, &previous)?;
            promote_to_incremental_in(transaction, provider, Some(previous))?;
            complete_bootstrap_metadata(transaction)?;
            let promoted = load_state(transaction)?.ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "incremental authenticated anchor disappeared during promotion".to_owned(),
                )
            })?;
            refresh_incremental_transaction(transaction, provider, promoted, deletion)?
        }
        None => {
            let snapshot = canonical_snapshot(transaction)?;
            let state = new_state(provider, snapshot, None, deletion)?;
            insert_state(transaction, &state)?;
            promote_to_incremental_in(transaction, provider, Some(state))?;
            complete_bootstrap_metadata(transaction)?;
            load_state(transaction)?.ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "incremental authenticated anchor disappeared after bootstrap".to_owned(),
                )
            })?
        }
    };
    insert_state(transaction, &state)
}

pub(crate) fn report(
    connection: &Connection,
    provider: &dyn KeyProvider,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    let Some(state) = load_state(connection)? else {
        let snapshot = canonical_snapshot(connection)?;
        return report_with_anomalies(None, &snapshot, vec![AuthenticatedAnomaly::AnchorMissing]);
    };
    if state.schema_version == AUTHENTICATED_STATE_SCHEMA_VERSION {
        return report_incremental(connection, provider, state);
    }

    let snapshot = canonical_snapshot(connection)?;
    let replayed_event_count = count_replayed_events(connection)?;
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
    if replayed_event_count > 0 {
        anomalies.push(AuthenticatedAnomaly::EventReplayed);
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
        schema_version: state
            .map(|state| state.schema_version)
            .unwrap_or(AUTHENTICATED_STATE_V1_SCHEMA_VERSION),
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
        schema_version: AUTHENTICATED_STATE_V1_SCHEMA_VERSION,
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
        operation_seq: 0,
        pending_change_id: 0,
        event_order_context: Vec::new(),
        event_content_context: Vec::new(),
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
    if state.schema_version == AUTHENTICATED_STATE_V1_SCHEMA_VERSION {
        return canonical_state_v1(state);
    }
    canonical_fields(&[
        ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
        ("schema", &state.schema_version.to_le_bytes()),
        ("epoch", &state.chain_epoch.to_le_bytes()),
        ("chain-start", state.chain_start_mac.as_bytes()),
        ("generation", &state.key_generation.to_le_bytes()),
        ("operation", &state.operation_seq.to_le_bytes()),
        ("pending", &state.pending_change_id.to_le_bytes()),
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
        ("updated", state.updated_at.as_bytes()),
        ("order-context", Sha256::digest(&state.event_order_context).as_slice()),
        ("content-context", Sha256::digest(&state.event_content_context).as_slice()),
    ])
}

fn canonical_state_v1(state: &AuthenticatedState) -> Vec<u8> {
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

/// Read the complete identity used to revalidate a read-side authentication
/// snapshot after the writer has acquired SQLite's IMMEDIATE lock.  A digest
/// alone is not sufficient: the operation position, deletion boundary, and
/// all authenticated maps are part of the identity.
pub(crate) fn anchor_identity(
    connection: &Connection,
) -> Result<Option<AuthenticatedAnchorIdentity>, GhostraceError> {
    Ok(load_state(connection)?.map(|state| AuthenticatedAnchorIdentity {
        schema_version: state.schema_version,
        chain_epoch: state.chain_epoch,
        chain_start_mac: state.chain_start_mac,
        key_generation: state.key_generation,
        operation_seq: state.operation_seq,
        pending_change_id: state.pending_change_id,
        event_count: state.event_count,
        max_ingest_seq: state.max_ingest_seq,
        head_mac: state.head_mac,
        event_order_digest: state.event_order_digest,
        event_set_digest: state.event_set_digest,
        event_content_digest: state.event_content_digest,
        cursor_digest: state.cursor_digest,
        policy_digest: state.policy_digest,
        diagnostic_digest: state.diagnostic_digest,
        deletion_count: state.deletion_count,
        deletion_digest: state.deletion_digest,
        updated_at: state.updated_at,
        event_order_context: state.event_order_context,
        event_content_context: state.event_content_context,
    }))
}

/// The bounded writer-side check. Full row recomputation is deliberately not
/// called here; the caller has already performed it on a read snapshot when
/// the connection's data_version changed. This check only authenticates the
/// durable anchor and operation position under the retained write guard.
pub(crate) fn require_anchor_valid(
    connection: &Connection,
    provider: &dyn KeyProvider,
) -> Result<(), GhostraceError> {
    let state = load_state(connection)?.ok_or_else(|| {
        GhostraceError::AuthenticatedStateInvalid("authenticated anchor is missing".to_owned())
    })?;
    state.validate()?;
    let key = provider.key_for_generation(state.key_generation)?;
    if head_mac(&state, &key) != state.head_mac {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated anchor MAC is invalid".to_owned(),
        ));
    }
    if state.schema_version == AUTHENTICATED_STATE_SCHEMA_VERSION {
        ensure_incremental_tables_in(connection)?;
        let operation_head: Option<String> = connection
            .query_row(
                "SELECT head_mac FROM authenticated_operations
                 WHERE operation_seq = ?1",
                params![state.operation_seq as i64],
                |row| {
                    bounded_text_ref(row, 0, "authenticated operation head MAC", MAX_MAC_HEX_BYTES)
                        .map_err(sql_conversion_error)
                },
            )
            .optional()?;
        if operation_head.as_deref() != Some(state.head_mac.as_str()) {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation head does not match anchor".to_owned(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn bootstrap_allowed(connection: &Connection) -> Result<bool, GhostraceError> {
    let value: Option<String> = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata
             WHERE metadata_key = 'authenticated_state_bootstrap'",
            [],
            |row| {
                bounded_text_ref(row, 0, "authenticated bootstrap metadata", MAX_STATE_KEY_BYTES)
                    .map_err(sql_conversion_error)
            },
        )
        .optional()?;
    Ok(value.as_deref() == Some("pending"))
}

fn ensure_incremental_tables_in(connection: &Connection) -> Result<(), GhostraceError> {
    if !incremental_schema_complete(connection)? {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "incremental authentication schema is incomplete".to_owned(),
        ));
    }
    Ok(())
}

fn incremental_schema_complete(connection: &Connection) -> Result<bool, GhostraceError> {
    for table in AUTHENTICATED_TABLES {
        let exists: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(false);
        }
    }

    // The journal never uses attached databases or temporary objects.  A
    // temporary table/trigger can shadow a main-schema object while remaining
    // invisible in sqlite_master, and an attached schema can otherwise hide an
    // unreviewed trigger from this contract. Refuse both before any
    // authenticated mutation is allowed to proceed.
    let mut database_list = connection.prepare("PRAGMA database_list")?;
    let mut databases = database_list.query([])?;
    while let Some(row) = databases.next()? {
        let name = bounded_text_ref(row, 1, "SQLite database name", MAX_DATABASE_NAME_BYTES)?;
        if !matches!(name.as_str(), "main" | "temp") {
            return Ok(false);
        }
    }
    drop(databases);
    drop(database_list);

    let temporary_object_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_temp_master
         WHERE type IN ('table', 'index', 'view', 'trigger')",
        [],
        |row| row.get(0),
    )?;
    if temporary_object_count != 0 {
        return Ok(false);
    }

    let main_trigger_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger'",
        [],
        |row| row.get(0),
    )?;
    if main_trigger_count != AUTHENTICATION_TRIGGERS.len() as i64 {
        return Ok(false);
    }

    let mut seen = HashSet::with_capacity(AUTHENTICATION_TRIGGERS.len());
    let mut statement = connection.prepare(
        "SELECT name, tbl_name, sql
         FROM sqlite_master WHERE type = 'trigger' ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name = bounded_text_ref(row, 0, "SQLite trigger name", MAX_TRIGGER_NAME_BYTES)?;
        let table_name =
            bounded_text_ref(row, 1, "SQLite trigger table name", MAX_TABLE_NAME_BYTES)?;
        let sql = bounded_utf8_ref(row, 2, "SQLite trigger SQL", MAX_AUTH_TRIGGER_SQL_BYTES)?;
        let Some(expected) = AUTHENTICATION_TRIGGERS.iter().find(|trigger| trigger.name == name)
        else {
            // Unknown triggers, including a trigger that deletes the pending
            // recorder row after the approved trigger runs, fail closed.
            return Ok(false);
        };
        // SQLite stores the compiled trigger SQL in sqlite_master. Compare its
        // complete normalized definition and target table, not a few
        // substrings: wrong-key, no-op, or side-effecting definitions are not
        // part of the compiled migration contract.
        if table_name != expected.table_name
            || normalize_trigger_sql(&sql) != normalize_trigger_sql(expected.sql)
            || !seen.insert(name)
        {
            return Ok(false);
        }
    }
    Ok(seen.len() == AUTHENTICATION_TRIGGERS.len())
}

fn normalize_trigger_sql(sql: &str) -> String {
    // sqlite_master preserves source formatting, while SQLite's parser accepts
    // arbitrary whitespace around punctuation.  Canonicalize the token stream
    // rather than comparing whitespace-separated strings: all keywords,
    // identifiers, operators, punctuation, and string literals remain part of
    // the contract, but formatting/comments do not create false refusals.
    let characters = sql.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(sql.len());
    let mut quote = None::<char>;
    let mut pending_word_boundary = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if let Some(delimiter) = quote {
            normalized.push(character);
            if character == delimiter {
                if characters.get(index + 1) == Some(&delimiter) {
                    normalized.push(delimiter);
                    index += 2;
                    continue;
                }
                quote = None;
            }
            index += 1;
            continue;
        }
        if (character == '\'' || character == '"' || character == '`')
            && characters.get(index + 1).is_some()
        {
            quote = Some(character);
            normalized.push(character);
            pending_word_boundary = false;
            index += 1;
            continue;
        }
        if character == '-' && characters.get(index + 1) == Some(&'-') {
            pending_word_boundary = true;
            index += 2;
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if character == '/' && characters.get(index + 1) == Some(&'*') {
            pending_word_boundary = true;
            index += 2;
            while index + 1 < characters.len()
                && !(characters[index] == '*' && characters[index + 1] == '/')
            {
                index += 1;
            }
            index = (index + 2).min(characters.len());
            continue;
        }
        if character.is_whitespace() {
            pending_word_boundary = true;
            index += 1;
            continue;
        }
        // The semicolon stored in migration source is a statement terminator,
        // not a trigger-program token.  Removing it also matches sqlite_master,
        // which omits the final terminator.
        if character == ';' {
            index += 1;
            continue;
        }
        let word_character =
            character.is_ascii_alphanumeric() || character == '_' || character == '$';
        let previous_word_character = normalized.chars().last().is_some_and(|previous| {
            previous.is_ascii_alphanumeric() || previous == '_' || previous == '$'
        });
        if pending_word_boundary && word_character && previous_word_character {
            normalized.push(' ');
        }
        pending_word_boundary = false;
        normalized.push(character.to_ascii_lowercase());
        index += 1;
    }
    normalized
}

fn validate_operation_change(change: &OperationChange) -> Result<(), GhostraceError> {
    table_name_static(change.table_name).map_err(|_| {
        GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation change table is invalid".to_owned(),
        )
    })?;
    if !valid_sha256_digest(&change.row_key_digest) {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation change key digest is invalid".to_owned(),
        ));
    }
    match change.table_name {
        "events" => {
            let locator = change.row_locator.parse::<u64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated event operation locator is invalid".to_owned(),
                )
            })?;
            if locator > i64::MAX as u64 || locator.to_string() != change.row_locator {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated event operation locator is not canonical".to_owned(),
                ));
            }
        }
        "cursors" | "policy_metadata" | "diagnostics" => {
            if !change.row_locator.is_empty() {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated operation locator is not canonical".to_owned(),
                ));
            }
        }
        _ => unreachable!("table_name_static validated operation table"),
    }
    match change.kind {
        "set" => {
            let digest = change.row_digest.as_deref().ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated set operation is missing its row digest".to_owned(),
                )
            })?;
            if !valid_sha256_digest(digest) {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated set operation row digest is invalid".to_owned(),
                ));
            }
        }
        "tombstone" => {
            if change.row_digest.is_some() {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated tombstone carries a row digest".to_owned(),
                ));
            }
        }
        _ => {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation change kind is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn canonical_operation_change(index: u64, change: &OperationChange) -> Vec<u8> {
    canonical_fields(&[
        ("index", &index.to_le_bytes()),
        ("table", change.table_name.as_bytes()),
        ("key-digest", change.row_key_digest.as_bytes()),
        ("locator", change.row_locator.as_bytes()),
        ("kind", change.kind.as_bytes()),
        ("row-digest", change.row_digest.as_deref().unwrap_or("<tombstone>").as_bytes()),
    ])
}

fn operation_delta_digest(changes: &[OperationChange]) -> Result<String, GhostraceError> {
    let mut digest = Sha256::new();
    digest.update(b"ghostrace:authenticated:v2:operation-delta\0");
    for (index, change) in changes.iter().enumerate() {
        validate_operation_change(change)?;
        let index = u64::try_from(index).map_err(|_| {
            GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation change index overflow".to_owned(),
            )
        })?;
        digest.update(canonical_operation_change(index, change));
    }
    Ok(sha_digest(digest.finalize().as_slice()))
}

fn stream_operation_changes<F>(
    connection: &Connection,
    operation_seq: u64,
    mut visitor: F,
) -> Result<String, GhostraceError>
where
    F: FnMut(u64, &OperationChange) -> Result<(), GhostraceError>,
{
    let mut statement = connection.prepare(
        "SELECT change_index, table_name, row_key_digest, row_locator,
                change_kind, row_digest
         FROM authenticated_operation_changes
         WHERE operation_seq = ?1 ORDER BY change_index",
    )?;
    let mut rows = statement.query(params![operation_seq as i64])?;
    let mut digest = Sha256::new();
    digest.update(b"ghostrace:authenticated:v2:operation-delta\0");
    let mut expected_index = 0_u64;
    while let Some(row) = rows.next()? {
        let index = to_u64(row.get::<_, i64>(0)?, "operation change index")?;
        if index != expected_index {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation change order is invalid".to_owned(),
            ));
        }
        let table_name =
            bounded_text_ref(row, 1, "authenticated operation change table", MAX_TABLE_NAME_BYTES)?;
        let kind = bounded_text_ref(
            row,
            4,
            "authenticated operation change kind",
            MAX_OPERATION_KIND_BYTES,
        )?;
        let change = OperationChange {
            table_name: table_name_static(&table_name).map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated operation change table is invalid".to_owned(),
                )
            })?,
            row_key_digest: bounded_text_ref(
                row,
                2,
                "authenticated operation change key digest",
                MAX_DIGEST_TEXT_BYTES,
            )?,
            row_locator: bounded_text_ref(
                row,
                3,
                "authenticated operation change locator",
                MAX_EVENT_LOCATOR_BYTES,
            )?,
            kind: match kind.as_str() {
                "set" => "set",
                "tombstone" => "tombstone",
                _ => {
                    return Err(GhostraceError::AuthenticatedStateInvalid(
                        "authenticated operation change kind is invalid".to_owned(),
                    ))
                }
            },
            row_digest: bounded_optional_text_ref(
                row,
                5,
                "authenticated operation change row digest",
                MAX_DIGEST_TEXT_BYTES,
            )?,
        };
        validate_operation_change(&change)?;
        digest.update(canonical_operation_change(index, &change));
        visitor(index, &change)?;
        expected_index = expected_index.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation change index overflow".to_owned(),
            )
        })?;
    }
    Ok(sha_digest(digest.finalize().as_slice()))
}

fn operation_change_from_commitment(commitment: &CommitmentRow) -> OperationChange {
    OperationChange {
        table_name: commitment.table_name,
        row_key_digest: commitment.row_key_digest.clone(),
        row_locator: commitment.row_locator.clone(),
        kind: "set",
        row_digest: Some(commitment.row_digest.clone()),
    }
}

fn bootstrap_operation_changes(
    commitments: &[CommitmentRow],
) -> Result<Vec<OperationChange>, GhostraceError> {
    let mut changes = commitments.iter().map(operation_change_from_commitment).collect::<Vec<_>>();
    changes.sort_by(|left, right| {
        (
            left.table_name,
            left.row_key_digest.as_str(),
            left.row_locator.as_str(),
            left.row_digest.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                right.table_name,
                right.row_key_digest.as_str(),
                right.row_locator.as_str(),
                right.row_digest.as_deref().unwrap_or_default(),
            ))
    });
    let mut seen = BTreeMap::new();
    for change in &changes {
        validate_operation_change(change)?;
        if seen.insert((change.table_name, change.row_key_digest.as_str()), ()).is_some() {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated bootstrap contains duplicate commitment keys".to_owned(),
            ));
        }
    }
    Ok(changes)
}

fn insert_operation_record(
    connection: &Connection,
    state: &AuthenticatedState,
    previous_head: &str,
    key: &[u8; 32],
    changes: &[OperationChange],
) -> Result<(), GhostraceError> {
    let delta_digest = operation_delta_digest(changes)?;
    insert_operation_record_with_digest(connection, state, previous_head, key, &delta_digest)?;
    for (index, change) in changes.iter().enumerate() {
        let index = u64::try_from(index).map_err(|_| {
            GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation change index overflow".to_owned(),
            )
        })?;
        insert_operation_change(connection, state.operation_seq, index, change)?;
    }
    Ok(())
}

fn insert_operation_record_with_digest(
    connection: &Connection,
    state: &AuthenticatedState,
    previous_head: &str,
    key: &[u8; 32],
    delta_digest: &str,
) -> Result<(), GhostraceError> {
    connection.execute(
        "INSERT INTO authenticated_operations(
             operation_seq, key_generation, chain_epoch, previous_head_mac,
             head_mac, delta_digest, operation_mac, committed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            state.operation_seq as i64,
            state.key_generation,
            state.chain_epoch as i64,
            previous_head,
            state.head_mac.as_str(),
            delta_digest,
            operation_mac(
                key,
                state.operation_seq,
                state.key_generation,
                state.chain_epoch,
                previous_head,
                delta_digest,
                &state.head_mac,
                &state.updated_at,
            ),
            state.updated_at.as_str(),
        ],
    )?;
    Ok(())
}

fn insert_operation_change(
    connection: &Connection,
    operation_seq: u64,
    index: u64,
    change: &OperationChange,
) -> Result<(), GhostraceError> {
    validate_operation_change(change)?;
    let index = i64::try_from(index).map_err(|_| {
        GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation change index overflow".to_owned(),
        )
    })?;
    connection.execute(
        "INSERT INTO authenticated_operation_changes(
             operation_seq, change_index, table_name, row_key_digest,
             row_locator, change_kind, row_digest
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            operation_seq as i64,
            index,
            change.table_name,
            change.row_key_digest.as_str(),
            change.row_locator.as_str(),
            change.kind,
            change.row_digest.as_deref(),
        ],
    )?;
    Ok(())
}

fn verify_v1_state(
    connection: &Connection,
    provider: &dyn KeyProvider,
    state: &AuthenticatedState,
) -> Result<(), GhostraceError> {
    if state.schema_version != AUTHENTICATED_STATE_V1_SCHEMA_VERSION {
        return Ok(());
    }
    let snapshot = canonical_snapshot(connection)?;
    let matches = snapshot.event_count == state.event_count
        && snapshot.max_ingest_seq == state.max_ingest_seq
        && snapshot.event_order_digest == state.event_order_digest
        && snapshot.event_set_digest == state.event_set_digest
        && snapshot.event_content_digest == state.event_content_digest
        && snapshot.cursor_digest == state.cursor_digest
        && snapshot.policy_digest == state.policy_digest
        && snapshot.diagnostic_digest == state.diagnostic_digest;
    let key = provider.key_for_generation(state.key_generation)?;
    if !matches || head_mac(state, &key) != state.head_mac {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "legacy authenticated state failed v1 migration verification".to_owned(),
        ));
    }
    Ok(())
}

fn promote_to_incremental_in(
    connection: &Transaction<'_>,
    provider: &dyn KeyProvider,
    previous: Option<AuthenticatedState>,
) -> Result<(), GhostraceError> {
    ensure_incremental_tables_in(connection)?;
    let snapshot = incremental_snapshot(connection)?;
    let current_generation = provider.key_generation();
    let current_key = provider.key_for_generation(current_generation)?;
    let previous_head = previous.as_ref().map(|state| state.head_mac.as_str());
    let boundary_change =
        previous.as_ref().is_some_and(|state| state.key_generation != current_generation);
    let chain_epoch = if boundary_change {
        previous.as_ref().and_then(|state| state.chain_epoch.checked_add(1)).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("chain epoch overflow".to_owned())
        })?
    } else {
        previous.as_ref().map_or(0, |state| state.chain_epoch)
    };
    let chain_start_mac = if boundary_change {
        let material = canonical_fields(&[
            ("kind", b"v2-key-generation-boundary"),
            ("previous-head", previous_head.unwrap_or_default().as_bytes()),
            ("epoch", &chain_epoch.to_le_bytes()),
            ("generation", &current_generation.to_le_bytes()),
        ]);
        keyed_hex(&current_key, &material)
    } else if let Some(previous) = previous.as_ref() {
        previous.chain_start_mac.clone()
    } else {
        keyed_hex(
            &current_key,
            &canonical_fields(&[
                ("kind", b"v2-chain-start"),
                ("epoch", &chain_epoch.to_le_bytes()),
                ("generation", &current_generation.to_le_bytes()),
            ]),
        )
    };
    let pending_change_id = max_pending_change_id(connection)?;
    let mut state = AuthenticatedState {
        state_key: "journal".to_owned(),
        schema_version: AUTHENTICATED_STATE_SCHEMA_VERSION,
        chain_epoch,
        chain_start_mac,
        head_mac: String::new(),
        key_generation: current_generation,
        event_count: snapshot.event_count,
        max_ingest_seq: snapshot.max_ingest_seq,
        event_order_digest: snapshot.event_order_digest,
        event_set_digest: snapshot.event_set_digest,
        event_content_digest: snapshot.event_content_digest,
        cursor_digest: snapshot.cursor_digest,
        policy_digest: snapshot.policy_digest,
        diagnostic_digest: snapshot.diagnostic_digest,
        deletion_count: previous.as_ref().map_or(0, |state| state.deletion_count),
        deletion_digest: previous.as_ref().map_or_else(
            || EMPTY_DELETION_DIGEST.to_owned(),
            |state| state.deletion_digest.clone(),
        ),
        updated_at: Utc::now().to_rfc3339(),
        operation_seq: 0,
        pending_change_id,
        event_order_context: Vec::new(),
        event_content_context: Vec::new(),
    };
    state.head_mac = head_mac(&state, &current_key);
    state.validate()?;

    connection.execute("DELETE FROM authenticated_commitments", [])?;
    for commitment in &snapshot.commitments {
        insert_commitment(connection, commitment)?;
    }
    connection.execute("DELETE FROM authenticated_operations", [])?;
    connection.execute("DELETE FROM authenticated_operation_changes", [])?;
    let bootstrap_changes = bootstrap_operation_changes(&snapshot.commitments)?;
    insert_operation_record(
        connection,
        &state,
        &state.chain_start_mac,
        &current_key,
        &bootstrap_changes,
    )?;
    insert_state(connection, &state)?;
    connection.execute(
        "DELETE FROM authenticated_pending_changes WHERE change_id <= ?1",
        params![pending_change_id as i64],
    )?;
    Ok(())
}

fn refresh_incremental_transaction(
    transaction: &Transaction<'_>,
    provider: &dyn KeyProvider,
    previous: AuthenticatedState,
    deletion: Option<&AuthenticatedDeletionMarker>,
) -> Result<AuthenticatedState, GhostraceError> {
    ensure_incremental_tables_in(transaction)?;
    let generation = provider.key_generation();
    let key = provider.key_for_generation(generation)?;
    let boundary_change = deletion.is_some() || generation != previous.key_generation;
    let next_operation_seq =
        (!boundary_change).then(|| previous.operation_seq.checked_add(1)).flatten();
    let mut operation_delta = (!boundary_change).then(|| {
        let mut digest = Sha256::new();
        digest.update(b"ghostrace:authenticated:v2:operation-delta\0");
        digest
    });
    let mut state = previous.clone();
    let mut removed_max = false;
    let mut latest_pending = previous.pending_change_id;
    let mut change_index = 0_u64;
    let mut pending_statement = transaction.prepare(
        "SELECT change_id, table_name, row_key, operation
         FROM authenticated_pending_changes
         WHERE change_id > ?1 ORDER BY change_id",
    )?;
    let mut pending_rows = pending_statement.query(params![previous.pending_change_id as i64])?;
    while let Some(row) = pending_rows.next()? {
        let change = pending_change_from_row(row)?;
        validate_pending_change(&change)?;
        let old = pending_commitment(transaction, &change)?;
        apply_pending_change(transaction, &mut state, &change, &mut removed_max)?;
        let current = current_commitment(transaction, &change.table_name, &change.row_key)?;
        let operation_change = if change.operation == "delete" {
            let old = old.ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated delete targeted a missing committed row".to_owned(),
                )
            })?;
            OperationChange {
                table_name: old.table_name,
                row_key_digest: old.row_key_digest,
                row_locator: old.row_locator,
                kind: "tombstone",
                row_digest: None,
            }
        } else if let Some(current) = current {
            OperationChange {
                table_name: current.table_name,
                row_key_digest: current.row_key_digest,
                row_locator: current.row_locator,
                kind: "set",
                row_digest: Some(current.row_digest),
            }
        } else {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated insert/update targeted a missing live row".to_owned(),
            ));
        };
        validate_operation_change(&operation_change)?;
        if let Some(digest) = operation_delta.as_mut() {
            let operation_seq = next_operation_seq.ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "incremental operation sequence disappeared".to_owned(),
                )
            })?;
            digest.update(canonical_operation_change(change_index, &operation_change));
            insert_operation_change(transaction, operation_seq, change_index, &operation_change)?;
            change_index = change_index.checked_add(1).ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated operation change index overflow".to_owned(),
                )
            })?;
        }
        latest_pending = change.change_id;
    }
    drop(pending_rows);
    drop(pending_statement);
    if removed_max {
        let value: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(ingest_seq), 0) FROM events",
            [],
            |row| row.get(0),
        )?;
        state.max_ingest_seq = to_u64(value, "maximum ingest sequence")?;
    }
    state.chain_epoch = if boundary_change {
        previous.chain_epoch.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("chain epoch overflow".to_owned())
        })?
    } else {
        previous.chain_epoch
    };
    if boundary_change {
        let marker =
            deletion.map(canonical_deletion).unwrap_or_else(|| b"key-generation-boundary".to_vec());
        state.chain_start_mac = keyed_hex(
            &key,
            &canonical_fields(&[
                ("kind", b"v2-chain-boundary"),
                ("previous-head", previous.head_mac.as_bytes()),
                ("epoch", &state.chain_epoch.to_le_bytes()),
                ("generation", &generation.to_le_bytes()),
                ("marker", &marker),
            ]),
        );
    }
    if let Some(marker) = deletion {
        state.deletion_count = previous.deletion_count.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("deletion count overflow".to_owned())
        })?;
        state.deletion_digest = deletion_digest(&previous.deletion_digest, marker);
    }
    state.schema_version = AUTHENTICATED_STATE_SCHEMA_VERSION;
    state.key_generation = generation;
    state.pending_change_id = latest_pending;
    state.updated_at = Utc::now().to_rfc3339();
    if boundary_change {
        // Rotation and explicit retention are authenticated epoch boundaries.
        // Rebase the durable map under the retained IMMEDIATE transaction and
        // scrub the old operation log, so startup does not require retired
        // generations while the hot path remains incremental.
        let snapshot = incremental_snapshot(transaction)?;
        state.event_count = snapshot.event_count;
        state.max_ingest_seq = snapshot.max_ingest_seq;
        state.event_order_digest = snapshot.event_order_digest;
        state.event_set_digest = snapshot.event_set_digest;
        state.event_content_digest = snapshot.event_content_digest;
        state.cursor_digest = snapshot.cursor_digest;
        state.policy_digest = snapshot.policy_digest;
        state.diagnostic_digest = snapshot.diagnostic_digest;
        state.operation_seq = 0;
        state.head_mac = head_mac(&state, &key);
        state.validate()?;
        transaction.execute("DELETE FROM authenticated_operations", [])?;
        transaction.execute("DELETE FROM authenticated_operation_changes", [])?;
        let bootstrap_changes = bootstrap_operation_changes(&snapshot.commitments)?;
        insert_operation_record(
            transaction,
            &state,
            &state.chain_start_mac,
            &key,
            &bootstrap_changes,
        )?;
    } else {
        state.operation_seq = next_operation_seq.ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("operation sequence overflow".to_owned())
        })?;
        state.head_mac = head_mac(&state, &key);
        state.validate()?;
        let operation_delta = operation_delta.ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid(
                "incremental operation delta builder disappeared".to_owned(),
            )
        })?;
        let delta_digest = sha_digest(operation_delta.finalize().as_slice());
        insert_operation_record_with_digest(
            transaction,
            &state,
            &previous.head_mac,
            &key,
            &delta_digest,
        )?;
    }
    transaction.execute(
        "DELETE FROM authenticated_pending_changes WHERE change_id <= ?1",
        params![state.pending_change_id as i64],
    )?;
    Ok(state)
}

fn validate_pending_change(change: &PendingChange) -> Result<(), GhostraceError> {
    table_name_static(&change.table_name).map_err(|_| {
        GhostraceError::AuthenticatedStateInvalid(
            "authenticated pending change table is invalid".to_owned(),
        )
    })?;
    if !matches!(change.operation.as_str(), "insert" | "update" | "delete") {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated pending change operation is invalid".to_owned(),
        ));
    }
    match change.table_name.as_str() {
        "events" => {
            let sequence = change.row_key.parse::<u64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated event pending key is invalid".to_owned(),
                )
            })?;
            if sequence > i64::MAX as u64 {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated event pending key is too large".to_owned(),
                ));
            }
        }
        "cursors" => {
            let (source, collector) = split_row_key(&change.row_key)?;
            if source.is_empty() || collector.is_empty() {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated cursor pending key is empty".to_owned(),
                ));
            }
        }
        "policy_metadata" => {
            let (profile_id, _) = split_policy_key(&change.row_key)?;
            if profile_id.is_empty() {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated policy pending key is empty".to_owned(),
                ));
            }
        }
        "diagnostics" => {
            let diagnostic_id = change.row_key.parse::<i64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "authenticated diagnostic pending key is invalid".to_owned(),
                )
            })?;
            if diagnostic_id < 0 {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated diagnostic pending key is negative".to_owned(),
                ));
            }
        }
        _ => unreachable!("table_name_static validated pending table"),
    }
    Ok(())
}

fn max_pending_change_id(connection: &Connection) -> Result<u64, GhostraceError> {
    let value: i64 = connection.query_row(
        "SELECT COALESCE(MAX(change_id), 0) FROM authenticated_pending_changes",
        [],
        |row| row.get(0),
    )?;
    to_u64(value, "pending change sequence")
}

fn pending_change_from_row(row: &Row<'_>) -> Result<PendingChange, GhostraceError> {
    Ok(PendingChange {
        change_id: to_u64(row.get::<_, i64>(0)?, "pending change sequence")?,
        table_name: bounded_text_ref(row, 1, "authenticated pending table", MAX_TABLE_NAME_BYTES)?,
        row_key: bounded_key_ref(row, 2, "authenticated pending row key", MAX_LOGICAL_KEY_BYTES)?,
        operation: bounded_text_ref(
            row,
            3,
            "authenticated pending operation",
            MAX_OPERATION_KIND_BYTES,
        )?,
    })
}

fn pending_commitment(
    connection: &Connection,
    change: &PendingChange,
) -> Result<Option<CommitmentRow>, GhostraceError> {
    let key_digest = row_key_digest(&change.table_name, &change.row_key);
    if change.table_name == "events" {
        Ok(connection
            .query_row(
                "SELECT table_name, row_key_digest, row_locator, row_digest
                 FROM authenticated_commitments
                 WHERE table_name = 'events' AND row_locator = ?1",
                params![change.row_key],
                commitment_from_row,
            )
            .optional()?)
    } else {
        Ok(connection
            .query_row(
                "SELECT table_name, row_key_digest, row_locator, row_digest
                 FROM authenticated_commitments
                 WHERE table_name = ?1 AND row_key_digest = ?2",
                params![change.table_name, key_digest],
                commitment_from_row,
            )
            .optional()?)
    }
}

fn apply_pending_change(
    connection: &Connection,
    state: &mut AuthenticatedState,
    change: &PendingChange,
    removed_max: &mut bool,
) -> Result<(), GhostraceError> {
    let old = pending_commitment(connection, change)?;
    if let Some(old) = old.as_ref() {
        remove_commitment_from_state(state, old, removed_max)?;
        connection.execute(
            "DELETE FROM authenticated_commitments
             WHERE table_name = ?1 AND row_key_digest = ?2",
            params![old.table_name, old.row_key_digest],
        )?;
    }
    let current = if change.operation == "delete" {
        None
    } else {
        current_commitment(connection, &change.table_name, &change.row_key)?
    };
    if let Some(current) = current.as_ref() {
        add_commitment_to_state(state, current)?;
        insert_commitment(connection, current)?;
    }
    Ok(())
}

fn commitment_from_row(row: &Row<'_>) -> rusqlite::Result<CommitmentRow> {
    let table_name =
        bounded_text_ref(row, 0, "authenticated commitment table", MAX_TABLE_NAME_BYTES)
            .map_err(sql_conversion_error)?;
    Ok(CommitmentRow {
        table_name: table_name_static(&table_name)?,
        row_key_digest: bounded_text_ref(
            row,
            1,
            "authenticated commitment key digest",
            MAX_DIGEST_TEXT_BYTES,
        )
        .map_err(sql_conversion_error)?,
        row_locator: bounded_text_ref(
            row,
            2,
            "authenticated commitment locator",
            MAX_EVENT_LOCATOR_BYTES,
        )
        .map_err(sql_conversion_error)?,
        row_digest: bounded_text_ref(
            row,
            3,
            "authenticated commitment row digest",
            MAX_DIGEST_TEXT_BYTES,
        )
        .map_err(sql_conversion_error)?,
    })
}

fn table_name_static(value: &str) -> Result<&'static str, rusqlite::Error> {
    match value {
        "events" => Ok("events"),
        "cursors" => Ok("cursors"),
        "policy_metadata" => Ok("policy_metadata"),
        "diagnostics" => Ok("diagnostics"),
        _ => Err(rusqlite::Error::InvalidParameterName(
            "unknown authenticated commitment table".to_owned(),
        )),
    }
}

fn insert_commitment(
    connection: &Connection,
    commitment: &CommitmentRow,
) -> Result<(), GhostraceError> {
    connection.execute(
        "INSERT INTO authenticated_commitments(
             table_name, row_key_digest, row_locator, row_digest
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(table_name, row_key_digest) DO UPDATE SET
             row_locator=excluded.row_locator, row_digest=excluded.row_digest",
        params![
            commitment.table_name,
            commitment.row_key_digest,
            commitment.row_locator,
            commitment.row_digest,
        ],
    )?;
    Ok(())
}

fn remove_commitment_from_state(
    state: &mut AuthenticatedState,
    commitment: &CommitmentRow,
    removed_max: &mut bool,
) -> Result<(), GhostraceError> {
    match commitment.table_name {
        "events" => {
            state.event_order_digest = xor_map_term(
                &state.event_order_digest,
                map_term("events-order", commitment, true, false),
            )?;
            state.event_set_digest = xor_map_term(
                &state.event_set_digest,
                map_term("events-set", commitment, false, false),
            )?;
            state.event_content_digest = xor_map_term(
                &state.event_content_digest,
                map_term("events-content", commitment, true, true),
            )?;
            state.event_count = state.event_count.checked_sub(1).ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "incremental event count underflow".to_owned(),
                )
            })?;
            let old_max = commitment.row_locator.parse::<u64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "stored event commitment locator is invalid".to_owned(),
                )
            })?;
            if old_max == state.max_ingest_seq {
                *removed_max = true;
            }
        }
        "cursors" => {
            state.cursor_digest =
                xor_map_term(&state.cursor_digest, map_term("cursors", commitment, false, true))?;
        }
        "policy_metadata" => {
            state.policy_digest = xor_map_term(
                &state.policy_digest,
                map_term("policy_metadata", commitment, false, true),
            )?;
        }
        "diagnostics" => {
            state.diagnostic_digest = xor_map_term(
                &state.diagnostic_digest,
                map_term("diagnostics", commitment, false, true),
            )?;
        }
        _ => {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "unknown authenticated commitment table".to_owned(),
            ));
        }
    }
    Ok(())
}

fn add_commitment_to_state(
    state: &mut AuthenticatedState,
    commitment: &CommitmentRow,
) -> Result<(), GhostraceError> {
    match commitment.table_name {
        "events" => {
            state.event_order_digest = xor_map_term(
                &state.event_order_digest,
                map_term("events-order", commitment, true, false),
            )?;
            state.event_set_digest = xor_map_term(
                &state.event_set_digest,
                map_term("events-set", commitment, false, false),
            )?;
            state.event_content_digest = xor_map_term(
                &state.event_content_digest,
                map_term("events-content", commitment, true, true),
            )?;
            state.event_count = state.event_count.checked_add(1).ok_or_else(|| {
                GhostraceError::AuthenticatedStateInvalid(
                    "incremental event count overflow".to_owned(),
                )
            })?;
            let ingest_seq = commitment.row_locator.parse::<u64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "event commitment locator is invalid".to_owned(),
                )
            })?;
            state.max_ingest_seq = state.max_ingest_seq.max(ingest_seq);
        }
        "cursors" => {
            state.cursor_digest =
                xor_map_term(&state.cursor_digest, map_term("cursors", commitment, false, true))?;
        }
        "policy_metadata" => {
            state.policy_digest = xor_map_term(
                &state.policy_digest,
                map_term("policy_metadata", commitment, false, true),
            )?;
        }
        "diagnostics" => {
            state.diagnostic_digest = xor_map_term(
                &state.diagnostic_digest,
                map_term("diagnostics", commitment, false, true),
            )?;
        }
        _ => {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "unknown authenticated commitment table".to_owned(),
            ));
        }
    }
    Ok(())
}

fn map_term(
    namespace: &str,
    commitment: &CommitmentRow,
    include_locator: bool,
    include_row: bool,
) -> [u8; 32] {
    let locator: &[u8] = if include_locator { commitment.row_locator.as_bytes() } else { b"" };
    let mut fields = vec![
        ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
        ("namespace", namespace.as_bytes()),
        ("key", commitment.row_key_digest.as_bytes()),
        ("locator", locator),
    ];
    if include_row {
        fields.push(("row", commitment.row_digest.as_bytes()));
    }
    let digest = Sha256::digest(canonical_fields(&fields));
    digest.as_slice().try_into().expect("SHA-256 has a fixed 32-byte output")
}

fn xor_map_term(digest: &str, term: [u8; 32]) -> Result<String, GhostraceError> {
    let mut value = decode_sha_digest(digest)?;
    for (left, right) in value.iter_mut().zip(term) {
        *left ^= right;
    }
    Ok(encode_sha_digest(&value))
}

fn decode_sha_digest(value: &str) -> Result<[u8; 32], GhostraceError> {
    if !valid_sha256_digest(value) {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "incremental digest is not canonical".to_owned(),
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes()[7..].chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(chunk[0])? << 4) | hex_value(chunk[1])?;
    }
    Ok(bytes)
}

fn encode_sha_digest(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(71);
    output.push_str("sha256:");
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn hex_value(value: u8) -> Result<u8, GhostraceError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(GhostraceError::AuthenticatedStateInvalid(
            "incremental digest contains non-hex bytes".to_owned(),
        )),
    }
}

fn row_key_digest(table_name: &str, row_key: &str) -> String {
    sha_digest(
        Sha256::digest(canonical_fields(&[
            ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
            ("table", table_name.as_bytes()),
            ("key", row_key.as_bytes()),
        ]))
        .as_slice(),
    )
}

fn row_commitment_digest(table_name: &str, canonical: &[u8]) -> String {
    sha_digest(
        Sha256::digest(canonical_fields(&[
            ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
            ("table", table_name.as_bytes()),
            ("row", canonical),
        ]))
        .as_slice(),
    )
}

fn current_commitment(
    connection: &Connection,
    table_name: &str,
    row_key: &str,
) -> Result<Option<CommitmentRow>, GhostraceError> {
    match table_name {
        "events" => {
            let raw = connection
                .query_row(
                    "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at,
                            source, kind, collector_instance, source_cursor,
                            provenance_version, policy_profile_id, policy_profile_version,
                            evidence, parent_event_id, payload_ciphertext
                     FROM events WHERE ingest_seq = ?1",
                    params![row_key],
                    |row| {
                        let ingest_seq = to_u64_sql(row.get(0)?, "event ingest sequence")?;
                        let event_id = bounded_text_ref(row, 1, "event ID", MAX_EVENT_ID_BYTES)
                            .map_err(sql_conversion_error)?;
                        let canonical = canonical_event_row(row).map_err(sql_conversion_error)?;
                        Ok((ingest_seq, event_id, canonical))
                    },
                )
                .optional()?;
            Ok(raw.map(|(ingest_seq, event_id, canonical)| CommitmentRow {
                table_name: "events",
                row_key_digest: row_key_digest(table_name, &event_id),
                row_locator: ingest_seq.to_string(),
                row_digest: row_commitment_digest(table_name, &canonical),
            }))
        }
        "cursors" => {
            let (source, collector) = split_row_key(row_key)?;
            let raw = connection
                .query_row(
                    "SELECT source, collector_instance, source_cursor, updated_at, epoch,
                            state, cursor_kind, policy_profile_id, policy_profile_version,
                            last_event_id, boundary_json
                     FROM cursors WHERE source = ?1 AND collector_instance = ?2",
                    params![source, collector],
                    |row| canonical_cursor_row(row).map_err(sql_conversion_error),
                )
                .optional()?;
            Ok(raw.map(|canonical| CommitmentRow {
                table_name: "cursors",
                row_key_digest: row_key_digest(table_name, row_key),
                row_locator: String::new(),
                row_digest: row_commitment_digest(table_name, &canonical),
            }))
        }
        "policy_metadata" => {
            let (profile_id, profile_version) = split_policy_key(row_key)?;
            let raw = connection
                .query_row(
                    "SELECT profile_id, profile_version, profile_json, recorded_at
                     FROM policy_metadata WHERE profile_id = ?1 AND profile_version = ?2",
                    params![profile_id, profile_version],
                    |row| canonical_policy_row(row).map_err(sql_conversion_error),
                )
                .optional()?;
            Ok(raw.map(|canonical| CommitmentRow {
                table_name: "policy_metadata",
                row_key_digest: row_key_digest(table_name, row_key),
                row_locator: String::new(),
                row_digest: row_commitment_digest(table_name, &canonical),
            }))
        }
        "diagnostics" => {
            let diagnostic_id = row_key.parse::<i64>().map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(
                    "diagnostic commitment key is invalid".to_owned(),
                )
            })?;
            let raw = connection
                .query_row(
                    "SELECT diagnostic_id, code, detail, created_at
                     FROM diagnostics WHERE diagnostic_id = ?1",
                    params![diagnostic_id],
                    |row| canonical_diagnostic_row(row).map_err(sql_conversion_error),
                )
                .optional()?;
            Ok(raw.map(|canonical| CommitmentRow {
                table_name: "diagnostics",
                row_key_digest: row_key_digest(table_name, row_key),
                row_locator: String::new(),
                row_digest: row_commitment_digest(table_name, &canonical),
            }))
        }
        _ => Err(GhostraceError::AuthenticatedStateInvalid(
            "unknown pending authenticated table".to_owned(),
        )),
    }
}

fn split_row_key(row_key: &str) -> Result<(&str, &str), GhostraceError> {
    row_key.split_once('\0').ok_or_else(|| {
        GhostraceError::AuthenticatedStateInvalid(
            "cursor commitment key is not canonical".to_owned(),
        )
    })
}

fn split_policy_key(row_key: &str) -> Result<(&str, i64), GhostraceError> {
    let (profile_id, version) = row_key.split_once('\0').ok_or_else(|| {
        GhostraceError::AuthenticatedStateInvalid(
            "policy commitment key is not canonical".to_owned(),
        )
    })?;
    let version = version.parse::<i64>().map_err(|_| {
        GhostraceError::AuthenticatedStateInvalid(
            "policy commitment version is not canonical".to_owned(),
        )
    })?;
    Ok((profile_id, version))
}

fn sql_conversion_error(error: GhostraceError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Blob, Box::new(error))
}

fn bounded_auth_bytes(label: &str, value: &[u8], max_bytes: usize) -> Result<(), GhostraceError> {
    if value.len() > max_bytes {
        return Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} exceeds the authenticated bound"
        )));
    }
    Ok(())
}

/// Read authenticated SQLite text without first asking rusqlite to allocate a
/// `String`. `ValueRef::Text` is backed by sqlite3_column_bytes(), so embedded
/// NUL bytes are counted and rejected rather than truncated by SQLite's TEXT
/// length() semantics.
fn bounded_text_ref(
    row: &Row<'_>,
    index: usize,
    label: &str,
    max_bytes: usize,
) -> Result<String, GhostraceError> {
    let text = bounded_utf8_ref(row, index, label, max_bytes)?;
    if text.chars().any(char::is_control) {
        return Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} contains a control character"
        )));
    }
    Ok(text)
}

fn bounded_utf8_ref(
    row: &Row<'_>,
    index: usize,
    label: &str,
    max_bytes: usize,
) -> Result<String, GhostraceError> {
    let value = row.get_ref(index).map_err(GhostraceError::from)?;
    let bytes = match value {
        ValueRef::Text(bytes) => bytes,
        _ => {
            return Err(GhostraceError::AuthenticatedStateInvalid(format!(
                "{label} has an invalid SQLite type"
            )))
        }
    };
    bounded_auth_bytes(label, bytes, max_bytes)?;
    let text = std::str::from_utf8(bytes).map_err(|_| {
        GhostraceError::AuthenticatedStateInvalid(format!("{label} is not valid UTF-8"))
    })?;
    Ok(text.to_owned())
}

fn bounded_optional_text_ref(
    row: &Row<'_>,
    index: usize,
    label: &str,
    max_bytes: usize,
) -> Result<Option<String>, GhostraceError> {
    let value = row.get_ref(index).map_err(GhostraceError::from)?;
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => {
            bounded_auth_bytes(label, bytes, max_bytes)?;
            let text = std::str::from_utf8(bytes).map_err(|_| {
                GhostraceError::AuthenticatedStateInvalid(format!("{label} is not valid UTF-8"))
            })?;
            if text.chars().any(char::is_control) {
                return Err(GhostraceError::AuthenticatedStateInvalid(format!(
                    "{label} contains a control character"
                )));
            }
            Ok(Some(text.to_owned()))
        }
        _ => Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} has an invalid SQLite type"
        ))),
    }
}

fn bounded_key_ref(
    row: &Row<'_>,
    index: usize,
    label: &str,
    max_bytes: usize,
) -> Result<String, GhostraceError> {
    let text = bounded_utf8_ref(row, index, label, max_bytes)?;
    if text.chars().any(|character| character.is_control() && character != '\0') {
        return Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} contains a non-canonical control character"
        )));
    }
    Ok(text)
}

fn bounded_blob_ref(
    row: &Row<'_>,
    index: usize,
    label: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, GhostraceError> {
    let value = row.get_ref(index).map_err(GhostraceError::from)?;
    let bytes = match value {
        ValueRef::Blob(bytes) => bytes,
        _ => {
            return Err(GhostraceError::AuthenticatedStateInvalid(format!(
                "{label} has an invalid SQLite type"
            )))
        }
    };
    bounded_auth_bytes(label, bytes, max_bytes)?;
    Ok(bytes.to_vec())
}

fn canonical_event_row(row: &Row<'_>) -> Result<Vec<u8>, GhostraceError> {
    let ingest_seq = to_u64(row.get::<_, i64>(0)?, "event ingest sequence")?;
    let event_id = bounded_text_ref(row, 1, "event ID", MAX_EVENT_ID_BYTES)?;
    let mut content = Vec::new();
    put_field(&mut content, "seq", &ingest_seq.to_le_bytes());
    put_field(&mut content, "event", event_id.as_bytes());
    put_field(&mut content, "schema", &row.get::<_, i64>(2)?.to_le_bytes());
    put_field(
        &mut content,
        "observed",
        bounded_text_ref(row, 3, "event observed_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
    );
    put_field(
        &mut content,
        "ingested",
        bounded_text_ref(row, 4, "event ingested_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
    );
    put_field(
        &mut content,
        "source",
        bounded_text_ref(row, 5, "event source", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(
        &mut content,
        "kind",
        bounded_text_ref(row, 6, "event kind", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(
        &mut content,
        "collector",
        bounded_text_ref(row, 7, "event collector", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_optional_field(
        &mut content,
        "cursor",
        bounded_optional_text_ref(row, 8, "event source cursor", MAX_CURSOR_BYTES)?,
    )?;
    put_field(
        &mut content,
        "provenance",
        bounded_text_ref(row, 9, "event provenance", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(
        &mut content,
        "policy-id",
        bounded_text_ref(row, 10, "event policy profile", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(&mut content, "policy-version", &row.get::<_, i64>(11)?.to_le_bytes());
    put_field(
        &mut content,
        "evidence",
        bounded_text_ref(row, 12, "event evidence", MAX_EVENT_PAYLOAD_BYTES)?.as_bytes(),
    );
    put_optional_field(
        &mut content,
        "parent",
        bounded_optional_text_ref(row, 13, "event parent", MAX_EVENT_ID_BYTES)?,
    )?;
    let ciphertext = bounded_blob_ref(row, 14, "event ciphertext", MAX_EVENT_CIPHERTEXT_BYTES)?;
    put_field(&mut content, "ciphertext", &ciphertext);
    Ok(canonical_fields(&[("row", &content)]))
}

fn canonical_cursor_row(row: &Row<'_>) -> Result<Vec<u8>, GhostraceError> {
    let mut bytes = Vec::new();
    for (index, label, max_bytes) in [
        (0, "cursor source", MAX_IDENTIFIER_BYTES),
        (1, "cursor collector", MAX_IDENTIFIER_BYTES),
        (2, "cursor source value", MAX_CURSOR_BYTES),
        (3, "cursor updated_at", MAX_TIMESTAMP_BYTES),
    ] {
        put_field(&mut bytes, "text", bounded_text_ref(row, index, label, max_bytes)?.as_bytes());
    }
    put_field(&mut bytes, "epoch", &row.get::<_, i64>(4)?.to_le_bytes());
    put_field(
        &mut bytes,
        "state",
        bounded_text_ref(row, 5, "cursor state", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(
        &mut bytes,
        "kind",
        bounded_text_ref(row, 6, "cursor kind", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_optional_field(
        &mut bytes,
        "policy-id",
        bounded_optional_text_ref(row, 7, "cursor policy profile", MAX_IDENTIFIER_BYTES)?,
    )?;
    put_optional_field(
        &mut bytes,
        "policy-version",
        row.get::<_, Option<i64>>(8)?.map(|value| value.to_string()),
    )?;
    put_optional_field(
        &mut bytes,
        "last-event",
        bounded_optional_text_ref(row, 9, "cursor last event", MAX_EVENT_ID_BYTES)?,
    )?;
    put_optional_field(
        &mut bytes,
        "boundary",
        bounded_optional_text_ref(row, 10, "cursor boundary", MAX_BOUNDARY_JSON_BYTES)?,
    )?;
    Ok(canonical_fields(&[("row", &bytes)]))
}

fn canonical_policy_row(row: &Row<'_>) -> Result<Vec<u8>, GhostraceError> {
    let mut bytes = Vec::new();
    put_field(
        &mut bytes,
        "id",
        bounded_text_ref(row, 0, "policy profile id", MAX_IDENTIFIER_BYTES)?.as_bytes(),
    );
    put_field(&mut bytes, "version", &row.get::<_, i64>(1)?.to_le_bytes());
    put_field(
        &mut bytes,
        "json",
        bounded_text_ref(row, 2, "policy JSON", MAX_POLICY_JSON_BYTES)?.as_bytes(),
    );
    put_field(
        &mut bytes,
        "recorded",
        bounded_text_ref(row, 3, "policy recorded_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
    );
    Ok(canonical_fields(&[("row", &bytes)]))
}

fn canonical_diagnostic_row(row: &Row<'_>) -> Result<Vec<u8>, GhostraceError> {
    let mut bytes = Vec::new();
    put_field(&mut bytes, "id", &row.get::<_, i64>(0)?.to_le_bytes());
    put_field(
        &mut bytes,
        "code",
        bounded_text_ref(row, 1, "diagnostic code", MAX_DIAGNOSTIC_CODE_BYTES)?.as_bytes(),
    );
    put_field(
        &mut bytes,
        "detail",
        bounded_text_ref(row, 2, "diagnostic detail", MAX_DIAGNOSTIC_DETAIL_BYTES)?.as_bytes(),
    );
    put_field(
        &mut bytes,
        "created",
        bounded_text_ref(row, 3, "diagnostic created_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
    );
    Ok(canonical_fields(&[("row", &bytes)]))
}

fn incremental_snapshot(connection: &Connection) -> Result<IncrementalSnapshot, GhostraceError> {
    #[cfg(test)]
    FULL_SNAPSHOT_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));

    // This is the explicit full-map checkpoint used at bootstrap, startup
    // verification, and rotation/retention boundaries. It is intentionally
    // O(number of live rows) and is not part of the normal one-operation hot
    // path, which consumes only trigger-recorded changes.

    let mut event_order = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut event_set = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut event_content = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut cursor = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut policy = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut diagnostic = decode_sha_digest(EMPTY_INCREMENTAL_DIGEST)?;
    let mut commitments = Vec::new();
    let mut event_count = 0_u64;
    let mut max_ingest_seq = 0_u64;
    let mut statement = connection.prepare(
        "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                kind, collector_instance, source_cursor, provenance_version,
                policy_profile_id, policy_profile_version, evidence, parent_event_id,
                payload_ciphertext
         FROM events ORDER BY ingest_seq ASC",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let ingest_seq = to_u64(row.get::<_, i64>(0)?, "event ingest sequence")?;
        let event_id = bounded_text_ref(row, 1, "event ID", MAX_EVENT_ID_BYTES)?;
        let canonical = canonical_event_row(row)?;
        let commitment = CommitmentRow {
            table_name: "events",
            row_key_digest: row_key_digest("events", &event_id),
            row_locator: ingest_seq.to_string(),
            row_digest: row_commitment_digest("events", &canonical),
        };
        event_order = xor_bytes(event_order, map_term("events-order", &commitment, true, false));
        event_set = xor_bytes(event_set, map_term("events-set", &commitment, false, false));
        event_content =
            xor_bytes(event_content, map_term("events-content", &commitment, true, true));
        commitments.push(commitment);
        event_count = event_count.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("event count overflow".to_owned())
        })?;
        max_ingest_seq = max_ingest_seq.max(ingest_seq);
    }

    let mut statement = connection.prepare(
        "SELECT source, collector_instance, source_cursor, updated_at, epoch, state,
                cursor_kind, policy_profile_id, policy_profile_version, last_event_id,
                boundary_json
         FROM cursors ORDER BY source, collector_instance",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let source = bounded_text_ref(row, 0, "cursor source", MAX_IDENTIFIER_BYTES)?;
        let collector = bounded_text_ref(row, 1, "cursor collector", MAX_IDENTIFIER_BYTES)?;
        let row_key = join_auth_key(&source, &collector, "cursor logical key")?;
        let canonical = canonical_cursor_row(row)?;
        let commitment = CommitmentRow {
            table_name: "cursors",
            row_key_digest: row_key_digest("cursors", &row_key),
            row_locator: String::new(),
            row_digest: row_commitment_digest("cursors", &canonical),
        };
        cursor = xor_bytes(cursor, map_term("cursors", &commitment, false, true));
        commitments.push(commitment);
    }

    let mut statement = connection.prepare(
        "SELECT profile_id, profile_version, profile_json, recorded_at
         FROM policy_metadata ORDER BY profile_id, profile_version",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let profile_id = bounded_text_ref(row, 0, "policy profile id", MAX_IDENTIFIER_BYTES)?;
        let profile_version: i64 = row.get(1)?;
        let row_key =
            join_auth_key(&profile_id, &profile_version.to_string(), "policy logical key")?;
        let canonical = canonical_policy_row(row)?;
        let commitment = CommitmentRow {
            table_name: "policy_metadata",
            row_key_digest: row_key_digest("policy_metadata", &row_key),
            row_locator: String::new(),
            row_digest: row_commitment_digest("policy_metadata", &canonical),
        };
        policy = xor_bytes(policy, map_term("policy_metadata", &commitment, false, true));
        commitments.push(commitment);
    }

    let mut statement = connection.prepare(
        "SELECT diagnostic_id, code, detail, created_at FROM diagnostics ORDER BY diagnostic_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let diagnostic_id: i64 = row.get(0)?;
        let row_key = diagnostic_id.to_string();
        let canonical = canonical_diagnostic_row(row)?;
        let commitment = CommitmentRow {
            table_name: "diagnostics",
            row_key_digest: row_key_digest("diagnostics", &row_key),
            row_locator: String::new(),
            row_digest: row_commitment_digest("diagnostics", &canonical),
        };
        diagnostic = xor_bytes(diagnostic, map_term("diagnostics", &commitment, false, true));
        commitments.push(commitment);
    }

    Ok(IncrementalSnapshot {
        event_count,
        max_ingest_seq,
        event_order_digest: encode_sha_digest(&event_order),
        event_set_digest: encode_sha_digest(&event_set),
        event_content_digest: encode_sha_digest(&event_content),
        cursor_digest: encode_sha_digest(&cursor),
        policy_digest: encode_sha_digest(&policy),
        diagnostic_digest: encode_sha_digest(&diagnostic),
        commitments,
    })
}

fn xor_bytes(mut left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    for (left, right) in left.iter_mut().zip(right) {
        *left ^= right;
    }
    left
}

fn report_incremental(
    connection: &Connection,
    provider: &dyn KeyProvider,
    state: AuthenticatedState,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    if state.validate().is_err() {
        return report_incremental_with_anomalies(
            &state,
            vec![AuthenticatedAnomaly::AnchorInvalid],
        );
    }
    if !incremental_schema_complete(connection)? {
        return report_incremental_with_anomalies(
            &state,
            vec![AuthenticatedAnomaly::AnchorInvalid],
        );
    }
    let snapshot = incremental_snapshot(connection)?;
    let replayed_event_count = count_replayed_events(connection)?;
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
    if replayed_event_count > 0 {
        anomalies.push(AuthenticatedAnomaly::EventReplayed);
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
    if max_pending_change_id(connection)? > state.pending_change_id {
        anomalies.push(AuthenticatedAnomaly::AnchorInvalid);
    }
    if !commitments_match(connection, &snapshot.commitments)? {
        anomalies.push(AuthenticatedAnomaly::AnchorInvalid);
    }
    if verify_operation_chain(connection, provider, &state, &snapshot.commitments).is_err() {
        anomalies.push(AuthenticatedAnomaly::AnchorInvalid);
    }
    match provider.key_for_generation(state.key_generation) {
        Ok(key) if head_mac(&state, &key) != state.head_mac => {
            anomalies.push(AuthenticatedAnomaly::AnchorInvalid);
        }
        Ok(_) => {}
        Err(_) => anomalies.push(AuthenticatedAnomaly::KeyUnavailable),
    }
    report_incremental_with_anomalies_state(&state, snapshot, anomalies)
}

fn report_incremental_with_anomalies(
    state: &AuthenticatedState,
    anomalies: Vec<AuthenticatedAnomaly>,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    let snapshot = CanonicalSnapshot {
        event_count: state.event_count,
        max_ingest_seq: state.max_ingest_seq,
        event_order_digest: state.event_order_digest.clone(),
        event_set_digest: state.event_set_digest.clone(),
        event_content_digest: state.event_content_digest.clone(),
        cursor_digest: state.cursor_digest.clone(),
        policy_digest: state.policy_digest.clone(),
        diagnostic_digest: state.diagnostic_digest.clone(),
    };
    report_with_anomalies(Some(state), &snapshot, anomalies)
}

fn report_incremental_with_anomalies_state(
    state: &AuthenticatedState,
    snapshot: IncrementalSnapshot,
    anomalies: Vec<AuthenticatedAnomaly>,
) -> Result<AuthenticatedStateReport, GhostraceError> {
    let snapshot = CanonicalSnapshot {
        event_count: snapshot.event_count,
        max_ingest_seq: snapshot.max_ingest_seq,
        event_order_digest: snapshot.event_order_digest,
        event_set_digest: snapshot.event_set_digest,
        event_content_digest: snapshot.event_content_digest,
        cursor_digest: snapshot.cursor_digest,
        policy_digest: snapshot.policy_digest,
        diagnostic_digest: snapshot.diagnostic_digest,
    };
    report_with_anomalies(Some(state), &snapshot, anomalies)
}

fn commitments_match(
    connection: &Connection,
    expected: &[CommitmentRow],
) -> Result<bool, GhostraceError> {
    let count: i64 =
        connection
            .query_row("SELECT COUNT(*) FROM authenticated_commitments", [], |row| row.get(0))?;
    if count < 0 || count as usize != expected.len() {
        return Ok(false);
    }
    let mut statement = connection.prepare(
        "SELECT table_name, row_key_digest, row_locator, row_digest
         FROM authenticated_commitments",
    )?;
    let mut rows = statement.query([])?;
    let mut actual = HashSet::with_capacity(expected.len());
    while let Some(row) = rows.next()? {
        actual.insert((
            bounded_text_ref(row, 0, "authenticated commitment table", MAX_TABLE_NAME_BYTES)?,
            bounded_text_ref(row, 1, "authenticated commitment key digest", MAX_DIGEST_TEXT_BYTES)?,
            bounded_text_ref(row, 2, "authenticated commitment locator", MAX_EVENT_LOCATOR_BYTES)?,
            bounded_text_ref(row, 3, "authenticated commitment row digest", MAX_DIGEST_TEXT_BYTES)?,
        ));
    }
    let expected = expected
        .iter()
        .map(|row| {
            (
                row.table_name.to_owned(),
                row.row_key_digest.clone(),
                row.row_locator.clone(),
                row.row_digest.clone(),
            )
        })
        .collect::<HashSet<_>>();
    Ok(actual == expected)
}

fn operation_record_from_row(row: &Row<'_>) -> Result<OperationRecord, GhostraceError> {
    Ok(OperationRecord {
        sequence: to_u64(row.get::<_, i64>(0)?, "operation sequence")?,
        generation: to_u32_sql(row.get(1)?, "operation key generation")?,
        epoch: to_u64_sql(row.get(2)?, "operation chain epoch")?,
        previous_head: bounded_text_ref(
            row,
            3,
            "authenticated operation previous head",
            MAX_MAC_HEX_BYTES,
        )?,
        head: bounded_text_ref(row, 4, "authenticated operation head", MAX_MAC_HEX_BYTES)?,
        delta_digest: bounded_text_ref(
            row,
            5,
            "authenticated operation delta digest",
            MAX_DIGEST_TEXT_BYTES,
        )?,
        operation_mac: bounded_text_ref(row, 6, "authenticated operation MAC", MAX_MAC_HEX_BYTES)?,
        committed_at: bounded_text_ref(
            row,
            7,
            "authenticated operation committed_at",
            MAX_TIMESTAMP_BYTES,
        )?,
    })
}

fn verify_operation_chain(
    connection: &Connection,
    provider: &dyn KeyProvider,
    state: &AuthenticatedState,
    expected_commitments: &[CommitmentRow],
) -> Result<(), GhostraceError> {
    let mut statement = connection.prepare(
        "SELECT operation_seq, key_generation, chain_epoch, previous_head_mac,
                head_mac, delta_digest, operation_mac, committed_at
         FROM authenticated_operations ORDER BY operation_seq",
    )?;
    let mut rows = statement.query([])?;
    let mut replayed = BTreeMap::<(String, String), (String, String)>::new();
    let mut expected_seq = 0_u64;
    let mut previous_head = None::<String>;
    let mut final_head = None::<String>;
    let mut final_generation = None::<u32>;
    let mut final_epoch = None::<u64>;
    let mut operation_count = 0_u64;
    while let Some(row) = rows.next()? {
        // Keep only one authenticated operation record alive. A corrupted or
        // very long history must not first materialize the complete ledger.
        let record = operation_record_from_row(row)?;
        if record.sequence != expected_seq
            || !valid_sha256_digest(&record.delta_digest)
            || !valid_mac_hex(&record.operation_mac)
            || record.committed_at.is_empty()
            || !valid_mac_hex(&record.previous_head)
            || !valid_mac_hex(&record.head)
        {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation sequence is invalid".to_owned(),
            ));
        }
        if record.sequence > 0
            && (Some(record.generation) != final_generation || record.epoch != state.chain_epoch)
        {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation generation or epoch changed without a boundary".to_owned(),
            ));
        }
        if let Some(previous_head) = previous_head.as_deref() {
            if record.previous_head != previous_head {
                return Err(GhostraceError::AuthenticatedStateInvalid(
                    "authenticated operation chain is truncated".to_owned(),
                ));
            }
        } else if record.sequence == 0
            && (record.epoch != state.chain_epoch || record.previous_head != state.chain_start_mac)
        {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation chain root is invalid".to_owned(),
            ));
        }
        let key = provider.key_for_generation(record.generation)?;
        let expected_operation_mac = operation_mac(
            &key,
            record.sequence,
            record.generation,
            record.epoch,
            &record.previous_head,
            &record.delta_digest,
            &record.head,
            &record.committed_at,
        );
        if expected_operation_mac != record.operation_mac {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation MAC is invalid".to_owned(),
            ));
        }
        if record.sequence > 0 && record.epoch != state.chain_epoch {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation epoch is invalid".to_owned(),
            ));
        }
        let expected_delta = stream_operation_changes(connection, record.sequence, |_, change| {
            if record.sequence == 0 {
                if change.kind != "set" {
                    return Err(GhostraceError::AuthenticatedStateInvalid(
                        "authenticated operation bootstrap map is invalid".to_owned(),
                    ));
                }
                let row_digest = change.row_digest.as_ref().ok_or_else(|| {
                    GhostraceError::AuthenticatedStateInvalid(
                        "authenticated operation bootstrap row is missing".to_owned(),
                    )
                })?;
                if replayed
                    .insert(
                        (change.table_name.to_owned(), change.row_key_digest.clone()),
                        (change.row_locator.clone(), row_digest.clone()),
                    )
                    .is_some()
                {
                    return Err(GhostraceError::AuthenticatedStateInvalid(
                        "authenticated operation bootstrap map is duplicated".to_owned(),
                    ));
                }
            } else {
                let key = (change.table_name.to_owned(), change.row_key_digest.clone());
                match change.kind {
                    "set" => {
                        let row_digest = change.row_digest.as_ref().ok_or_else(|| {
                            GhostraceError::AuthenticatedStateInvalid(
                                "authenticated set operation row is missing".to_owned(),
                            )
                        })?;
                        replayed.insert(key, (change.row_locator.clone(), row_digest.clone()));
                    }
                    "tombstone" => {
                        if replayed.remove(&key).is_none() {
                            return Err(GhostraceError::AuthenticatedStateInvalid(
                                "authenticated tombstone removed a missing row".to_owned(),
                            ));
                        }
                    }
                    _ => unreachable!("validate_operation_change validates operation kind"),
                }
            }
            Ok(())
        })?;
        if expected_delta != record.delta_digest {
            return Err(GhostraceError::AuthenticatedStateInvalid(
                "authenticated operation delta is invalid".to_owned(),
            ));
        }
        expected_seq = expected_seq.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("operation sequence overflow".to_owned())
        })?;
        operation_count = operation_count.checked_add(1).ok_or_else(|| {
            GhostraceError::AuthenticatedStateInvalid("operation count overflow".to_owned())
        })?;
        previous_head = Some(record.head.clone());
        final_generation = Some(record.generation);
        final_epoch = Some(record.epoch);
        final_head = Some(record.head);
    }
    if operation_count == 0 {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation chain is empty".to_owned(),
        ));
    }
    if expected_seq != state.operation_seq.saturating_add(1)
        || final_head.as_deref() != Some(state.head_mac.as_str())
        || final_generation != Some(state.key_generation)
        || final_epoch != Some(state.chain_epoch)
    {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation chain does not end at the anchor".to_owned(),
        ));
    }
    if replayed.len() != expected_commitments.len()
        || expected_commitments.iter().any(|commitment| {
            replayed.get(&(commitment.table_name.to_owned(), commitment.row_key_digest.clone()))
                != Some(&(commitment.row_locator.clone(), commitment.row_digest.clone()))
        })
    {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation replay does not match live commitments".to_owned(),
        ));
    }
    let orphan_changes: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM authenticated_operation_changes AS changes
         LEFT JOIN authenticated_operations AS operations
           ON operations.operation_seq = changes.operation_seq
         WHERE operations.operation_seq IS NULL",
        [],
        |row| row.get(0),
    )?;
    if orphan_changes != 0 {
        return Err(GhostraceError::AuthenticatedStateInvalid(
            "authenticated operation history contains orphan changes".to_owned(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Each argument is a distinct authenticated wire field.
fn operation_mac(
    key: &[u8; 32],
    sequence: u64,
    generation: u32,
    epoch: u64,
    previous_head: &str,
    delta_digest: &str,
    state_head: &str,
    committed_at: &str,
) -> String {
    keyed_hex(
        key,
        &canonical_fields(&[
            ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
            ("operation", &sequence.to_le_bytes()),
            ("generation", &generation.to_le_bytes()),
            ("epoch", &epoch.to_le_bytes()),
            ("previous", previous_head.as_bytes()),
            ("delta", delta_digest.as_bytes()),
            ("state-head", state_head.as_bytes()),
            ("committed", committed_at.as_bytes()),
        ]),
    )
}

fn valid_mac_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical_snapshot(connection: &Connection) -> Result<CanonicalSnapshot, GhostraceError> {
    #[cfg(test)]
    FULL_SNAPSHOT_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));

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
        let event_id = bounded_text_ref(row, 1, "event ID", MAX_EVENT_ID_BYTES)?;
        let mut order_bytes = Vec::new();
        put_field(&mut order_bytes, "seq", &ingest_seq.to_le_bytes());
        put_field(&mut order_bytes, "event", event_id.as_bytes());
        event_order.update(&order_bytes);
        event_set.push(event_id.clone());

        let mut content = Vec::new();
        put_field(&mut content, "seq", &ingest_seq.to_le_bytes());
        put_field(&mut content, "event", event_id.as_bytes());
        put_field(&mut content, "schema", &row.get::<_, i64>(2)?.to_le_bytes());
        put_field(
            &mut content,
            "observed",
            bounded_text_ref(row, 3, "event observed_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
        );
        put_field(
            &mut content,
            "ingested",
            bounded_text_ref(row, 4, "event ingested_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
        );
        put_field(
            &mut content,
            "source",
            bounded_text_ref(row, 5, "event source", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(
            &mut content,
            "kind",
            bounded_text_ref(row, 6, "event kind", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(
            &mut content,
            "collector",
            bounded_text_ref(row, 7, "event collector", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_optional_field(
            &mut content,
            "cursor",
            bounded_optional_text_ref(row, 8, "event source cursor", MAX_CURSOR_BYTES)?,
        )?;
        put_field(
            &mut content,
            "provenance",
            bounded_text_ref(row, 9, "event provenance", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(
            &mut content,
            "policy-id",
            bounded_text_ref(row, 10, "event policy profile", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(&mut content, "policy-version", &row.get::<_, i64>(11)?.to_le_bytes());
        put_field(
            &mut content,
            "evidence",
            bounded_text_ref(row, 12, "event evidence", MAX_EVENT_PAYLOAD_BYTES)?.as_bytes(),
        );
        put_optional_field(
            &mut content,
            "parent",
            bounded_optional_text_ref(row, 13, "event parent", MAX_EVENT_ID_BYTES)?,
        )?;
        let ciphertext = bounded_blob_ref(row, 14, "event ciphertext", MAX_EVENT_CIPHERTEXT_BYTES)?;
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

/// Count copied event rows without putting replay fingerprints on the write
/// path.  The verifier compares every event field except ingest sequence and
/// event identity, matching the canonical replay definition above.
fn count_replayed_events(connection: &Connection) -> Result<u64, GhostraceError> {
    let count: i64 = connection.query_row(
        "SELECT COALESCE(SUM(repeated_count), 0)
         FROM (
             SELECT COUNT(*) - 1 AS repeated_count
             FROM events
             GROUP BY schema_version, observed_at, ingested_at, source, kind,
                      collector_instance, source_cursor, provenance_version,
                      policy_profile_id, policy_profile_version, evidence,
                      parent_event_id, payload_ciphertext
             HAVING COUNT(*) > 1
         )",
        [],
        |row| row.get(0),
    )?;
    to_u64(count, "replayed event count")
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
        for (index, label, max_bytes) in [
            (0, "cursor source", MAX_IDENTIFIER_BYTES),
            (1, "cursor collector", MAX_IDENTIFIER_BYTES),
            (2, "cursor source value", MAX_CURSOR_BYTES),
            (3, "cursor updated_at", MAX_TIMESTAMP_BYTES),
        ] {
            put_field(
                &mut bytes,
                "text",
                bounded_text_ref(row, index, label, max_bytes)?.as_bytes(),
            );
        }
        put_field(&mut bytes, "epoch", &row.get::<_, i64>(4)?.to_le_bytes());
        put_field(
            &mut bytes,
            "state",
            bounded_text_ref(row, 5, "cursor state", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(
            &mut bytes,
            "kind",
            bounded_text_ref(row, 6, "cursor kind", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_optional_field(
            &mut bytes,
            "policy-id",
            bounded_optional_text_ref(row, 7, "cursor policy profile", MAX_IDENTIFIER_BYTES)?,
        )?;
        put_optional_field(
            &mut bytes,
            "policy-version",
            row.get::<_, Option<i64>>(8)?.map(|v| v.to_string()),
        )?;
        put_optional_field(
            &mut bytes,
            "last-event",
            bounded_optional_text_ref(row, 9, "cursor last event", MAX_EVENT_ID_BYTES)?,
        )?;
        put_optional_field(
            &mut bytes,
            "boundary",
            bounded_optional_text_ref(row, 10, "cursor boundary", MAX_BOUNDARY_JSON_BYTES)?,
        )?;
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
        put_field(
            &mut bytes,
            "id",
            bounded_text_ref(row, 0, "policy profile id", MAX_IDENTIFIER_BYTES)?.as_bytes(),
        );
        put_field(&mut bytes, "version", &row.get::<_, i64>(1)?.to_le_bytes());
        put_field(
            &mut bytes,
            "json",
            bounded_text_ref(row, 2, "policy JSON", MAX_POLICY_JSON_BYTES)?.as_bytes(),
        );
        put_field(
            &mut bytes,
            "recorded",
            bounded_text_ref(row, 3, "policy recorded_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
        );
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
        put_field(
            &mut bytes,
            "code",
            bounded_text_ref(row, 1, "diagnostic code", MAX_DIAGNOSTIC_CODE_BYTES)?.as_bytes(),
        );
        put_field(
            &mut bytes,
            "detail",
            bounded_text_ref(row, 2, "diagnostic detail", MAX_DIAGNOSTIC_DETAIL_BYTES)?.as_bytes(),
        );
        put_field(
            &mut bytes,
            "created",
            bounded_text_ref(row, 3, "diagnostic created_at", MAX_TIMESTAMP_BYTES)?.as_bytes(),
        );
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
                    diagnostic_digest, deletion_count, deletion_digest, updated_at,
                    operation_seq, pending_change_id, event_order_context,
                    event_content_context
             FROM authenticated_state WHERE state_key = 'journal'",
            [],
            |row| {
                Ok(AuthenticatedState {
                    state_key: bounded_text_ref(
                        row,
                        0,
                        "authenticated state key",
                        MAX_STATE_KEY_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    schema_version: to_u32_sql(row.get(1)?, "auth schema")?,
                    chain_epoch: to_u64_sql(row.get(2)?, "chain epoch")?,
                    chain_start_mac: bounded_text_ref(
                        row,
                        3,
                        "authenticated chain start MAC",
                        MAX_MAC_HEX_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    head_mac: bounded_text_ref(row, 4, "authenticated head MAC", MAX_MAC_HEX_BYTES)
                        .map_err(sql_conversion_error)?,
                    key_generation: to_u32_sql(row.get(5)?, "key generation")?,
                    event_count: to_u64_sql(row.get(6)?, "event count")?,
                    max_ingest_seq: to_u64_sql(row.get(7)?, "max ingest sequence")?,
                    event_order_digest: bounded_text_ref(
                        row,
                        8,
                        "authenticated event-order digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    event_set_digest: bounded_text_ref(
                        row,
                        9,
                        "authenticated event-set digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    event_content_digest: bounded_text_ref(
                        row,
                        10,
                        "authenticated event-content digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    cursor_digest: bounded_text_ref(
                        row,
                        11,
                        "authenticated cursor digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    policy_digest: bounded_text_ref(
                        row,
                        12,
                        "authenticated policy digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    diagnostic_digest: bounded_text_ref(
                        row,
                        13,
                        "authenticated diagnostic digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    deletion_count: to_u64_sql(row.get(14)?, "deletion count")?,
                    deletion_digest: bounded_text_ref(
                        row,
                        15,
                        "authenticated deletion digest",
                        MAX_DIGEST_TEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    updated_at: bounded_text_ref(
                        row,
                        16,
                        "authenticated updated_at",
                        MAX_TIMESTAMP_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    operation_seq: to_u64_sql(row.get(17)?, "operation sequence")?,
                    pending_change_id: to_u64_sql(row.get(18)?, "pending change sequence")?,
                    event_order_context: bounded_blob_ref(
                        row,
                        19,
                        "authenticated event-order context",
                        MAX_CONTEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
                    event_content_context: bounded_blob_ref(
                        row,
                        20,
                        "authenticated event-content context",
                        MAX_CONTEXT_BYTES,
                    )
                    .map_err(sql_conversion_error)?,
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
            diagnostic_digest, deletion_count, deletion_digest, updated_at,
            operation_seq, pending_change_id, event_order_context, event_content_context
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
         ON CONFLICT(state_key) DO UPDATE SET
            schema_version=excluded.schema_version, chain_epoch=excluded.chain_epoch,
            chain_start_mac=excluded.chain_start_mac, head_mac=excluded.head_mac,
            key_generation=excluded.key_generation, event_count=excluded.event_count,
            max_ingest_seq=excluded.max_ingest_seq, event_order_digest=excluded.event_order_digest,
            event_set_digest=excluded.event_set_digest, event_content_digest=excluded.event_content_digest,
            cursor_digest=excluded.cursor_digest, policy_digest=excluded.policy_digest,
            diagnostic_digest=excluded.diagnostic_digest, deletion_count=excluded.deletion_count,
            deletion_digest=excluded.deletion_digest, updated_at=excluded.updated_at,
            operation_seq=excluded.operation_seq, pending_change_id=excluded.pending_change_id,
            event_order_context=excluded.event_order_context,
            event_content_context=excluded.event_content_context",
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
            state.operation_seq as i64,
            state.pending_change_id as i64,
            state.event_order_context,
            state.event_content_context,
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

fn join_auth_key(left: &str, right: &str, label: &str) -> Result<String, GhostraceError> {
    let total =
        left.len().checked_add(1).and_then(|length| length.checked_add(right.len())).ok_or_else(
            || GhostraceError::AuthenticatedStateInvalid(format!("{label} is too large")),
        )?;
    if total > MAX_LOGICAL_KEY_BYTES {
        return Err(GhostraceError::AuthenticatedStateInvalid(format!(
            "{label} exceeds the authenticated bound"
        )));
    }
    Ok(format!("{left}\0{right}"))
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
    if value.len() > MAX_EVENT_PAYLOAD_BYTES {
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

#[cfg(test)]
mod migration_lifecycle_tests {
    use super::*;
    use crate::{DeterministicKeyProvider, Journal};

    #[test]
    fn bootstrap_gate_is_closed_and_idempotent_after_promotion() {
        let mut connection = Connection::open_in_memory().expect("metadata connection");
        connection
            .execute_batch(
                "CREATE TABLE journal_metadata(
                    metadata_key TEXT PRIMARY KEY,
                    metadata_value TEXT NOT NULL
                );
                INSERT INTO journal_metadata(metadata_key, metadata_value)
                VALUES ('authenticated_state_bootstrap', 'pending');",
            )
            .expect("bootstrap metadata");

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("promotion transaction");
        complete_bootstrap_metadata(&transaction).expect("close bootstrap gate");
        transaction.commit().expect("commit gate");

        let state: String = connection
            .query_row(
                "SELECT metadata_value FROM journal_metadata
                 WHERE metadata_key = 'authenticated_state_bootstrap'",
                [],
                |row| row.get(0),
            )
            .expect("closed gate");
        assert_eq!(state, "complete");

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("repeat promotion transaction");
        complete_bootstrap_metadata(&transaction).expect("already closed gate remains valid");
        transaction.commit().expect("commit repeated gate");
    }

    #[test]
    fn bootstrap_gate_rejects_missing_metadata_instead_of_rearming() {
        let mut connection = Connection::open_in_memory().expect("metadata connection");
        connection
            .execute_batch(
                "CREATE TABLE journal_metadata(
                    metadata_key TEXT PRIMARY KEY,
                    metadata_value TEXT NOT NULL
                );",
            )
            .expect("metadata schema");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("promotion transaction");
        let error = complete_bootstrap_metadata(&transaction).expect_err("missing gate");
        assert!(matches!(error, GhostraceError::AuthenticatedStateInvalid(_)));
        transaction.rollback().expect("rollback failed promotion");
    }

    #[test]
    fn v2_open_closes_stale_pending_gate_before_anchor_deletion() {
        let directory = tempfile::tempdir().expect("synthetic journal directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("private synthetic directory");
        }
        let path = directory.path().join("journal.sqlite3");
        let journal = Journal::open_fixture(
            &path,
            DeterministicKeyProvider::from_seed("bootstrap-gate-lifecycle"),
        )
        .expect("journal");
        journal.initialize_authenticated_state().expect("initial anchor");

        let connection = Connection::open(&path).expect("metadata connection");
        connection
            .execute(
                "UPDATE journal_metadata SET metadata_value = 'pending'
                 WHERE metadata_key = 'authenticated_state_bootstrap'",
                [],
            )
            .expect("simulate stale promotion marker");
        drop(connection);

        journal.initialize_authenticated_state().expect("v2 open closes stale bootstrap marker");
        let connection = Connection::open(&path).expect("verification connection");
        let metadata: String = connection
            .query_row(
                "SELECT metadata_value FROM journal_metadata
                 WHERE metadata_key = 'authenticated_state_bootstrap'",
                [],
                |row| row.get(0),
            )
            .expect("bootstrap marker");
        assert_eq!(metadata, "complete");
        connection
            .execute("DELETE FROM authenticated_state WHERE state_key = 'journal'", [])
            .expect("delete anchor");
        drop(connection);

        let error = journal
            .initialize_authenticated_state()
            .expect_err("completed gate must refuse anchor rebootstrap");
        assert!(matches!(error, GhostraceError::AuthenticatedStateInvalid(_)));
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use crate::{
        DeterministicKeyProvider, DiagnosticRecord, IngestionOrigin, Journal, PolicyProfile,
    };

    /// A linear aggregate must not let mutually cancelling row edits bypass
    /// the keyed anchor. All mutation inputs here are synthetic fixture data;
    /// the mutation side never reads or uses the configured journal key.
    #[test]
    fn cancelling_diagnostic_edits_and_rewritten_commitments_are_refused() {
        const ROWS: usize = 257;
        const MASK_WORDS: usize = ROWS.div_ceil(64);
        type BasisEntry = ([u8; 32], [u64; MASK_WORDS]);

        let directory = tempfile::tempdir().expect("synthetic directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("private synthetic directory");
        }
        let path = directory.path().join("cancellation.sqlite3");
        let journal = Journal::open_fixture(
            &path,
            DeterministicKeyProvider::from_seed("synthetic-cancellation-regression"),
        )
        .expect("fixture journal");
        let diagnostics = (0..ROWS)
            .map(|index| DiagnosticRecord::new("synthetic.status", format!("original-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .expect("bounded diagnostics");
        journal
            .ingest_batch_with_diagnostics(
                &IngestionOrigin::fixture(),
                &[],
                &PolicyProfile::deny_by_default("cancellation-fixture"),
                &diagnostics,
            )
            .expect("authorized seed");
        assert!(journal.authenticated_state_report().expect("seed report").valid);

        let mut connection = Connection::open(&path).expect("unkeyed mutation connection");
        let mut basis: [Option<BasisEntry>; 256] = [None; 256];
        let mut alternatives = Vec::with_capacity(ROWS);
        let mut cancelling = None;
        for index in 0..ROWS {
            let id = (index + 1).to_string();
            let original = current_commitment(&connection, "diagnostics", &id)
                .expect("original commitment")
                .expect("existing diagnostic");
            let detail = format!("substituted-{index}");
            let canonical = connection
                .query_row(
                    "SELECT diagnostic_id, code, ?1, created_at FROM diagnostics
                     WHERE diagnostic_id = ?2",
                    params![detail, index as i64 + 1],
                    |row| Ok(canonical_diagnostic_row(row).expect("alternative canonical row")),
                )
                .expect("alternative row");
            let mut alternative = original.clone();
            alternative.row_digest = row_commitment_digest("diagnostics", &canonical);
            let mut difference = xor_bytes(
                map_term("diagnostics", &original, false, true),
                map_term("diagnostics", &alternative, false, true),
            );
            alternatives.push((detail, alternative));
            let mut mask = [0_u64; MASK_WORDS];
            mask[index / 64] |= 1 << (index % 64);
            for bit in 0..256 {
                if difference[bit / 8] & (1 << (bit % 8)) == 0 {
                    continue;
                }
                if let Some((vector, previous_mask)) = basis[bit] {
                    difference = xor_bytes(difference, vector);
                    for (word, previous) in mask.iter_mut().zip(previous_mask) {
                        *word ^= previous;
                    }
                } else {
                    basis[bit] = Some((difference, mask));
                    break;
                }
            }
            if difference == [0; 32] {
                cancelling = Some(mask);
                break;
            }
        }
        let mask = cancelling.expect("257 vectors in 256 dimensions have a dependency");
        let trigger: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger' AND name = 'authenticated_diagnostics_au'",
                [],
                |row| row.get(0),
            )
            .expect("original trigger definition");
        let transaction = connection.transaction().expect("mutation transaction");
        transaction
            .execute_batch("DROP TRIGGER authenticated_diagnostics_au")
            .expect("temporarily disable unkeyed change recording");
        let mut changed = 0;
        for (index, (detail, commitment)) in alternatives.iter().enumerate() {
            if mask[index / 64] & (1 << (index % 64)) == 0 {
                continue;
            }
            transaction
                .execute(
                    "UPDATE diagnostics SET detail = ?1 WHERE diagnostic_id = ?2",
                    params![detail, index as i64 + 1],
                )
                .expect("substitute synthetic diagnostic");
            transaction
                .execute(
                    "UPDATE authenticated_commitments SET row_digest = ?1
                     WHERE table_name = 'diagnostics' AND row_key_digest = ?2",
                    params![commitment.row_digest, commitment.row_key_digest],
                )
                .expect("rewrite unkeyed row commitment");
            changed += 1;
        }
        assert!(changed > 0, "at least one diagnostic must change");
        transaction.execute_batch(&trigger).expect("restore exact trigger");
        transaction.commit().expect("commit unkeyed mutation");
        let report = journal.authenticated_state_report().expect("full verification report");
        assert!(
            !report.valid,
            "full verification accepted {changed} cancelling edits without the journal key: {report:?}"
        );
    }
}
