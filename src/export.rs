//! Versioned JSONL export for bounded fixtures.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
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
    model::{EventKind, EventPayload, EventSource, GapPayload, EVENT_SCHEMA_VERSION},
    ordering::ORDERING_CONTRACT_VERSION,
    policy::PolicyProfile,
    storage,
};

pub const EXPORT_VERSION: u32 = 1;

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
    /// manifest line.  Excluding the manifest avoids a self-referential hash.
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
    let output_path = output_path.as_ref();
    if journal.path().is_some_and(|journal_path| journal_path == output_path) {
        return Err(GhostraceError::ExportSourceConflict);
    }
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
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|source| GhostraceError::Io { path: parent.to_path_buf(), source })?;
    storage::set_private_file_permissions(temporary.path())?;

    let events = journal.ordered_events()?;
    let event_lines =
        events.iter().map(serialize_event_record).collect::<Result<Vec<_>, GhostraceError>>()?;
    let manifest = build_manifest(&events, &event_lines);
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        write_jsonl(&mut writer, &manifest)?;
        for line in &event_lines {
            writer
                .write_all(line)
                .and_then(|_| writer.write_all(b"\n"))
                .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
        }
        writer
            .flush()
            .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| GhostraceError::Io { path: output_path.to_path_buf(), source })?;
    if force {
        temporary.persist(output_path).map_err(|error| GhostraceError::Io {
            path: output_path.to_path_buf(),
            source: error.error,
        })?;
    } else {
        match temporary.persist_noclobber(output_path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
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
    crate::export_schema::validate_export(output_path)?;
    Ok(manifest)
}

fn write_jsonl<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), GhostraceError> {
    // Keeping one serializer per line gives bounded memory for future large fixtures.
    serde_json::to_writer(&mut *writer, value)?;
    writer
        .write_all(b"\n")
        .map_err(|source| GhostraceError::Io { path: PathBuf::from("<export>"), source })?;
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

fn build_manifest(events: &[StoredEvent], event_lines: &[Vec<u8>]) -> ExportManifest {
    let policy_profiles = events
        .iter()
        .map(|stored| {
            (
                stored.event.policy_profile_id.as_str().to_owned(),
                stored.event.policy_profile_version,
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|(id, version)| ExportPolicyProfile { id, version })
        .collect();
    let gap_records = events
        .iter()
        .filter_map(|stored| match &stored.event.payload {
            EventPayload::Gap(GapPayload { source, reason_code, dropped_count, .. }) => {
                Some(ExportGap {
                    event_id: stored.event.event_id,
                    source: *source,
                    reason_code: reason_code.as_str().to_owned(),
                    dropped_count: *dropped_count,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let collector_status = events
        .iter()
        .rev()
        .find_map(|stored| match stored.event.kind {
            EventKind::CollectorStarted => Some("running"),
            EventKind::CollectorStopped => Some("stopped"),
            _ => None,
        })
        .unwrap_or("unknown")
        .to_owned();
    let mut body = Vec::new();
    for line in event_lines {
        body.extend_from_slice(line);
        body.push(b'\n');
    }
    let mut schema_versions = BTreeMap::new();
    schema_versions.insert("manifest".to_owned(), 1);
    schema_versions.insert("event".to_owned(), EVENT_SCHEMA_VERSION);
    schema_versions.insert("gap".to_owned(), 1);
    schema_versions.insert("claim".to_owned(), 1);
    schema_versions.insert("policy".to_owned(), 1);
    schema_versions.insert("source_coverage".to_owned(), 1);
    let mut record_counts = BTreeMap::new();
    record_counts.insert("event".to_owned(), events.len() as u64);
    record_counts.insert("gap".to_owned(), 0);
    record_counts.insert("claim".to_owned(), 0);
    record_counts.insert("policy".to_owned(), 0);
    record_counts.insert("source_coverage".to_owned(), 0);
    let mut byte_counts = BTreeMap::new();
    byte_counts.insert("event".to_owned(), body.len() as u64);
    byte_counts.insert("gap".to_owned(), 0);
    byte_counts.insert("claim".to_owned(), 0);
    byte_counts.insert("policy".to_owned(), 0);
    byte_counts.insert("source_coverage".to_owned(), 0);
    let mut record_digests = BTreeMap::new();
    record_digests.insert("event".to_owned(), sha256_digest(&body));
    let empty_digest = sha256_digest(&[]);
    record_digests.insert("gap".to_owned(), empty_digest.clone());
    record_digests.insert("claim".to_owned(), empty_digest.clone());
    record_digests.insert("policy".to_owned(), empty_digest.clone());
    record_digests.insert("source_coverage".to_owned(), empty_digest);
    ExportManifest {
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
        query_scope: ExportQueryScope::default(),
        policy_profiles,
        coverage: ExportCoverage {
            event_count: events.len(),
            gap_count: gap_records.len(),
            warning: (!gap_records.is_empty()).then(|| "coverage gaps are present".to_owned()),
        },
        collector_status,
        gaps: gap_records,
    }
}
