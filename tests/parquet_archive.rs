//! Parquet cold archive: explicit creation, 0600 atomic publish, lossless
//! round trip against the JSONL export, and detection of mismatches.

#![cfg(feature = "parquet")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use ghostrace::{
    parquet_archive::{
        archive_row, record_from_row, verify_parquet_archive, write_parquet_archive,
    },
    GhostraceError,
};
use serde_json::Value;

fn ghostrace(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ghostrace")).args(args).output().expect("ghostrace runs")
}

/// Export a checked-in fixture through the real preview/confirm flow.
fn export_fixture(fixture: &str, output: &Path) {
    let output = output.to_str().expect("utf8");
    let preview = ghostrace(&["preview", "--fixture", fixture, "--output", output]);
    assert!(preview.status.success());
    let preview: Value = serde_json::from_slice(&preview.stdout).expect("preview JSON");
    let export = ghostrace(&[
        "export",
        "--fixture",
        fixture,
        "--output",
        output,
        "--confirm-plan",
        preview["plan_digest"].as_str().expect("plan digest"),
        "--confirm-snapshot",
        preview["snapshot_digest"].as_str().expect("snapshot digest"),
    ]);
    assert!(export.status.success(), "{}", String::from_utf8_lossy(&export.stderr));
}

fn entries(directory: &Path) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(directory)
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn archive_round_trips_every_record_and_publishes_owner_only() {
    let directory = tempfile::tempdir().expect("tempdir");
    let export = directory.path().join("export.jsonl");
    let archive = directory.path().join("archive.parquet");
    export_fixture("fixtures/causal-chain.jsonl", &export);
    let before = fs::read(&export).expect("export bytes");

    let receipt = write_parquet_archive(&export, &archive).expect("archive");
    assert_eq!(receipt.row_count, 8);
    assert_eq!(receipt.profile_schema_id, "ghostrace.parquet-archive-profile");
    assert_eq!(fs::read(&export).expect("export bytes"), before, "the export is untouched");
    assert_eq!(&fs::read(&archive).expect("archive")[..4], b"PAR1");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&archive).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    assert_eq!(entries(directory.path()), [archive.clone(), export.clone()], "no temporary left");

    let verified = verify_parquet_archive(&archive, &export).expect("verify");
    assert_eq!(verified, receipt);

    // Every record rebuilds from its row, and gap rows carry gap columns.
    let text = fs::read_to_string(&export).expect("export text");
    let mut gaps = 0;
    for line in text.lines().skip(1) {
        let record: Value = serde_json::from_str(line).expect("record");
        let row = archive_row(&record).expect("row");
        assert_eq!(record_from_row(&row).expect("rebuilt"), record);
        if row["kind"] == "gap" {
            gaps += 1;
            assert_eq!(row["gap_reason_code"], "fixture_omitted_window");
            assert_eq!(row["gap_dropped_count"], 3);
            assert_eq!(row["gap_root_ids_json"], "[]");
            assert!(row["gap_remediation_json"].is_null());
        } else {
            assert!(row["gap_source"].is_null());
        }
    }
    assert_eq!(gaps, 1);
}

#[test]
fn archive_never_replaces_an_existing_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let export = directory.path().join("export.jsonl");
    let archive = directory.path().join("archive.parquet");
    export_fixture("fixtures/causal-chain.jsonl", &export);
    fs::write(&archive, b"keep me").expect("existing");
    assert!(matches!(
        write_parquet_archive(&export, &archive),
        Err(GhostraceError::ArchiveExists(_))
    ));
    assert_eq!(fs::read(&archive).expect("existing"), b"keep me");
    assert_eq!(entries(directory.path()).len(), 2);
}

#[test]
fn verification_rejects_another_export_and_a_damaged_archive() {
    let directory = tempfile::tempdir().expect("tempdir");
    let first = directory.path().join("first.jsonl");
    let second = directory.path().join("second.jsonl");
    let archive = directory.path().join("archive.parquet");
    export_fixture("fixtures/causal-chain.jsonl", &first);
    let altered = directory.path().join("altered.jsonl");
    let fixture = fs::read_to_string("fixtures/causal-chain.jsonl").expect("fixture");
    fs::write(&altered, fixture.replace("fixture-shell-1", "fixture-shell-2")).expect("altered");
    export_fixture(altered.to_str().expect("utf8"), &second);
    write_parquet_archive(&first, &archive).expect("archive");

    assert!(matches!(
        verify_parquet_archive(&archive, &second),
        Err(GhostraceError::ArchiveInvalid(_))
    ));

    let mut bytes = fs::read(&archive).expect("archive");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xff;
    let damaged = directory.path().join("damaged.parquet");
    fs::write(&damaged, bytes).expect("damaged");
    assert!(verify_parquet_archive(&damaged, &first).is_err());
}

#[test]
fn cli_requires_explicit_confirmation_and_warns_about_plaintext() {
    let directory = tempfile::tempdir().expect("tempdir");
    let export = directory.path().join("export.jsonl");
    let archive = directory.path().join("archive.parquet");
    export_fixture("fixtures/causal-chain.jsonl", &export);
    let args =
        ["archive", "--export", export.to_str().unwrap(), "--output", archive.to_str().unwrap()];

    let refused = ghostrace(&args);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("unencrypted copy"));
    assert!(!archive.exists());

    let written = ghostrace(&[&args[..], &["--yes"]].concat());
    assert!(written.status.success(), "{}", String::from_utf8_lossy(&written.stderr));
    let receipt: Value = serde_json::from_slice(&written.stdout).expect("receipt");
    assert_eq!(receipt["row_count"], 8);

    let verified = ghostrace(&[
        "verify-archive",
        "--archive",
        archive.to_str().unwrap(),
        "--export",
        export.to_str().unwrap(),
    ]);
    assert!(verified.status.success());
}
