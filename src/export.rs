//! Versioned JSONL export with bounded streaming and atomic publication.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::Builder as TempFileBuilder;
use uuid::Uuid;

use crate::{
    correlation::{CORRELATION_RULE_REGISTRY_VERSION, CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION},
    crypto::DeterministicKeyProvider,
    error::GhostraceError,
    export_schema::{
        sha256_digest, ExportQueryScope, EXPORT_EVENT_SCHEMA_ID, EXPORT_MANIFEST_SCHEMA_ID,
        EXPORT_REGISTRY_VERSION,
    },
    fixture::ingest_fixture,
    journal::{Journal, StoredEvent},
    model::{
        EventKind, EventPayload, EventSource, GapPayload, PolicyProfileId, SnapshotDigest,
        EVENT_SCHEMA_VERSION,
    },
    ordering::ORDERING_CONTRACT_VERSION,
    policy::PolicyProfile,
    storage,
};

pub const EXPORT_VERSION: u32 = 1;

/// Maximum encoded JSON bytes for one manifest or event line, excluding its
/// trailing newline.  The writer and validator enforce the same bound so a
/// malformed line cannot force an unbounded allocation.
pub const MAX_EXPORT_RECORD_BYTES: usize = 1024 * 1024;
/// Metadata collections are deliberately bounded even when the event body is
/// much larger. A future registry version can raise these limits explicitly.
pub const MAX_EXPORT_POLICY_PROFILES: usize = 4_096;
pub const MAX_EXPORT_GAPS: usize = 4_096;
pub const MAX_EXPORT_EVENT_RECORDS: usize = 1_000_000;

const EXPORT_BUFFER_BYTES: usize = 64 * 1024;
pub const EXPORT_PREVIEW_SCHEMA_VERSION: u32 = 1;
pub const EXPORT_PLAINTEXT_WARNING: &str =
    "This export writes authorized plaintext metadata outside encrypted journal storage; review and confirm this exact plan before continuing.";

/// The destination class is deliberately coarse. It lets a confirmation bind
/// to whether a new file or an existing file will be touched without retaining
/// the user-selected path in a preview or receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDestinationClass {
    NewFile,
    ExistingFile,
}

/// Field inventory shown in the declassification preview. The v1 export
/// schema carries the complete authorized event envelope; keeping this list
/// explicit makes that fact reviewable and gives a future schema version a
/// stable place to add true field-level redaction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportField {
    EventId,
    ObservedAt,
    IngestedAt,
    Source,
    Kind,
    CollectorInstance,
    SourceCursor,
    ProvenanceVersion,
    PolicyProfile,
    Evidence,
    ParentEventId,
    Payload,
}

impl ExportField {
    fn authorized_event_v1() -> Vec<Self> {
        vec![
            Self::EventId,
            Self::ObservedAt,
            Self::IngestedAt,
            Self::Source,
            Self::Kind,
            Self::CollectorInstance,
            Self::SourceCursor,
            Self::ProvenanceVersion,
            Self::PolicyProfile,
            Self::Evidence,
            Self::ParentEventId,
            Self::Payload,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRedactionPlan {
    pub schema_version: u32,
    pub fields: Vec<ExportField>,
}

impl Default for ExportRedactionPlan {
    fn default() -> Self {
        Self::authorized_event_v1()
    }
}

impl ExportRedactionPlan {
    pub fn authorized_event_v1() -> Self {
        Self {
            schema_version: EXPORT_PREVIEW_SCHEMA_VERSION,
            fields: ExportField::authorized_event_v1(),
        }
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != EXPORT_PREVIEW_SCHEMA_VERSION
            || self.fields != ExportField::authorized_event_v1()
        {
            return Err(GhostraceError::ExportPlanInvalid(
                "v1 execution requires the complete authorized event field inventory".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn fields(&self) -> &[ExportField] {
        &self.fields
    }
}

/// A typed, immutable query shape for the preview and execution gate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportQuery {
    pub source: Option<EventSource>,
    pub observed_from: Option<DateTime<Utc>>,
    pub observed_until: Option<DateTime<Utc>>,
    pub include_coverage: bool,
}

impl Default for ExportQuery {
    fn default() -> Self {
        Self { source: None, observed_from: None, observed_until: None, include_coverage: true }
    }
}

impl ExportQuery {
    pub fn all_committed() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if !self.include_coverage
            || self.observed_from.zip(self.observed_until).is_some_and(|(from, until)| from > until)
        {
            return Err(GhostraceError::ExportPlanInvalid(
                "export query coverage must be enabled and its time range ordered".to_owned(),
            ));
        }
        Ok(())
    }

    fn matches(&self, event: &crate::model::EventEnvelope) -> bool {
        self.source.is_none_or(|source| source == event.source)
            && self.observed_from.is_none_or(|from| event.observed_at >= from)
            && self.observed_until.is_none_or(|until| event.observed_at <= until)
    }

    fn scope(&self) -> ExportQueryScope {
        ExportQueryScope {
            kind: if self.source.is_none()
                && self.observed_from.is_none()
                && self.observed_until.is_none()
            {
                "all_committed".to_owned()
            } else {
                "filtered".to_owned()
            },
            source: self.source,
            observed_from: self.observed_from.map(|value| value.to_rfc3339()),
            observed_until: self.observed_until.map(|value| value.to_rfc3339()),
            include_coverage: self.include_coverage,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub query: ExportQuery,
    pub redaction: ExportRedactionPlan,
    pub force: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPlan {
    pub schema_version: u32,
    pub query: ExportQuery,
    pub redaction: ExportRedactionPlan,
    pub policy_profile_id: PolicyProfileId,
    pub policy_profile_version: u32,
    pub policy_scope_digest: SnapshotDigest,
    pub destination_class: ExportDestinationClass,
    pub force: bool,
}

impl ExportPlan {
    fn build(
        policy: &PolicyProfile,
        request: &ExportRequest,
        destination_class: ExportDestinationClass,
    ) -> Result<Self, GhostraceError> {
        request.query.validate()?;
        request.redaction.validate()?;
        let policy_document = policy.to_document()?;
        Ok(Self {
            schema_version: EXPORT_PREVIEW_SCHEMA_VERSION,
            query: request.query.clone(),
            redaction: request.redaction.clone(),
            policy_profile_id: PolicyProfileId::try_from(policy.id.clone())?,
            policy_profile_version: policy.version,
            policy_scope_digest: policy_document.scope_digest()?,
            destination_class,
            force: request.force,
        })
    }

    pub fn digest(&self) -> Result<SnapshotDigest, GhostraceError> {
        if self.schema_version != EXPORT_PREVIEW_SCHEMA_VERSION {
            return Err(GhostraceError::ExportPlanInvalid(
                "unsupported export preview schema version".to_owned(),
            ));
        }
        let bytes = serde_json::to_vec(self)?;
        digest_as_snapshot(&bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPreview {
    pub schema_version: u32,
    pub plan: ExportPlan,
    pub plan_digest: SnapshotDigest,
    pub snapshot_digest: SnapshotDigest,
    pub snapshot_event_count: u64,
    pub snapshot_boundary: u64,
    pub event_count: u64,
    pub sources: Vec<EventSource>,
    pub observed_from: Option<String>,
    pub observed_until: Option<String>,
    pub policy_profiles: Vec<ExportPolicyProfile>,
    pub gaps: Vec<ExportGap>,
    pub collector_status: String,
    pub plaintext_warning: String,
}

impl ExportPreview {
    /// Confirm the exact rendered preview. The returned value is required by
    /// the execution API and carries no destination path or event payload.
    pub fn confirm(self) -> ExportConfirmation {
        ExportConfirmation { preview: self }
    }

    pub fn plan_digest(&self) -> &SnapshotDigest {
        &self.plan_digest
    }

    pub fn snapshot_digest(&self) -> &SnapshotDigest {
        &self.snapshot_digest
    }

    pub fn warning(&self) -> &str {
        &self.plaintext_warning
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportConfirmation {
    preview: ExportPreview,
}

impl ExportConfirmation {
    pub fn plan_digest(&self) -> &SnapshotDigest {
        &self.preview.plan_digest
    }

    pub fn snapshot_digest(&self) -> &SnapshotDigest {
        &self.preview.snapshot_digest
    }

    pub fn preview(&self) -> &ExportPreview {
        &self.preview
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportReceipt {
    pub schema_version: u32,
    pub plan_digest: SnapshotDigest,
    pub snapshot_digest: SnapshotDigest,
    pub manifest_digest: SnapshotDigest,
    pub destination_class: ExportDestinationClass,
    pub policy_profile_id: PolicyProfileId,
    pub policy_profile_version: u32,
    pub event_count: u64,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportResult {
    pub manifest: ExportManifest,
    pub receipt: ExportReceipt,
}

#[derive(Clone, Debug, Default)]
pub struct ExportCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ExportCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExportOptions {
    pub force: bool,
    pub cancellation: Option<ExportCancellation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportCoverage {
    pub event_count: usize,
    pub gap_count: usize,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportGap {
    pub event_id: Uuid,
    pub source: EventSource,
    pub reason_code: String,
    pub dropped_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportManifest {
    pub record_type: String,
    pub schema_id: String,
    pub schema_version: u32,
    pub registry_version: u32,
    pub export_version: u32,
    pub tool_version: String,
    pub event_schema_version: u32,
    pub ordering_contract_version: u32,
    pub correlation_rule_registry_version: u32,
    pub correlation_rule_version: u32,
    pub schema_versions: BTreeMap<String, u32>,
    /// Counts, byte lengths, and digests cover the JSONL body after the
    /// manifest line. Excluding the manifest avoids a self-referential hash.
    pub record_counts: BTreeMap<String, u64>,
    pub byte_counts: BTreeMap<String, u64>,
    pub record_digests: BTreeMap<String, String>,
    pub query_scope: ExportQueryScope,
    pub policy_profiles: Vec<ExportPolicyProfile>,
    pub coverage: ExportCoverage,
    pub collector_status: String,
    pub gaps: Vec<ExportGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPolicyProfile {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportEventRecord<'a> {
    pub record_type: &'static str,
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub ingest_seq: u64,
    pub event: &'a crate::model::EventEnvelope,
}

pub fn export_fixture(
    fixture_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    force: bool,
) -> Result<ExportManifest, GhostraceError> {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("fixture-export-v1"))?;
    let policy = PolicyProfile::fixture_default();
    ingest_fixture(fixture_path, &journal, &policy)?;
    export_journal(&journal, output_path, force)
}

pub fn export_journal(
    journal: &Journal,
    output_path: impl AsRef<Path>,
    force: bool,
) -> Result<ExportManifest, GhostraceError> {
    export_journal_with_options(journal, output_path, ExportOptions { force, cancellation: None })
}

/// Build a bounded, privacy-safe declassification preview. The scan occurs on
/// one read snapshot and retains only digests, counts, source/profile labels,
/// and bounded gap metadata; it never retains an event payload in the preview.
pub fn preview_export(
    journal: &Journal,
    policy: &PolicyProfile,
    request: &ExportRequest,
    output_path: impl AsRef<Path>,
) -> Result<ExportPreview, GhostraceError> {
    let output_path = output_path.as_ref();
    if journal.path().is_some_and(|journal_path| journal_path == output_path) {
        return Err(GhostraceError::ExportSourceConflict);
    }
    let destination_class = destination_class(output_path, request.force)?;
    let plan = ExportPlan::build(policy, request, destination_class)?;
    let plan_digest = plan.digest()?;
    let (snapshot, stats) = inspect_snapshot(journal, &plan)?;
    let sources = stats.sources.iter().copied().collect();
    let policy_profiles = stats.policy_profiles();
    let observed_from = stats.observed_from.map(|value| value.to_rfc3339());
    let observed_until = stats.observed_until.map(|value| value.to_rfc3339());
    let collector_status = stats.collector_status_value();
    Ok(ExportPreview {
        schema_version: EXPORT_PREVIEW_SCHEMA_VERSION,
        plan,
        plan_digest,
        snapshot_digest: snapshot.digest,
        snapshot_event_count: snapshot.event_count,
        snapshot_boundary: snapshot.boundary,
        event_count: stats.event_count,
        sources,
        observed_from,
        observed_until,
        policy_profiles,
        gaps: stats.gaps,
        collector_status,
        plaintext_warning: EXPORT_PLAINTEXT_WARNING.to_owned(),
    })
}

/// Execute exactly the confirmed preview. A changed policy, destination class,
/// query/redaction plan, or journal snapshot fails before publication and never
/// changes an existing destination.
pub fn export_journal_with_confirmation(
    journal: &Journal,
    output_path: impl AsRef<Path>,
    confirmation: ExportConfirmation,
    policy: &PolicyProfile,
) -> Result<ExportResult, GhostraceError> {
    let output_path = output_path.as_ref();
    if journal.path().is_some_and(|journal_path| journal_path == output_path) {
        return Err(GhostraceError::ExportSourceConflict);
    }
    let request = ExportRequest {
        query: confirmation.preview.plan.query.clone(),
        redaction: confirmation.preview.plan.redaction.clone(),
        force: confirmation.preview.plan.force,
    };
    let destination_class = destination_class(output_path, request.force)?;
    let current_plan = ExportPlan::build(policy, &request, destination_class)?;
    if current_plan.digest()? != *confirmation.plan_digest()
        || current_plan.policy_profile_id != confirmation.preview.plan.policy_profile_id
        || current_plan.policy_profile_version != confirmation.preview.plan.policy_profile_version
    {
        return Err(GhostraceError::ExportConfirmationMismatch);
    }
    let manifest = export_journal_inner(
        journal,
        output_path,
        request.force,
        None,
        None,
        None,
        Some(&current_plan.query),
        Some(confirmation.snapshot_digest()),
    )?;
    let receipt = ExportReceipt {
        schema_version: EXPORT_PREVIEW_SCHEMA_VERSION,
        plan_digest: confirmation.preview.plan_digest,
        snapshot_digest: confirmation.preview.snapshot_digest,
        manifest_digest: manifest_digest(output_path)?,
        destination_class: current_plan.destination_class,
        policy_profile_id: current_plan.policy_profile_id,
        policy_profile_version: current_plan.policy_profile_version,
        event_count: manifest.coverage.event_count as u64,
        recorded_at: Utc::now(),
    };
    Ok(ExportResult { manifest, receipt })
}

/// Alias with a concise name for callers that treat the confirmation as the
/// explicit declassification capability.
pub fn export_confirmed(
    journal: &Journal,
    output_path: impl AsRef<Path>,
    confirmation: ExportConfirmation,
    policy: &PolicyProfile,
) -> Result<ExportResult, GhostraceError> {
    export_journal_with_confirmation(journal, output_path, confirmation, policy)
}

/// Stream a journal through a private body spool, then publish one complete
/// manifest-plus-body artifact with an atomic same-directory rename. The body
/// spool and final temporary file are removed automatically on every error,
/// cancellation, or process unwind; an existing destination is not touched
/// until the final rename succeeds.
pub fn export_journal_with_options(
    journal: &Journal,
    output_path: impl AsRef<Path>,
    options: ExportOptions,
) -> Result<ExportManifest, GhostraceError> {
    export_journal_inner(
        journal,
        output_path.as_ref(),
        options.force,
        options.cancellation.as_ref(),
        None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn export_journal_inner(
    journal: &Journal,
    output_path: &Path,
    force: bool,
    cancellation: Option<&ExportCancellation>,
    write_budget: Option<WriteBudget>,
    cancel_after_events: Option<u64>,
    query: Option<&ExportQuery>,
    expected_snapshot_digest: Option<&SnapshotDigest>,
) -> Result<ExportManifest, GhostraceError> {
    if journal.path().is_some_and(|journal_path| journal_path == output_path) {
        return Err(GhostraceError::ExportSourceConflict);
    }
    let mut run = ExportRun {
        output_path,
        cancellation,
        write_budget,
        cancel_after_events,
        events_written: 0,
    };
    run.check_cancelled()?;

    let output_exists = if force {
        storage::validate_existing_artifact_for_overwrite(output_path)?
    } else {
        storage::validate_existing_artifact(output_path)?
    };
    if output_exists && !force {
        return Err(GhostraceError::ExportExists(output_path.to_path_buf()));
    }
    let parent = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    storage::ensure_artifact_parent(parent)?;

    // Pass one decrypts and serializes one event at a time. Only the private
    // spool and bounded per-record buffer survive this pass.
    let mut body_temporary = TempFileBuilder::new()
        .prefix(".ghostrace-export-incomplete-body-")
        .tempfile_in(parent)
        .map_err(|source| GhostraceError::Io { path: parent.to_path_buf(), source })?;
    storage::set_private_file_permissions(body_temporary.path())?;
    let mut stats = ExportStats::default();
    let mut snapshot = SnapshotAccumulator::default();
    {
        let mut writer =
            BufWriter::with_capacity(EXPORT_BUFFER_BYTES, body_temporary.as_file_mut());
        journal.for_each_ordered_event(|stored| {
            run.check_cancelled()?;
            let line = serialize_event_record(&stored)?;
            if line.len() > MAX_EXPORT_RECORD_BYTES {
                return Err(GhostraceError::ExportInvalid(format!(
                    "event record exceeds the {MAX_EXPORT_RECORD_BYTES}-byte bound"
                )));
            }
            snapshot.observe(&stored, &line)?;
            if query.is_some_and(|query| !query.matches(&stored.event)) {
                return Ok(());
            }
            run.write_bytes(&mut writer, &line)?;
            run.write_bytes(&mut writer, b"\n")?;
            stats.observe(&stored, &line)?;
            run.observe_event_written();
            Ok(())
        })?;
        writer
            .flush()
            .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    }
    let snapshot = snapshot.finish();
    if expected_snapshot_digest.is_some_and(|expected| expected != &snapshot.digest) {
        return Err(GhostraceError::ExportSnapshotChanged);
    }
    body_temporary
        .as_file()
        .sync_all()
        .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    run.check_cancelled()?;
    let manifest =
        stats.into_manifest(query.map_or_else(ExportQuery::default, Clone::clone).scope())?;

    // Pass two places the now-known manifest before copying the private body
    // spool in bounded chunks. The destination remains absent or unchanged
    // until this temporary file is flushed and atomically renamed.
    let mut temporary = TempFileBuilder::new()
        .prefix(".ghostrace-export-incomplete-final-")
        .tempfile_in(parent)
        .map_err(|source| GhostraceError::Io { path: parent.to_path_buf(), source })?;
    storage::set_private_file_permissions(temporary.path())?;
    {
        let mut writer = BufWriter::with_capacity(EXPORT_BUFFER_BYTES, temporary.as_file_mut());
        write_jsonl(&mut run, &mut writer, &manifest)?;
        copy_body(&mut run, body_temporary.path(), &mut writer)?;
        writer
            .flush()
            .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    // Validate the complete temporary artifact before the rename. This keeps
    // an internal serializer or digest regression from ever publishing a
    // destination that merely looks complete because it has a manifest line.
    crate::export_schema::validate_export(temporary.path())?;
    run.check_cancelled()?;

    if force {
        temporary.persist(output_path).map_err(|error| GhostraceError::Io {
            path: output_path.to_path_buf(),
            source: error.error,
        })?;
    } else {
        match temporary.persist_noclobber(output_path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == ErrorKind::AlreadyExists => {
                return Err(GhostraceError::ExportExists(output_path.to_path_buf()));
            }
            Err(error) => {
                return Err(GhostraceError::Io {
                    path: output_path.to_path_buf(),
                    source: error.error,
                });
            }
        }
    }
    storage::set_private_file_permissions(output_path)?;
    storage::sync_directory(parent)?;
    crate::export_schema::validate_export(output_path)?;
    Ok(manifest)
}

fn write_jsonl<T: Serialize>(
    run: &mut ExportRun<'_>,
    writer: &mut impl Write,
    value: &T,
) -> Result<(), GhostraceError> {
    let line = serde_json::to_vec(value)?;
    if line.len() > MAX_EXPORT_RECORD_BYTES {
        return Err(GhostraceError::ExportInvalid(format!(
            "JSONL record exceeds the {MAX_EXPORT_RECORD_BYTES}-byte bound"
        )));
    }
    run.write_bytes(writer, &line)?;
    run.write_bytes(writer, b"\n")
}

fn copy_body(
    run: &mut ExportRun<'_>,
    body_path: &Path,
    writer: &mut impl Write,
) -> Result<(), GhostraceError> {
    let file = File::open(body_path)
        .map_err(|source| GhostraceError::Io { path: body_path.to_path_buf(), source })?;
    let mut reader = BufReader::with_capacity(EXPORT_BUFFER_BYTES, file);
    let mut buffer = [0_u8; EXPORT_BUFFER_BYTES];
    loop {
        run.check_cancelled()?;
        let bytes = reader
            .read(&mut buffer)
            .map_err(|source| GhostraceError::Io { path: body_path.to_path_buf(), source })?;
        if bytes == 0 {
            break;
        }
        run.write_bytes(writer, &buffer[..bytes])?;
    }
    Ok(())
}

fn serialize_event_record(stored: &StoredEvent) -> Result<Vec<u8>, GhostraceError> {
    Ok(serde_json::to_vec(&ExportEventRecord {
        record_type: "event",
        schema_id: EXPORT_EVENT_SCHEMA_ID,
        schema_version: EVENT_SCHEMA_VERSION,
        ingest_seq: stored.ingest_seq,
        event: &stored.event,
    })?)
}

#[derive(Default)]
struct ExportStats {
    event_count: u64,
    body_bytes: u64,
    body_digest: Sha256,
    policy_profiles: BTreeSet<(String, u32)>,
    gaps: Vec<ExportGap>,
    collector_status: &'static str,
    sources: BTreeSet<EventSource>,
    observed_from: Option<DateTime<Utc>>,
    observed_until: Option<DateTime<Utc>>,
}

impl ExportStats {
    fn observe(&mut self, stored: &StoredEvent, line: &[u8]) -> Result<(), GhostraceError> {
        if self.event_count >= MAX_EXPORT_EVENT_RECORDS as u64 {
            return Err(GhostraceError::ExportInvalid(format!(
                "export exceeds the {MAX_EXPORT_EVENT_RECORDS}-event bound"
            )));
        }
        let line_bytes = u64::try_from(line.len())
            .map_err(|_| GhostraceError::ExportInvalid("record byte count overflow".to_owned()))?;
        self.body_bytes = self
            .body_bytes
            .checked_add(line_bytes + 1)
            .ok_or_else(|| GhostraceError::ExportInvalid("body byte count overflow".to_owned()))?;
        self.body_digest.update(line);
        self.body_digest.update(b"\n");
        self.event_count += 1;
        self.sources.insert(stored.event.source);
        self.observed_from = Some(
            self.observed_from
                .map_or(stored.event.observed_at, |current| current.min(stored.event.observed_at)),
        );
        self.observed_until = Some(
            self.observed_until
                .map_or(stored.event.observed_at, |current| current.max(stored.event.observed_at)),
        );

        let profile = (
            stored.event.policy_profile_id.as_str().to_owned(),
            stored.event.policy_profile_version,
        );
        if self.policy_profiles.insert(profile)
            && self.policy_profiles.len() > MAX_EXPORT_POLICY_PROFILES
        {
            return Err(GhostraceError::ExportInvalid(format!(
                "export exceeds the {MAX_EXPORT_POLICY_PROFILES}-profile bound"
            )));
        }
        if let EventPayload::Gap(GapPayload { source, reason_code, dropped_count, .. }) =
            &stored.event.payload
        {
            if self.gaps.len() >= MAX_EXPORT_GAPS {
                return Err(GhostraceError::ExportInvalid(format!(
                    "export exceeds the {MAX_EXPORT_GAPS}-gap metadata bound"
                )));
            }
            self.gaps.push(ExportGap {
                event_id: stored.event.event_id,
                source: *source,
                reason_code: reason_code.as_str().to_owned(),
                dropped_count: *dropped_count,
            });
        }
        self.collector_status = match stored.event.kind {
            EventKind::CollectorStarted => "running",
            EventKind::CollectorStopped => "stopped",
            _ => self.collector_status,
        };
        Ok(())
    }

    fn policy_profiles(&self) -> Vec<ExportPolicyProfile> {
        self.policy_profiles
            .iter()
            .cloned()
            .map(|(id, version)| ExportPolicyProfile { id, version })
            .collect()
    }

    fn collector_status_value(&self) -> String {
        if self.collector_status.is_empty() {
            "unknown".to_owned()
        } else {
            self.collector_status.to_owned()
        }
    }

    fn into_manifest(
        self,
        query_scope: ExportQueryScope,
    ) -> Result<ExportManifest, GhostraceError> {
        let event_count = usize::try_from(self.event_count).map_err(|_| {
            GhostraceError::ExportInvalid("event count exceeds platform size".to_owned())
        })?;
        let collector_status = self.collector_status_value();
        let policy_profiles = self
            .policy_profiles
            .into_iter()
            .map(|(id, version)| ExportPolicyProfile { id, version })
            .collect::<Vec<_>>();
        let mut schema_versions = BTreeMap::new();
        schema_versions.insert("manifest".to_owned(), 1);
        schema_versions.insert("event".to_owned(), EVENT_SCHEMA_VERSION);
        schema_versions.insert("gap".to_owned(), 1);
        schema_versions.insert("claim".to_owned(), 1);
        schema_versions.insert("policy".to_owned(), 1);
        schema_versions.insert("source_coverage".to_owned(), 1);
        let mut record_counts = BTreeMap::new();
        record_counts.insert("event".to_owned(), self.event_count);
        record_counts.insert("gap".to_owned(), 0);
        record_counts.insert("claim".to_owned(), 0);
        record_counts.insert("policy".to_owned(), 0);
        record_counts.insert("source_coverage".to_owned(), 0);
        let mut byte_counts = BTreeMap::new();
        byte_counts.insert("event".to_owned(), self.body_bytes);
        byte_counts.insert("gap".to_owned(), 0);
        byte_counts.insert("claim".to_owned(), 0);
        byte_counts.insert("policy".to_owned(), 0);
        byte_counts.insert("source_coverage".to_owned(), 0);
        let event_digest = hex_digest(self.body_digest.finalize().as_slice());
        let empty_digest = sha256_digest(&[]);
        let mut record_digests = BTreeMap::new();
        record_digests.insert("event".to_owned(), event_digest);
        record_digests.insert("gap".to_owned(), empty_digest.clone());
        record_digests.insert("claim".to_owned(), empty_digest.clone());
        record_digests.insert("policy".to_owned(), empty_digest.clone());
        record_digests.insert("source_coverage".to_owned(), empty_digest);
        let gap_count = self.gaps.len();
        Ok(ExportManifest {
            record_type: "manifest".to_owned(),
            schema_id: EXPORT_MANIFEST_SCHEMA_ID.to_owned(),
            schema_version: 1,
            registry_version: EXPORT_REGISTRY_VERSION,
            export_version: EXPORT_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            event_schema_version: EVENT_SCHEMA_VERSION,
            ordering_contract_version: ORDERING_CONTRACT_VERSION,
            correlation_rule_registry_version: CORRELATION_RULE_REGISTRY_VERSION,
            correlation_rule_version: CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION,
            schema_versions,
            record_counts,
            byte_counts,
            record_digests,
            query_scope,
            policy_profiles,
            coverage: ExportCoverage {
                event_count,
                gap_count,
                warning: (gap_count > 0).then(|| "coverage gaps are present".to_owned()),
            },
            collector_status,
            gaps: self.gaps,
        })
    }
}

#[derive(Default)]
struct SnapshotAccumulator {
    event_count: u64,
    boundary: u64,
    digest: Sha256,
}

struct SnapshotSummary {
    event_count: u64,
    boundary: u64,
    digest: SnapshotDigest,
}

impl SnapshotAccumulator {
    fn observe(&mut self, stored: &StoredEvent, line: &[u8]) -> Result<(), GhostraceError> {
        if self.event_count >= MAX_EXPORT_EVENT_RECORDS as u64 {
            return Err(GhostraceError::ExportInvalid(format!(
                "journal snapshot exceeds the {MAX_EXPORT_EVENT_RECORDS}-event bound"
            )));
        }
        self.digest.update(line);
        self.digest.update(b"\n");
        self.event_count += 1;
        self.boundary = self.boundary.max(stored.ingest_seq);
        Ok(())
    }

    fn finish(self) -> SnapshotSummary {
        SnapshotSummary {
            event_count: self.event_count,
            boundary: self.boundary,
            digest: snapshot_digest_from_hash(self.digest.finalize().as_slice())
                .expect("SHA-256 digest always has the canonical length"),
        }
    }
}

fn inspect_snapshot(
    journal: &Journal,
    plan: &ExportPlan,
) -> Result<(SnapshotSummary, ExportStats), GhostraceError> {
    let mut snapshot = SnapshotAccumulator::default();
    let mut stats = ExportStats::default();
    journal.for_each_ordered_event(|stored| {
        let line = serialize_event_record(&stored)?;
        if line.len() > MAX_EXPORT_RECORD_BYTES {
            return Err(GhostraceError::ExportInvalid(format!(
                "event record exceeds the {MAX_EXPORT_RECORD_BYTES}-byte bound"
            )));
        }
        snapshot.observe(&stored, &line)?;
        if plan.query.matches(&stored.event) {
            stats.observe(&stored, &line)?;
        }
        Ok(())
    })?;
    Ok((snapshot.finish(), stats))
}

fn destination_class(
    output_path: &Path,
    force: bool,
) -> Result<ExportDestinationClass, GhostraceError> {
    let exists = if force {
        storage::validate_existing_artifact_for_overwrite(output_path)?
    } else {
        storage::validate_existing_artifact(output_path)?
    };
    Ok(if exists { ExportDestinationClass::ExistingFile } else { ExportDestinationClass::NewFile })
}

fn digest_as_snapshot(bytes: &[u8]) -> Result<SnapshotDigest, GhostraceError> {
    SnapshotDigest::try_from(format!("sha256:{}", sha256_digest(bytes)))
}

fn snapshot_digest_from_hash(bytes: &[u8]) -> Result<SnapshotDigest, GhostraceError> {
    SnapshotDigest::try_from(format!("sha256:{}", hex_digest(bytes)))
}

fn manifest_digest(path: &Path) -> Result<SnapshotDigest, GhostraceError> {
    let file = File::open(path)
        .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    let mut reader = BufReader::with_capacity(EXPORT_BUFFER_BYTES, file);
    let mut line = Vec::new();
    let bytes = reader
        .read_until(b'\n', &mut line)
        .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    if bytes == 0 || !line.ends_with(b"\n") {
        return Err(GhostraceError::ExportInvalid(
            "published export has no complete manifest line".to_owned(),
        ));
    }
    digest_as_snapshot(&line)
}

struct ExportRun<'a> {
    output_path: &'a Path,
    cancellation: Option<&'a ExportCancellation>,
    write_budget: Option<WriteBudget>,
    cancel_after_events: Option<u64>,
    events_written: u64,
}

impl ExportRun<'_> {
    fn check_cancelled(&self) -> Result<(), GhostraceError> {
        if self.cancellation.is_some_and(ExportCancellation::is_cancelled) {
            Err(GhostraceError::ExportCancelled)
        } else {
            Ok(())
        }
    }

    fn write_bytes(&mut self, writer: &mut impl Write, bytes: &[u8]) -> Result<(), GhostraceError> {
        self.check_cancelled()?;
        if let Some(budget) = &mut self.write_budget {
            let amount = u64::try_from(bytes.len()).map_err(|_| {
                GhostraceError::ExportInvalid("write byte count exceeds platform size".to_owned())
            })?;
            let next = budget
                .written
                .checked_add(amount)
                .ok_or_else(|| simulated_disk_full(self.output_path))?;
            if next > budget.fail_after {
                return Err(simulated_disk_full(self.output_path));
            }
            budget.written = next;
        }
        writer
            .write_all(bytes)
            .map_err(|source| GhostraceError::Io { path: self.output_path.to_path_buf(), source })
    }

    fn observe_event_written(&mut self) {
        self.events_written = self.events_written.saturating_add(1);
        if self.cancel_after_events.is_some_and(|limit| self.events_written >= limit) {
            if let Some(cancellation) = self.cancellation {
                cancellation.cancel();
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct WriteBudget {
    fail_after: u64,
    written: u64,
}

impl WriteBudget {
    #[cfg(test)]
    fn fail_after(bytes: u64) -> Self {
        Self { fail_after: bytes, written: 0 }
    }
}

fn simulated_disk_full(path: &Path) -> GhostraceError {
    GhostraceError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(ErrorKind::WriteZero, "simulated disk full"),
    }
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::*;
    use crate::{fixture::ingest_fixture, policy::PolicyProfile};

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
    }

    fn journal() -> Journal {
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("0084-export-unit"))
            .expect("journal");
        ingest_fixture(fixture(), &journal, &PolicyProfile::fixture_default()).expect("ingest");
        journal
    }

    #[test]
    fn simulated_disk_full_during_final_copy_preserves_the_old_destination() {
        let directory = tempdir().expect("directory");
        let complete = directory.path().join("complete.jsonl");
        export_journal(&journal(), &complete, false).expect("baseline export");
        let bytes = fs::read(&complete).expect("baseline bytes");
        let manifest_bytes =
            bytes.iter().position(|byte| *byte == b'\n').expect("manifest newline") + 1;
        let body_bytes = bytes.len() - manifest_bytes;
        fs::remove_file(&complete).expect("remove baseline");

        let output = directory.path().join("destination.jsonl");
        fs::write(&output, b"previous complete export\n").expect("seed destination");
        let failure_after = (body_bytes + manifest_bytes + body_bytes - 1) as u64;
        let error = export_journal_inner(
            &journal(),
            &output,
            true,
            None,
            Some(WriteBudget::fail_after(failure_after)),
            None,
            None,
            None,
        )
        .expect_err("simulated disk full");
        assert!(matches!(error, GhostraceError::Io { .. }));
        assert_eq!(fs::read(&output).expect("destination"), b"previous complete export\n");
        let entries = fs::read_dir(directory.path()).expect("directory").count();
        assert_eq!(entries, 1, "failed export must remove both private temporaries");
    }

    #[test]
    fn cancellation_after_a_streamed_record_removes_the_unpublished_spool() {
        let directory = tempdir().expect("directory");
        let output = directory.path().join("cancelled.jsonl");
        let cancellation = ExportCancellation::new();
        let error = export_journal_inner(
            &journal(),
            &output,
            false,
            Some(&cancellation),
            None,
            Some(1),
            None,
            None,
        )
        .expect_err("mid-stream cancellation");
        assert!(matches!(error, GhostraceError::ExportCancelled));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(directory.path()).expect("directory").count(), 0);
    }
}
