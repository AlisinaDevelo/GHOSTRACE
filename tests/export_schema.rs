use std::{fs, path::PathBuf};

use ghostrace::{
    export_fixture, validate_export, validate_registry, EXPORT_EVENT_SCHEMA_ID,
    EXPORT_REGISTRY_VERSION, EXPORT_SCHEMA_REGISTRY_JSON,
};
use serde_json::Value;
use tempfile::tempdir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn registry_is_versioned_strict_and_every_schema_has_a_golden() {
    let registry = validate_registry(EXPORT_SCHEMA_REGISTRY_JSON).expect("checked-in registry");
    assert_eq!(registry.registry_version, EXPORT_REGISTRY_VERSION);
    assert_eq!(registry.schemas.len(), 6);
    let mut ids =
        registry.schemas.iter().map(|schema| schema.schema_id.as_str()).collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "ghostrace.claim-record",
            "ghostrace.event-envelope",
            "ghostrace.export-manifest",
            "ghostrace.gap-record",
            "ghostrace.policy-record",
            "ghostrace.source-coverage",
        ]
    );

    let registry_schema: Value = serde_json::from_str(
        &fs::read_to_string(root().join("schemas/export-registry-schema-v1.json"))
            .expect("registry schema"),
    )
    .expect("registry schema JSON");
    let registry_value: Value =
        serde_json::from_str(EXPORT_SCHEMA_REGISTRY_JSON).expect("registry JSON");
    let registry_validator =
        jsonschema::options().build(&registry_schema).expect("registry schema compiles");
    assert!(registry_validator.is_valid(&registry_value));

    for descriptor in &registry.schemas {
        let schema: Value = serde_json::from_str(
            &fs::read_to_string(root().join(&descriptor.schema_path)).expect("schema file"),
        )
        .expect("schema JSON");
        let golden: Value = serde_json::from_str(
            &fs::read_to_string(root().join(&descriptor.golden_path)).expect("golden file"),
        )
        .expect("golden JSON");
        let validator = jsonschema::options().build(&schema).expect("schema compiles");
        assert!(validator.is_valid(&golden), "golden does not validate: {}", descriptor.schema_id);

        let mut unknown = golden.clone();
        unknown
            .as_object_mut()
            .expect("golden object")
            .insert("unknown_field".to_owned(), Value::Bool(true));
        assert!(!validator.is_valid(&unknown), "unknown field accepted: {}", descriptor.schema_id);
    }
}

#[test]
fn export_manifest_binds_schema_versions_counts_bytes_and_digest() {
    let directory = tempdir().expect("tempdir");
    let output = directory.path().join("export.jsonl");
    let fixture = root().join("fixtures/causal-chain.jsonl");
    let manifest = export_fixture(&fixture, &output, false).expect("export");
    let validated = validate_export(&output).expect("complete export validation");
    assert_eq!(validated.manifest, manifest);
    assert_eq!(validated.event_count, 8);
    assert_eq!(validated.manifest.schema_id, "ghostrace.export-manifest");
    assert_eq!(validated.manifest.schema_versions["event"], 1);
    assert_eq!(validated.manifest.record_counts["event"], 8);
    assert_eq!(validated.manifest.byte_counts["event"] as usize, validated.body_bytes);
    assert_eq!(validated.manifest.record_digests["event"], validated.body_sha256);
    assert_eq!(validated.manifest.query_scope.kind, "all_committed");
}

#[test]
fn export_validator_rejects_mixed_versions_unknown_fields_and_digest_drift() {
    let directory = tempdir().expect("tempdir");
    let output = directory.path().join("export.jsonl");
    let fixture = root().join("fixtures/causal-chain.jsonl");
    export_fixture(&fixture, &output, false).expect("export");
    let original = fs::read_to_string(&output).expect("export text");

    let mut lines = original.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut event: Value = serde_json::from_str(&lines[1]).expect("event JSON");
    event["schema_id"] =
        Value::String(EXPORT_EVENT_SCHEMA_ID.replace("event-envelope", "event-envelope-v2"));
    lines[1] = serde_json::to_string(&event).expect("event JSON");
    fs::write(&output, format!("{}\n", lines.join("\n"))).expect("mutated export");
    assert!(validate_export(&output).is_err());

    fs::write(&output, original.as_bytes()).expect("restore export");
    let mut lines = original.lines().map(str::to_owned).collect::<Vec<_>>();
    lines.swap(1, 2);
    fs::write(&output, format!("{}\n", lines.join("\n"))).expect("reordered export");
    assert!(validate_export(&output).is_err());

    fs::write(&output, original.as_bytes()).expect("restore export");
    let mut lines = original.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut manifest: Value = serde_json::from_str(&lines[0]).expect("manifest JSON");
    manifest["unknown_field"] = Value::Bool(true);
    lines[0] = serde_json::to_string(&manifest).expect("manifest JSON");
    fs::write(&output, format!("{}\n", lines.join("\n"))).expect("mutated manifest");
    assert!(validate_export(&output).is_err());

    fs::write(&output, original.as_bytes()).expect("restore export");
    let mut lines = original.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut manifest: Value = serde_json::from_str(&lines[0]).expect("manifest JSON");
    manifest["record_digests"]["event"] = Value::String("0".repeat(64));
    lines[0] = serde_json::to_string(&manifest).expect("manifest JSON");
    fs::write(&output, format!("{}\n", lines.join("\n"))).expect("mutated digest");
    assert!(validate_export(&output).is_err());
}
