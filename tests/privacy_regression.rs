use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use ghostrace::{
    explain, export_fixture, ingest_fixture, read_fixture, DeterministicKeyProvider, Journal,
    PolicyProfile,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tempfile::tempdir;
use uuid::Uuid;

const CORPUS_MANIFEST: &str = include_str!("fixtures/privacy-regression-v1.json");

const PRIVATE_BROWSER_CASE: &str = "private-browser-marker";

#[derive(Debug, Deserialize)]
struct CorpusManifest {
    schema_version: u32,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    id: String,
    field: String,
    mode: String,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn shell_fixture_event() -> Value {
    let fixture = fs::read_to_string(fixture_path()).expect("fixture");
    let line = fixture.lines().nth(1).expect("shell fixture event");
    serde_json::from_str(line).expect("shell fixture JSON")
}

fn browser_fixture_event() -> Value {
    let fixture = fs::read_to_string(fixture_path()).expect("fixture");
    let line = fixture.lines().nth(4).expect("browser fixture event");
    serde_json::from_str(line).expect("browser fixture JSON")
}

fn malicious_fixture(case_id: &str, field: &str) -> (tempfile::TempDir, PathBuf, String) {
    let sentinel = format!("PRIVACY_CORPUS_SENTINEL_{case_id}");
    let mut event = shell_fixture_event();
    event["payload"]["data"][field] = json!(sentinel);
    let directory = tempdir().expect("case tempdir");
    let path = directory.path().join("case.jsonl");
    fs::write(&path, serde_json::to_string(&event).expect("case JSON")).expect("write case");
    (directory, path, sentinel)
}

fn private_browser_fixture() -> (tempfile::TempDir, PathBuf, String) {
    let sentinel = format!("PRIVACY_CORPUS_SENTINEL_{PRIVATE_BROWSER_CASE}");
    let mut event = browser_fixture_event();
    event["payload"]["data"]["private_context"] = json!(true);
    event["payload"]["data"]["private_marker"] = json!(sentinel);
    let directory = tempdir().expect("case tempdir");
    let path = directory.path().join("case.jsonl");
    fs::write(&path, serde_json::to_string(&event).expect("case JSON")).expect("write case");
    (directory, path, sentinel)
}

fn assert_redacted(case_id: &str, sentinel: &str, text: &str) {
    assert!(!text.contains(sentinel), "{case_id}");
}

fn exercise_case(case_id: &str, path: &Path, sentinel: &str) {
    let read_error = read_fixture(path).expect_err(case_id);
    assert_redacted(case_id, sentinel, &read_error.to_string());
    assert_redacted(case_id, sentinel, &format!("{read_error:?}"));

    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed(case_id)).expect(case_id);
    let ingest_error =
        ingest_fixture(path, &journal, &PolicyProfile::fixture_default()).expect_err(case_id);
    assert_redacted(case_id, sentinel, &ingest_error.to_string());
    assert_redacted(case_id, sentinel, &format!("{ingest_error:?}"));
    assert!(journal.events().expect(case_id).is_empty(), "{case_id}");

    let explanation_error = explain(
        &journal,
        Uuid::parse_str("00000000-0000-4000-8000-000000000002").expect("event ID"),
    )
    .expect_err(case_id);
    assert_redacted(case_id, sentinel, &explanation_error.to_string());
    assert_redacted(case_id, sentinel, &format!("{explanation_error:?}"));

    let output = path.parent().expect("case parent").join("export.jsonl");
    let export_error = export_fixture(path, &output, false).expect_err(case_id);
    assert_redacted(case_id, sentinel, &export_error.to_string());
    assert_redacted(case_id, sentinel, &format!("{export_error:?}"));
    assert!(!output.exists(), "{case_id}");

    for (command, output_path) in [
        ("demo", None),
        ("export", Some(path.parent().expect("case parent").join("cli-export.jsonl"))),
    ] {
        let mut cli = Command::new(env!("CARGO_BIN_EXE_ghostrace"));
        cli.arg(command).arg("--fixture").arg(path);
        if command == "demo" {
            cli.arg("--event").arg("00000000-0000-4000-8000-000000000002");
        } else {
            cli.arg("--output").arg(output_path.as_ref().expect("export output"));
        }
        let output = cli.output().expect(case_id);
        assert!(!output.status.success(), "{case_id}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_redacted(case_id, sentinel, &stdout);
        assert_redacted(case_id, sentinel, &stderr);
        if let Some(output_path) = output_path {
            assert!(!output_path.exists(), "{case_id}");
        }
    }
}

#[test]
fn prohibited_data_corpus_is_rejected_without_retention_or_echo() {
    let manifest: CorpusManifest = serde_json::from_str(CORPUS_MANIFEST).expect("manifest");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.cases.len(), 7);

    for case in manifest.cases {
        if case.mode == "private_browser" {
            assert_eq!(case.id, PRIVATE_BROWSER_CASE);
            let (_directory, path, sentinel) = private_browser_fixture();
            exercise_case(&case.id, &path, &sentinel);
        } else {
            assert_eq!(case.mode, "unknown_field", "{}", case.id);
            let (_directory, path, sentinel) = malicious_fixture(&case.id, &case.field);
            exercise_case(&case.id, &path, &sentinel);
        }
    }
}
