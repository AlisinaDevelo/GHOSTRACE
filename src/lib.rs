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
    EventPayload, EventSource, Evidence, FileOperation, FilesystemChangedPayload,
    FrontmostAppChangedPayload, GapPayload, GitSnapshotPayload, PathClass,
    PolicyBlockedSummaryPayload, SanitizedUrl, ShellFinishedPayload, ShellStartedPayload,
    ShellStatus, Source, SourceErrorPayload, EVENT_SCHEMA_VERSION, PROVENANCE_VERSION,
};
pub use policy::{PolicyDecision, PolicyProfile, PolicyReason};

pub const EVENT_SCHEMA_JSON: &str = include_str!("../schemas/event-envelope-v1.json");

pub fn capture() -> Result<(), GhostraceError> {
    Err(GhostraceError::LiveCaptureDisabled)
}
