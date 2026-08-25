use std::{fs, path::PathBuf};

use ghostrace::{
    export_journal_with_options, ingest_fixture, validate_export, DeterministicKeyProvider,
    ExportCancellation, ExportOptions, GhostraceError, Journal, PolicyProfile,
    MAX_EXPORT_RECORD_BYTES,
};
use tempfile::tempdir;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn journal() -> Journal {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("0084-export-stream"))
        .expect("journal");
    ingest_fixture(fixture(), &journal, &PolicyProfile::fixture_default()).expect("ingest");
    journal
}

#[test]
fn cancelled_export_never_presents_a_destination() {
    let directory = tempdir().expect("directory");
    let output = directory.path().join("cancelled.jsonl");
    let cancellation = ExportCancellation::new();
    cancellation.cancel();
    let error = export_journal_with_options(
        &journal(),
        &output,
        ExportOptions { force: false, cancellation: Some(cancellation) },
    )
    .expect_err("cancelled export");
    assert!(matches!(error, GhostraceError::ExportCancelled));
    assert!(!output.exists(), "cancelled export must not publish a destination");
}

#[test]
fn cancellation_preserves_an_existing_destination_during_forced_export() {
    let directory = tempdir().expect("directory");
    let output = directory.path().join("existing.jsonl");
    fs::write(&output, b"previous complete export\n").expect("seed destination");
    let cancellation = ExportCancellation::new();
    cancellation.cancel();
    let error = export_journal_with_options(
        &journal(),
        &output,
        ExportOptions { force: true, cancellation: Some(cancellation) },
    )
    .expect_err("cancelled export");
    assert!(matches!(error, GhostraceError::ExportCancelled));
    assert_eq!(fs::read(&output).expect("destination"), b"previous complete export\n");
}

#[test]
fn successful_streaming_export_remains_fully_validated() {
    let directory = tempdir().expect("directory");
    let output = directory.path().join("complete.jsonl");
    let manifest =
        export_journal_with_options(&journal(), &output, ExportOptions::default()).expect("export");
    let validation = validate_export(&output).expect("validation");
    assert_eq!(validation.manifest, manifest);
    assert_eq!(validation.event_count, 8);
}

#[test]
fn validator_rejects_an_unbounded_record_before_json_allocation() {
    let directory = tempdir().expect("directory");
    let output = directory.path().join("oversized.jsonl");
    fs::write(&output, vec![b'x'; MAX_EXPORT_RECORD_BYTES + 2]).expect("oversized line");
    let error = validate_export(&output).expect_err("oversized line");
    assert!(error.to_string().contains("bound"));
}
