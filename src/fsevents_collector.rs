//! Explicitly enabled, selected-root FSEvents collection.
//!
//! This module is the first source adapter above the native stream boundary.
//! It requires a consumed [`ConsentConfirmation`], a versioned policy that
//! enables both filesystem and lifecycle events, and an exact mapping from
//! opaque root IDs to canonical paths. Paths are used only in memory for the
//! containment check and are represented by a SHA-256 digest in the journal.
//! The adapter never opens a reported path or reads its contents.
//!
//! Canonicalization here is a startup-time prerequisite for path-free metadata.
//! A separate descriptor-backed [`SelectedRoot::open_contained`] boundary is
//! available to later consumers that must open an existing path; it refuses
//! symlink replacement, hard-link aliases, and component races without reading
//! file content. Exclusion precedence and cursor recovery remain later gates.

use std::{
    collections::{BTreeSet, VecDeque},
    fs::{File, Metadata},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::{
    ffi::CString,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    consent::{ConsentConfirmation, ConsentReceipt, ConsentState, ConsentStateMachine},
    cursor::{CursorIdentity, CursorToken, ReplayBoundary},
    error::GhostraceError,
    fsevents::{
        CallbackHealth, FseventsError, FseventsEvent, FseventsOptions, FseventsStream, StreamState,
    },
    fsevents_flags::{FseventsEventFlag, FseventsEvidenceStatus, NormalizedFseventsEvent},
    journal::{DiagnosticRecord, Journal},
    model::{
        CollectorLifecyclePayload, EntryKind, EventEnvelope, EventKind, EventPayload, EventSource,
        Evidence, FileOperation, FilesystemChangedPayload, IngestionOrigin, InstanceLabel,
        PathClass, PathDigest, PolicyBlockedSummaryPayload, ReasonCode, RootId, SnapshotDigest,
        SourceCursor,
    },
    policy::{PolicyDocument, PolicyProfile},
    volume::VolumeIdentity,
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
    identity: FilesystemIdentity,
    volume: VolumeIdentity,
}

/// A descriptor-backed path that was opened without following any symlink in
/// the selected-root walk. The contained file is deliberately not exposed as
/// a `Read` implementation: this boundary proves authorization and metadata
/// identity without turning the collector into a content reader.
#[derive(Debug)]
pub struct ContainedFile {
    file: File,
    identity: FilesystemIdentity,
}

impl ContainedFile {
    /// Return metadata from the already-authorized descriptor, never from the
    /// path name that was used to reach it.
    pub fn metadata(&self) -> Result<Metadata, FseventsCollectorError> {
        self.file.metadata().map_err(|_| FseventsCollectorError::ContainedOpenRefused {
            reason: "contained descriptor metadata unavailable",
        })
    }

    /// Return whether the descriptor still names the same filesystem object
    /// observed when it was admitted.
    pub fn identity_is_stable(&self) -> Result<bool, FseventsCollectorError> {
        let metadata = self.metadata()?;
        Ok(filesystem_identity(&metadata) == self.identity)
    }
}

/// The filesystem identity used for containment, never exported as event data.
///
/// Device identity is deliberately part of the comparison. A lexical path
/// prefix cannot establish that a reported path is on the selected volume when
/// another volume is mounted below that prefix. The inode component prevents a
/// replaced root directory from silently inheriting the old scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FilesystemIdentity {
    device: u64,
    inode: u64,
}

#[derive(Debug)]
struct ResolvedPath {
    canonical_existing: PathBuf,
    identity: FilesystemIdentity,
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
        let requested_path = canonical_path.into();
        if !requested_path.is_absolute() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "root path must be absolute",
            });
        }
        // Resolve OS aliases such as `/var` -> `/private/var` before the
        // no-follow descriptor walk. Every remaining user-controlled
        // component is then opened without following symlinks.
        let canonical_path = std::fs::canonicalize(&requested_path).map_err(|_| {
            FseventsCollectorError::InvalidRoot { reason: "root path cannot be canonicalized" }
        })?;
        let expected_identity =
            filesystem_identity(&std::fs::metadata(&canonical_path).map_err(|_| {
                FseventsCollectorError::InvalidRoot { reason: "root identity cannot be read" }
            })?);
        let expected_volume = VolumeIdentity::from_path(&canonical_path).map_err(|_| {
            FseventsCollectorError::InvalidRoot { reason: "root volume identity cannot be read" }
        })?;
        run_test_root_open_hook(&canonical_path);
        let descriptor = open_directory_nofollow(&canonical_path)?;
        let descriptor_metadata = descriptor.metadata().map_err(|_| {
            FseventsCollectorError::InvalidRoot { reason: "root descriptor cannot be read" }
        })?;
        if !descriptor_metadata.is_dir() {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "selected root is not a directory",
            });
        }
        let identity = filesystem_identity(&descriptor_metadata);
        if identity != expected_identity {
            return Err(FseventsCollectorError::ContainedOpenRace);
        }
        let volume = VolumeIdentity::from_path(&canonical_path).map_err(|_| {
            FseventsCollectorError::InvalidRoot { reason: "root volume identity cannot be read" }
        })?;
        if volume != expected_volume {
            return Err(FseventsCollectorError::ContainedOpenRace);
        }
        Ok(Self { id: RootId::try_from(id.into())?, canonical_path, identity, volume })
    }

    pub fn id(&self) -> &RootId {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.canonical_path
    }

    /// Stable volume evidence for cursor binding. The display name is never
    /// part of this value.
    pub fn volume_identity(&self) -> &VolumeIdentity {
        &self.volume
    }

    /// Return whether a reported path belongs to this root under the operating
    /// system's filesystem identity rules.
    ///
    /// No case folding or Unicode normalization is invented here. Existing
    /// components are resolved by `realpath`/`canonicalize`; a missing leaf is
    /// checked through its nearest existing ancestor. This preserves the
    /// distinction between case-sensitive, case-insensitive, and
    /// normalization-sensitive volumes while still rejecting `..`, lexical
    /// prefix tricks, symlink escapes, and different-device descendants.
    pub fn contains_path(&self, path: &Path) -> bool {
        resolve_path(path).is_some_and(|resolved| self.contains_resolved(&resolved))
    }

    /// Digest a path in this root's explicit scope. The digest is stable only
    /// for the same root ID, root filesystem identity, OS canonicalization, and
    /// path bytes; it is not a cross-volume or cross-scope identifier.
    pub fn path_digest(&self, path: &Path) -> Result<PathDigest, FseventsCollectorError> {
        if !self.contains_path(path) {
            return Err(FseventsCollectorError::InvalidRoot {
                reason: "path is outside the selected root",
            });
        }
        digest_path_scoped(path, self)
    }

    /// Open an existing path through a descriptor walk rooted at this selected
    /// directory. Every component is opened with `O_NOFOLLOW`; intermediate
    /// descriptors remain the authority for the next lookup, so a rename or
    /// replacement of a parent cannot redirect the walk. Regular files with
    /// multiple hard links are refused because their content may be aliased
    /// outside the selected scope. Symlink and hard-link FSEvents are still
    /// represented as source facts by the metadata collector and are never
    /// opened here.
    pub fn open_contained(&self, path: &Path) -> Result<ContainedFile, FseventsCollectorError> {
        #[cfg(unix)]
        {
            open_contained_unix(self, path, |_| {})
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(FseventsCollectorError::ContainedOpenRefused {
                reason: "descriptor containment is unavailable on this platform",
            })
        }
    }

    fn contains_resolved(&self, resolved: &ResolvedPath) -> bool {
        let root_is_current = std::fs::metadata(&self.canonical_path)
            .ok()
            .map(|metadata| filesystem_identity(&metadata))
            == Some(self.identity);
        root_is_current
            && resolved.identity.device == self.identity.device
            && resolved.canonical_existing.starts_with(&self.canonical_path)
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

    #[error("selected roots must be on one volume for a single replay boundary")]
    MixedRootVolumes,

    #[error("the policy must enable both filesystem and lifecycle events")]
    MissingPolicySource,

    #[error("the consent confirmation does not match the policy document")]
    ConsentPolicyMismatch,

    #[error("contained path open refused: {reason}")]
    ContainedOpenRefused { reason: &'static str },

    #[error("selected-root identity changed during descriptor validation")]
    ContainedOpenRace,

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
    replay_boundary: ReplayBoundary,
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
        if roots.windows(2).any(|pair| pair[0].volume != pair[1].volume) {
            return Err(FseventsCollectorError::MixedRootVolumes);
        }

        let root_scope_digest = digest_root_scope(&roots)?;
        let exclusions_digest =
            digest_identifiers(policy.excluded_roots.iter().map(String::as_str))?;
        let cursor_identity = CursorIdentity::for_volume(
            EventSource::Filesystem,
            config.collector_instance.clone(),
            config.options.stream_mode,
            roots[0].volume.clone(),
        )?;
        let replay_configuration =
            config.options.replay_configuration(root_scope_digest, exclusions_digest)?;
        let replay_boundary = ReplayBoundary::new(cursor_identity, replay_configuration)?;

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
            replay_boundary,
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
            self.submit_gap(overflowed, None, None, None)?;
        }

        let mut committed = Vec::new();
        for event in events {
            let normalized = event.normalize_flags();
            self.record_status(&normalized.status);
            let source_cursor = SourceCursor::try_from(format!("cursor-{}", event.event_id))?;
            if let Some(state) = self.journal.cursor_state(&self.replay_boundary.identity)? {
                let current = CursorToken::new(state.token.raw().clone());
                let candidate = CursorToken::new(source_cursor.clone());
                if let (Some(current_position), Some(candidate_position)) =
                    (current.position(), candidate.position())
                {
                    let missing =
                        candidate_position.saturating_sub(current_position.saturating_add(1));
                    if missing > 0 {
                        let gap_cursor =
                            SourceCursor::try_from(format!("cursor-{}", candidate_position - 1))?;
                        self.submit_gap(
                            missing.min(u128::from(u64::MAX)) as u64,
                            Some(gap_cursor),
                            Some(state.token.raw().clone()),
                            Some(source_cursor.clone()),
                        )?;
                    }
                }
            }
            let Some(root) = self.root_for_path(&event.path) else {
                self.blocked_events = self.blocked_events.saturating_add(1);
                continue;
            };
            let root_id = root.id.clone();
            let decision =
                self.policy.decide(EventSource::Filesystem, Some(root_id.as_str()), false);
            if !decision.is_allowed() {
                self.blocked_events = self.blocked_events.saturating_add(1);
                continue;
            }
            let path_digest = root.path_digest(&event.path)?;
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
                Some(source_cursor),
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
            match self.writer.submit_with_boundary(
                self.origin.clone(),
                vec![event],
                self.policy.clone(),
                diagnostics,
                Some(self.replay_boundary.clone()),
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

    fn submit_gap(
        &mut self,
        dropped_count: u64,
        source_cursor: Option<SourceCursor>,
        from_cursor: Option<SourceCursor>,
        to_cursor: Option<SourceCursor>,
    ) -> Result<(), FseventsCollectorError> {
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
                from_cursor,
                to_cursor,
            }),
            source_cursor,
            self.policy.id.clone(),
            self.policy.version,
            Evidence::Unknown,
            None,
        )?;
        match self.writer.submit_with_boundary(
            self.origin.clone(),
            vec![event],
            self.policy.clone(),
            Vec::new(),
            Some(self.replay_boundary.clone()),
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
            .filter(|root| root.contains_path(path))
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

fn digest_identifiers<'a, I>(identifiers: I) -> Result<SnapshotDigest, GhostraceError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut identifiers = identifiers.into_iter().map(str::to_owned).collect::<Vec<_>>();
    identifiers.sort();
    let canonical = serde_json::to_vec(&identifiers)?;
    let digest = Sha256::digest(canonical);
    let encoded = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    SnapshotDigest::try_from(format!("sha256:{encoded}"))
}

fn digest_root_scope(roots: &[SelectedRoot]) -> Result<SnapshotDigest, GhostraceError> {
    let mut roots = roots
        .iter()
        .map(|root| {
            (
                root.id.as_str().to_owned(),
                root.volume.fingerprint().as_str().to_owned(),
                root.identity.device,
                root.identity.inode,
            )
        })
        .collect::<Vec<_>>();
    roots.sort();
    let canonical = serde_json::to_vec(&roots)?;
    let digest = Sha256::digest(canonical);
    let encoded = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    SnapshotDigest::try_from(format!("sha256:{encoded}"))
}

fn filesystem_identity(metadata: &Metadata) -> FilesystemIdentity {
    #[cfg(unix)]
    {
        FilesystemIdentity { device: metadata.dev(), inode: metadata.ino() }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        FilesystemIdentity { device: 0, inode: 0 }
    }
}

fn hard_link_count(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        metadata.nlink()
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        1
    }
}

fn run_test_root_open_hook(path: &Path) {
    #[cfg(test)]
    ROOT_OPEN_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(path);
        }
    });
    #[cfg(not(test))]
    let _ = path;
}

#[cfg(test)]
type RootOpenHook = Box<dyn FnOnce(&Path) + 'static>;

#[cfg(test)]
thread_local! {
    static ROOT_OPEN_HOOK: std::cell::RefCell<Option<RootOpenHook>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn install_root_open_test_hook(hook: RootOpenHook) {
    ROOT_OPEN_HOOK.with(|slot| {
        assert!(slot.borrow_mut().replace(hook).is_none(), "root open hook already installed");
    });
}

#[cfg(unix)]
fn open_directory_nofollow(path: &Path) -> Result<File, FseventsCollectorError> {
    if !path.is_absolute()
        || path.components().any(|component| component == std::path::Component::ParentDir)
    {
        return Err(FseventsCollectorError::ContainedOpenRefused {
            reason: "root path is not an absolute, parent-free path",
        });
    }
    let mut descriptor =
        File::open("/").map_err(|_| FseventsCollectorError::ContainedOpenRefused {
            reason: "filesystem root descriptor unavailable",
        })?;
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        descriptor = open_component_nofollow(
            descriptor.as_raw_fd(),
            name,
            true,
            "selected-root directory component refused",
        )?;
    }
    Ok(descriptor)
}

#[cfg(unix)]
fn open_component_nofollow(
    directory_fd: std::os::fd::RawFd,
    name: &std::ffi::OsStr,
    directory: bool,
    reason: &'static str,
) -> Result<File, FseventsCollectorError> {
    let name = CString::new(name.as_bytes()).map_err(|_| {
        FseventsCollectorError::ContainedOpenRefused { reason: "path component contains NUL" }
    })?;
    let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    if directory {
        flags |= libc::O_DIRECTORY;
    }
    // SAFETY: `directory_fd` is an owned directory descriptor, `name` is a
    // NUL-terminated component with no interior NUL, and the flags request a
    // read-only no-follow descriptor. The returned fd is owned below.
    let fd = unsafe { libc::openat(directory_fd, name.as_ptr(), flags) };
    if fd < 0 {
        return Err(FseventsCollectorError::ContainedOpenRefused { reason });
    }
    // SAFETY: `fd` is freshly returned by openat and is transferred exactly
    // once into the File owner.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_contained_unix<F>(
    root: &SelectedRoot,
    path: &Path,
    mut before_component: F,
) -> Result<ContainedFile, FseventsCollectorError>
where
    F: FnMut(usize),
{
    if !path.is_absolute()
        || path.components().any(|component| component == std::path::Component::ParentDir)
    {
        return Err(FseventsCollectorError::ContainedOpenRefused {
            reason: "contained path is not an absolute, parent-free path",
        });
    }
    let relative = path.strip_prefix(&root.canonical_path).map_err(|_| {
        FseventsCollectorError::ContainedOpenRefused {
            reason: "contained path is outside the selected root",
        }
    })?;
    let root_descriptor = open_directory_nofollow(&root.canonical_path)?;
    let root_metadata =
        root_descriptor.metadata().map_err(|_| FseventsCollectorError::ContainedOpenRefused {
            reason: "root descriptor metadata unavailable",
        })?;
    if filesystem_identity(&root_metadata) != root.identity {
        return Err(FseventsCollectorError::ContainedOpenRace);
    }

    let components = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name),
            std::path::Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Ok(ContainedFile { identity: root.identity, file: root_descriptor });
    }

    let mut directory = root_descriptor;
    for (index, component) in components.iter().enumerate() {
        before_component(index);
        let last = index + 1 == components.len();
        let child = open_component_nofollow(
            directory.as_raw_fd(),
            component,
            !last,
            if last {
                "contained leaf refused by no-follow policy"
            } else {
                "contained parent refused by no-follow policy"
            },
        )?;
        let metadata =
            child.metadata().map_err(|_| FseventsCollectorError::ContainedOpenRefused {
                reason: "contained descriptor metadata unavailable",
            })?;
        let identity = filesystem_identity(&metadata);
        if identity.device != root.identity.device {
            return Err(FseventsCollectorError::ContainedOpenRefused {
                reason: "contained path crosses a filesystem device",
            });
        }
        if last {
            if metadata.is_file() && hard_link_count(&metadata) > 1 {
                return Err(FseventsCollectorError::ContainedOpenRefused {
                    reason: "contained file has an external hard-link alias",
                });
            }
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(FseventsCollectorError::ContainedOpenRefused {
                    reason: "contained leaf is not a regular file or directory",
                });
            }
            return Ok(ContainedFile { file: child, identity });
        }
        directory = child;
    }
    Err(FseventsCollectorError::ContainedOpenRefused {
        reason: "contained path walk ended without a leaf",
    })
}

fn resolve_path(path: &Path) -> Option<ResolvedPath> {
    if !path.is_absolute()
        || path.components().any(|component| component == std::path::Component::ParentDir)
    {
        return None;
    }

    let mut candidate = path.to_path_buf();
    loop {
        if let Ok(canonical_existing) = std::fs::canonicalize(&candidate) {
            let metadata = std::fs::metadata(&canonical_existing).ok()?;
            return Some(ResolvedPath {
                canonical_existing,
                identity: filesystem_identity(&metadata),
            });
        }
        if !candidate.pop() {
            return None;
        }
    }
}

fn digest_path_scoped(
    path: &Path,
    root: &SelectedRoot,
) -> Result<PathDigest, FseventsCollectorError> {
    let digest_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(target_os = "macos")]
    let path_bytes = {
        use std::os::unix::ffi::OsStrExt;
        digest_path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(target_os = "macos"))]
    let path_bytes = digest_path.to_string_lossy().as_bytes().to_vec();
    let mut hasher = Sha256::new();
    hasher.update(b"ghostrace-fsevents-path-digest-v2\0");
    hasher.update(root.id.as_str().as_bytes());
    hasher.update(root.identity.device.to_le_bytes());
    hasher.update(root.identity.inode.to_le_bytes());
    hasher.update(root.volume.fingerprint().as_str().as_bytes());
    hasher.update(path_bytes);
    let digest = hasher.finalize();
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
        EVENT_FLAG_ITEM_CREATED, EVENT_FLAG_ITEM_IS_FILE, EVENT_FLAG_ITEM_IS_HARDLINK,
        EVENT_FLAG_ITEM_IS_SYMLINK, EVENT_FLAG_ITEM_MODIFIED, EVENT_FLAG_ITEM_REMOVED,
        EVENT_FLAG_ITEM_RENAMED,
    };

    #[test]
    fn operation_mapping_is_deterministic_and_path_digest_is_not_path_text() {
        let directory = tempfile::tempdir().expect("root");
        let root = SelectedRoot::new("root-main", directory.path()).expect("root");
        let secret_path = root.path().join("collector-secret");
        let created = crate::fsevents::FseventsEvent {
            path: secret_path.clone(),
            event_id: 1,
            flags: EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE,
        };
        let normalized = created.normalize_flags();
        assert_eq!(operation_for(&normalized), Some(FileOperation::Created));
        assert_eq!(entry_kind_for(&normalized), EntryKind::File);
        let digest = root.path_digest(&created.path).expect("digest");
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

    #[test]
    fn containment_rejects_a_lexical_prefix_trick_and_parent_traversal() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root_path = directory.path().join("selected");
        let sibling_path = directory.path().join("selected-sibling");
        std::fs::create_dir(&root_path).expect("root");
        std::fs::create_dir(&sibling_path).expect("sibling");
        let root = SelectedRoot::new("root-main", &root_path).expect("root");

        assert!(root.contains_path(&root.path().join("missing/child")));
        assert!(!root.contains_path(&sibling_path.join("child")));
        assert!(!root.contains_path(&root.path().join("missing/../escape")));
    }

    #[test]
    fn mixed_volume_identity_cannot_be_overridden_by_a_matching_path_prefix() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = SelectedRoot::new("root-main", directory.path()).expect("root");
        let resolved = ResolvedPath {
            canonical_existing: root.path().join("mounted-volume").to_path_buf(),
            identity: FilesystemIdentity {
                device: root.identity.device.saturating_add(1),
                inode: root.identity.inode,
            },
        };
        assert!(!root.contains_resolved(&resolved));
    }

    #[test]
    fn replacing_the_selected_root_invalidates_the_original_identity() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root_path = directory.path().join("selected");
        let replacement_path = directory.path().join("replacement");
        std::fs::create_dir(&root_path).expect("root");
        let root = SelectedRoot::new("root-main", &root_path).expect("root");
        std::fs::rename(&root_path, &replacement_path).expect("move original root");
        std::fs::create_dir(&root_path).expect("replacement root");

        assert!(!root.contains_path(&root_path.join("new-child")));
    }

    #[test]
    fn digest_scope_includes_root_identity_and_uses_os_canonical_bytes() {
        let directory = tempfile::tempdir().expect("root");
        let root = SelectedRoot::new("root-main", directory.path()).expect("root");
        let other_scope = SelectedRoot::new("root-other", directory.path()).expect("root");
        let path = root.path().join("café");
        std::fs::write(&path, b"fixture").expect("file");

        let first = root.path_digest(&path).expect("digest");
        let second = other_scope.path_digest(&path).expect("digest");
        assert_ne!(first, second);
        assert!(!first.as_str().contains("café"));
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_open_refuses_symlink_and_hard_link_aliases_without_reading_content() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("root");
        let root_path = directory.path().join("selected");
        std::fs::create_dir(&root_path).expect("selected root");
        let root = SelectedRoot::new("root-main", &root_path).expect("root");
        let outside = directory.path().join("outside.txt");
        std::fs::write(&outside, b"outside-secret").expect("outside");

        let symlink_path = root.path().join("symlink.txt");
        symlink(&outside, &symlink_path).expect("symlink");
        assert!(root.open_contained(&symlink_path).is_err());

        let hard_link_path = root.path().join("hard-link.txt");
        std::fs::hard_link(&outside, &hard_link_path).expect("hard link");
        assert!(root.open_contained(&hard_link_path).is_err());

        let regular = root.path().join("regular.txt");
        std::fs::write(&regular, b"inside-secret").expect("regular");
        let descriptor = root.open_contained(&regular).expect("regular descriptor");
        assert!(descriptor.metadata().expect("metadata").is_file());
        assert!(descriptor.identity_is_stable().expect("identity"));
    }

    #[cfg(unix)]
    #[test]
    fn descriptor_walk_denies_replacement_of_every_path_component() {
        use std::os::unix::fs::symlink;

        for swapped_component in 0..3 {
            let directory = tempfile::tempdir().expect("root");
            let root_path = directory.path().join("selected");
            std::fs::create_dir(&root_path).expect("selected root");
            let first = root_path.join("first");
            let second = first.join("second");
            let leaf = second.join("leaf.txt");
            std::fs::create_dir(&first).expect("first");
            std::fs::create_dir(&second).expect("second");
            std::fs::write(&leaf, b"inside-secret").expect("leaf");
            let root = SelectedRoot::new("root-main", &root_path).expect("root");

            let outside_directory = directory.path().join("outside-directory");
            std::fs::create_dir(&outside_directory).expect("outside directory");
            let outside_file = directory.path().join("outside-file");
            std::fs::write(&outside_file, b"outside-secret").expect("outside file");
            let swap_path = match swapped_component {
                0 => first.clone(),
                1 => second.clone(),
                _ => leaf.clone(),
            };
            let moved_path = directory.path().join(format!("moved-{swapped_component}"));
            let replacement = if swapped_component == 2 {
                outside_file.clone()
            } else {
                outside_directory.clone()
            };
            let path = leaf.clone();
            let result = open_contained_unix(&root, &path, |index| {
                if index == swapped_component {
                    std::fs::rename(&swap_path, &moved_path).expect("move component");
                    symlink(&replacement, &swap_path).expect("replace component");
                }
            });
            assert!(result.is_err(), "component {swapped_component} must be denied");
        }
    }

    #[cfg(unix)]
    #[test]
    fn root_selection_refuses_replacement_between_identity_and_descriptor_open() {
        let directory = tempfile::tempdir().expect("root");
        let root_path = directory.path().join("selected");
        let moved_path = directory.path().join("selected-moved");
        std::fs::create_dir(&root_path).expect("selected root");
        let original = root_path.clone();
        let moved = moved_path.clone();
        let replacement = root_path.clone();
        install_root_open_test_hook(Box::new(move |_| {
            std::fs::rename(&original, &moved).expect("move root");
            std::fs::create_dir(&replacement).expect("replacement root");
        }));

        let result = SelectedRoot::from_canonical_path("root-main", &root_path);
        assert!(matches!(result, Err(FseventsCollectorError::ContainedOpenRace)));
    }

    #[test]
    fn link_flags_remain_source_facts_without_authorizing_an_open() {
        let symlink_event = crate::fsevents_flags::normalize_fsevents_event(
            1,
            EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_SYMLINK,
        );
        assert_eq!(entry_kind_for(&symlink_event), EntryKind::Symlink);

        let hard_link_event = crate::fsevents_flags::normalize_fsevents_event(
            2,
            EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_IS_HARDLINK,
        );
        assert_eq!(entry_kind_for(&hard_link_event), EntryKind::File);
        assert!(hard_link_event.flags.contains(FseventsEventFlag::ItemIsHardlink));
    }
}
