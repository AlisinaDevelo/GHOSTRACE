use std::path::PathBuf;

use thiserror::Error;

/// Errors returned by the fixture-only vertical slice.
#[derive(Debug, Error)]
pub enum GhostraceError {
    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("invalid browser URL: {0}")]
    InvalidUrl(String),

    #[error("private browser context is not accepted by default")]
    PrivateContext,

    #[error("policy denied event: {reason}")]
    PolicyDenied { reason: String },

    #[error("encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O operation failed")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("fixture line {line}: {message}")]
    FixtureLine { line: usize, message: String },

    #[error("event not found: {0}")]
    EventNotFound(uuid::Uuid),

    #[error("export destination already exists; pass --force to overwrite")]
    ExportExists(PathBuf),

    #[error("export destination must differ from the source journal")]
    ExportSourceConflict,

    #[error("journal directory permissions are not private (expected no group/world access)")]
    InsecurePermissions(PathBuf),

    #[error("journal path is unsafe")]
    UnsafePath,

    #[error("journal path changed during secure open")]
    PathRace,

    #[error("journal path owner is unexpected")]
    UnexpectedOwner,

    #[error("journal path has unexpected hard links")]
    UnexpectedHardLinks,

    #[error("fixture event provenance is invalid")]
    FixtureProvenance,

    #[error("ingestion origin does not authorize event")]
    OriginRejected,

    #[error("ingestion origin capability is not bound to this event")]
    OriginCapabilityMismatch,

    #[error("ingestion origin does not allow this event class")]
    OriginEventClass,

    #[error("an origin instance is required to construct an event")]
    OriginInstanceRequired,

    #[error("live capture is intentionally disabled until policy/cursor/Keychain gates land")]
    LiveCaptureDisabled,

    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),

    #[error("unsupported policy document schema version: {0}")]
    UnsupportedPolicySchema(u32),

    #[error("policy document migration rejected: {0}")]
    PolicyMigration(String),

    #[error("consent state transition rejected: {0}")]
    ConsentTransition(String),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("migration ledger is invalid: {0}")]
    MigrationLedger(String),

    #[error("migration checksum mismatch for {migration_id}")]
    MigrationChecksumMismatch { migration_id: String },

    #[error("migration record is missing for {migration_id}")]
    MigrationRecordMissing { migration_id: String },

    #[error("migration order is invalid: expected version {expected}, found {found}")]
    MigrationOrder { expected: u32, found: u32 },

    #[error("future migration version is unsupported: {version}")]
    FutureMigration { version: u32 },

    #[error(
        "database downgrade is unsupported: recorded schema {recorded}, database schema {database}"
    )]
    UnsupportedDowngrade { recorded: u32, database: u32 },

    #[error("migration {migration_id} is partially applied")]
    PartialMigration { migration_id: String },

    #[error("invalid WAL policy: {0}")]
    InvalidWalPolicy(String),

    #[error(
        "WAL checkpoint refused with {frames_remaining} uncheckpointed frame(s) and {wal_bytes} bytes; limit is {max_wal_bytes} bytes"
    )]
    WalCheckpointRefused { frames_remaining: u64, wal_bytes: u64, max_wal_bytes: u64 },

    #[error("read snapshot exceeded its {max_ms}ms limit ({elapsed_ms}ms)")]
    LongReader { elapsed_ms: u64, max_ms: u64 },

    #[error("database snapshots are unavailable for an in-memory journal")]
    BackupUnavailable,

    #[error("database snapshot destination already exists")]
    BackupExists,

    #[error("a SQLite WAL or SHM sidecar cannot be used as an independent backup")]
    SidecarBackupRefused,

    #[error("invalid writer configuration: {0}")]
    InvalidWriterConfig(String),

    #[error("writer queue is full for source {event_source}")]
    WriterQueueFull { event_source: crate::model::EventSource },

    #[error("writer queue wait exceeded {max_wait_ms}ms for source {event_source}")]
    WriterQueueWaitTimeout { event_source: crate::model::EventSource, max_wait_ms: u64 },

    #[error("writer batch has {items} item(s), exceeding the {max_items}-item bound")]
    WriterBatchBound { items: usize, max_items: usize },

    #[error("writer request requires {bytes} bytes, exceeding the {max_bytes}-byte bound")]
    WriterMemoryBound { bytes: u64, max_bytes: u64 },

    #[error("writer has stopped")]
    WriterStopped,

    #[error("writer request was cancelled before commit")]
    WriterCancelled,

    #[error("writer acknowledgement wait exceeded {max_wait_ms}ms")]
    WriterAckTimeout { max_wait_ms: u64 },

    #[error("writer exhausted its {attempts} bounded attempt(s)")]
    WriterRetryExhausted { attempts: u32 },

    #[error("writer batch contains multiple event sources")]
    WriterMixedSources,

    #[error("invalid writer diagnostic: {0}")]
    InvalidWriterDiagnostic(String),

    #[error("cursor regressed for source {event_source}")]
    CursorRegression { event_source: crate::model::EventSource },

    #[error("cursor skipped an unmarked range for source {event_source}")]
    CursorSkipped { event_source: crate::model::EventSource },

    #[error("cursor ordering is unknown for source {event_source}; an explicit reset or wrap is required")]
    CursorOrderingUnknown { event_source: crate::model::EventSource },

    #[error("cursor conflict refused for source {event_source}")]
    CursorConflict { event_source: crate::model::EventSource },

    #[error("cursor policy changed without a reset for source {event_source}")]
    CursorPolicyMismatch { event_source: crate::model::EventSource },

    #[error("replay boundary changed without an explicit reset for source {event_source}")]
    CursorBoundaryMismatch { event_source: crate::model::EventSource },

    #[error("cursor is invalidated for source {event_source}")]
    CursorInvalidated { event_source: crate::model::EventSource },

    #[error("cursor control requires an existing state for source {event_source}")]
    CursorStateMissing { event_source: crate::model::EventSource },

    #[error("cursor reset or wrap requires an explicitly typed ordered token")]
    CursorControlInvalid,

    #[error("fault injection fired at {point}")]
    InjectedFault { point: String },

    #[error("invalid fault plan: {0}")]
    InvalidFaultPlan(String),
}

/// Errors from payload encryption and authenticated decryption.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("key provider failed: {0}")]
    KeyProvider(String),

    #[error("ciphertext is truncated")]
    Truncated,

    #[error("ciphertext authentication failed")]
    AuthenticationFailed,

    #[error("ciphertext encoding error: {0}")]
    Encoding(String),

    #[error("operating-system randomness failed")]
    Random,
}

impl From<chacha20poly1305::aead::Error> for CryptoError {
    fn from(_: chacha20poly1305::aead::Error) -> Self {
        Self::AuthenticationFailed
    }
}
