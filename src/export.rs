//! Versioned JSONL export for bounded fixtures.

use std::{
    collections::BTreeSet,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use tempfile::NamedTempFile;
use uuid::Uuid;

use crate::{
    correlation::{CORRELATION_RULE_REGISTRY_VERSION, CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION},
    crypto::DeterministicKeyProvider,
    error::GhostraceError,
    fixture::ingest_fixture,
    journal::{Journal, StoredEvent},
    model::{EventKind, EventPayload, EventSource, GapPayload, EVENT_SCHEMA_VERSION},
    ordering::ORDERING_CONTRACT_VERSION,
    policy::PolicyProfile,
    storage,
};

pub const EXPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportCoverage {
    pub event_count: usize,
    pub gap_count: usize,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportGap {
    pub event_id: Uuid,
    pub source: EventSource,
    pub reason_code: String,
    pub dropped_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportManifest {
    pub record_type: &'static str,
    pub export_version: u32,
    pub event_schema_version: u32,
    pub ordering_contract_version: u32,
    pub correlation_rule_registry_version: u32,
    pub correlation_rule_version: u32,
    pub policy_profiles: Vec<ExportPolicyProfile>,
    pub coverage: ExportCoverage,
    pub collector_status: String,
    pub gaps: Vec<ExportGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportPolicyProfile {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportEventRecord<'a> {
    pub record_type: &'static str,
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
    let manifest = build_manifest(&events);
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        write_jsonl(&mut writer, &manifest)?;
        for stored in &events {
            write_jsonl(
                &mut writer,
                &ExportEventRecord {
                    record_type: "event",
                    ingest_seq: stored.ingest_seq,
                    event: &stored.event,
                },
            )?;
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

fn build_manifest(events: &[StoredEvent]) -> ExportManifest {
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
    ExportManifest {
        record_type: "manifest",
        export_version: EXPORT_VERSION,
        event_schema_version: EVENT_SCHEMA_VERSION,
        ordering_contract_version: ORDERING_CONTRACT_VERSION,
        correlation_rule_registry_version: CORRELATION_RULE_REGISTRY_VERSION,
        correlation_rule_version: CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION,
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
