//! Versioned export and manifest contracts.
//!
//! The registry is intentionally small and explicit.  A consumer must validate
//! the manifest before treating any JSONL body as complete; record types and
//! versions are not inferred from whatever happens to deserialize.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    error::GhostraceError,
    export::ExportManifest,
    model::{EventEnvelope, EventKind, EventPayload, EventSource, EVENT_SCHEMA_VERSION},
    ordering::{StableOrderKey, ORDERING_CONTRACT_VERSION},
};

pub const EXPORT_REGISTRY_VERSION: u32 = 1;
pub const EXPORT_MANIFEST_SCHEMA_ID: &str = "ghostrace.export-manifest";
pub const EXPORT_EVENT_SCHEMA_ID: &str = "ghostrace.event-envelope";
pub const EXPORT_GAP_SCHEMA_ID: &str = "ghostrace.gap-record";
pub const EXPORT_CLAIM_SCHEMA_ID: &str = "ghostrace.claim-record";
pub const EXPORT_POLICY_SCHEMA_ID: &str = "ghostrace.policy-record";
pub const EXPORT_SOURCE_COVERAGE_SCHEMA_ID: &str = "ghostrace.source-coverage";
pub const EXPORT_SCHEMA_REGISTRY_JSON: &str = include_str!("../schemas/export-registry-v1.json");

/// The compatibility class is deliberately a closed vocabulary. `strict`
/// means unknown fields are rejected; a future additive class may only be
/// introduced by a new registry version and an explicit consumer decision.
pub const STRICT_COMPATIBILITY: &str = "strict";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaDescriptor {
    pub schema_id: String,
    pub version: u32,
    pub compatibility: String,
    pub strict_unknown_fields: bool,
    pub record_type: String,
    pub format: String,
    pub schema_path: String,
    pub golden_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaRegistry {
    pub registry_version: u32,
    pub schemas: Vec<SchemaDescriptor>,
}

impl SchemaRegistry {
    pub fn checked_in() -> Result<Self, GhostraceError> {
        validate_registry(EXPORT_SCHEMA_REGISTRY_JSON)
    }

    fn expected() -> BTreeMap<&'static str, (&'static str, &'static str)> {
        BTreeMap::from([
            (EXPORT_MANIFEST_SCHEMA_ID, ("manifest", "schemas/export-manifest-v1.json")),
            (EXPORT_EVENT_SCHEMA_ID, ("event", "schemas/event-envelope-v1.json")),
            (EXPORT_GAP_SCHEMA_ID, ("gap", "schemas/gap-record-v1.json")),
            (EXPORT_CLAIM_SCHEMA_ID, ("claim", "schemas/claim-record-v1.json")),
            (EXPORT_POLICY_SCHEMA_ID, ("policy", "schemas/policy-record-v1.json")),
            (
                EXPORT_SOURCE_COVERAGE_SCHEMA_ID,
                ("source_coverage", "schemas/source-coverage-v1.json"),
            ),
        ])
    }

    pub fn descriptor(&self, schema_id: &str) -> Option<&SchemaDescriptor> {
        self.schemas.iter().find(|descriptor| descriptor.schema_id == schema_id)
    }

    pub fn schema_versions(&self) -> BTreeMap<String, u32> {
        self.schemas
            .iter()
            .map(|descriptor| (descriptor.record_type.clone(), descriptor.version))
            .collect()
    }
}

/// Validate the checked-in registry shape before any schema is trusted.
pub fn validate_registry(input: &str) -> Result<SchemaRegistry, GhostraceError> {
    let registry: SchemaRegistry = serde_json::from_str(input)
        .map_err(|error| GhostraceError::ExportInvalid(format!("registry JSON: {error}")))?;
    if registry.registry_version != EXPORT_REGISTRY_VERSION {
        return Err(GhostraceError::ExportInvalid(format!(
            "unsupported registry version {}",
            registry.registry_version
        )));
    }
    let expected = SchemaRegistry::expected();
    if registry.schemas.len() != expected.len() {
        return Err(GhostraceError::ExportInvalid(format!(
            "registry declares {} schemas; expected {}",
            registry.schemas.len(),
            expected.len()
        )));
    }
    let mut seen = BTreeSet::new();
    let mut seen_record_types = BTreeSet::new();
    for descriptor in &registry.schemas {
        if !seen.insert(descriptor.schema_id.as_str()) {
            return Err(GhostraceError::ExportInvalid(format!(
                "duplicate schema id {}",
                descriptor.schema_id
            )));
        }
        let Some((record_type, schema_path)) = expected.get(descriptor.schema_id.as_str()) else {
            return Err(GhostraceError::ExportInvalid(format!(
                "undeclared schema id {}",
                descriptor.schema_id
            )));
        };
        if descriptor.version != 1
            || descriptor.compatibility != STRICT_COMPATIBILITY
            || !descriptor.strict_unknown_fields
            || descriptor.record_type != *record_type
            || descriptor.format != "json"
            || descriptor.schema_path != *schema_path
        {
            return Err(GhostraceError::ExportInvalid(format!(
                "invalid descriptor for {}",
                descriptor.schema_id
            )));
        }
        if !seen_record_types.insert(descriptor.record_type.as_str()) {
            return Err(GhostraceError::ExportInvalid(format!(
                "duplicate record type {}",
                descriptor.record_type
            )));
        }
        if descriptor.golden_path.is_empty()
            || Path::new(&descriptor.schema_path).is_absolute()
            || Path::new(&descriptor.golden_path).is_absolute()
            || descriptor.schema_path.split('/').any(|part| part == "..")
            || descriptor.golden_path.split('/').any(|part| part == "..")
        {
            return Err(GhostraceError::ExportInvalid(format!(
                "unsafe paths for {}",
                descriptor.schema_id
            )));
        }
    }
    Ok(registry)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportQueryScope {
    pub kind: String,
    pub source: Option<EventSource>,
    pub observed_from: Option<String>,
    pub observed_until: Option<String>,
    pub include_coverage: bool,
}

impl Default for ExportQueryScope {
    fn default() -> Self {
        Self {
            kind: "all_committed".to_owned(),
            source: None,
            observed_from: None,
            observed_until: None,
            include_coverage: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportValidation {
    pub manifest: ExportManifest,
    pub event_count: usize,
    pub body_bytes: usize,
    pub body_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedExportEventRecord {
    record_type: String,
    schema_id: String,
    schema_version: u32,
    ingest_seq: u64,
    event: EventEnvelope,
}

/// Validate an export completely, including its declared body counts, bytes,
/// digest, record schema identifiers, and event schema versions.
pub fn validate_export(path: impl AsRef<Path>) -> Result<ExportValidation, GhostraceError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| GhostraceError::ExportInvalid(format!("export is not UTF-8: {error}")))?;
    if !text.ends_with('\n') {
        return Err(GhostraceError::ExportInvalid("export must end with a newline".to_owned()));
    }
    let mut lines = text.split_terminator('\n');
    let manifest_line =
        lines.next().ok_or_else(|| GhostraceError::ExportInvalid("export is empty".to_owned()))?;
    let manifest: ExportManifest = serde_json::from_str(manifest_line).map_err(|error| {
        GhostraceError::ExportInvalid(format!("manifest is invalid or has unknown fields: {error}"))
    })?;
    let registry = SchemaRegistry::checked_in()?;
    if manifest.record_type != "manifest"
        || manifest.schema_id != EXPORT_MANIFEST_SCHEMA_ID
        || manifest.schema_version != 1
        || manifest.registry_version != EXPORT_REGISTRY_VERSION
        || manifest.export_version != crate::export::EXPORT_VERSION
        || manifest.event_schema_version != EVENT_SCHEMA_VERSION
        || manifest.tool_version.is_empty()
    {
        return Err(GhostraceError::ExportInvalid(
            "manifest identity or version is undeclared".to_owned(),
        ));
    }
    let expected_versions = registry.schema_versions();
    if manifest.schema_versions != expected_versions {
        return Err(GhostraceError::ExportInvalid(
            "manifest schema_versions do not match the registry".to_owned(),
        ));
    }
    if manifest.query_scope.kind != "all_committed" || !manifest.query_scope.include_coverage {
        return Err(GhostraceError::ExportInvalid("unsupported query scope".to_owned()));
    }
    let mut event_count = 0usize;
    let mut body_bytes = 0usize;
    let mut body_digest = Sha256::new();
    let mut last_order = None;
    let mut seen_ingest_seq = BTreeSet::new();
    let mut policy_profiles = BTreeSet::new();
    let mut gap_records = Vec::new();
    let mut collector_status = "unknown";
    for line in lines {
        if line.is_empty() {
            return Err(GhostraceError::ExportInvalid("blank record line".to_owned()));
        }
        let record: OwnedExportEventRecord = serde_json::from_str(line).map_err(|error| {
            GhostraceError::ExportInvalid(format!(
                "record is invalid or has unknown fields: {error}"
            ))
        })?;
        if record.record_type != "event"
            || record.schema_id != EXPORT_EVENT_SCHEMA_ID
            || record.schema_version != EVENT_SCHEMA_VERSION
            || record.event.schema_version != EVENT_SCHEMA_VERSION
        {
            return Err(GhostraceError::ExportInvalid(
                "record type or version is undeclared or mixed".to_owned(),
            ));
        }
        let order = StableOrderKey {
            contract_version: ORDERING_CONTRACT_VERSION,
            source_observed_at: Some(record.event.observed_at),
            ingest_seq: record.ingest_seq,
            event_id: record.event.event_id,
        };
        if !seen_ingest_seq.insert(record.ingest_seq)
            || last_order.is_some_and(|previous| order <= previous)
        {
            return Err(GhostraceError::ExportInvalid(
                "event stable order is not unique and ordered".to_owned(),
            ));
        }
        last_order = Some(order);
        event_count += 1;
        policy_profiles.insert((
            record.event.policy_profile_id.as_str().to_owned(),
            record.event.policy_profile_version,
        ));
        if let EventPayload::Gap(gap) = &record.event.payload {
            gap_records.push(crate::export::ExportGap {
                event_id: record.event.event_id,
                source: gap.source,
                reason_code: gap.reason_code.as_str().to_owned(),
                dropped_count: gap.dropped_count,
            });
        }
        match record.event.kind {
            EventKind::CollectorStarted => collector_status = "running",
            EventKind::CollectorStopped => collector_status = "stopped",
            _ => {}
        }
        body_bytes = body_bytes
            .checked_add(line.len() + 1)
            .ok_or_else(|| GhostraceError::ExportInvalid("body byte count overflow".to_owned()))?;
        body_digest.update(line.as_bytes());
        body_digest.update(b"\n");
    }
    let digest = hex_digest(body_digest.finalize().as_slice());
    let mut expected_counts = BTreeMap::new();
    expected_counts.insert("event".to_owned(), event_count as u64);
    expected_counts.insert("gap".to_owned(), 0);
    expected_counts.insert("claim".to_owned(), 0);
    expected_counts.insert("policy".to_owned(), 0);
    expected_counts.insert("source_coverage".to_owned(), 0);
    let mut expected_bytes = BTreeMap::new();
    expected_bytes.insert("event".to_owned(), body_bytes as u64);
    expected_bytes.insert("gap".to_owned(), 0);
    expected_bytes.insert("claim".to_owned(), 0);
    expected_bytes.insert("policy".to_owned(), 0);
    expected_bytes.insert("source_coverage".to_owned(), 0);
    let empty_digest = sha256_digest(&[]);
    let mut expected_digests = BTreeMap::new();
    expected_digests.insert("event".to_owned(), digest.clone());
    expected_digests.insert("gap".to_owned(), empty_digest.clone());
    expected_digests.insert("claim".to_owned(), empty_digest.clone());
    expected_digests.insert("policy".to_owned(), empty_digest.clone());
    expected_digests.insert("source_coverage".to_owned(), empty_digest);
    if manifest.record_counts != expected_counts
        || manifest.byte_counts != expected_bytes
        || manifest.record_digests != expected_digests
    {
        return Err(GhostraceError::ExportInvalid(
            "manifest body counts, bytes, or digest do not match".to_owned(),
        ));
    }
    if manifest.coverage.event_count != event_count {
        return Err(GhostraceError::ExportInvalid(
            "manifest coverage event count does not match body".to_owned(),
        ));
    }
    let expected_profiles = policy_profiles
        .into_iter()
        .map(|(id, version)| crate::export::ExportPolicyProfile { id, version })
        .collect::<Vec<_>>();
    if manifest.policy_profiles != expected_profiles
        || manifest.gaps != gap_records
        || manifest.coverage.gap_count != gap_records.len()
        || manifest.coverage.warning
            != (!gap_records.is_empty()).then(|| "coverage gaps are present".to_owned())
        || manifest.collector_status != collector_status
    {
        return Err(GhostraceError::ExportInvalid(
            "manifest policy, gap, or collector coverage does not match body".to_owned(),
        ));
    }
    Ok(ExportValidation { manifest, event_count, body_bytes, body_sha256: digest })
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing a String cannot fail");
    }
    encoded
}

pub(crate) fn sha256_digest(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes).as_slice())
}
