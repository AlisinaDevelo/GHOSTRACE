//! GHOSTRACE fixture-only vertical slice.
//!
//! This package intentionally has no live collector, network, telemetry,
//! screen/audio capture, or keylogging implementation.  The public boundary is
//! fixture ingestion into an encrypted local SQLite journal.

pub mod claims;
pub mod consent;
pub mod correlation;
pub mod crypto;
pub mod cursor;
pub mod error;
pub mod exclusion;
pub mod explain;
pub mod export;
pub mod export_schema;
pub mod fault;
pub mod fixture;
pub mod fsevents;
pub mod fsevents_collector;
pub mod fsevents_flags;
pub mod integrity;
pub mod journal;
pub mod key_lifecycle;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod model;
pub mod ordering;
pub mod policy;
pub mod query;
pub mod residue;
pub mod retention;
pub(crate) mod storage;
pub mod volume;
pub mod wal;
pub mod writer;

pub use claims::{
    render_claim, ClaimLocale, ClaimTemplateDescriptor, ClaimTemplateId, EvidenceRequirement,
    GapBehavior, ProhibitedImplication, RenderedClaim, RequiredFact, CLAIM_GRAMMAR_VERSION,
};
pub use consent::{
    ConsentConfirmation, ConsentPreview, ConsentReceipt, ConsentState, ConsentStateMachine,
    ConsentTransitionKind, MAX_CONSENT_PREVIEW_ITEMS,
};
pub use correlation::{
    evaluate as evaluate_correlation, explanation_identity, explanation_identity_for_rule_version,
    rule_descriptors, CorrelationEvidenceOutput, CorrelationExclusion, CorrelationFixtureClass,
    CorrelationIdentity, CorrelationInputField, CorrelationQuery, CorrelationReason,
    CorrelationResult, CorrelationRuleBounds, CorrelationRuleDescriptor, CorrelationRuleId,
    CORRELATION_RULE_REGISTRY_VERSION, CORRELATION_RULE_SCHEMA_VERSION,
    CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION, MAX_CORRELATION_INPUT_EVENTS,
    MAX_CORRELATION_WINDOW_SECONDS,
};
pub use crypto::{
    decrypt_payload, encrypt_payload, CiphertextEnvelope, DeterministicKeyProvider, KeyAlgorithm,
    KeyMetadata, KeyProvider, CIPHERTEXT_ENVELOPE_VERSION, MAX_CIPHERTEXT_BYTES,
};
pub use cursor::{
    CursorIdentity, CursorKind, CursorOrder, CursorState, CursorStatus, CursorStreamMode,
    CursorToken, CursorTransition, ReplayBoundary, ReplayConfiguration, CURSOR_CONTRACT_VERSION,
    REPLAY_BOUNDARY_CONTRACT_VERSION,
};
pub use error::{CryptoError, GhostraceError};
pub use exclusion::{
    ExclusionAction, ExclusionDecision, ExclusionKind, ExclusionPolicy, ExclusionPolicyHistory,
    ExclusionReason, ExclusionRule, ExclusionSubject, EXCLUSION_POLICY_SCHEMA_VERSION,
    MAX_EXCLUSION_PATTERN_BYTES, MAX_EXCLUSION_POLICY_VERSIONS, MAX_EXCLUSION_RULES,
    MAX_EXCLUSION_SUBJECT_BYTES,
};
pub use explain::{explain, CoverageSummary, Explanation, ExplanationStatement};
pub use export::{
    export_confirmed, export_fixture, export_journal, export_journal_with_confirmation,
    export_journal_with_options, preview_export, ExportCancellation, ExportConfirmation,
    ExportDestinationClass, ExportField, ExportManifest, ExportOptions, ExportPlan,
    ExportPolicyProfile, ExportPreview, ExportQuery, ExportReceipt, ExportRedactionPlan,
    ExportRequest, ExportResult, EXPORT_PLAINTEXT_WARNING, EXPORT_PREVIEW_SCHEMA_VERSION,
    EXPORT_VERSION, MAX_EXPORT_EVENT_RECORDS, MAX_EXPORT_GAPS, MAX_EXPORT_POLICY_PROFILES,
    MAX_EXPORT_RECORD_BYTES,
};
pub use export_schema::{
    validate_export, validate_registry, ExportQueryScope, ExportValidation, SchemaDescriptor,
    SchemaRegistry, EXPORT_CLAIM_SCHEMA_ID, EXPORT_EVENT_SCHEMA_ID, EXPORT_GAP_SCHEMA_ID,
    EXPORT_MANIFEST_SCHEMA_ID, EXPORT_POLICY_SCHEMA_ID, EXPORT_REGISTRY_VERSION,
    EXPORT_SCHEMA_REGISTRY_JSON, EXPORT_SOURCE_COVERAGE_SCHEMA_ID,
};
pub use fault::{FaultAction, FaultPlan, FaultPoint, FaultSchedule};
pub use fixture::{ingest_fixture, read_fixture, FixtureIngestReport};
pub use fsevents::{
    CallbackHealth, FseventsError, FseventsEvent, FseventsOptions, FseventsStream,
    HistoryCursorRange, StartupCursor, StartupCursorDecision, StartupCursorError,
    StartupCursorRejection, StreamState, DEFAULT_LATENCY, EVENT_ID_SINCE_NOW, FLAG_FILE_EVENTS,
    FLAG_FULL_HISTORY, FLAG_NO_DEFER, FLAG_USE_CF_TYPES, FLAG_USE_EXTENDED_DATA, FLAG_WATCH_ROOT,
    FLAG_WITH_DOC_ID,
};
pub use fsevents_collector::{
    CollectedFilesystemEvent, CollectorCoverageState, CollectorState, CollectorStatus,
    ContainedFile, FseventsCollector, FseventsCollectorConfig, FseventsCollectorError,
    InternalPathPolicy, SelectedRoot, DEFAULT_HISTORY_TIMEOUT, MAX_INTERNAL_PATHS,
    MAX_INTERNAL_PATH_BYTES, MAX_PENDING_EVENTS, MAX_SELECTED_ROOTS, MAX_TRANSPORT_DEDUP_ENTRIES,
    MAX_TRANSPORT_DEDUP_EVENT_ID_SPAN,
};
pub use fsevents_flags::{
    normalize_fsevents_event, FseventsBoundaryReason, FseventsCompleteness,
    FseventsContradictionReason, FseventsEventFlag, FseventsEvidenceStatus, FseventsFlagSet,
    FseventsRescanReason, NormalizedFseventsEvent, DOCUMENTED_EVENT_FLAGS,
    DOCUMENTED_EVENT_FLAG_MASK, EVENT_FLAG_EVENT_IDS_WRAPPED, EVENT_FLAG_HISTORY_DONE,
    EVENT_FLAG_ITEM_CHANGE_OWNER, EVENT_FLAG_ITEM_CLONED, EVENT_FLAG_ITEM_CREATED,
    EVENT_FLAG_ITEM_FINDER_INFO_MOD, EVENT_FLAG_ITEM_INODE_META_MOD, EVENT_FLAG_ITEM_IS_DIR,
    EVENT_FLAG_ITEM_IS_FILE, EVENT_FLAG_ITEM_IS_HARDLINK, EVENT_FLAG_ITEM_IS_LAST_HARDLINK,
    EVENT_FLAG_ITEM_IS_SYMLINK, EVENT_FLAG_ITEM_MODIFIED, EVENT_FLAG_ITEM_REMOVED,
    EVENT_FLAG_ITEM_RENAMED, EVENT_FLAG_ITEM_XATTR_MOD, EVENT_FLAG_KERNEL_DROPPED,
    EVENT_FLAG_MOUNT, EVENT_FLAG_MUST_SCAN_SUB_DIRS, EVENT_FLAG_NONE, EVENT_FLAG_OWN_EVENT,
    EVENT_FLAG_ROOT_CHANGED, EVENT_FLAG_UNMOUNT, EVENT_FLAG_USER_DROPPED,
    FSEVENTS_NORMALIZED_SCHEMA_VERSION,
};
pub use integrity::{
    IntegrityForeignKeyViolation, IntegrityReport, INTEGRITY_REPORT_SCHEMA_VERSION,
};
pub use journal::{
    AppliedMigration, BackupReceipt, DiagnosticRecord, Journal, RetentionDeletionReceipt,
    StoredEvent,
};
pub use key_lifecycle::{
    DestructionConfirmation, DestructionReason, DestructionScope, KeyDestructionReceipt,
    KeyLifecycleError, KeyRing, KeyRotation, RotationCheckpoint, RotationPhase,
    KEY_LIFECYCLE_SCHEMA_VERSION, MAX_KEY_GENERATIONS,
};
#[cfg(target_os = "macos")]
pub use keychain::{MacOsKeychainProvider, JOURNAL_KEYCHAIN_ACCOUNT, JOURNAL_KEYCHAIN_SERVICE};
pub use model::{
    AppChange, ApplicationId, BookmarkChange, BookmarkId, BranchName,
    BrowserBookmarkChangedPayload, BrowserName, BrowserNavigationPayload, BrowserUrl,
    CollectorInstanceId, CollectorLifecyclePayload, Confidence, EntryKind, EventEnvelope, EventId,
    EventKind, EventPayload, EventSource, Evidence, FileOperation, FilesystemChangedPayload,
    FilesystemObservation, FixtureOrigin, FolderId, FrontmostAppChangedPayload, GapPayload,
    GapRemediation, GitObjectId, GitSnapshotPayload, ImportOrigin, IngestionOrigin,
    IngestionOriginKind, InstanceLabel, LiveOrigin, OpaqueIdentifier, PathClass, PathDigest,
    PolicyBlockedSummaryPayload, PolicyProfileId, ProvenanceVersion, ReasonCode, RenamePairing,
    RepairOrigin, RepositoryId, RootId, SanitizedUrl, SessionId, ShellFinishedPayload, ShellKind,
    ShellStartedPayload, ShellStatus, SnapshotDigest, Source, SourceCursor, SourceErrorPayload,
    EVENT_SCHEMA_VERSION, IMPORT_PROVENANCE_VERSION, LIVE_PROVENANCE_VERSION,
    MAX_APP_IDENTIFIER_BYTES, MAX_BRANCH_BYTES, MAX_BROWSER_URL_BYTES, MAX_CURSOR_BYTES,
    MAX_EVENT_PAYLOAD_BYTES, MAX_IDENTIFIER_BYTES, PROVENANCE_VERSION, REPAIR_PROVENANCE_VERSION,
    SHA256_DIGEST_BYTES,
};
pub use ordering::{
    analyze_temporal_observations, compare_event_order, StableOrderKey, TemporalAnalysis,
    TemporalEvidenceBasis, TemporalObservation, TemporalOrderDecision, ORDERING_CONTRACT_VERSION,
    TEMPORAL_DELAY_THRESHOLD_SECONDS, TEMPORAL_OBSERVATION_SCHEMA_VERSION,
};
pub use policy::{
    PolicyChange, PolicyDecision, PolicyDecisionRecord, PolicyDiagnostic, PolicyDocument,
    PolicyHistory, PolicyMigration, PolicyMigrationOutcome, PolicyOutcome, PolicyProfile,
    PolicyReason, POLICY_DOCUMENT_SCHEMA_VERSION,
};
pub use query::{
    CoverageGap, CoverageInterval, CoverageStatus, CoverageStatusKind, QueryCoverage, QueryPage,
    QueryRequest, COVERAGE_CONTRACT_VERSION, DEFAULT_QUERY_PAGE_SIZE, MAX_COVERAGE_MARKERS,
    MAX_QUERY_PAGE_SIZE, QUERY_CONTRACT_VERSION, QUERY_TOKEN_TTL_SECONDS,
};
pub use residue::{
    DeletionMode, DeletionModeDescription, ResidueArtifactKind, ResidueArtifactSummary,
    ResidueReport, RESIDUE_REPORT_SCHEMA_VERSION,
};
pub use retention::{
    RetentionConfirmation, RetentionGapSummary, RetentionPlan, RetentionPolicy,
    RetentionSelectionReason, DEFAULT_RETENTION_DAYS, LEGACY_KEY_GENERATION,
    MAX_RETENTION_GAP_SUMMARIES, RETENTION_DELETION_SCHEMA_VERSION, RETENTION_PLAN_SCHEMA_VERSION,
};
pub use volume::{
    MountState, VolumeIdentity, VolumeIdentityError, VolumeObservation, VolumeTransition,
    VOLUME_IDENTITY_CONTRACT_VERSION,
};
pub use wal::{CheckpointMode, WalCheckpointReport, WalPolicy};
pub use writer::{
    KeyUnavailablePolicy, QueueFullPolicy, WriteAck, WriteTicket, Writer, WriterConfig, WriterGap,
    WriterGapReason, WriterOutcome, WriterSubmission, DEFAULT_MAX_BATCH_ITEMS,
    DEFAULT_MAX_MEMORY_BYTES, DEFAULT_MAX_RETRIES, DEFAULT_MAX_WAIT_MS, DEFAULT_QUEUE_ITEMS,
};

pub const EVENT_SCHEMA_JSON: &str = include_str!("../schemas/event-envelope-v1.json");
pub const POLICY_DOCUMENT_SCHEMA_JSON: &str = include_str!("../schemas/policy-document-v1.json");
pub const EXCLUSION_POLICY_SCHEMA_JSON: &str = include_str!("../schemas/exclusion-policy-v1.json");
pub const FSEVENTS_NORMALIZED_SCHEMA_JSON: &str =
    include_str!("../schemas/fsevents-normalized-v1.json");
pub const FSEVENTS_STARTUP_SCHEMA_JSON: &str = include_str!("../schemas/fsevents-startup-v1.json");
pub const KEY_LIFECYCLE_SCHEMA_JSON: &str = include_str!("../schemas/key-lifecycle-v1.json");

pub fn capture() -> Result<(), GhostraceError> {
    Err(GhostraceError::LiveCaptureDisabled)
}
