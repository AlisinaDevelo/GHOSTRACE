//! Checked-in JSONL fixture ingestion.  This is the only ingestion source in
//! the vertical slice; no filesystem watcher, shell hook, browser bridge, or
//! network client is linked into the package.

use std::{fs::File, io::Read, path::Path};

use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::GhostraceError,
    journal::Journal,
    model::{EventEnvelope, EventKind, PROVENANCE_VERSION},
    policy::PolicyProfile,
};

pub const MAX_FIXTURE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_FIXTURE_LINE_BYTES: usize = 64 * 1024;
pub const MAX_FIXTURE_EVENTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixtureIngestReport {
    pub event_ids: Vec<Uuid>,
    pub ingest_sequences: Vec<u64>,
    pub gap_event_ids: Vec<Uuid>,
    pub policy_profile_id: String,
}

pub fn read_fixture(path: impl AsRef<Path>) -> Result<Vec<EventEnvelope>, GhostraceError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    let mut bytes = Vec::new();
    file.take(MAX_FIXTURE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    if bytes.len() as u64 > MAX_FIXTURE_BYTES {
        return Err(GhostraceError::InvalidEvent(format!(
            "fixture exceeds the {MAX_FIXTURE_BYTES}-byte limit"
        )));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| GhostraceError::InvalidEvent("fixture must be UTF-8".to_owned()))?;
    let mut events = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        if line.len() > MAX_FIXTURE_LINE_BYTES {
            return Err(GhostraceError::FixtureLine {
                line: line_number,
                message: format!("line exceeds the {MAX_FIXTURE_LINE_BYTES}-byte limit"),
            });
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if events.len() == MAX_FIXTURE_EVENTS {
            return Err(GhostraceError::InvalidEvent(format!(
                "fixture exceeds the {MAX_FIXTURE_EVENTS}-event limit"
            )));
        }
        let event: EventEnvelope = serde_json::from_str(trimmed).map_err(|error| {
            let category = match error.classify() {
                serde_json::error::Category::Io => "I/O",
                serde_json::error::Category::Syntax => "syntax",
                serde_json::error::Category::Data => "data",
                serde_json::error::Category::Eof => "end-of-input",
            };
            GhostraceError::FixtureLine {
                line: line_number,
                message: format!("invalid {category} at column {}", error.column()),
            }
        })?;
        events.push(event);
    }
    if events.is_empty() {
        return Err(GhostraceError::InvalidEvent("fixture contains no events".to_owned()));
    }
    Ok(events)
}

pub fn ingest_fixture(
    path: impl AsRef<Path>,
    journal: &Journal,
    policy: &PolicyProfile,
) -> Result<FixtureIngestReport, GhostraceError> {
    let events = read_fixture(path)?;
    if events.iter().any(|event| {
        event.provenance_version != PROVENANCE_VERSION
            || !event.collector_instance.starts_with("fixture-")
    }) {
        return Err(GhostraceError::FixtureProvenance);
    }
    let ingest_sequences = journal.ingest_batch(&events, policy)?;
    let event_ids = events.iter().map(|event| event.event_id).collect();
    let gap_event_ids = events
        .iter()
        .filter(|event| event.kind == EventKind::Gap)
        .map(|event| event.event_id)
        .collect();
    Ok(FixtureIngestReport {
        event_ids,
        ingest_sequences,
        gap_event_ids,
        policy_profile_id: policy.id.clone(),
    })
}
