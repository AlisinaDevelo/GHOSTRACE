use chrono::{TimeZone, Utc};
use ghostrace::{
    checked_in_shell_metadata, shell_metadata_fields, validate_shell_metadata, PathClass,
    PathDigest, SessionId, ShellExecutableId, ShellExecutionMetadata, ShellStatus,
    ShellWorkingDirectory, SHELL_METADATA_GOLDEN_JSON, SHELL_METADATA_SCHEMA_JSON,
    SHELL_METADATA_SCHEMA_VERSION,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn timestamp(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().expect("fixture timestamp")
}

fn digest() -> PathDigest {
    PathDigest::try_from("sha256:0000000000000000000000000000000000000000000000000000000000000000")
        .expect("digest")
}

fn metadata(
    status: ShellStatus,
    exit_code: Option<i32>,
    signal: Option<u8>,
) -> ShellExecutionMetadata {
    ShellExecutionMetadata::new(
        SessionId::try_from("wrapper-session-test").expect("session"),
        ShellExecutableId::try_from("zsh").expect("executable"),
        ShellWorkingDirectory::new(PathClass::WorkspaceRelative, digest()),
        timestamp(1_745_600_000),
        timestamp(1_745_600_001),
        status,
        exit_code,
        signal,
    )
    .expect("valid shell metadata")
}

#[derive(Debug, Deserialize)]
struct AdversarialManifest {
    schema_version: u32,
    cases: Vec<AdversarialCase>,
}

#[derive(Debug, Deserialize)]
struct AdversarialCase {
    id: String,
    field: String,
    value: Value,
}

#[test]
fn schema_golden_and_field_classifications_are_strict() {
    let schema: Value = serde_json::from_str(SHELL_METADATA_SCHEMA_JSON).expect("schema JSON");
    let golden: Value = serde_json::from_str(SHELL_METADATA_GOLDEN_JSON).expect("golden JSON");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("schema compiles");
    assert!(validator.is_valid(&golden));
    assert_eq!(checked_in_shell_metadata().expect("checked-in metadata").schema_version, 1);
    assert_eq!(SHELL_METADATA_SCHEMA_VERSION, 1);

    let classifications =
        schema["x-ghostrace-field-classification"].as_array().expect("field classifications");
    assert_eq!(classifications.len(), shell_metadata_fields().len());
    assert_eq!(
        serde_json::to_value(shell_metadata_fields()).expect("registry JSON"),
        Value::Array(classifications.clone())
    );
    assert!(classifications.iter().all(|field| {
        field["semantic"].as_str().is_some()
            && field["sensitivity"].as_str().is_some()
            && field["required"] == true
    }));

    let mut unknown = golden.clone();
    unknown["arguments"] = json!(["--secret"]);
    assert!(!validator.is_valid(&unknown));
    assert!(serde_json::from_value::<ShellExecutionMetadata>(unknown).is_err());
}

#[test]
fn valid_outcomes_round_trip_and_preserve_only_allowed_fields() {
    for (status, exit_code, signal) in [
        (ShellStatus::Succeeded, Some(0), None),
        (ShellStatus::Failed, Some(23), None),
        (ShellStatus::Signaled, None, Some(9)),
        (ShellStatus::Unknown, None, None),
    ] {
        let value = metadata(status, exit_code, signal);
        value.validate().expect("outcome validation");
        let encoded = serde_json::to_string(&value).expect("metadata JSON");
        let decoded: ShellExecutionMetadata = serde_json::from_str(&encoded).expect("round trip");
        assert_eq!(decoded, value);
        for forbidden in [
            "arguments",
            "argv",
            "environment",
            "env",
            "stdin",
            "stdout",
            "output",
            "shell_history",
            "aliases",
            "command_text",
            "expanded_command",
        ] {
            assert!(!encoded.contains(forbidden), "forbidden field serialized: {forbidden}");
        }
    }
}

#[test]
fn semantic_validation_rejects_inconsistent_outcomes_times_and_identities() {
    let base =
        serde_json::to_value(metadata(ShellStatus::Succeeded, Some(0), None)).expect("base JSON");
    let cases = [
        ("reverse time", "ended_at", json!("2026-08-26T08:59:59Z")),
        ("zero signal", "signal", json!(0)),
        ("signaled exit", "status", json!("signaled")),
        ("failed zero", "status", json!("failed")),
        ("path executable", "executable_id", json!("/bin/zsh")),
        ("credential executable", "executable_id", json!("password-helper")),
    ];
    for (id, field, value) in cases {
        let mut candidate = base.clone();
        candidate[field] = value;
        if id == "signaled exit" {
            candidate["signal"] = json!(9);
        }
        let error = serde_json::from_value::<ShellExecutionMetadata>(candidate)
            .expect_err("adversarial metadata must reject");
        assert!(!error.to_string().contains("password-helper"));
        assert!(!error.to_string().contains("/bin/zsh"));
    }

    let too_long = ShellExecutionMetadata::new(
        SessionId::try_from("wrapper-session-test").expect("session"),
        ShellExecutableId::try_from("zsh").expect("executable"),
        ShellWorkingDirectory::new(PathClass::WorkspaceRelative, digest()),
        timestamp(1_745_600_000),
        timestamp(1_745_600_000 + 7 * 24 * 60 * 60 + 1),
        ShellStatus::Succeeded,
        Some(0),
        None,
    );
    assert!(too_long.is_err());
}

#[test]
fn adversarial_fixture_rejects_secrets_and_raw_shell_state_without_echo() {
    let manifest: AdversarialManifest =
        serde_json::from_str(include_str!("fixtures/shell-metadata-adversarial-v1.json"))
            .expect("adversarial fixture");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.cases.len(), 18);
    let schema: Value = serde_json::from_str(SHELL_METADATA_SCHEMA_JSON).expect("schema JSON");
    let validator = jsonschema::options().build(&schema).expect("schema compiles");
    let golden: Value = serde_json::from_str(SHELL_METADATA_GOLDEN_JSON).expect("golden JSON");

    for case in manifest.cases {
        let mut candidate = golden.clone();
        if case.field == "path" {
            candidate["working_directory"]["path"] = case.value.clone();
        } else if case.field == "signal" && case.id == "signal_without_status" {
            candidate["signal"] = case.value.clone();
        } else if case.field == "exit_code" {
            candidate["status"] = json!("failed");
            candidate["exit_code"] = case.value.clone();
        } else {
            candidate[&case.field] = case.value.clone();
        }
        if case.id != "reverse_time" {
            assert!(!validator.is_valid(&candidate), "schema accepted {}", case.id);
        }
        let encoded = serde_json::to_string(&candidate).expect("candidate JSON");
        let error = validate_shell_metadata(&encoded).expect_err("metadata must reject");
        assert!(!error.to_string().contains("sentinel"), "{} echoed sentinel", case.id);
        assert!(!error.to_string().contains("/Users/alice/private"), "{} echoed path", case.id);
    }
}
