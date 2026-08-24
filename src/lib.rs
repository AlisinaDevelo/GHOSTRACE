//! GHOSTRACE fixture-only vertical slice.
//!
//! This package intentionally has no live collector, network, telemetry,
//! screen/audio capture, or keylogging implementation.  The public boundary is
//! fixture ingestion into an encrypted local SQLite journal.

pub mod crypto;
pub mod error;
pub mod explain;
pub mod export;
pub mod fixture;
pub mod journal;
pub mod model;
pub mod policy;

pub use crypto::{decrypt_payload, encrypt_payload, DeterministicKeyProvider, KeyProvider};
pub use error::{CryptoError, GhostraceError};
pub use explain::{explain, CoverageSummary, Explanation, ExplanationStatement};
pub use export::{
    export_fixture, export_journal, ExportManifest, ExportPolicyProfile, EXPORT_VERSION,
};
pub use fixture::{ingest_fixture, read_fixture, FixtureIngestReport};
pub use journal::{Journal, StoredEvent};
pub use model::{
    AppChange, BookmarkChange, BrowserBookmarkChangedPayload, BrowserNavigationPayload, BrowserUrl,
    CollectorLifecyclePayload, Confidence, EntryKind, EventEnvelope, EventId, EventKind,
    EventPayload, EventSource, Evidence, FileOperation, FilesystemChangedPayload, FixtureOrigin,
    FrontmostAppChangedPayload, GapPayload, GitSnapshotPayload, ImportOrigin, IngestionOrigin,
    IngestionOriginKind, LiveOrigin, PathClass, PolicyBlockedSummaryPayload, RepairOrigin,
    SanitizedUrl, ShellFinishedPayload, ShellStartedPayload, ShellStatus, Source,
    SourceErrorPayload, EVENT_SCHEMA_VERSION, IMPORT_PROVENANCE_VERSION, LIVE_PROVENANCE_VERSION,
    MAX_APP_IDENTIFIER_BYTES, MAX_BRANCH_BYTES, MAX_BROWSER_URL_BYTES, MAX_CURSOR_BYTES,
    MAX_EVENT_PAYLOAD_BYTES, MAX_IDENTIFIER_BYTES, PROVENANCE_VERSION, REPAIR_PROVENANCE_VERSION,
    SHA256_DIGEST_BYTES,
};
pub use policy::{PolicyDecision, PolicyProfile, PolicyReason};

pub const EVENT_SCHEMA_JSON: &str = include_str!("../schemas/event-envelope-v1.json");

pub fn capture() -> Result<(), GhostraceError> {
    Err(GhostraceError::LiveCaptureDisabled)
}
