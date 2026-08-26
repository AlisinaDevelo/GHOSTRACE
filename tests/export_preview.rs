use std::{fs, path::PathBuf};

use chrono::Duration;
use ghostrace::{
    export_journal_with_confirmation, ingest_fixture, preview_export, read_fixture,
    DeterministicKeyProvider, EventEnvelope, EventSource, ExportRequest, ExportResult,
    GhostraceError, IngestionOrigin, Journal, PolicyProfile, SourceCursor,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use uuid::Uuid;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn journal(seed: &str) -> (Journal, PolicyProfile) {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed(seed)).expect("journal");
    let policy = PolicyProfile::fixture_default();
    ingest_fixture(fixture(), &journal, &policy).expect("ingest");
    (journal, policy)
}

#[test]
fn preview_and_confirmed_execution_share_digests_and_hide_the_destination_path() {
    let (journal, policy) = journal("0085-preview");
    let directory = tempdir().expect("directory");
    let output = directory.path().join("authorized-export.jsonl");
    let preview =
        preview_export(&journal, &policy, &ExportRequest::default(), &output).expect("preview");

    assert_eq!(preview.event_count, 8);
    assert_eq!(preview.sources.len(), 5);
    assert_eq!(preview.policy_profiles.len(), 1);
    assert_eq!(preview.gaps.len(), 1);
    assert_eq!(preview.plan.redaction.fields().len(), 12);
    assert!(preview.warning().contains("plaintext metadata"));
    let rendered = serde_json::to_string(&preview).expect("preview JSON");
    assert!(!rendered.contains(&output.to_string_lossy().to_string()));
    assert!(!rendered.contains("/private/tmp"));

    let confirmation = preview.confirm();
    let expected_plan_digest = confirmation.plan_digest().clone();
    let expected_snapshot_digest = confirmation.snapshot_digest().clone();
    let ExportResult { manifest, receipt } =
        export_journal_with_confirmation(&journal, &output, confirmation, &policy)
            .expect("confirmed export");

    assert_eq!(receipt.plan_digest, expected_plan_digest);
    assert_eq!(receipt.snapshot_digest, expected_snapshot_digest);
    assert_eq!(receipt.event_count, 8);
    assert_eq!(receipt.destination_class, ghostrace::ExportDestinationClass::NewFile);
    assert_eq!(manifest.coverage.event_count, 8);
    let export_bytes = fs::read(&output).expect("export");
    let first_line_end = export_bytes.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    let first_line = &export_bytes[..first_line_end];
    let manifest_digest =
        Sha256::digest(first_line).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let manifest_digest = format!("sha256:{manifest_digest}");
    let receipt_json = serde_json::to_string(&receipt).expect("receipt JSON");
    assert!(!receipt_json.contains(&output.to_string_lossy().to_string()));
    assert!(receipt_json.contains("manifest_digest"));
    assert_eq!(receipt.manifest_digest.as_str(), manifest_digest);
    assert_eq!(first_line.first(), Some(&b'{'));
}

#[test]
fn filtered_query_is_bound_in_the_plan_and_manifest() {
    let (journal, policy) = journal("0085-filter");
    let directory = tempdir().expect("directory");
    let output = directory.path().join("git-export.jsonl");
    let request = ExportRequest {
        query: ghostrace::ExportQuery { source: Some(EventSource::Git), ..Default::default() },
        ..Default::default()
    };
    let preview = preview_export(&journal, &policy, &request, &output).expect("preview");
    assert_eq!(preview.event_count, 1);
    assert_eq!(preview.sources, vec![EventSource::Git]);
    let result = export_journal_with_confirmation(&journal, &output, preview.confirm(), &policy)
        .expect("filtered export");
    assert_eq!(result.manifest.query_scope.kind, "filtered");
    assert_eq!(result.manifest.query_scope.source, Some(EventSource::Git));
    assert_eq!(result.manifest.coverage.event_count, 1);
}

#[test]
fn policy_change_after_preview_requires_reconfirmation_without_publishing() {
    let (journal, policy) = journal("0085-policy-change");
    let directory = tempdir().expect("directory");
    let output = directory.path().join("stale-policy.jsonl");
    let preview =
        preview_export(&journal, &policy, &ExportRequest::default(), &output).expect("preview");
    let mut changed = policy.clone();
    changed.version += 1;
    let error = export_journal_with_confirmation(&journal, &output, preview.confirm(), &changed)
        .expect_err("stale policy must be rejected");
    assert!(matches!(error, GhostraceError::ExportConfirmationMismatch));
    assert!(!output.exists());
}

#[test]
fn journal_snapshot_change_after_preview_is_rejected_and_temporary_files_are_removed() {
    let (journal, policy) = journal("0085-snapshot-change");
    let directory = tempdir().expect("directory");
    let output = directory.path().join("stale-snapshot.jsonl");
    let preview =
        preview_export(&journal, &policy, &ExportRequest::default(), &output).expect("preview");
    let template = read_fixture(fixture()).expect("fixture").remove(0);
    let source_cursor =
        template.source_cursor().map(|value| SourceCursor::try_from(value.to_owned()).unwrap());
    let extra_origin = IngestionOrigin::fixture_instance("fixture-extra").expect("origin");
    let extra = EventEnvelope::new(
        &extra_origin,
        Uuid::new_v4(),
        template.observed_at + Duration::seconds(30),
        template.ingested_at + Duration::seconds(30),
        template.source,
        template.kind,
        template.payload.clone(),
        source_cursor,
        policy.id.clone(),
        policy.version,
        template.evidence,
        None,
    )
    .expect("extra event");
    journal.ingest(&extra_origin, &extra, &policy).expect("extra event ingest");

    let error = export_journal_with_confirmation(&journal, &output, preview.confirm(), &policy)
        .expect_err("stale snapshot must be rejected");
    assert!(matches!(error, GhostraceError::ExportSnapshotChanged));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).expect("directory").count(), 0);
}

#[test]
fn preview_and_receipt_are_safe_to_print_as_diagnostics() {
    let (journal, policy) = journal("0085-diagnostics");
    let directory = tempdir().expect("directory");
    let output = directory.path().join("diagnostic-export.jsonl");
    let preview =
        preview_export(&journal, &policy, &ExportRequest::default(), &output).expect("preview");
    let rendered: Value = serde_json::to_value(&preview).expect("preview value");
    assert!(rendered.get("plaintext_warning").is_some());
    assert!(rendered.get("plan_digest").is_some());
    assert!(rendered.get("snapshot_digest").is_some());
    assert!(rendered.get("destination_path").is_none());
}
