//! SQLite journal and migration runner.

use std::{
    collections::HashSet,
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    crypto::{decrypt_payload, encrypt_payload, KeyProvider, SharedKeyProvider},
    cursor::{CursorIdentity, CursorKind, CursorState, CursorStatus, CursorToken, ReplayBoundary},
    error::GhostraceError,
    fault::{FaultPlan, FaultPoint},
    model::{
        CollectorInstanceId, EventEnvelope, EventKind, EventSource, Evidence, IngestionOrigin,
        OriginBinding, PolicyProfileId, ProvenanceVersion, SourceCursor, EVENT_SCHEMA_VERSION,
    },
    policy::PolicyProfile,
    query::{
        decode_page_token, make_token, validate_token_request, QueryOrderKey, QueryPage,
        QueryRequest, QueryTokenPayload,
    },
    storage,
    wal::{CheckpointMode, WalCheckpointReport, WalPolicy},
};

const MIGRATION_LEDGER: &str = include_str!("../migrations/0000_migration_ledger.sql");
const MIGRATION: &str = include_str!("../migrations/0001_init.sql");
const MIGRATION_METADATA: &str = include_str!("../migrations/0002_journal_metadata.sql");
const MIGRATION_CURSOR_CONTRACT: &str = include_str!("../migrations/0003_cursor_contract.sql");
const MIGRATION_REPLAY_BOUNDARY: &str = include_str!("../migrations/0004_replay_boundary.sql");
const MIGRATION_TOOL_VERSION: &str = concat!("ghostrace/", env!("CARGO_PKG_VERSION"));
const MIGRATION_MODE_KEY: &str = "mode";

#[derive(Clone, Copy, Debug)]
struct MigrationSpec {
    id: &'static str,
    version: u32,
    schema_version: u32,
    sql: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppliedMigration {
    pub migration_id: String,
    pub version: u32,
    pub checksum: String,
    pub schema_version: u32,
    pub tool_version: String,
    pub applied_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub ingest_seq: u64,
    pub event: EventEnvelope,
}

/// A bounded, privacy-safe diagnostic written in the same transaction as an
/// ingestion batch.  Diagnostics deliberately carry a stable code and short
/// detail only; callers must not put paths, payloads, or other untrusted text
/// in either field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticRecord {
    code: String,
    detail: String,
}

impl DiagnosticRecord {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Result<Self, GhostraceError> {
        let record = Self { code: code.into(), detail: detail.into() };
        record.validate()?;
        Ok(record)
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    fn validate(&self) -> Result<(), GhostraceError> {
        if self.code.is_empty()
            || self.code.len() > 64
            || !self
                .code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(GhostraceError::InvalidWriterDiagnostic(
                "code must be 1-64 ASCII identifier bytes".to_owned(),
            ));
        }
        if self.detail.len() > 512 || self.detail.chars().any(char::is_control) {
            return Err(GhostraceError::InvalidWriterDiagnostic(
                "detail must be at most 512 non-control bytes".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupReceipt {
    pub bytes: u64,
    pub wal_bytes: u64,
    pub frames_in_wal: u64,
    pub frames_checkpointed: u64,
}

#[derive(Clone)]
pub struct Journal {
    conn: Arc<Mutex<Connection>>,
    key_provider: SharedKeyProvider,
    path: Option<PathBuf>,
    wal_policy: WalPolicy,
    faults: FaultPlan,
}

impl Journal {
    pub fn new_in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory(provider)
    }

    pub fn open_in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory(provider)
    }

    /// Opens a file-backed journal through the hardened local path boundary.
    /// Live collection remains disabled, but path creation is held to the same
    /// ownership, mode, no-follow, and sidecar checks required by production.
    pub fn open_fixture<P, K>(path: P, provider: K) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
        K: KeyProvider + 'static,
    {
        Self::open_fixture_with_policy(path, provider, WalPolicy::default())
    }

    pub fn open_fixture_with_policy<P, K>(
        path: P,
        provider: K,
        wal_policy: WalPolicy,
    ) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
        K: KeyProvider + 'static,
    {
        Self::open_with_provider(path, Arc::new(provider), wal_policy, FaultPlan::none())
    }

    /// Opens a fixture journal with an explicit deterministic fault plan.
    /// Production callers should use [`Journal::open_fixture_with_policy`],
    /// which supplies the inert plan.
    pub fn open_fixture_with_fault_plan<P, K>(
        path: P,
        provider: K,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
        K: KeyProvider + 'static,
    {
        Self::open_fixture_with_policy_and_fault_plan(path, provider, WalPolicy::default(), faults)
    }

    pub fn open_fixture_with_policy_and_fault_plan<P, K>(
        path: P,
        provider: K,
        wal_policy: WalPolicy,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
        K: KeyProvider + 'static,
    {
        Self::open_with_provider(path, Arc::new(provider), wal_policy, faults)
    }

    fn open_with_provider<P>(
        path: P,
        provider: SharedKeyProvider,
        wal_policy: WalPolicy,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_path_buf();
        faults.hit(FaultPoint::StorageBeforeOpen)?;
        let connection = storage::open_database(&path)?;
        faults.hit(FaultPoint::StorageAfterOpen)?;
        Self::from_connection(connection, provider, Some(path), wal_policy, faults)
    }

    pub fn in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory_with_policy(provider, WalPolicy::default())
    }

    pub fn in_memory_with_policy<K>(
        provider: K,
        wal_policy: WalPolicy,
    ) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::from_connection(
            Connection::open_in_memory()?,
            Arc::new(provider),
            None,
            wal_policy,
            FaultPlan::none(),
        )
    }

    pub fn in_memory_with_fault_plan<K>(
        provider: K,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory_with_policy_and_fault_plan(provider, WalPolicy::default(), faults)
    }

    pub fn in_memory_with_policy_and_fault_plan<K>(
        provider: K,
        wal_policy: WalPolicy,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::from_connection(
            Connection::open_in_memory()?,
            Arc::new(provider),
            None,
            wal_policy,
            faults,
        )
    }

    fn from_connection(
        connection: Connection,
        provider: SharedKeyProvider,
        path: Option<PathBuf>,
        wal_policy: WalPolicy,
        faults: FaultPlan,
    ) -> Result<Self, GhostraceError> {
        let mut connection = connection;
        configure_connection(&connection, path.is_some(), wal_policy)?;
        run_migrations(&mut connection, &faults)?;
        if let Some(path) = path.as_deref() {
            faults.hit(FaultPoint::StorageBeforeVerify)?;
            storage::verify_database_artifacts(path)?;
            faults.hit(FaultPoint::StorageAfterVerify)?;
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
            key_provider: provider,
            path,
            wal_policy,
            faults,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn wal_policy(&self) -> WalPolicy {
        self.wal_policy
    }

    /// Replace the inert plan after opening a journal.  This is useful for
    /// exercising write, checkpoint, backup, and control transitions without
    /// also faulting the migration/open path.
    pub fn with_fault_plan(mut self, faults: FaultPlan) -> Self {
        self.faults = faults;
        self
    }

    /// Returns the SQLite mode.  File-backed journals should report `wal`; an
    /// in-memory SQLite connection reports `memory` because SQLite has no WAL
    /// file to coordinate there.
    pub fn journal_mode(&self) -> Result<String, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    pub fn synchronous_mode(&self) -> Result<String, GhostraceError> {
        let connection = self.lock_connection()?;
        let value: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        Ok(match value {
            0 => "OFF",
            1 => "NORMAL",
            2 => "FULL",
            3 => "EXTRA",
            _ => "UNKNOWN",
        }
        .to_owned())
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, GhostraceError> {
        let connection = self.lock_connection()?;
        let value: i64 = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        Ok(value == 1)
    }

    pub fn schema_version_count(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("SELECT COUNT(*) FROM schema_versions", [], |row| row.get(0))?)
    }

    pub fn schema_version(&self) -> Result<u32, GhostraceError> {
        let connection = self.lock_connection()?;
        read_user_version(&connection)
    }

    pub fn applied_migrations(&self) -> Result<Vec<AppliedMigration>, GhostraceError> {
        let connection = self.lock_connection()?;
        load_applied_migrations(&connection)
    }

    pub fn wal_autocheckpoint_pages(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))?)
    }

    pub fn busy_timeout_ms(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("PRAGMA busy_timeout", [], |row| row.get(0))?)
    }

    pub fn journal_size_limit_bytes(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        let value: i64 = connection.query_row("PRAGMA journal_size_limit", [], |row| row.get(0))?;
        Ok(value.max(0) as u64)
    }

    /// Run a bounded checkpoint and return the SQLite frame counts plus the
    /// observed `-wal` sidecar size. A checkpoint that cannot bring the WAL
    /// back under policy is an actionable refusal, not a silent best effort.
    pub fn checkpoint(&self, mode: CheckpointMode) -> Result<WalCheckpointReport, GhostraceError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(WalCheckpointReport::memory(mode, self.wal_policy.max_wal_bytes));
        };
        self.faults.hit(FaultPoint::CheckpointBefore)?;
        let connection = self.lock_connection()?;
        let (busy, frames_in_wal, frames_checkpointed): (i64, i64, i64) = connection.query_row(
            &format!("PRAGMA wal_checkpoint({})", mode.pragma_name()),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let wal_bytes = storage::wal_size_bytes(path)?;
        self.faults.hit(FaultPoint::StorageBeforeVerify)?;
        storage::verify_database_artifacts(path)?;
        self.faults.hit(FaultPoint::StorageAfterVerify)?;
        let frames_in_wal = frames_in_wal.max(0) as u64;
        let frames_checkpointed = frames_checkpointed.max(0) as u64;
        let report = WalCheckpointReport {
            mode,
            busy: busy != 0,
            frames_in_wal,
            frames_checkpointed,
            frames_remaining: frames_in_wal.saturating_sub(frames_checkpointed),
            wal_bytes,
            max_wal_bytes: self.wal_policy.max_wal_bytes,
        };
        if report.wal_bytes > report.max_wal_bytes || report.frames_remaining > 0 {
            return Err(GhostraceError::WalCheckpointRefused {
                frames_remaining: report.frames_remaining,
                wal_bytes: report.wal_bytes,
                max_wal_bytes: report.max_wal_bytes,
            });
        }
        self.faults.hit(FaultPoint::CheckpointAfter)?;
        Ok(report)
    }

    /// Perform the bounded shutdown checkpoint. Callers that keep the journal
    /// open may use `checkpoint` directly; shutdown always requests truncation
    /// so a clean close does not leave an unbounded sidecar behind.
    pub fn shutdown(&self) -> Result<WalCheckpointReport, GhostraceError> {
        self.checkpoint(CheckpointMode::Truncate)
    }

    /// Execute a read-only transaction on a separate connection for a
    /// file-backed journal. The transaction is rolled back when its elapsed
    /// lifetime exceeds the configured reader limit so it cannot pin the WAL.
    pub fn with_read_snapshot<T, F>(&self, reader: F) -> Result<T, GhostraceError>
    where
        F: FnOnce(&Connection) -> Result<T, GhostraceError>,
    {
        if let Some(path) = self.path.as_deref() {
            let connection = storage::open_read_only_database(path)?;
            configure_reader_connection(&connection, self.wal_policy)?;
            return run_read_snapshot(&connection, self.wal_policy, reader);
        }

        let connection = self.lock_connection()?;
        run_read_snapshot(&connection, self.wal_policy, reader)
    }

    /// Checkpoint the active WAL, then copy only the database file. A sidecar
    /// path is rejected because it is not a valid independent SQLite backup.
    pub fn backup_snapshot<P: AsRef<Path>>(
        &self,
        destination: P,
    ) -> Result<BackupReceipt, GhostraceError> {
        let Some(path) = self.path.as_deref() else {
            return Err(GhostraceError::BackupUnavailable);
        };
        self.faults.hit(FaultPoint::BackupBeforeCopy)?;
        let connection = self.lock_connection()?;
        let (busy, frames_in_wal, frames_checkpointed): (i64, i64, i64) =
            connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        let wal_bytes = storage::wal_size_bytes(path)?;
        storage::verify_database_artifacts(path)?;
        let frames_in_wal = frames_in_wal.max(0) as u64;
        let frames_checkpointed = frames_checkpointed.max(0) as u64;
        let frames_remaining = frames_in_wal.saturating_sub(frames_checkpointed);
        if busy != 0 || frames_remaining > 0 || wal_bytes > self.wal_policy.max_wal_bytes {
            return Err(GhostraceError::WalCheckpointRefused {
                frames_remaining,
                wal_bytes,
                max_wal_bytes: self.wal_policy.max_wal_bytes,
            });
        }
        let destination = destination.as_ref();
        let bytes = storage::copy_database_snapshot(path, destination)?;
        self.faults.hit(FaultPoint::BackupAfterCopy)?;
        Ok(BackupReceipt { bytes, wal_bytes, frames_in_wal, frames_checkpointed })
    }

    pub fn ingest(
        &self,
        origin: &IngestionOrigin,
        event: &EventEnvelope,
        policy: &PolicyProfile,
    ) -> Result<u64, GhostraceError> {
        Ok(self.ingest_batch(origin, std::slice::from_ref(event), policy)?[0])
    }

    /// Ingest one event while binding its cursor advancement to a durable
    /// volume and stream configuration boundary.
    pub fn ingest_with_boundary(
        &self,
        origin: &IngestionOrigin,
        event: &EventEnvelope,
        policy: &PolicyProfile,
        boundary: &ReplayBoundary,
    ) -> Result<u64, GhostraceError> {
        Ok(self.ingest_batch_with_boundary(
            origin,
            std::slice::from_ref(event),
            policy,
            &[],
            Some(boundary),
        )?[0])
    }

    /// Inserts accepted events and cursor progress in one SQLite transaction.
    /// Parent references must already exist or point to an earlier event in this
    /// batch; a missing parent is rejected before commit.
    pub fn ingest_batch(
        &self,
        origin: &IngestionOrigin,
        events: &[EventEnvelope],
        policy: &PolicyProfile,
    ) -> Result<Vec<u64>, GhostraceError> {
        self.ingest_batch_with_diagnostics(origin, events, policy, &[])
    }

    /// Inserts accepted events, cursor progress, the policy reference, and
    /// bounded diagnostics in one SQLite transaction.  An acknowledgement from
    /// the durable writer is allowed only after this method has committed.
    pub fn ingest_batch_with_diagnostics(
        &self,
        origin: &IngestionOrigin,
        events: &[EventEnvelope],
        policy: &PolicyProfile,
        diagnostics: &[DiagnosticRecord],
    ) -> Result<Vec<u64>, GhostraceError> {
        self.ingest_batch_with_boundary(origin, events, policy, diagnostics, None)
    }

    /// Insert accepted events, cursor progress, policy evidence, diagnostics,
    /// and (when supplied) the replay boundary in one SQLite transaction.
    /// A changed boundary is refused before any event or cursor row is
    /// committed; callers must establish an explicit reset epoch first.
    pub fn ingest_batch_with_boundary(
        &self,
        origin: &IngestionOrigin,
        events: &[EventEnvelope],
        policy: &PolicyProfile,
        diagnostics: &[DiagnosticRecord],
        boundary: Option<&ReplayBoundary>,
    ) -> Result<Vec<u64>, GhostraceError> {
        let mut ids = HashSet::with_capacity(events.len());
        for event in events {
            origin.validate_event(event)?;
            event.validate()?;
            policy.authorize(event)?;
            if !ids.insert(event.event_id) {
                return Err(GhostraceError::InvalidEvent(format!(
                    "duplicate event_id in batch: {}",
                    event.event_id
                )));
            }
        }
        validate_boundary_events(boundary, events)?;
        for diagnostic in diagnostics {
            diagnostic.validate()?;
        }
        self.faults.hit(FaultPoint::IngestBeforeTransaction)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        self.faults.hit(FaultPoint::IngestAfterTransaction)?;
        record_policy_profile(&transaction, policy)?;
        let sequences = insert_events(
            &transaction,
            events,
            self.key_provider.as_ref(),
            &self.faults,
            boundary,
        )?;
        insert_diagnostics(&transaction, diagnostics, &self.faults)?;
        self.faults.hit(FaultPoint::IngestBeforeCommit)?;
        transaction.commit()?;
        self.faults.hit(FaultPoint::IngestAfterCommit)?;
        if let Some(path) = self.path.as_deref() {
            self.faults.hit(FaultPoint::StorageBeforeVerify)?;
            storage::verify_database_artifacts(path)?;
            self.faults.hit(FaultPoint::StorageAfterVerify)?;
        }
        Ok(sequences)
    }

    /// Read the durable state for one source/collector cursor identity.
    pub fn cursor_state(
        &self,
        identity: &CursorIdentity,
    ) -> Result<Option<CursorState>, GhostraceError> {
        let connection = self.lock_connection()?;
        read_cursor_state(&connection, identity)
    }

    /// Read the durable row selected by source and collector instance without
    /// hiding a stored volume or stream-identity mismatch. A restarting source
    /// must inspect that mismatch and emit a recovery gap instead of silently
    /// starting a fresh `SinceNow` stream.
    pub(crate) fn cursor_state_for_recovery(
        &self,
        identity: &CursorIdentity,
    ) -> Result<Option<CursorState>, GhostraceError> {
        let connection = self.lock_connection()?;
        read_cursor_state_for_recovery(&connection, identity)
    }

    /// Explicitly establish a new cursor epoch after a collector reset.  A
    /// reset is durable control state; the first event in the new epoch is
    /// accepted only after this operation has committed.
    pub fn reset_cursor(
        &self,
        identity: &CursorIdentity,
        cursor: &SourceCursor,
        policy: &PolicyProfile,
    ) -> Result<(), GhostraceError> {
        self.control_cursor(identity, cursor, policy, CursorStatus::Reset, None)
    }

    /// Establish a reset epoch and bind it to the supplied live replay
    /// boundary. The following first event must use the same boundary.
    pub fn reset_cursor_with_boundary(
        &self,
        identity: &CursorIdentity,
        cursor: &SourceCursor,
        policy: &PolicyProfile,
        boundary: &ReplayBoundary,
    ) -> Result<(), GhostraceError> {
        validate_boundary_identity(identity, boundary)?;
        self.control_cursor(identity, cursor, policy, CursorStatus::Reset, Some(boundary))
    }

    /// Explicitly record a source wrap.  Wrapped cursors must use the typed
    /// `wrap-<epoch>-<position>` representation so a wrap cannot be inferred
    /// from an opaque token.
    pub fn wrap_cursor(
        &self,
        identity: &CursorIdentity,
        cursor: &SourceCursor,
        policy: &PolicyProfile,
    ) -> Result<(), GhostraceError> {
        self.control_cursor(identity, cursor, policy, CursorStatus::Wrapped, None)
    }

    pub fn wrap_cursor_with_boundary(
        &self,
        identity: &CursorIdentity,
        cursor: &SourceCursor,
        policy: &PolicyProfile,
        boundary: &ReplayBoundary,
    ) -> Result<(), GhostraceError> {
        validate_boundary_identity(identity, boundary)?;
        self.control_cursor(identity, cursor, policy, CursorStatus::Wrapped, Some(boundary))
    }

    /// Invalidate a cursor after a source replacement or integrity failure.
    /// Ingestion remains fail-closed until [`Journal::reset_cursor`] or
    /// [`Journal::wrap_cursor`] establishes a new epoch.
    pub fn invalidate_cursor(&self, identity: &CursorIdentity) -> Result<(), GhostraceError> {
        self.faults.hit(FaultPoint::ControlBeforeTransaction)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        self.faults.hit(FaultPoint::ControlAfterTransaction)?;
        let changed = transaction.execute(
            "UPDATE cursors SET state = 'invalidated' WHERE source = ?1 AND collector_instance = ?2",
            params![identity.source.to_string(), identity.collector_instance()],
        )?;
        if changed == 0 {
            return Err(GhostraceError::CursorStateMissing { event_source: identity.source });
        }
        self.faults.hit(FaultPoint::ControlBeforeCommit)?;
        transaction.commit()?;
        self.faults.hit(FaultPoint::ControlAfterCommit)?;
        Ok(())
    }

    fn control_cursor(
        &self,
        identity: &CursorIdentity,
        cursor: &SourceCursor,
        policy: &PolicyProfile,
        status: CursorStatus,
        boundary: Option<&ReplayBoundary>,
    ) -> Result<(), GhostraceError> {
        let token = CursorToken::new(cursor.clone());
        if !token.is_ordered()
            || !matches!(token.kind(), CursorKind::Reset | CursorKind::Wrap)
            || (status == CursorStatus::Reset && token.kind() != CursorKind::Reset)
            || (status == CursorStatus::Wrapped && token.kind() != CursorKind::Wrap)
        {
            return Err(GhostraceError::CursorControlInvalid);
        }
        self.faults.hit(FaultPoint::ControlBeforeTransaction)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        self.faults.hit(FaultPoint::ControlAfterTransaction)?;
        record_policy_profile(&transaction, policy)?;
        let current: Option<(u64, Option<String>)> = transaction
            .query_row(
                "SELECT epoch, boundary_json FROM cursors
                 WHERE source = ?1 AND collector_instance = ?2",
                params![identity.source.to_string(), identity.collector_instance()],
                |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        if let Some((_, stored_json)) = current.as_ref() {
            let stored = parse_boundary_json(stored_json.clone())?;
            if stored.as_ref() != boundary {
                return Err(GhostraceError::CursorBoundaryMismatch {
                    event_source: identity.source,
                });
            }
        }
        let epoch = token
            .epoch()
            .unwrap_or_else(|| current.as_ref().map(|(epoch, _)| epoch + 1).unwrap_or(0));
        transaction.execute(
            "INSERT INTO cursors(
                source, collector_instance, source_cursor, updated_at, epoch, state,
                cursor_kind, policy_profile_id, policy_profile_version, last_event_id,
                boundary_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10)
             ON CONFLICT(source, collector_instance) DO UPDATE SET
                source_cursor=excluded.source_cursor, updated_at=excluded.updated_at,
                epoch=excluded.epoch, state=excluded.state, cursor_kind=excluded.cursor_kind,
                policy_profile_id=excluded.policy_profile_id,
                policy_profile_version=excluded.policy_profile_version,
                last_event_id=NULL, boundary_json=excluded.boundary_json",
            params![
                identity.source.to_string(),
                identity.collector_instance(),
                cursor.as_str(),
                Utc::now().to_rfc3339(),
                epoch as i64,
                status.as_str(),
                token.kind().as_str(),
                policy.id,
                policy.version,
                boundary.map(serde_json::to_string).transpose()?,
            ],
        )?;
        self.faults.hit(FaultPoint::ControlBeforeCommit)?;
        transaction.commit()?;
        self.faults.hit(FaultPoint::ControlAfterCommit)?;
        Ok(())
    }

    pub fn diagnostic_count(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("SELECT COUNT(*) FROM diagnostics", [], |row| row.get(0))?)
    }

    pub fn event(&self, event_id: Uuid) -> Result<EventEnvelope, GhostraceError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(EVENT_SELECT_BY_ID)?;
        let row = statement.query_row(params![event_id.to_string()], row_to_stored).optional()?;
        match row {
            Some(raw) => decode_stored(raw, self.key_provider.as_ref()).map(|stored| stored.event),
            None => Err(GhostraceError::EventNotFound(event_id)),
        }
    }

    pub fn events(&self) -> Result<Vec<StoredEvent>, GhostraceError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                    kind, collector_instance, source_cursor, provenance_version,
                    policy_profile_id, policy_profile_version, evidence, parent_event_id,
                    payload_ciphertext
             FROM events ORDER BY ingest_seq ASC",
        )?;
        let mut rows = statement.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(decode_stored(row_to_stored(row)?, self.key_provider.as_ref())?);
        }
        Ok(result)
    }

    /// Read one bounded page from a logical ingest snapshot.  The read
    /// transaction captures the initial upper bound; subsequent pages use the
    /// authenticated token's bound and therefore ignore later ingest. Rows
    /// removed by retention remain absent, which is explicit snapshot
    /// semantics rather than a fabricated tombstone.
    pub fn query_page(
        &self,
        request: &QueryRequest,
        page_token: Option<&str>,
    ) -> Result<QueryPage, GhostraceError> {
        request.validate()?;
        let token = page_token
            .map(|encoded| {
                decode_page_token(encoded, self.key_provider.as_ref(), Utc::now().timestamp())
            })
            .transpose()?;
        self.with_read_snapshot(|connection| {
            let storage_schema_version = read_user_version(connection)?;
            let compiled_schema_version =
                migration_specs().last().expect("migration catalog is non-empty").schema_version;
            if storage_schema_version != compiled_schema_version {
                return Err(GhostraceError::QuerySchemaChanged);
            }
            if let Some(token) = token.as_ref() {
                validate_token_request(token, request, storage_schema_version)?;
            }
            read_query_page(
                connection,
                self.key_provider.as_ref(),
                request,
                token.as_ref(),
                storage_schema_version,
            )
        })
    }

    pub fn raw_payload_ciphertext(&self, event_id: Uuid) -> Result<Vec<u8>, GhostraceError> {
        let connection = self.lock_connection()?;
        let value = connection
            .query_row(
                "SELECT payload_ciphertext FROM events WHERE event_id = ?1",
                params![event_id.to_string()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        value.ok_or(GhostraceError::EventNotFound(event_id))
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, GhostraceError> {
        self.conn.lock().map_err(|_| GhostraceError::Migration("journal mutex poisoned".to_owned()))
    }
}

fn record_policy_profile(
    transaction: &Transaction<'_>,
    profile: &PolicyProfile,
) -> Result<(), GhostraceError> {
    let json = serde_json::to_string(profile)?;
    let existing = transaction
        .query_row(
            "SELECT profile_json FROM policy_metadata
             WHERE profile_id = ?1 AND profile_version = ?2",
            params![profile.id, profile.version],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if existing != json {
            return Err(GhostraceError::InvalidEvent(
                "a policy ID and version cannot be reused with different rules".to_owned(),
            ));
        }
        return Ok(());
    }
    transaction.execute(
        "INSERT INTO policy_metadata(profile_id, profile_version, profile_json, recorded_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![profile.id, profile.version, json, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn validate_boundary_events(
    boundary: Option<&ReplayBoundary>,
    events: &[EventEnvelope],
) -> Result<(), GhostraceError> {
    let Some(boundary) = boundary else {
        return Ok(());
    };
    for event in events {
        if event.source != boundary.identity.source
            || event.collector_instance() != boundary.identity.collector_instance()
        {
            return Err(GhostraceError::CursorBoundaryMismatch { event_source: event.source });
        }
    }
    Ok(())
}

fn validate_boundary_identity(
    identity: &CursorIdentity,
    boundary: &ReplayBoundary,
) -> Result<(), GhostraceError> {
    if boundary.identity != *identity {
        return Err(GhostraceError::CursorBoundaryMismatch { event_source: identity.source });
    }
    Ok(())
}

fn validate_boundary_transition(
    event: &EventEnvelope,
    current: Option<&CursorState>,
    candidate: Option<&ReplayBoundary>,
) -> Result<(), GhostraceError> {
    let Some(current) = current else {
        return Ok(());
    };
    if candidate.is_none() && event_requires_recovery_gap(event) {
        // A path-free startup/recovery gap documents a boundary mismatch
        // without pretending that the new adapter configuration can bind to
        // the old replay contract.
        return Ok(());
    }
    match (current.boundary.as_ref(), candidate) {
        (Some(stored), Some(candidate)) if stored == candidate => Ok(()),
        (None, None) => Ok(()),
        (None, Some(_)) if current.last_event_id.is_none() => Ok(()),
        _ => Err(GhostraceError::CursorBoundaryMismatch { event_source: event.source }),
    }
}

fn insert_events(
    transaction: &Transaction<'_>,
    events: &[EventEnvelope],
    provider: &dyn KeyProvider,
    faults: &FaultPlan,
    boundary: Option<&ReplayBoundary>,
) -> Result<Vec<u64>, GhostraceError> {
    let mut sequences = Vec::with_capacity(events.len());
    for event in events {
        let token = event.source_cursor.as_ref().map(|cursor| CursorToken::new(cursor.clone()));
        let recovery_gap = event_requires_recovery_gap(event);
        let current = if token.is_some() || boundary.is_some() || recovery_gap {
            load_cursor_state_transaction(transaction, event.source, event.collector_instance())?
        } else {
            None
        };
        validate_boundary_transition(event, current.as_ref(), boundary)?;
        let existing_by_id = transaction
            .query_row(EVENT_SELECT_BY_ID, params![event.event_id.to_string()], row_to_stored)
            .optional()?;
        if let Some(raw) = existing_by_id {
            let existing = decode_stored(raw, provider)?;
            if !same_event_semantics(&existing.event, event) {
                return Err(GhostraceError::CursorConflict { event_source: event.source });
            }
            // A replay is acknowledged with its original durable sequence.  In
            // particular, a replay after a later cursor does not regress the
            // cursor table and does not create a second encrypted payload.
            sequences.push(existing.ingest_seq);
            continue;
        }

        if let Some(parent_id) = event.parent_event_id {
            let exists: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM events WHERE event_id = ?1",
                    params![parent_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(GhostraceError::InvalidEvent(format!(
                    "parent event does not exist: {parent_id}"
                )));
            }
        }

        if let Some(token) = token.as_ref() {
            if let Some(raw) = transaction
                .query_row(
                    EVENT_SELECT_BY_CURSOR,
                    params![
                        event.source.to_string(),
                        event.collector_instance(),
                        token.raw().as_str()
                    ],
                    row_to_stored,
                )
                .optional()?
            {
                let existing = decode_stored(raw, provider)?;
                if same_event_semantics(&existing.event, event) {
                    // Event IDs are part of the semantic identity, so this
                    // branch is only possible if the database was tampered
                    // with.  Fail closed rather than silently aliasing events.
                    return Err(GhostraceError::CursorConflict { event_source: event.source });
                }
                return Err(GhostraceError::CursorConflict { event_source: event.source });
            }
            validate_cursor_transition(event, token, current.as_ref())?;
        }

        let payload = serde_json::to_vec(&event.payload)?;
        let aad = associated_data(event)?;
        faults.hit(FaultPoint::KeyBeforeAccess)?;
        let ciphertext = encrypt_payload(provider, &aad, &payload)?;
        faults.hit(FaultPoint::KeyAfterAccess)?;
        faults.hit(FaultPoint::EventBeforeInsert)?;
        transaction.execute(
            "INSERT INTO events(
                event_id, schema_version, observed_at, ingested_at, source, kind,
                collector_instance, source_cursor, provenance_version, policy_profile_id,
                policy_profile_version, evidence, parent_event_id, payload_ciphertext
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                event.event_id.to_string(),
                event.schema_version,
                event.observed_at.to_rfc3339(),
                event.ingested_at.to_rfc3339(),
                event.source.to_string(),
                event.kind.to_string(),
                event.collector_instance(),
                event.source_cursor(),
                event.provenance_version(),
                event.policy_profile_id.as_str(),
                event.policy_profile_version,
                serde_json::to_string(&event.evidence)?,
                event.parent_event_id.map(|id| id.to_string()),
                ciphertext,
            ],
        )?;
        faults.hit(FaultPoint::EventAfterInsert)?;
        let sequence = transaction.last_insert_rowid() as u64;
        if let Some(token) = token {
            let epoch =
                token.epoch().or_else(|| current.as_ref().map(|state| state.epoch)).unwrap_or(0);
            let cursor_status = if recovery_gap { "invalidated" } else { "active" };
            faults.hit(FaultPoint::CursorBeforeUpdate)?;
            transaction.execute(
                "INSERT INTO cursors(
                    source, collector_instance, source_cursor, updated_at, epoch, state,
                    cursor_kind, policy_profile_id, policy_profile_version, last_event_id,
                    boundary_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(source, collector_instance) DO UPDATE SET
                    source_cursor=excluded.source_cursor, updated_at=excluded.updated_at,
                    epoch=excluded.epoch, state=excluded.state, cursor_kind=excluded.cursor_kind,
                    policy_profile_id=excluded.policy_profile_id,
                    policy_profile_version=excluded.policy_profile_version,
                    last_event_id=excluded.last_event_id,
                    boundary_json=excluded.boundary_json",
                params![
                    event.source.to_string(),
                    event.collector_instance(),
                    token.raw().as_str(),
                    event.ingested_at.to_rfc3339(),
                    epoch as i64,
                    cursor_status,
                    token.kind().as_str(),
                    event.policy_profile_id.as_str(),
                    event.policy_profile_version,
                    event.event_id.to_string(),
                    boundary.map(serde_json::to_string).transpose()?,
                ],
            )?;
            faults.hit(FaultPoint::CursorAfterUpdate)?;
        } else if recovery_gap && current.is_some() {
            // Keep an incomparable recovery gap and the recovery gate in one
            // SQLite transaction. A crash cannot leave the journal looking
            // continuously covered after the gap was already visible.
            faults.hit(FaultPoint::CursorBeforeUpdate)?;
            transaction.execute(
                "UPDATE cursors SET state = 'invalidated'
                 WHERE source = ?1 AND collector_instance = ?2",
                params![event.source.to_string(), event.collector_instance()],
            )?;
            faults.hit(FaultPoint::CursorAfterUpdate)?;
        }
        sequences.push(sequence);
    }
    Ok(sequences)
}

const EVENT_SELECT_BY_ID: &str =
    "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
        kind, collector_instance, source_cursor, provenance_version,
        policy_profile_id, policy_profile_version, evidence, parent_event_id,
        payload_ciphertext
 FROM events WHERE event_id = ?1";

const EVENT_SELECT_BY_CURSOR: &str =
    "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
        kind, collector_instance, source_cursor, provenance_version,
        policy_profile_id, policy_profile_version, evidence, parent_event_id,
        payload_ciphertext
 FROM events
 WHERE source = ?1 AND collector_instance = ?2 AND source_cursor = ?3
 ORDER BY ingest_seq ASC LIMIT 1";

fn same_event_semantics(left: &EventEnvelope, right: &EventEnvelope) -> bool {
    serde_json::to_vec(left).ok() == serde_json::to_vec(right).ok()
}

fn event_requires_recovery_gap(event: &EventEnvelope) -> bool {
    matches!(
        &event.payload,
        crate::model::EventPayload::Gap(payload)
            if payload.remediation.is_some() && payload.reason_code.as_str() != "cursor_jump"
    )
}

fn validate_cursor_transition(
    event: &EventEnvelope,
    candidate: &CursorToken,
    current: Option<&CursorState>,
) -> Result<(), GhostraceError> {
    let Some(current) = current else {
        return Ok(());
    };
    if current.invalidated() {
        return Err(GhostraceError::CursorInvalidated { event_source: event.source });
    }
    if current.last_event_id.is_some()
        && (current.policy_profile_id.as_deref() != Some(event.policy_profile_id.as_str())
            || current.policy_profile_version != Some(event.policy_profile_version))
    {
        return Err(GhostraceError::CursorPolicyMismatch { event_source: event.source });
    }
    if current.can_accept_first_event() && current.token.raw() == candidate.raw() {
        return Ok(());
    }
    match current.token.transition(candidate) {
        crate::cursor::CursorTransition::Advance => {
            if !matches!(
                event.kind,
                EventKind::Gap | EventKind::CollectorStarted | EventKind::CollectorStopped
            ) && current.token.epoch() == candidate.epoch()
                && current.token.position().zip(candidate.position()).is_some_and(
                    |(current_position, candidate_position)| {
                        candidate_position > current_position.saturating_add(1)
                    },
                )
            {
                return Err(GhostraceError::CursorSkipped { event_source: event.source });
            }
            Ok(())
        }
        crate::cursor::CursorTransition::Reset | crate::cursor::CursorTransition::Wrap => {
            if candidate.epoch().is_some_and(|epoch| epoch > current.epoch) {
                Ok(())
            } else {
                Err(GhostraceError::CursorRegression { event_source: event.source })
            }
        }
        crate::cursor::CursorTransition::Duplicate => {
            Err(GhostraceError::CursorConflict { event_source: event.source })
        }
        crate::cursor::CursorTransition::Regression => {
            Err(GhostraceError::CursorRegression { event_source: event.source })
        }
        crate::cursor::CursorTransition::Unknown => {
            Err(GhostraceError::CursorOrderingUnknown { event_source: event.source })
        }
        crate::cursor::CursorTransition::Initial => Ok(()),
    }
}

fn insert_diagnostics(
    transaction: &Transaction<'_>,
    diagnostics: &[DiagnosticRecord],
    faults: &FaultPlan,
) -> Result<(), GhostraceError> {
    for diagnostic in diagnostics {
        faults.hit(FaultPoint::DiagnosticBeforeInsert)?;
        transaction.execute(
            "INSERT INTO diagnostics(code, detail, created_at) VALUES (?1, ?2, ?3)",
            params![diagnostic.code, diagnostic.detail, Utc::now().to_rfc3339()],
        )?;
        faults.hit(FaultPoint::DiagnosticAfterInsert)?;
    }
    Ok(())
}

fn load_cursor_state_transaction(
    transaction: &Transaction<'_>,
    source: EventSource,
    collector_instance: &str,
) -> Result<Option<CursorState>, GhostraceError> {
    let row = transaction
        .query_row(
            "SELECT source_cursor, epoch, state, cursor_kind, policy_profile_id,
                    policy_profile_version, last_event_id, boundary_json
             FROM cursors WHERE source = ?1 AND collector_instance = ?2",
            params![source.to_string(), collector_instance],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(raw, epoch, status, kind, policy_id, policy_version, last_event_id, boundary_json)| {
            let _stored_kind = CursorKind::parse(&kind)?;
            let cursor = SourceCursor::try_from(raw)?;
            let epoch = u64::try_from(epoch).map_err(|_| {
                GhostraceError::MigrationLedger("cursor epoch is negative".to_owned())
            })?;
            let policy_profile_version = policy_version
                .map(|version| {
                    u32::try_from(version).map_err(|_| {
                        GhostraceError::MigrationLedger(
                            "cursor policy version is out of range".to_owned(),
                        )
                    })
                })
                .transpose()?;
            let boundary = parse_boundary_json(boundary_json)?;
            Ok(CursorState {
                identity: boundary
                    .as_ref()
                    .map(|boundary| boundary.identity.clone())
                    .unwrap_or(CursorIdentity::new(source, collector_instance.to_owned())?),
                token: CursorToken::new(cursor),
                status: CursorStatus::parse(&status)?,
                epoch,
                policy_profile_id: policy_id,
                policy_profile_version,
                last_event_id,
                boundary,
            })
        },
    )
    .transpose()
}

fn read_cursor_state(
    connection: &Connection,
    identity: &CursorIdentity,
) -> Result<Option<CursorState>, GhostraceError> {
    read_cursor_state_with_filter(connection, identity, true)
}

fn read_cursor_state_for_recovery(
    connection: &Connection,
    identity: &CursorIdentity,
) -> Result<Option<CursorState>, GhostraceError> {
    read_cursor_state_with_filter(connection, identity, false)
}

fn read_cursor_state_with_filter(
    connection: &Connection,
    identity: &CursorIdentity,
    require_identity_match: bool,
) -> Result<Option<CursorState>, GhostraceError> {
    let row = connection
        .query_row(
            "SELECT source_cursor, epoch, state, cursor_kind, policy_profile_id,
                    policy_profile_version, last_event_id, boundary_json
             FROM cursors WHERE source = ?1 AND collector_instance = ?2",
            params![identity.source.to_string(), identity.collector_instance()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(raw, epoch, status, kind, policy_id, policy_version, last_event_id, boundary_json)| {
            let _stored_kind = CursorKind::parse(&kind)?;
            let cursor = SourceCursor::try_from(raw)?;
            let epoch = u64::try_from(epoch).map_err(|_| {
                GhostraceError::MigrationLedger("cursor epoch is negative".to_owned())
            })?;
            let policy_profile_version = policy_version
                .map(|version| {
                    u32::try_from(version).map_err(|_| {
                        GhostraceError::MigrationLedger(
                            "cursor policy version is out of range".to_owned(),
                        )
                    })
                })
                .transpose()?;
            let boundary = parse_boundary_json(boundary_json)?;
            let stored_identity = boundary
                .as_ref()
                .map(|boundary| boundary.identity.clone())
                .unwrap_or_else(|| identity.clone());
            Ok(CursorState {
                identity: stored_identity,
                token: CursorToken::new(cursor),
                status: CursorStatus::parse(&status)?,
                epoch,
                policy_profile_id: policy_id,
                policy_profile_version,
                last_event_id,
                boundary,
            })
        },
    )
    .transpose()
    .map(|state| {
        if require_identity_match {
            state.filter(|state| {
                state.boundary.as_ref().is_none_or(|boundary| boundary.identity == *identity)
            })
        } else {
            state
        }
    })
}

fn parse_boundary_json(value: Option<String>) -> Result<Option<ReplayBoundary>, GhostraceError> {
    value.map(|value| serde_json::from_str(&value).map_err(GhostraceError::from)).transpose()
}

#[derive(Debug)]
struct RawStored {
    ingest_seq: u64,
    event_id: String,
    schema_version: u32,
    observed_at: String,
    ingested_at: String,
    source: String,
    kind: String,
    collector_instance: String,
    source_cursor: Option<String>,
    provenance_version: String,
    policy_profile_id: String,
    policy_profile_version: u32,
    evidence: String,
    parent_event_id: Option<String>,
    payload_ciphertext: Vec<u8>,
}

fn row_to_stored(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawStored> {
    Ok(RawStored {
        ingest_seq: row.get(0)?,
        event_id: row.get(1)?,
        schema_version: row.get(2)?,
        observed_at: row.get(3)?,
        ingested_at: row.get(4)?,
        source: row.get(5)?,
        kind: row.get(6)?,
        collector_instance: row.get(7)?,
        source_cursor: row.get(8)?,
        provenance_version: row.get(9)?,
        policy_profile_id: row.get(10)?,
        policy_profile_version: row.get(11)?,
        evidence: row.get(12)?,
        parent_event_id: row.get(13)?,
        payload_ciphertext: row.get(14)?,
    })
}

fn decode_stored(
    raw: RawStored,
    provider: &dyn KeyProvider,
) -> Result<StoredEvent, GhostraceError> {
    let aad = associated_data_raw(&raw)?;
    let event_id = Uuid::parse_str(&raw.event_id)
        .map_err(|_| GhostraceError::InvalidEvent("stored event_id is invalid".to_owned()))?;
    let schema_version = raw.schema_version;
    if schema_version != EVENT_SCHEMA_VERSION {
        return Err(GhostraceError::UnsupportedSchema(schema_version));
    }
    let observed_at = parse_timestamp(&raw.observed_at)?;
    let ingested_at = parse_timestamp(&raw.ingested_at)?;
    let source = parse_source(&raw.source)?;
    let kind = parse_kind(&raw.kind)?;
    let evidence: Evidence = serde_json::from_str(&raw.evidence)?;
    let payload: crate::model::EventPayload =
        serde_json::from_slice(&decrypt_payload(provider, &aad, &raw.payload_ciphertext)?)?;
    let parent_event_id =
        raw.parent_event_id.map(|id| Uuid::parse_str(&id)).transpose().map_err(|_| {
            GhostraceError::InvalidEvent("stored parent_event_id is invalid".to_owned())
        })?;
    let collector_instance = CollectorInstanceId::try_from(raw.collector_instance)?;
    let source_cursor = raw.source_cursor.map(SourceCursor::try_from).transpose()?;
    let provenance_version = ProvenanceVersion::try_from(raw.provenance_version)?;
    let policy_profile_id = PolicyProfileId::try_from(raw.policy_profile_id)?;
    let event = EventEnvelope::from_parts(
        EVENT_SCHEMA_VERSION,
        event_id,
        observed_at,
        ingested_at,
        source,
        kind,
        payload,
        collector_instance,
        source_cursor,
        provenance_version,
        policy_profile_id,
        raw.policy_profile_version,
        evidence,
        parent_event_id,
        OriginBinding::Stored,
    );
    event.validate()?;
    Ok(StoredEvent { ingest_seq: raw.ingest_seq, event })
}

fn read_query_page(
    connection: &Connection,
    provider: &dyn KeyProvider,
    request: &QueryRequest,
    token: Option<&QueryTokenPayload>,
    storage_schema_version: u32,
) -> Result<QueryPage, GhostraceError> {
    validate_query_policy_scope(connection, request)?;
    let snapshot_boundary = match token {
        Some(token) => token.snapshot_boundary,
        None => connection
            .query_row("SELECT COALESCE(MAX(ingest_seq), 0) FROM events", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(GhostraceError::from)
            .and_then(|value| u64::try_from(value).map_err(|_| GhostraceError::QueryInvalid))?,
    };
    let boundary = i64::try_from(snapshot_boundary).map_err(|_| GhostraceError::QueryInvalid)?;
    let source = request.source.map(|value| value.to_string());
    let kind = request.kind.map(|value| value.to_string());
    let observed_from = request.observed_from.map(|value| value.to_rfc3339());
    let observed_until = request.observed_until.map(|value| value.to_rfc3339());
    let last_observed =
        token.and_then(|value| value.last_order.as_ref().map(|order| order.observed_at.clone()));
    let last_sequence = token
        .and_then(|value| value.last_order.as_ref().map(|order| order.ingest_seq))
        .map(|value| i64::try_from(value).map_err(|_| GhostraceError::QueryInvalid))
        .transpose()?;
    let last_event_id =
        token.and_then(|value| value.last_order.as_ref().map(|order| order.event_id.to_string()));
    let limit = i64::try_from(request.page_size.saturating_add(1))
        .map_err(|_| GhostraceError::QueryInvalid)?;
    let mut statement = connection.prepare(
        "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                kind, collector_instance, source_cursor, provenance_version,
                policy_profile_id, policy_profile_version, evidence, parent_event_id,
                payload_ciphertext
         FROM events
         WHERE ingest_seq <= ?1
           AND policy_profile_id = ?2
           AND policy_profile_version = ?3
           AND (?4 IS NULL OR source = ?4)
           AND (?5 IS NULL OR kind = ?5)
           AND (?6 IS NULL OR observed_at >= ?6)
           AND (?7 IS NULL OR observed_at <= ?7)
           AND (
                ?8 IS NULL
                OR observed_at > ?8
                OR (observed_at = ?8 AND ingest_seq > ?9)
                OR (observed_at = ?8 AND ingest_seq = ?9 AND event_id > ?10)
           )
         ORDER BY observed_at ASC, ingest_seq ASC, event_id ASC
         LIMIT ?11",
    )?;
    let mut rows = statement.query(params![
        boundary,
        request.policy_profile_id.as_str(),
        request.policy_profile_version,
        source,
        kind,
        observed_from,
        observed_until,
        last_observed,
        last_sequence.unwrap_or(0),
        last_event_id,
        limit,
    ])?;
    let mut events = Vec::with_capacity(request.page_size);
    while let Some(row) = rows.next()? {
        events.push(decode_stored(row_to_stored(row)?, provider)?);
    }
    let has_more = events.len() > request.page_size;
    if has_more {
        events.pop();
    }
    let next_page_token = if has_more {
        events
            .last()
            .map(|stored| {
                make_token(
                    request,
                    storage_schema_version,
                    snapshot_boundary,
                    Some(QueryOrderKey {
                        observed_at: stored.event.observed_at.to_rfc3339(),
                        ingest_seq: stored.ingest_seq,
                        event_id: stored.event.event_id,
                    }),
                    provider,
                    Utc::now().timestamp(),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(QueryPage { events, next_page_token, snapshot_boundary })
}

fn validate_query_policy_scope(
    connection: &Connection,
    request: &QueryRequest,
) -> Result<(), GhostraceError> {
    let profile_json: Option<String> = connection
        .query_row(
            "SELECT profile_json FROM policy_metadata
             WHERE profile_id = ?1 AND profile_version = ?2",
            params![request.policy_profile_id.as_str(), request.policy_profile_version],
            |row| row.get(0),
        )
        .optional()?;
    let Some(profile_json) = profile_json else {
        return Ok(());
    };
    let profile: PolicyProfile =
        serde_json::from_str(&profile_json).map_err(|_| GhostraceError::QueryScopeMismatch)?;
    let digest = profile
        .to_document()
        .and_then(|document| document.scope_digest())
        .map_err(|_| GhostraceError::QueryScopeMismatch)?;
    if profile.id != request.policy_profile_id.as_str()
        || profile.version != request.policy_profile_version
        || digest != request.scope_digest
    {
        return Err(GhostraceError::QueryScopeMismatch);
    }
    Ok(())
}

#[derive(Serialize)]
struct MetadataAad<'a> {
    domain: &'static str,
    event_id: &'a str,
    schema_version: u32,
    observed_at: &'a str,
    ingested_at: &'a str,
    source: &'a str,
    kind: &'a str,
    collector_instance: &'a str,
    source_cursor: Option<&'a str>,
    provenance_version: &'a str,
    policy_profile_id: &'a str,
    policy_profile_version: u32,
    evidence: &'a str,
    parent_event_id: Option<&'a str>,
}

fn associated_data(event: &EventEnvelope) -> Result<Vec<u8>, GhostraceError> {
    let event_id = event.event_id.to_string();
    let observed_at = event.observed_at.to_rfc3339();
    let ingested_at = event.ingested_at.to_rfc3339();
    let source = event.source.to_string();
    let kind = event.kind.to_string();
    let evidence = serde_json::to_string(&event.evidence)?;
    let parent_event_id = event.parent_event_id.map(|id| id.to_string());
    serialize_associated_data(MetadataAad {
        domain: "ghostrace:event-metadata:v1",
        event_id: &event_id,
        schema_version: event.schema_version,
        observed_at: &observed_at,
        ingested_at: &ingested_at,
        source: &source,
        kind: &kind,
        collector_instance: event.collector_instance(),
        source_cursor: event.source_cursor(),
        provenance_version: event.provenance_version(),
        policy_profile_id: event.policy_profile_id.as_str(),
        policy_profile_version: event.policy_profile_version,
        evidence: &evidence,
        parent_event_id: parent_event_id.as_deref(),
    })
}

fn associated_data_raw(raw: &RawStored) -> Result<Vec<u8>, GhostraceError> {
    serialize_associated_data(MetadataAad {
        domain: "ghostrace:event-metadata:v1",
        event_id: &raw.event_id,
        schema_version: raw.schema_version,
        observed_at: &raw.observed_at,
        ingested_at: &raw.ingested_at,
        source: &raw.source,
        kind: &raw.kind,
        collector_instance: &raw.collector_instance,
        source_cursor: raw.source_cursor.as_deref(),
        provenance_version: &raw.provenance_version,
        policy_profile_id: &raw.policy_profile_id,
        policy_profile_version: raw.policy_profile_version,
        evidence: &raw.evidence,
        parent_event_id: raw.parent_event_id.as_deref(),
    })
}

fn serialize_associated_data(metadata: MetadataAad<'_>) -> Result<Vec<u8>, GhostraceError> {
    Ok(serde_json::to_vec(&metadata)?)
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, GhostraceError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| GhostraceError::InvalidEvent("stored timestamp is invalid".to_owned()))
}

fn parse_source(value: &str) -> Result<EventSource, GhostraceError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| GhostraceError::InvalidEvent("stored source is invalid".to_owned()))
}

fn parse_kind(value: &str) -> Result<crate::model::EventKind, GhostraceError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| GhostraceError::InvalidEvent("stored event kind is invalid".to_owned()))
}

fn migration_specs() -> [MigrationSpec; 5] {
    [
        MigrationSpec {
            id: "0000_migration_ledger",
            version: 0,
            schema_version: 0,
            sql: MIGRATION_LEDGER,
        },
        MigrationSpec { id: "0001_init", version: 1, schema_version: 1, sql: MIGRATION },
        MigrationSpec {
            id: "0002_journal_metadata",
            version: 2,
            schema_version: 2,
            sql: MIGRATION_METADATA,
        },
        MigrationSpec {
            id: "0003_cursor_contract",
            version: 3,
            schema_version: 3,
            sql: MIGRATION_CURSOR_CONTRACT,
        },
        MigrationSpec {
            id: "0004_replay_boundary",
            version: 4,
            schema_version: 4,
            sql: MIGRATION_REPLAY_BOUNDARY,
        },
    ]
}

fn migration_checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    let mut checksum = String::with_capacity(64);
    for byte in digest {
        write!(&mut checksum, "{byte:02x}").expect("writing to a string cannot fail");
    }
    checksum
}

fn run_migrations(connection: &mut Connection, faults: &FaultPlan) -> Result<(), GhostraceError> {
    // The ledger is validated before any event query or write is allowed. A
    // missing tail is safe to apply; a gap, mutation, or schema ahead of the
    // compiled catalog is never guessed at.
    let specs = migration_specs();
    initialize_migration_ledger(connection, &specs, faults)?;

    let mut records = load_applied_migrations(connection)?;
    let applied_count = validate_applied_prefix(&records, &specs)?;
    let latest_schema = specs.last().expect("migration catalog is non-empty").schema_version;
    let user_version = read_user_version(connection)?;
    let schema_versions = read_schema_versions(connection)?;
    let schema_max = schema_versions.last().copied().unwrap_or(0);
    if user_version > latest_schema || schema_max > latest_schema {
        return Err(GhostraceError::FutureMigration { version: user_version.max(schema_max) });
    }
    let record_schema =
        specs.get(applied_count.saturating_sub(1)).map(|spec| spec.schema_version).unwrap_or(0);
    if user_version < record_schema || schema_max < record_schema {
        return Err(GhostraceError::UnsupportedDowngrade {
            recorded: record_schema,
            database: user_version.min(schema_max),
        });
    }
    if user_version > record_schema || schema_max > record_schema {
        let next = specs.get(applied_count).map(|spec| spec.id).unwrap_or("unknown");
        return Err(GhostraceError::PartialMigration { migration_id: next.to_owned() });
    }

    for spec in specs.iter().skip(applied_count) {
        if schema_versions.contains(&spec.schema_version) || user_version >= spec.schema_version {
            return Err(GhostraceError::PartialMigration { migration_id: spec.id.to_owned() });
        }
        apply_migration(connection, spec, faults)?;
        records = load_applied_migrations(connection)?;
        validate_applied_prefix(&records, &specs)?;
    }

    validate_final_schema(connection, &specs)
}

fn initialize_migration_ledger(
    connection: &mut Connection,
    specs: &[MigrationSpec; 5],
    faults: &FaultPlan,
) -> Result<(), GhostraceError> {
    let ledger_exists = table_exists(connection, "migration_records")?;
    if !ledger_exists {
        let legacy_candidate = table_exists(connection, "schema_versions")?;
        let mode = if legacy_candidate { "legacy-candidate" } else { "new" };
        faults.hit(FaultPoint::MigrationBeforeTransaction)?;
        let transaction = connection.transaction()?;
        transaction.execute_batch(specs[0].sql)?;
        faults.hit(FaultPoint::MigrationAfterSql)?;
        insert_applied_migration(&transaction, &specs[0])?;
        transaction.execute(
            "INSERT INTO migration_state(state_key, state_value) VALUES (?1, ?2)",
            params![MIGRATION_MODE_KEY, mode],
        )?;
        faults.hit(FaultPoint::MigrationBeforeCommit)?;
        transaction.commit()?;
        faults.hit(FaultPoint::MigrationAfterCommit)?;
        if legacy_candidate {
            adopt_legacy_v1(connection, &specs[1], faults)?;
        }
        return Ok(());
    }

    let mode = migration_state(connection)?;
    if mode == "legacy-candidate" {
        adopt_legacy_v1(connection, &specs[1], faults)?;
    } else if mode != "new" && mode != "legacy-v1" {
        return Err(GhostraceError::MigrationLedger("unknown migration mode".to_owned()));
    }
    Ok(())
}

fn adopt_legacy_v1(
    connection: &mut Connection,
    spec: &MigrationSpec,
    faults: &FaultPlan,
) -> Result<(), GhostraceError> {
    validate_legacy_v1(connection)?;
    let records = load_applied_migrations(connection)?;
    if records.len() != 1 || records[0].version != 0 {
        return Err(GhostraceError::MigrationLedger(
            "legacy adoption requires only the ledger bootstrap record".to_owned(),
        ));
    }
    faults.hit(FaultPoint::MigrationBeforeTransaction)?;
    let transaction = connection.transaction()?;
    transaction.execute_batch("PRAGMA user_version = 1")?;
    faults.hit(FaultPoint::MigrationAfterSql)?;
    insert_applied_migration(&transaction, spec)?;
    transaction.execute(
        "UPDATE migration_state SET state_value = ?1 WHERE state_key = ?2",
        params!["legacy-v1", MIGRATION_MODE_KEY],
    )?;
    faults.hit(FaultPoint::MigrationBeforeCommit)?;
    transaction.commit()?;
    faults.hit(FaultPoint::MigrationAfterCommit)?;
    Ok(())
}

fn validate_legacy_v1(connection: &Connection) -> Result<(), GhostraceError> {
    let versions = read_schema_versions(connection)?;
    if versions != [1] {
        return Err(GhostraceError::PartialMigration { migration_id: "0001_init".to_owned() });
    }
    for table in ["events", "cursors", "policy_metadata", "diagnostics"] {
        if !table_exists(connection, table)? {
            return Err(GhostraceError::PartialMigration { migration_id: "0001_init".to_owned() });
        }
    }
    let user_version = read_user_version(connection)?;
    if user_version > 1 {
        return Err(GhostraceError::FutureMigration { version: user_version });
    }
    Ok(())
}

fn apply_migration(
    connection: &mut Connection,
    spec: &MigrationSpec,
    faults: &FaultPlan,
) -> Result<(), GhostraceError> {
    faults.hit(FaultPoint::MigrationBeforeTransaction)?;
    let transaction = connection.transaction()?;
    transaction.execute_batch(spec.sql)?;
    faults.hit(FaultPoint::MigrationAfterSql)?;
    maybe_crash_after_migration_sql(spec.id);
    if spec.version > 0 {
        transaction.execute(
            "INSERT OR IGNORE INTO schema_versions(version, applied_at) VALUES (?1, ?2)",
            params![spec.schema_version, Utc::now().to_rfc3339()],
        )?;
    }
    transaction.execute_batch(&format!("PRAGMA user_version = {}", spec.schema_version))?;
    insert_applied_migration(&transaction, spec)?;
    faults.hit(FaultPoint::MigrationBeforeCommit)?;
    transaction.commit()?;
    faults.hit(FaultPoint::MigrationAfterCommit)?;
    Ok(())
}

fn insert_applied_migration(
    transaction: &Transaction<'_>,
    spec: &MigrationSpec,
) -> Result<(), GhostraceError> {
    transaction.execute(
        "INSERT INTO migration_records(
            migration_id, version, checksum, schema_version, tool_version, applied_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            spec.id,
            spec.version,
            migration_checksum(spec.sql),
            spec.schema_version,
            MIGRATION_TOOL_VERSION,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn validate_applied_prefix(
    records: &[AppliedMigration],
    specs: &[MigrationSpec; 5],
) -> Result<usize, GhostraceError> {
    for (index, record) in records.iter().enumerate() {
        let Some(spec) = specs.get(index) else {
            return Err(GhostraceError::FutureMigration { version: record.version });
        };
        if record.version > spec.version {
            if record.version > specs.last().expect("migration catalog is non-empty").version {
                return Err(GhostraceError::FutureMigration { version: record.version });
            }
            return Err(GhostraceError::MigrationRecordMissing {
                migration_id: spec.id.to_owned(),
            });
        }
        if record.version != spec.version || record.migration_id != spec.id {
            return Err(GhostraceError::MigrationOrder {
                expected: spec.version,
                found: record.version,
            });
        }
        if record.checksum != migration_checksum(spec.sql) {
            return Err(GhostraceError::MigrationChecksumMismatch {
                migration_id: spec.id.to_owned(),
            });
        }
        if record.schema_version != spec.schema_version || record.tool_version.is_empty() {
            return Err(GhostraceError::MigrationLedger(
                "migration record metadata is inconsistent".to_owned(),
            ));
        }
        if record.applied_at.is_empty() {
            return Err(GhostraceError::MigrationLedger(
                "migration record timestamp is empty".to_owned(),
            ));
        }
    }
    Ok(records.len())
}

fn validate_final_schema(
    connection: &Connection,
    specs: &[MigrationSpec; 5],
) -> Result<(), GhostraceError> {
    let records = load_applied_migrations(connection)?;
    if records.len() != specs.len() {
        let next = specs.get(records.len()).map(|spec| spec.id).unwrap_or("unknown");
        return Err(GhostraceError::MigrationRecordMissing { migration_id: next.to_owned() });
    }
    validate_applied_prefix(&records, specs)?;
    let expected_versions: Vec<u32> =
        specs.iter().filter(|spec| spec.version > 0).map(|spec| spec.schema_version).collect();
    if read_schema_versions(connection)? != expected_versions {
        return Err(GhostraceError::MigrationLedger(
            "schema version rows do not match applied migrations".to_owned(),
        ));
    }
    let expected_schema = specs.last().expect("migration catalog is non-empty").schema_version;
    let user_version = read_user_version(connection)?;
    if user_version != expected_schema {
        if user_version < expected_schema {
            return Err(GhostraceError::UnsupportedDowngrade {
                recorded: expected_schema,
                database: user_version,
            });
        }
        return Err(GhostraceError::FutureMigration { version: user_version });
    }
    let format: Option<String> = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata WHERE metadata_key = 'format'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if format.as_deref() != Some("ghostrace-journal-v1") {
        return Err(GhostraceError::PartialMigration {
            migration_id: "0002_journal_metadata".to_owned(),
        });
    }
    Ok(())
}

fn load_applied_migrations(
    connection: &Connection,
) -> Result<Vec<AppliedMigration>, GhostraceError> {
    let mut statement = connection.prepare(
        "SELECT migration_id, version, checksum, schema_version, tool_version, applied_at
         FROM migration_records ORDER BY version ASC",
    )?;
    let mut rows = statement.query([])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let version = row.get::<_, i64>(1)?;
        let schema_version = row.get::<_, i64>(3)?;
        records.push(AppliedMigration {
            migration_id: row.get(0)?,
            version: u32::try_from(version).map_err(|_| {
                GhostraceError::MigrationLedger("migration version is out of range".to_owned())
            })?,
            checksum: row.get(2)?,
            schema_version: u32::try_from(schema_version).map_err(|_| {
                GhostraceError::MigrationLedger("schema version is out of range".to_owned())
            })?,
            tool_version: row.get(4)?,
            applied_at: row.get(5)?,
        });
    }
    Ok(records)
}

fn migration_state(connection: &Connection) -> Result<String, GhostraceError> {
    connection
        .query_row(
            "SELECT state_value FROM migration_state WHERE state_key = ?1",
            params![MIGRATION_MODE_KEY],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| GhostraceError::MigrationLedger("migration mode is missing".to_owned()))
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, GhostraceError> {
    let exists: i64 = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table],
        |row| row.get(0),
    )?;
    Ok(exists != 0)
}

fn read_schema_versions(connection: &Connection) -> Result<Vec<u32>, GhostraceError> {
    if !table_exists(connection, "schema_versions")? {
        return Ok(Vec::new());
    }
    let mut statement =
        connection.prepare("SELECT version FROM schema_versions ORDER BY version")?;
    let mut rows = statement.query([])?;
    let mut versions = Vec::new();
    while let Some(row) = rows.next()? {
        let version = row.get::<_, i64>(0)?;
        versions.push(u32::try_from(version).map_err(|_| {
            GhostraceError::MigrationLedger("schema version is out of range".to_owned())
        })?);
    }
    Ok(versions)
}

fn read_user_version(connection: &Connection) -> Result<u32, GhostraceError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    u32::try_from(version).map_err(|_| {
        GhostraceError::MigrationLedger("SQLite user_version is out of range".to_owned())
    })
}

fn maybe_crash_after_migration_sql(migration_id: &str) {
    // This hook is inert unless the migration integration test explicitly sets
    // the environment variable. Keeping it profile-independent lets the same
    // crash-recovery assertion run in both debug and release device lanes.
    if std::env::var("GHOSTRACE_TEST_MIGRATION_CRASH").ok().as_deref() == Some(migration_id) {
        std::process::abort();
    }
}

fn configure_connection(
    connection: &Connection,
    file_backed: bool,
    wal_policy: WalPolicy,
) -> Result<(), GhostraceError> {
    wal_policy.validate()?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(wal_policy.busy_timeout())?;
    connection.pragma_update(None, "wal_autocheckpoint", wal_policy.autocheckpoint_pages)?;
    connection.pragma_update(None, "journal_size_limit", wal_policy.max_wal_bytes as i64)?;
    if file_backed {
        connection.pragma_update(None, "journal_mode", "WAL")?;
    }
    connection.pragma_update(None, "synchronous", "FULL")?;
    Ok(())
}

fn configure_reader_connection(
    connection: &Connection,
    wal_policy: WalPolicy,
) -> Result<(), GhostraceError> {
    wal_policy.validate()?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(wal_policy.busy_timeout())?;
    Ok(())
}

fn run_read_snapshot<T, F>(
    connection: &Connection,
    wal_policy: WalPolicy,
    reader: F,
) -> Result<T, GhostraceError>
where
    F: FnOnce(&Connection) -> Result<T, GhostraceError>,
{
    let started = Instant::now();
    connection.execute_batch("BEGIN DEFERRED")?;
    let result = reader(connection);
    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    if elapsed_ms > wal_policy.max_reader_ms {
        let _ = connection.execute_batch("ROLLBACK");
        return Err(GhostraceError::LongReader { elapsed_ms, max_ms: wal_policy.max_reader_ms });
    }
    match result {
        Ok(value) => {
            connection.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}
