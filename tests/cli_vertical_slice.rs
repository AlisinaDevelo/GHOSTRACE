use std::{ffi::OsStr, fs, process::Command};

use serde_json::Value;
use tempfile::tempdir;

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn run(args: &[&OsStr]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ghostrace"));
    for arg in args {
        command.arg(arg);
    }
    command.output().expect("run ghostrace")
}

fn assert_success(output: &std::process::Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn durable_fixture_cli_path_is_reopenable_deterministic_and_capture_disabled() {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let journal = directory.path().join("journal.sqlite3");
    let export = directory.path().join("export.jsonl");
    let fixture = fixture_path();
    let event = "00000000-0000-4000-8000-000000000008";

    let init = run(&[OsStr::new("init"), OsStr::new("--journal"), journal.as_os_str()]);
    assert_success(&init, "init");
    assert!(String::from_utf8_lossy(&init.stdout).contains("initialized"));
    assert!(journal.is_file());

    let repeat_init = run(&[OsStr::new("init"), OsStr::new("--journal"), journal.as_os_str()]);
    assert_success(&repeat_init, "repeat init");

    let ingest = run(&[
        OsStr::new("ingest"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--fixture"),
        fixture.as_os_str(),
    ]);
    assert_success(&ingest, "ingest");
    assert!(String::from_utf8_lossy(&ingest.stdout).contains("ingested 8 event(s)"));

    let first_explain = run(&[
        OsStr::new("explain"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--event"),
        OsStr::new(event),
    ]);
    assert_success(&first_explain, "first explain");
    let first_json: Value =
        serde_json::from_slice(&first_explain.stdout).expect("explanation JSON");
    assert_eq!(first_json["target_event_id"], event);
    assert_eq!(first_json["chain_event_ids"].as_array().expect("chain").len(), 8);
    assert_eq!(first_json["coverage"]["gap_event_count"], 1);

    let second_explain = run(&[
        OsStr::new("explain"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--event"),
        OsStr::new(event),
    ]);
    assert_success(&second_explain, "second explain");
    assert_eq!(first_explain.stdout, second_explain.stdout);

    let preview = run(&[
        OsStr::new("preview"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--output"),
        export.as_os_str(),
    ]);
    assert_success(&preview, "preview export");
    let preview_json: Value = serde_json::from_slice(&preview.stdout).expect("preview JSON");
    let plan_digest = preview_json["plan_digest"].as_str().expect("plan digest");
    let snapshot_digest = preview_json["snapshot_digest"].as_str().expect("snapshot digest");

    let unconfirmed_output = directory.path().join("unconfirmed.jsonl");
    let unconfirmed = run(&[
        OsStr::new("export"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--output"),
        unconfirmed_output.as_os_str(),
    ]);
    assert!(!unconfirmed.status.success());
    assert!(
        String::from_utf8_lossy(&unconfirmed.stderr).contains("explicit plaintext confirmation")
    );
    assert!(!unconfirmed_output.exists());

    let export_result = run(&[
        OsStr::new("export"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--output"),
        export.as_os_str(),
        OsStr::new("--confirm-plan"),
        OsStr::new(plan_digest),
        OsStr::new("--confirm-snapshot"),
        OsStr::new(snapshot_digest),
    ]);
    assert_success(&export_result, "export");
    let records = fs::read_to_string(&export).expect("export file");
    let first_record: Value =
        serde_json::from_str(records.lines().next().expect("manifest")).expect("manifest JSON");
    assert_eq!(first_record["record_type"], "manifest");
    assert_eq!(first_record["export_version"], 1);
    assert_eq!(first_record["coverage"]["event_count"], 8);
    assert_eq!(first_record["coverage"]["gap_count"], 1);
    assert_eq!(records.lines().count(), 9);

    let validate = run(&[OsStr::new("validate"), OsStr::new("--export"), export.as_os_str()]);
    assert_success(&validate, "validate export");
    assert!(String::from_utf8_lossy(&validate.stdout).contains("validated 8 event(s)"));

    let retention = run(&[
        OsStr::new("retention-plan"),
        OsStr::new("--journal"),
        journal.as_os_str(),
        OsStr::new("--before"),
        OsStr::new("2026-01-01T00:00:08Z"),
        OsStr::new("--source"),
        OsStr::new("filesystem"),
        OsStr::new("--root-id"),
        OsStr::new("workspace-demo"),
    ]);
    assert_success(&retention, "retention plan");
    let retention_json: Value = serde_json::from_slice(&retention.stdout).expect("retention JSON");
    assert_eq!(retention_json["snapshot_event_count"], 8);
    assert_eq!(retention_json["affected_event_count"], 1);
    assert!(retention_json["candidate_set_digest"].as_str().is_some());
    assert!(retention_json["non_goals"].as_array().expect("non-goals").iter().any(|item| {
        item.as_str().is_some_and(|value| value.contains("legal holds are not implemented"))
    }));

    let residue =
        run(&[OsStr::new("residue-report"), OsStr::new("--journal"), journal.as_os_str()]);
    assert_success(&residue, "residue report");
    let residue_json: Value = serde_json::from_slice(&residue.stdout).expect("residue JSON");
    assert_eq!(residue_json["schema_version"], 1);
    assert_eq!(residue_json["modes"].as_array().expect("modes").len(), 4);
    assert_eq!(residue_json["external_backup_count"], 0);
    assert!(residue_json["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .any(|item| { item["kind"] == "database" && item["regular_file_count"] == 1 }));
    assert!(!String::from_utf8_lossy(&residue.stdout).contains("journal.sqlite3"));

    let capture = run(&[OsStr::new("capture")]);
    assert!(!capture.status.success());
    assert!(String::from_utf8_lossy(&capture.stderr).contains("intentionally disabled"));
}
