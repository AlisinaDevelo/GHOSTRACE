//! Strict v1 profile for an optional Parquet-derived archive.
//!
//! The profile is a contract, not a writer.  The canonical encrypted journal
//! and the versioned JSONL export remain authoritative; a future archive
//! writer must validate this profile and every row before publishing a
//! Parquet file.  Keeping the profile separate from the writer makes the
//! plaintext boundary, schema evolution, and deletion limits reviewable.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::GhostraceError;

/// Profile schema version.
pub const PARQUET_ARCHIVE_PROFILE_VERSION: u32 = 1;
/// Stable identity for the profile contract.
pub const PARQUET_ARCHIVE_PROFILE_SCHEMA_ID: &str = "ghostrace.parquet-archive-profile";
/// Canonical source schema represented by every archive row.
pub const PARQUET_ARCHIVE_SOURCE_SCHEMA_ID: &str = "ghostrace.event-envelope";
/// Maximum number of declared archive columns.
pub const MAX_PARQUET_ARCHIVE_COLUMNS: usize = 23;
/// Maximum serialized row size accepted by a future streaming writer.
pub const MAX_PARQUET_ARCHIVE_ROW_BYTES: usize = 1024 * 1024;
/// Maximum rows accepted by one derived archive.
pub const MAX_PARQUET_ARCHIVE_ROWS: u64 = 10_000_000;
/// Maximum profile/foot metadata bytes accepted before publication.
pub const MAX_PARQUET_ARCHIVE_METADATA_BYTES: usize = 64 * 1024;

/// Checked-in profile JSON.  Consumers should validate it before use.
pub const PARQUET_ARCHIVE_PROFILE_JSON: &str =
    include_str!("../fixtures/parquet-archive-profile-v1.golden.json");

const EXPECTED_ORDER: &[&str] = &["observed_at", "ingest_seq", "event_id"];
const EXPECTED_GAP_COLUMNS: &[&str] = &[
    "gap_source",
    "gap_reason_code",
    "gap_dropped_count",
    "gap_from_cursor",
    "gap_to_cursor",
    "gap_volume_digest",
    "gap_root_ids_json",
    "gap_remediation_json",
];
const REQUIRED_GAP_COLUMNS: &[&str] = &["gap_source", "gap_reason_code", "gap_dropped_count"];

/// A physical Parquet column and its lossless source mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetColumn {
    pub name: String,
    pub physical_type: String,
    pub nullable: bool,
    pub source: String,
    pub semantic: String,
    pub conversion: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetOrdering {
    pub fields: Vec<String>,
    pub tie_break: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetGapMapping {
    pub event_kind: String,
    pub columns: Vec<String>,
    pub missing_behavior: String,
    pub omission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetProvenanceMapping {
    pub source_column: String,
    pub semantics: String,
    pub unknown_behavior: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetPolicyMapping {
    pub id_column: String,
    pub version_column: String,
    pub semantics: String,
    pub unknown_behavior: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetEvolutionPolicy {
    pub rule: String,
    pub additions: String,
    pub removals: String,
    pub type_changes: String,
    pub unknown_columns: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetStoragePolicy {
    pub compression: String,
    pub dictionary_encoding: String,
    pub statistics: String,
    pub page_index: String,
    pub encryption: String,
    pub plaintext_boundary: String,
    pub temporary_file_mode: String,
    pub atomic_publish: String,
    pub cleanup_on_failure: String,
    pub source_untouched: String,
    pub automatic_creation: String,
    pub deletion_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetBounds {
    pub max_columns: usize,
    pub max_row_bytes: usize,
    pub max_rows: u64,
    pub max_metadata_bytes: usize,
}

/// Strict profile for a derived Parquet archive.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParquetArchiveProfile {
    pub schema_id: String,
    pub schema_version: u32,
    pub format: String,
    pub compatibility: String,
    pub source_schema_id: String,
    pub source_schema_version: u32,
    pub columns: Vec<ParquetColumn>,
    pub ordering: ParquetOrdering,
    pub gap_mapping: ParquetGapMapping,
    pub provenance_mapping: ParquetProvenanceMapping,
    pub policy_mapping: ParquetPolicyMapping,
    pub evolution: ParquetEvolutionPolicy,
    pub storage: ParquetStoragePolicy,
    pub bounds: ParquetBounds,
}

impl ParquetArchiveProfile {
    /// Validate the full profile contract, including exact column mappings.
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_id != PARQUET_ARCHIVE_PROFILE_SCHEMA_ID
            || self.schema_version != PARQUET_ARCHIVE_PROFILE_VERSION
            || self.format != "parquet"
            || self.compatibility != "strict"
            || self.source_schema_id != PARQUET_ARCHIVE_SOURCE_SCHEMA_ID
            || self.source_schema_version != 1
            || self.columns.len() != MAX_PARQUET_ARCHIVE_COLUMNS
            || self.bounds.max_columns != MAX_PARQUET_ARCHIVE_COLUMNS
            || self.bounds.max_row_bytes != MAX_PARQUET_ARCHIVE_ROW_BYTES
            || self.bounds.max_rows != MAX_PARQUET_ARCHIVE_ROWS
            || self.bounds.max_metadata_bytes != MAX_PARQUET_ARCHIVE_METADATA_BYTES
        {
            return Err(profile_error("profile identity or bounds are invalid"));
        }

        let expected = expected_columns();
        for (index, (actual, (name, physical_type, nullable, source, semantic, conversion))) in
            self.columns.iter().zip(expected.iter()).enumerate()
        {
            if actual.name != *name
                || actual.physical_type != *physical_type
                || actual.nullable != *nullable
                || actual.source != *source
                || actual.semantic != *semantic
                || actual.conversion != *conversion
            {
                return Err(profile_error(format!("column {index} mapping is invalid")));
            }
        }

        if self.ordering.fields != EXPECTED_ORDER
            || self.ordering.tie_break != "ingest_seq_then_event_id"
            || self.gap_mapping.event_kind != "gap"
            || self.gap_mapping.columns != EXPECTED_GAP_COLUMNS
            || self.gap_mapping.missing_behavior != "null_for_non_gap_events"
            || self.gap_mapping.omission != "reject"
            || self.provenance_mapping.source_column != "provenance_version"
            || self.provenance_mapping.semantics != "exact_adapter_version"
            || self.provenance_mapping.unknown_behavior != "reject"
            || self.policy_mapping.id_column != "policy_profile_id"
            || self.policy_mapping.version_column != "policy_profile_version"
            || self.policy_mapping.semantics != "exact_policy_identity"
            || self.policy_mapping.unknown_behavior != "reject"
            || self.evolution.rule != "additive_nullable_only"
            || self.evolution.additions != "new_profile_version_required"
            || self.evolution.removals != "new_profile_version_required"
            || self.evolution.type_changes != "new_profile_version_required"
            || self.evolution.unknown_columns != "reject"
        {
            return Err(profile_error("ordering, mappings, or evolution policy is invalid"));
        }

        if self.storage.compression != "zstd"
            || self.storage.dictionary_encoding != "disabled"
            || self.storage.statistics != "disabled"
            || self.storage.page_index != "disabled"
            || self.storage.encryption != "not_assumed"
            || self.storage.plaintext_boundary != "explicit_derived_archive"
            || self.storage.temporary_file_mode != "0600"
            || self.storage.atomic_publish != "required"
            || self.storage.cleanup_on_failure != "required"
            || self.storage.source_untouched != "required"
            || self.storage.automatic_creation != "forbidden"
            || self.storage.deletion_scope != "external_copy_limits_explicit"
        {
            return Err(profile_error("storage privacy policy is invalid"));
        }

        Ok(())
    }

    /// Validate the number of rows a streaming writer is about to publish.
    pub fn validate_row_count(&self, row_count: u64) -> Result<(), GhostraceError> {
        self.validate()?;
        if row_count > MAX_PARQUET_ARCHIVE_ROWS {
            return Err(profile_error(format!(
                "archive exceeds the {MAX_PARQUET_ARCHIVE_ROWS}-row bound"
            )));
        }
        Ok(())
    }

    /// Validate rows one at a time without collecting them in memory.
    pub fn validate_rows<'a, I>(&self, rows: I) -> Result<u64, GhostraceError>
    where
        I: IntoIterator<Item = &'a Map<String, Value>>,
    {
        self.validate()?;
        let mut count = 0_u64;
        for row in rows {
            if count == MAX_PARQUET_ARCHIVE_ROWS {
                return Err(profile_error(format!(
                    "archive exceeds the {MAX_PARQUET_ARCHIVE_ROWS}-row bound"
                )));
            }
            self.validate_row(row)?;
            count += 1;
        }
        Ok(count)
    }

    /// Validate a bounded, already-normalized row before a writer encodes it.
    /// This rejects undeclared columns, nullability violations, type changes,
    /// and gap-field loss instead of coercing or truncating values.
    pub fn validate_row(&self, row: &Map<String, Value>) -> Result<(), GhostraceError> {
        self.validate()?;
        let encoded = serde_json::to_vec(row)?;
        if encoded.len() > MAX_PARQUET_ARCHIVE_ROW_BYTES {
            return Err(profile_error(format!(
                "row exceeds the {MAX_PARQUET_ARCHIVE_ROW_BYTES}-byte bound"
            )));
        }
        if row.len() != self.columns.len() {
            return Err(profile_error("row does not contain exactly the declared columns"));
        }

        for column in &self.columns {
            let Some(value) = row.get(&column.name) else {
                return Err(profile_error(format!("row is missing column {}", column.name)));
            };
            if value.is_null() {
                if !column.nullable {
                    return Err(profile_error(format!(
                        "non-nullable column {} is null",
                        column.name
                    )));
                }
                continue;
            }
            validate_value_type(column, value)?;
        }

        let kind = row
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| profile_error("kind must be a string"))?;
        let gap_values_present = EXPECTED_GAP_COLUMNS
            .iter()
            .filter_map(|column| row.get(*column))
            .any(|value| !value.is_null());
        if kind == "gap" {
            for column in REQUIRED_GAP_COLUMNS {
                if row.get(*column).is_none_or(Value::is_null) {
                    return Err(profile_error(format!("gap row is missing {column}")));
                }
            }
        } else if gap_values_present {
            return Err(profile_error("non-gap row carries gap mapping fields"));
        }
        if let Some(payload) = row.get("payload_json").and_then(Value::as_str) {
            let parsed: Value = serde_json::from_str(payload)
                .map_err(|_| profile_error("payload_json is not valid canonical JSON"))?;
            if serde_json::to_string(&parsed)? != payload {
                return Err(profile_error("payload_json is not canonical JSON"));
            }
        }
        Ok(())
    }
}

/// Parse and validate a profile document.
pub fn validate_profile(input: &str) -> Result<ParquetArchiveProfile, GhostraceError> {
    if input.len() > MAX_PARQUET_ARCHIVE_METADATA_BYTES {
        return Err(profile_error(format!(
            "profile metadata exceeds the {MAX_PARQUET_ARCHIVE_METADATA_BYTES}-byte bound"
        )));
    }
    let profile: ParquetArchiveProfile = serde_json::from_str(input)
        .map_err(|error| profile_error(format!("profile JSON is invalid: {error}")))?;
    profile.validate()?;
    Ok(profile)
}

/// Parse and validate the checked-in v1 profile.
pub fn checked_in_profile() -> Result<ParquetArchiveProfile, GhostraceError> {
    validate_profile(PARQUET_ARCHIVE_PROFILE_JSON)
}

fn profile_error(message: impl Into<String>) -> GhostraceError {
    GhostraceError::ExportInvalid(format!("Parquet archive profile: {}", message.into()))
}

fn validate_value_type(column: &ParquetColumn, value: &Value) -> Result<(), GhostraceError> {
    let valid = match column.physical_type.as_str() {
        "utf8" => value.is_string(),
        "uint32" => value.as_u64().is_some_and(|number| number <= u32::MAX as u64),
        "uint64" => value.as_u64().is_some(),
        "int64" | "timestamp_nanos_utc" => value.as_i64().is_some(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(profile_error(format!(
            "column {} does not match physical type {}",
            column.name, column.physical_type
        )))
    }
}

type ExpectedColumn = (&'static str, &'static str, bool, &'static str, &'static str, &'static str);

fn expected_columns() -> [ExpectedColumn; MAX_PARQUET_ARCHIVE_COLUMNS] {
    [
        ("event_id", "utf8", false, "event.event_id", "identity", "identity"),
        ("ingest_seq", "uint64", false, "record.ingest_seq", "total_order", "identity"),
        ("schema_version", "uint32", false, "event.schema_version", "schema_identity", "identity"),
        (
            "observed_at",
            "timestamp_nanos_utc",
            false,
            "event.observed_at",
            "source_time",
            "rfc3339_to_timestamp_nanos",
        ),
        (
            "ingested_at",
            "timestamp_nanos_utc",
            false,
            "event.ingested_at",
            "receipt_time",
            "rfc3339_to_timestamp_nanos",
        ),
        ("source", "utf8", false, "event.source", "source", "identity"),
        ("kind", "utf8", false, "event.kind", "event_kind", "identity"),
        (
            "collector_instance",
            "utf8",
            true,
            "event.collector_instance",
            "collector_identity",
            "identity",
        ),
        ("source_cursor", "utf8", true, "event.source_cursor", "source_cursor", "identity"),
        ("provenance_version", "utf8", false, "event.provenance_version", "provenance", "identity"),
        (
            "policy_profile_id",
            "utf8",
            false,
            "event.policy_profile_id",
            "policy_identity",
            "identity",
        ),
        (
            "policy_profile_version",
            "uint32",
            false,
            "event.policy_profile_version",
            "policy_version",
            "identity",
        ),
        ("evidence", "utf8", false, "event.evidence", "evidence_level", "identity"),
        ("parent_event_id", "utf8", true, "event.parent_event_id", "causal_parent", "identity"),
        ("payload_json", "utf8", false, "event.payload", "canonical_payload", "canonical_json"),
        ("gap_source", "utf8", true, "event.payload.source", "gap_source", "identity"),
        ("gap_reason_code", "utf8", true, "event.payload.reason_code", "gap_reason", "identity"),
        (
            "gap_dropped_count",
            "uint64",
            true,
            "event.payload.dropped_count",
            "gap_count",
            "identity",
        ),
        ("gap_from_cursor", "utf8", true, "event.payload.from_cursor", "gap_boundary", "identity"),
        ("gap_to_cursor", "utf8", true, "event.payload.to_cursor", "gap_boundary", "identity"),
        ("gap_volume_digest", "utf8", true, "event.payload.volume_digest", "gap_scope", "identity"),
        (
            "gap_root_ids_json",
            "utf8",
            true,
            "event.payload.root_ids",
            "gap_scope",
            "canonical_json",
        ),
        (
            "gap_remediation_json",
            "utf8",
            true,
            "event.payload.remediation",
            "gap_recovery",
            "canonical_json",
        ),
    ]
}
