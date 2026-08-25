//! GHOSTRACE fixture-only vertical slice.
//!
//! This package intentionally has no live collector, network, telemetry,
//! screen/audio capture, or keylogging implementation.  The public boundary is
//! fixture ingestion into an encrypted local SQLite journal.

pub mod consent;
pub mod crypto;
pub mod cursor;
pub mod error;
pub mod explain;
pub mod export;
pub mod fault;
pub mod fixture;
pub mod fsevents;
pub mod journal;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod model;
pub mod policy;
pub(crate) mod storage;
pub mod wal;
pub mod writer;

pub use consent::{ConsentReceipt, ConsentState, ConsentStateMachine, ConsentTransitionKind};
pub use crypto::{decrypt_payload, encrypt_payload, DeterministicKeyProvider, KeyProvider};
pub use cursor::{
    CursorIdentity, CursorKind, CursorOrder, CursorState, CursorStatus, CursorToken,
    CursorTransition, CURSOR_CONTRACT_VERSION,
};
pub use error::{CryptoError, GhostraceError};
pub use explain::{explain, CoverageSummary, Explanation, ExplanationStatement};
pub use export::{
    export_fixture, export_journal, ExportManifest, ExportPolicyProfile, EXPORT_VERSION,
};
pub use fault::{FaultAction, FaultPlan, FaultPoint, FaultSchedule};
pub use fixture::{ingest_fixture, read_fixture, FixtureIngestReport};
pub use fsevents::{
    CallbackHealth, FseventsError, FseventsEvent, FseventsOptions, FseventsStream, StreamState,
    DEFAULT_LATENCY, EVENT_ID_SINCE_NOW, FLAG_FILE_EVENTS, FLAG_FULL_HISTORY, FLAG_NO_DEFER,
    FLAG_USE_CF_TYPES, FLAG_USE_EXTENDED_DATA, FLAG_WITH_DOC_ID,
};
pub use journal::{AppliedMigration, BackupReceipt, DiagnosticRecord, Journal, StoredEvent};
#[cfg(target_os = "macos")]
pub use keychain::{MacOsKeychainProvider, JOURNAL_KEYCHAIN_ACCOUNT, JOURNAL_KEYCHAIN_SERVICE};
pub use model::{
    AppChange, ApplicationId, BookmarkChange, BookmarkId, BranchName,
    BrowserBookmarkChangedPayload, BrowserName, BrowserNavigationPayload, BrowserUrl,
    CollectorInstanceId, CollectorLifecyclePayload, Confidence, EntryKind, EventEnvelope, EventId,
    EventKind, EventPayload, EventSource, Evidence, FileOperation, FilesystemChangedPayload,
    FixtureOrigin, FolderId, FrontmostAppChangedPayload, GapPayload, GitObjectId,
    GitSnapshotPayload, ImportOrigin, IngestionOrigin, IngestionOriginKind, InstanceLabel,
    LiveOrigin, OpaqueIdentifier, PathClass, PathDigest, PolicyBlockedSummaryPayload,
    PolicyProfileId, ProvenanceVersion, ReasonCode, RepairOrigin, RepositoryId, RootId,
    SanitizedUrl, SessionId, ShellFinishedPayload, ShellKind, ShellStartedPayload, ShellStatus,
    SnapshotDigest, Source, SourceCursor, SourceErrorPayload, EVENT_SCHEMA_VERSION,
    IMPORT_PROVENANCE_VERSION, LIVE_PROVENANCE_VERSION, MAX_APP_IDENTIFIER_BYTES, MAX_BRANCH_BYTES,
    MAX_BROWSER_URL_BYTES, MAX_CURSOR_BYTES, MAX_EVENT_PAYLOAD_BYTES, MAX_IDENTIFIER_BYTES,
    PROVENANCE_VERSION, REPAIR_PROVENANCE_VERSION, SHA256_DIGEST_BYTES,
};
pub use policy::{
    PolicyChange, PolicyDecision, PolicyDecisionRecord, PolicyDiagnostic, PolicyDocument,
    PolicyHistory, PolicyMigration, PolicyMigrationOutcome, PolicyOutcome, PolicyProfile,
    PolicyReason, POLICY_DOCUMENT_SCHEMA_VERSION,
};
pub use wal::{CheckpointMode, WalCheckpointReport, WalPolicy};
pub use writer::{
    QueueFullPolicy, WriteAck, WriteTicket, Writer, WriterConfig, WriterGap, WriterGapReason,
    WriterOutcome, WriterSubmission, DEFAULT_MAX_BATCH_ITEMS, DEFAULT_MAX_MEMORY_BYTES,
    DEFAULT_MAX_RETRIES, DEFAULT_MAX_WAIT_MS, DEFAULT_QUEUE_ITEMS,
};

pub const EVENT_SCHEMA_JSON: &str = include_str!("../schemas/event-envelope-v1.json");
pub const POLICY_DOCUMENT_SCHEMA_JSON: &str = include_str!("../schemas/policy-document-v1.json");

pub fn capture() -> Result<(), GhostraceError> {
    Err(GhostraceError::LiveCaptureDisabled)
}
