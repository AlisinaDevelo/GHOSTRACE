use std::{fs, path::PathBuf};

use ghostrace::{
    checked_in_profile, validate_profile, ParquetArchiveProfile, MAX_PARQUET_ARCHIVE_ROWS,
    MAX_PARQUET_ARCHIVE_ROW_BYTES, PARQUET_ARCHIVE_PROFILE_JSON,
};
use serde_json::{Map, Value};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sample_row(kind: &str) -> Map<String, Value> {
    let payload = if kind == "gap" {
        serde_json::json!({"dropped_count":1,"reason_code":"recovery_test","source":"filesystem"})
    } else {
        serde_json::json!({"entry_kind":"file","operation":"modified"})
    };
    let mut row = Map::new();
    row.insert(
        "event_id".to_owned(),
        Value::String("00000000-0000-4000-8000-000000000001".to_owned()),
    );
    row.insert("ingest_seq".to_owned(), Value::from(1_u64));
    row.insert("schema_version".to_owned(), Value::from(1_u64));
    row.insert("observed_at".to_owned(), Value::from(1_735_689_600_000_000_000_i64));
    row.insert("ingested_at".to_owned(), Value::from(1_735_689_600_000_000_000_i64));
    row.insert("source".to_owned(), Value::String("filesystem".to_owned()));
    row.insert("kind".to_owned(), Value::String(kind.to_owned()));
    row.insert("collector_instance".to_owned(), Value::String("fixture-fs-1".to_owned()));
    row.insert("source_cursor".to_owned(), Value::String("cursor-0001".to_owned()));
    row.insert("provenance_version".to_owned(), Value::String("fixture-v1".to_owned()));
    row.insert("policy_profile_id".to_owned(), Value::String("fixture-default-v1".to_owned()));
    row.insert("policy_profile_version".to_owned(), Value::from(1_u64));
    row.insert("evidence".to_owned(), Value::String("direct".to_owned()));
    row.insert("parent_event_id".to_owned(), Value::Null);
    row.insert(
        "payload_json".to_owned(),
        Value::String(serde_json::to_string(&payload).expect("canonical payload")),
    );
    for column in [
        "gap_source",
        "gap_reason_code",
        "gap_dropped_count",
        "gap_from_cursor",
        "gap_to_cursor",
        "gap_volume_digest",
        "gap_root_ids_json",
        "gap_remediation_json",
    ] {
        row.insert(column.to_owned(), Value::Null);
    }
    if kind == "gap" {
        row.insert("gap_source".to_owned(), Value::String("filesystem".to_owned()));
        row.insert("gap_reason_code".to_owned(), Value::String("recovery_test".to_owned()));
        row.insert("gap_dropped_count".to_owned(), Value::from(1_u64));
        row.insert("gap_root_ids_json".to_owned(), Value::String("[]".to_owned()));
        row.insert("gap_remediation_json".to_owned(), Value::String("null".to_owned()));
    }
    row
}

fn profile() -> ParquetArchiveProfile {
    checked_in_profile().expect("checked-in profile")
}

#[test]
fn profile_is_strict_versioned_and_matches_schema() {
    let profile = profile();
    assert_eq!(profile.columns.len(), 23);
    assert_eq!(profile.ordering.fields, ["observed_at", "ingest_seq", "event_id"]);

    let schema: Value = serde_json::from_str(
        &fs::read_to_string(root().join("schemas/parquet-archive-profile-v1.json"))
            .expect("profile schema"),
    )
    .expect("profile schema JSON");
    let golden: Value = serde_json::from_str(PARQUET_ARCHIVE_PROFILE_JSON).expect("profile JSON");
    let validator = jsonschema::options().build(&schema).expect("profile schema compiles");
    assert!(validator.is_valid(&golden));

    let mut unknown = golden.clone();
    unknown
        .as_object_mut()
        .expect("profile object")
        .insert("unknown_field".to_owned(), Value::Bool(true));
    assert!(!validator.is_valid(&unknown));
    assert!(
        validate_profile(&serde_json::to_string(&unknown).expect("unknown profile JSON")).is_err()
    );
}

#[test]
fn rows_preserve_non_gap_and_gap_mappings_without_coercion() {
    let profile = profile();
    profile.validate_row(&sample_row("filesystem_changed")).expect("non-gap row");
    profile.validate_row(&sample_row("gap")).expect("gap row");

    let mut missing_gap = sample_row("gap");
    missing_gap.insert("gap_source".to_owned(), Value::Null);
    assert!(profile.validate_row(&missing_gap).is_err());

    let mut non_gap_gap_value = sample_row("filesystem_changed");
    non_gap_gap_value.insert("gap_source".to_owned(), Value::String("filesystem".to_owned()));
    assert!(profile.validate_row(&non_gap_gap_value).is_err());

    let mut wrong_type = sample_row("filesystem_changed");
    wrong_type.insert("ingest_seq".to_owned(), Value::String("1".to_owned()));
    assert!(profile.validate_row(&wrong_type).is_err());

    let mut noncanonical = sample_row("filesystem_changed");
    noncanonical
        .insert("payload_json".to_owned(), Value::String("{ \"b\": 2, \"a\": 1 }".to_owned()));
    assert!(profile.validate_row(&noncanonical).is_err());
}

#[test]
fn rows_reject_unknown_missing_and_oversized_data() {
    let profile = profile();

    profile.validate_row_count(1).expect("one row is bounded");
    assert!(profile.validate_row_count(MAX_PARQUET_ARCHIVE_ROWS + 1).is_err());
    assert_eq!(profile.validate_rows([&sample_row("filesystem_changed")]).expect("stream row"), 1);

    let mut unknown = sample_row("filesystem_changed");
    unknown.insert("future_column".to_owned(), Value::Null);
    assert!(profile.validate_row(&unknown).is_err());

    let mut missing = sample_row("filesystem_changed");
    missing.remove("event_id");
    assert!(profile.validate_row(&missing).is_err());

    let mut oversized = sample_row("filesystem_changed");
    oversized.insert(
        "payload_json".to_owned(),
        Value::String(format!("\"{}\"", "x".repeat(MAX_PARQUET_ARCHIVE_ROW_BYTES))),
    );
    assert!(profile.validate_row(&oversized).is_err());
}

#[test]
fn profile_rejects_lossy_conversion_and_leaky_storage_defaults() {
    let mut value: Value =
        serde_json::from_str(PARQUET_ARCHIVE_PROFILE_JSON).expect("profile JSON");

    value["storage"]["statistics"] = Value::String("enabled".to_owned());
    assert!(validate_profile(&serde_json::to_string(&value).expect("mutated profile")).is_err());

    let mut value: Value =
        serde_json::from_str(PARQUET_ARCHIVE_PROFILE_JSON).expect("profile JSON");
    value["columns"][0]["conversion"] = Value::String("truncate".to_owned());
    assert!(validate_profile(&serde_json::to_string(&value).expect("mutated profile")).is_err());

    let mut value: Value =
        serde_json::from_str(PARQUET_ARCHIVE_PROFILE_JSON).expect("profile JSON");
    value["columns"].as_array_mut().expect("columns").pop();
    assert!(validate_profile(&serde_json::to_string(&value).expect("mutated profile")).is_err());
}
