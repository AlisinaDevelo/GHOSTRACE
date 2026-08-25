//! Explicitly enabled, selected-root FSEvents collection.
//!
//! This module is the first source adapter above the native stream boundary.
//! It requires a consumed [`ConsentConfirmation`], a versioned policy that
//! enables both filesystem and lifecycle events, and an exact mapping from
//! opaque root IDs to canonical paths. Paths are used only in memory for the
//! containment check and are represented by a SHA-256 digest in the journal.
//! The adapter never opens a reported path or reads its contents.
//!
//! Canonicalization here is a startup-time prerequisite, not the complete path
//! policy. Symlink replacement, hard-link aliasing, open races, and exclusion
//! precedence remain the dedicated later gates in task 0014 and its children.

use std::{
    collections::{BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    consent::{ConsentConfirmation, ConsentReceipt, ConsentState, ConsentStateMachine},
    error::GhostraceError,
    fsevents::{
        CallbackHealth, FseventsError, FseventsEvent, FseventsOptions, FseventsStream, StreamState,
    },
    fsevents_flags::{FseventsEventFlag, FseventsEvidenceStatus, NormalizedFseventsEvent},
    journal::{DiagnosticRecord, Journal},
    model::{
        CollectorLifecyclePayload, EntryKind, EventEnvelope, EventKind, EventPayload, EventSource,
        Evidence, FileOperation, FilesystemChangedPayload, IngestionOrigin, InstanceLabel,
        PathClass, PathDigest, PolicyBlockedSummaryPayload, ReasonCode, RootId,
    },
    policy::{PolicyDocument, PolicyProfile},
    writer::{Writer, WriterConfig, WriterOutcome},
};

/// Upper bound on the number of selected roots in one collector instance.
pub const MAX_SELECTED_ROOTS: usize = 64;
/// Upper bound on copied callback events waiting for the owner thread.
pub const MAX_PENDING_EVENTS: usize = 4 * 1024;

/// A canonical path bound to one opaque policy root identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedRoot {
    id: RootId,
    canonical_path: PathBuf,
}

impl SelectedRoot {
    /// Canonicalize an existing directory before it is handed to FSEvents.
    /// The path is never exposed in a public diagnostic or event payload.
    pub fn new(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<Self, FseventsCollectorError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "root path must be absolute",
            });
        }
        let canonical_path = std::fs::canonicalize(&path).map_err(|_| {
            FseventsCollectorError::InvalidRoot { reason: "root path cannot be canonicalized" }
        })?;
        if !canonical_path.is_dir() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "selected root is not a directory",
            });
        }
        Self::from_canonical_path(id, canonical_path)
    }

    /// Construct a root from a path already canonicalized by a stronger
    /// platform policy. This is useful for a future race-resistant resolver.
    pub fn from_canonical_path(
        id: impl Into<String>,
        canonical_path: impl Into<PathBuf>,
    ) -> Result<Self, FseventsCollectorError> {
        let canonical_path = canonical_path.into();
        if !canonical_path.is_absolute() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "root path must be absolute",
            });
        }
        if !canonical_path.is_dir() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "selected root is not a directory",
            });
        }
        Ok(Self { id: RootId::try_from(id.into())?, canonical_path })
    }

    pub fn id(&self) -> &RootId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.canonical_path
    }
}

/// Runtime configuration for an explicitly enabled collector.
#[derive(Clone, Debug)]
pub struct FseventsCollectorConfig {
    pub options: FseventsOptions,
    pub writer: WriterConfig,
    pub collector_instance: String,
    pub instance_label: String,
    pub consent_at: DateTime<Utc>,
    pub actor: String,
    pub reason: String,
}

/// Lifecycle state exposed by the selected-root adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorState {
    Created,
    Running,
    Stopped,
    Revoked,
    Failed,
}

/// A path-free, normalized filesystem event returned after durable admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CollectedFilesystemEvent {
    pub root_id: RootId,
    pub path_class: PathClass,
    pub operation: FileOperation,
    pub entry_kind: EntryKind,
    pub path_digest: PathDigest,
    pub source_event_id: u64,
    pub evidence: Evidence,
    pub normalized: NormalizedFseventsEvent,
}

/// Bounded, inspectable collector counters. No path or callback payload is
/// retained here.
#[derive(Clone, Debug)]
pub struct CollectorStatus {
    pub state: CollectorState,
    pub stream_state: StreamState,
    pub consent_state: ConsentState,
    pub accepted_events: u64,
    pub blocked_events: u64,
    pub dropped_events: u64,
    pub coverage_boundaries: u64,
    pub rescan_required: u64,
    pub unsupported_events: u64,
    pub contradictory_events: u64,
    pub callback_health: CallbackHealth,
}

#[derive(Debug, Error)]
pub enum FseventsCollectorError {
    #[error("invalid selected root: {reason}")]
    InvalidRoot { reason: &'static str },

    #[error("the collector requires at least one selected root")]
    EmptyRoots,

    #[error("selected root count exceeds the bound of {MAX_SELECTED_ROOTS}")]
    TooManyRoots,

    #[error("selected root IDs must exactly match the policy scope")]
    RootScopeMismatch,

    #[error("the policy must enable both filesystem and lifecycle events")]
    MissingPolicySource,

    #[error("the consent confirmation does not match the policy document")]
    ConsentPolicyMismatch,

    #[error("collector cannot {action} in its current state")]
    InvalidState { action: &'static str },

    #[error("the lifecycle event could not be durably admitted")]
    LifecycleAdmissionGap,

    #[error(transparent)]
    Stream(#[from] FseventsError),

    #[error(transparent)]
    Core(#[from] GhostraceError),
}

#[derive(Default)]
struct PendingState {
    accepting: bool,
    events: VecDeque<FseventsEvent>,
    overflowed: u64,
}

/// The selected-root source adapter. It is intentionally owner-thread-bound
/// through the contained [`FseventsStream`].
pub struct FseventsCollector {
    stream: FseventsStream,
    pending: Arc<Mutex<PendingState>>,
    roots: Vec<SelectedRoot>,
    seen_path_digests: BTreeSet<PathDigest>,
    policy: PolicyProfile,
    consent: ConsentStateMachine,
    consent_receipt: ConsentReceipt,
    writer: Writer,
    journal: Journal,
    origin: IngestionOrigin,
    instance_label: InstanceLabel,
    state: CollectorState,
    accepted_events: u64,
    blocked_events: u64,
    dropped_events: u64,
    coverage_boundaries: u64,
    rescan_required: u64,
    unsupported_events: u64,
    contradictory_events: u64,
}

impl FseventsCollector {
    /// Build a collector only after the caller has rendered and consumed a
    /// consent preview. Construction does not schedule or start observation.
    pub fn new<I>(
        confirmation: ConsentConfirmation,
        document: PolicyDocument,
        roots: I,
        journal: Journal,
        config: FseventsCollectorConfig,
    ) -> Result<Self, FseventsCollectorError>
    where
        I: IntoIterator<Item = SelectedRoot>,
    {
        let policy = PolicyProfile::from_document(&document)?;
        if !policy.is_source_enabled(EventSource::Filesystem)
            || !policy.is_source_enabled(EventSource::Lifecycle)
        {
            return Err(FseventsCollectorError::MissingPolicySource);
        }

        let roots = roots.into_iter().collect::<Vec<_>>();
        validate_roots(&roots, &policy)?;

        let mut consent = ConsentStateMachine::new();
        let consent_receipt = consent.grant_preview(
            confirmation,
            config.consent_at,
            &config.actor,
            &config.reason,
        )?;
        let expected_digest = document.scope_digest()?;
        if consent_receipt.policy_id.as_str() != document.id
            || consent_receipt.policy_version != document.version
            || consent_receipt.scope_digest != expected_digest
        {
            return Err(FseventsCollectorError::ConsentPolicyMismatch);
        }

        let origin = IngestionOrigin::live(config.collector_instance)?;
        let instance_label = InstanceLabel::try_from(config.instance_label)?;
        let pending = Arc::new(Mutex::new(PendingState::default()));
        let callback_pending = Arc::clone(&pending);
        let paths = roots.iter().map(|root| root.path().to_path_buf()).collect::<Vec<_>>();
        let stream = FseventsStream::new(paths, config.options, move |batch| {
            let mut pending =
                callback_pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if !pending.accepting {
                pending.overflowed = pending.overflowed.saturating_add(batch.len() as u64);
                return;
            }
            for event in batch {
                if pending.events.len() >= MAX_PENDING_EVENTS {
                    pending.overflowed = pending.overflowed.saturating_add(1);
                } else {
                    pending.events.push_back(event.clone());
                }
            }
        })?;
        let writer = Writer::new(journal.clone(), config.writer)?;

        Ok(Self {
            stream,
            pending,
            roots,
            seen_path_digests: BTreeSet::new(),
            policy,
            consent,
            consent_receipt,
            writer,
            journal,
            origin,
            instance_label,
            state: CollectorState::Created,
            accepted_events: 0,
            blocked_events: 0,
            dropped_events: 0,
            coverage_boundaries: 0,
            rescan_required: 0,
            unsupported_events: 0,
            contradictory_events: 0,
        })
    }

    pub fn status(&self) -> CollectorStatus {
        CollectorStatus {
            state: self.state,
            stream_state: self.stream.state(),
            consent_state: self.consent.state(),
            accepted_events: self.accepted_events,
            blocked_events: self.blocked_events,
            dropped_events: self.dropped_events,
            coverage_boundaries: self.coverage_boundaries,
            rescan_required: self.rescan_required,
            unsupported_events: self.unsupported_events,
            contradictory_events: self.contradictory_events,
            callback_health: self.stream.callback_health(),
        }
    }

    pub fn consent_receipt(&self) -> &ConsentReceipt {
        &self.consent_receipt
    }

    pub fn journal(&self) -> Journal {
        self.journal.clone()
    }

    /// Schedule and start observation. This is the only method that enables
    /// the native stream, and it remains owner-thread-bound by the stream.
    pub fn start(&mut self) -> Result<(), FseventsCollectorError> {
        if !matches!(self.state, CollectorState::Created | CollectorState::Stopped)
            || self.consent.state().is_terminal()
        {
            return Err(FseventsCollectorError::InvalidState { action: "start" });
        }
        if self.state == CollectorState::Created {
            self.stream.schedule_on_current_run_loop()?;
        }
        self.submit_lifecycle(EventKind::CollectorStarted)?;
        self.set_accepting(true);
        if let Err(error) = self.stream.start() {
            self.set_accepting(false);
            self.state = CollectorState::Failed;
            return Err(error.into());
        }
        self.state = CollectorState::Running;
        Ok(())
    }

    /// Flush native callbacks, stop the stream, and durably record the stop
    /// transition. Pending callbacks observed before stop are drained first.
    pub fn stop(&mut self) -> Result<(), FseventsCollectorError> {
        if self.state != CollectorState::Running {
            return Err(FseventsCollectorError::InvalidState { action: "stop" });
        }
        self.stream.flush()?;
        self.stream.stop()?;
        self.set_accepting(false);
        let _ = self.drain_pending()?;
        self.submit_lifecycle(EventKind::CollectorStopped)?;
        self.state = CollectorState::Stopped;
        Ok(())
    }

    /// Revoke consent before stopping native observation. Pending callbacks are
    /// discarded after the synchronous terminal transition and cannot commit.
    pub fn revoke(
        &mut self,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<(), FseventsCollectorError> {
        if self.consent.state().is_terminal() {
            return Err(FseventsCollectorError::InvalidState { action: "revoke" });
        }
        self.consent.revoke(occurred_at, actor, reason)?;
        self.set_accepting(false);
        if self.state == CollectorState::Running {
            self.stream.stop()?;
            self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).events.clear();
            self.submit_lifecycle(EventKind::CollectorStopped)?;
        }
        self.state = CollectorState::Revoked;
        Ok(())
    }

    /// Drive the owner run loop for one bounded interval and durably admit the
    /// resulting metadata events. Available on macOS, where FSEvents exists.
    #[cfg(target_os = "macos")]
    pub fn run_current_run_loop_for(
        &mut self,
        duration: Duration,
    ) -> Result<Vec<CollectedFilesystemEvent>, FseventsCollectorError> {
        if self.state != CollectorState::Running {
            return Err(FseventsCollectorError::InvalidState { action: "drive the run loop" });
        }
        self.stream.run_current_run_loop_for(duration)?;
        self.drain_pending()
    }

    /// Flush currently delivered callbacks without waiting for another run-loop
    /// interval. This remains an explicit owner-thread operation.
    #[cfg(target_os = "macos")]
    pub fn flush(&mut self) -> Result<Vec<CollectedFilesystemEvent>, FseventsCollectorError> {
        if self.state != CollectorState::Running {
            return Err(FseventsCollectorError::InvalidState { action: "flush" });
        }
        self.stream.flush()?;
        self.drain_pending()
    }

    fn drain_pending(&mut self) -> Result<Vec<CollectedFilesystemEvent>, FseventsCollectorError> {
        let (events, overflowed) = {
            let mut pending = self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            (pending.events.drain(..).collect::<Vec<_>>(), std::mem::take(&mut pending.overflowed))
        };
        if self.state == CollectorState::Revoked {
            self.dropped_events = self.dropped_events.saturating_add(events.len() as u64);
            return Ok(Vec::new());
        }
        if overflowed > 0 {
            self.dropped_events = self.dropped_events.saturating_add(overflowed);
            self.submit_gap(overflowed)?;
        }

        let mut committed = Vec::new();
        for event in events {
            let normalized = event.normalize_flags();
            self.record_status(&normalized.status);
            let Some(root_id) = self.root_for_path(&event.path).map(|root| root.id.clone()) else {
                self.blocked_events = self.blocked_events.saturating_add(1);
                continue;
            };
            let decision =
                self.policy.decide(EventSource::Filesystem, Some(root_id.as_str()), false);
            if !decision.is_allowed() {
                self.blocked_events = self.blocked_events.saturating_add(1);
                continue;
            }
            let path_digest = digest_path(&event.path)?;
            let Some(mut operation) = operation_for(&normalized) else {
                continue;
            };
            // Some macOS releases report a metadata write with the creation
            // bit still set. Once the same path digest has been observed, the
            // repeated create bit is a bounded modify rather than a second
            // creation. Rename/remove precedence remains explicit below.
            if operation == FileOperation::Created && self.seen_path_digests.contains(&path_digest)
            {
                operation = FileOperation::Modified;
            }
            let collected = CollectedFilesystemEvent {
                root_id,
                path_class: PathClass::AbsoluteRedacted,
                operation,
                entry_kind: entry_kind_for(&normalized),
                path_digest,
                source_event_id: event.event_id,
                evidence: if normalized.is_complete() {
                    Evidence::Direct
                } else {
                    Evidence::Unknown
                },
                normalized: normalized.clone(),
            };
            let payload = EventPayload::FilesystemChanged(FilesystemChangedPayload {
                root_id: collected.root_id.clone(),
                path_class: collected.path_class,
                operation: collected.operation,
                entry_kind: collected.entry_kind,
                path_digest: Some(collected.path_digest.clone()),
                size_bytes: None,
            });
            let event = EventEnvelope::new(
                &self.origin,
                Uuid::new_v4(),
                Utc::now(),
                Utc::now(),
                EventSource::Filesystem,
                EventKind::FilesystemChanged,
                payload,
                None,
                self.policy.id.clone(),
                self.policy.version,
                collected.evidence,
                None,
            )?;
            let diagnostics = if collected.evidence == Evidence::Direct {
                Vec::new()
            } else {
                vec![DiagnosticRecord::new("fsevents.status", normalized.status_code())?]
            };
            match self.writer.submit(
                self.origin.clone(),
                vec![event],
                self.policy.clone(),
                diagnostics,
            )? {
                WriterOutcome::Committed(_) => {
                    self.accepted_events = self.accepted_events.saturating_add(1);
                    if collected.operation != FileOperation::Deleted {
                        self.seen_path_digests.insert(collected.path_digest.clone());
                    }
                    committed.push(collected);
                }
                WriterOutcome::Gap(gap) => {
                    self.dropped_events =
                        self.dropped_events.saturating_add(gap.event_count as u64);
                }
            }
        }
        self.submit_blocked_summary()?;
        Ok(committed)
    }

    fn submit_lifecycle(&mut self, kind: EventKind) -> Result<(), FseventsCollectorError> {
        let payload = EventPayload::CollectorStarted(CollectorLifecyclePayload {
            collector: EventSource::Filesystem,
            instance_label: self.instance_label.clone(),
        });
        let payload = match kind {
            EventKind::CollectorStarted => payload,
            EventKind::CollectorStopped => {
                EventPayload::CollectorStopped(CollectorLifecyclePayload {
                    collector: EventSource::Filesystem,
                    instance_label: self.instance_label.clone(),
                })
            }
            _ => return Err(FseventsCollectorError::InvalidState { action: "record lifecycle" }),
        };
        let event = EventEnvelope::new(
            &self.origin,
            Uuid::new_v4(),
            Utc::now(),
            Utc::now(),
            EventSource::Lifecycle,
            kind,
            payload,
            None,
            self.policy.id.clone(),
            self.policy.version,
            Evidence::Direct,
            None,
        )?;
        match self.writer.submit(
            self.origin.clone(),
            vec![event],
            self.policy.clone(),
            Vec::new(),
        )? {
            WriterOutcome::Committed(_) => Ok(()),
            WriterOutcome::Gap(_) => Err(FseventsCollectorError::LifecycleAdmissionGap),
        }
    }

    fn submit_gap(&mut self, dropped_count: u64) -> Result<(), FseventsCollectorError> {
        let event = EventEnvelope::new(
            &self.origin,
            Uuid::new_v4(),
            Utc::now(),
            Utc::now(),
            EventSource::Filesystem,
            EventKind::Gap,
            EventPayload::Gap(crate::model::GapPayload {
                source: EventSource::Filesystem,
                reason_code: ReasonCode::try_from("callback_queue_overflow")?,
                dropped_count,
                from_cursor: None,
                to_cursor: None,
            }),
            None,
            self.policy.id.clone(),
            self.policy.version,
            Evidence::Unknown,
            None,
        )?;
        match self.writer.submit(
            self.origin.clone(),
            vec![event],
            self.policy.clone(),
            Vec::new(),
        )? {
            WriterOutcome::Committed(_) => Ok(()),
            WriterOutcome::Gap(gap) => {
                self.dropped_events = self.dropped_events.saturating_add(gap.event_count as u64);
                Ok(())
            }
        }
    }

    fn submit_blocked_summary(&mut self) -> Result<(), FseventsCollectorError> {
        if self.blocked_events == 0 {
            return Ok(());
        }
        let blocked_count = self.blocked_events;
        self.blocked_events = 0;
        let event = EventEnvelope::new(
            &self.origin,
            Uuid::new_v4(),
            Utc::now(),
            Utc::now(),
            EventSource::Filesystem,
            EventKind::PolicyBlockedSummary,
            EventPayload::PolicyBlockedSummary(PolicyBlockedSummaryPayload {
                source: EventSource::Filesystem,
                reason_code: ReasonCode::try_from("outside_selected_scope")?,
                count: blocked_count,
            }),
            None,
            self.policy.id.clone(),
            self.policy.version,
            Evidence::Unknown,
            None,
        )?;
        match self.writer.submit(
            self.origin.clone(),
            vec![event],
            self.policy.clone(),
            Vec::new(),
        )? {
            WriterOutcome::Committed(_) => Ok(()),
            WriterOutcome::Gap(gap) => {
                self.dropped_events = self.dropped_events.saturating_add(gap.event_count as u64);
                Ok(())
            }
        }
    }

    fn root_for_path(&self, path: &Path) -> Option<&SelectedRoot> {
        self.roots
            .iter()
            .filter(|root| path.starts_with(root.path()))
            .max_by_key(|root| root.path().components().count())
    }

    fn record_status(&mut self, status: &FseventsEvidenceStatus) {
        match status {
            FseventsEvidenceStatus::Boundary { .. } => {
                self.coverage_boundaries = self.coverage_boundaries.saturating_add(1)
            }
            FseventsEvidenceStatus::RescanRequired { .. } => {
                self.rescan_required = self.rescan_required.saturating_add(1)
            }
            FseventsEvidenceStatus::Unsupported { .. } => {
                self.unsupported_events = self.unsupported_events.saturating_add(1)
            }
            FseventsEvidenceStatus::Contradictory { .. } => {
                self.contradictory_events = self.contradictory_events.saturating_add(1)
            }
            FseventsEvidenceStatus::Observed => {}
        }
    }

    fn set_accepting(&self, accepting: bool) {
        self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).accepting = accepting;
    }
}

impl Drop for FseventsCollector {
    fn drop(&mut self) {
        self.set_accepting(false);
    }
}

fn validate_roots(
    roots: &[SelectedRoot],
    policy: &PolicyProfile,
) -> Result<(), FseventsCollectorError> {
    if roots.is_empty() {
        return Err(FseventsCollectorError::EmptyRoots);
    }
    if roots.len() > MAX_SELECTED_ROOTS {
        return Err(FseventsCollectorError::TooManyRoots);
    }
    let mut ids = BTreeSet::new();
    for root in roots {
        if !ids.insert(root.id.as_str().to_owned()) {
            return Err(FseventsCollectorError::RootScopeMismatch);
        }
    }
    let selected = policy.selected_roots.iter().cloned().collect::<BTreeSet<_>>();
    if ids != selected {
        return Err(FseventsCollectorError::RootScopeMismatch);
    }
    Ok(())
}

fn digest_path(path: &Path) -> Result<PathDigest, FseventsCollectorError> {
    #[cfg(target_os = "macos")]
    let digest = {
        use std::os::unix::ffi::OsStrExt;
        Sha256::digest(path.as_os_str().as_bytes())
    };
    #[cfg(not(target_os = "macos"))]
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    let encoded = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Ok(PathDigest::try_from(format!("sha256:{encoded}"))?)
}

fn operation_for(event: &NormalizedFseventsEvent) -> Option<FileOperation> {
    if event.flags.contains(FseventsEventFlag::ItemRemoved) {
        Some(FileOperation::Deleted)
    } else if event.flags.contains(FseventsEventFlag::ItemRenamed) {
        Some(FileOperation::Renamed)
    } else if event.flags.contains(FseventsEventFlag::ItemCreated) {
        Some(FileOperation::Created)
    } else if event.flags.contains(FseventsEventFlag::ItemModified)
        || event.flags.contains(FseventsEventFlag::ItemInodeMetaMod)
        || event.flags.contains(FseventsEventFlag::ItemFinderInfoMod)
        || event.flags.contains(FseventsEventFlag::ItemChangeOwner)
        || event.flags.contains(FseventsEventFlag::ItemXattrMod)
        || event.flags.contains(FseventsEventFlag::ItemCloned)
    {
        Some(FileOperation::Modified)
    } else {
        None
    }
}

fn entry_kind_for(event: &NormalizedFseventsEvent) -> EntryKind {
    let file = event.flags.contains(FseventsEventFlag::ItemIsFile);
    let directory = event.flags.contains(FseventsEventFlag::ItemIsDir);
    let symlink = event.flags.contains(FseventsEventFlag::ItemIsSymlink);
    if symlink {
        EntryKind::Symlink
    } else if file && !directory {
        EntryKind::File
    } else if directory && !file {
        EntryKind::Directory
    } else {
        EntryKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsevents_flags::{
        EVENT_FLAG_ITEM_CREATED, EVENT_FLAG_ITEM_IS_FILE, EVENT_FLAG_ITEM_MODIFIED,
        EVENT_FLAG_ITEM_REMOVED, EVENT_FLAG_ITEM_RENAMED,
    };

    #[test]
    fn operation_mapping_is_deterministic_and_path_digest_is_not_path_text() {
        let created = crate::fsevents::FseventsEvent {
            path: PathBuf::from("/private/tmp/collector-secret"),
            event_id: 1,
            flags: EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE,
        };
        let normalized = created.normalize_flags();
        assert_eq!(operation_for(&normalized), Some(FileOperation::Created));
        assert_eq!(entry_kind_for(&normalized), EntryKind::File);
        let digest = digest_path(&created.path).expect("digest");
        assert!(!digest.as_str().contains("collector-secret"));

        for (flags, expected) in [
            (EVENT_FLAG_ITEM_MODIFIED | EVENT_FLAG_ITEM_IS_FILE, FileOperation::Modified),
            (EVENT_FLAG_ITEM_RENAMED | EVENT_FLAG_ITEM_IS_FILE, FileOperation::Renamed),
            (EVENT_FLAG_ITEM_REMOVED | EVENT_FLAG_ITEM_IS_FILE, FileOperation::Deleted),
        ] {
            let normalized = crate::fsevents::FseventsEvent {
                path: PathBuf::from("/private/tmp/file"),
                event_id: 2,
                flags,
            }
            .normalize_flags();
            assert_eq!(operation_for(&normalized), Some(expected));
        }
    }

    #[test]
    fn selected_root_scope_requires_an_exact_policy_mapping() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = SelectedRoot::from_canonical_path("root-main", directory.path()).expect("root");
        let mut policy = PolicyProfile::deny_by_default("scope-test");
        policy.select_root("root-main");
        assert!(validate_roots(std::slice::from_ref(&root), &policy).is_ok());
        let extra =
            SelectedRoot::from_canonical_path("root-extra", directory.path()).expect("root");
        assert!(matches!(
            validate_roots(&[root, extra], &policy),
            Err(FseventsCollectorError::RootScopeMismatch)
        ));
    }
}
