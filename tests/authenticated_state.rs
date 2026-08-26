use std::{fs, path::Path, process::Command};

use chrono::{TimeZone, Utc};
use ghostrace::{
    AuthenticatedAnomaly, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, GhostraceError, IngestionOrigin, Journal, PolicyProfile,
    ReasonCode, RetentionPolicy, SourceCursor,
};
use rusqlite::Connection;
use tempfile::tempdir;
use uuid::Uuid;

fn private_path(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempdir().expect("temporary directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join(name);
    (directory, path)
}

fn policy() -> PolicyProfile {
    let mut profile = PolicyProfile::deny_by_default("authenticated-test-policy");
    profile.enable_source(EventSource::Filesystem);
    profile
}

fn event(id: u128, cursor: &str) -> EventEnvelope {
    let origin =
        IngestionOrigin::fixture_instance("fixture-authenticated-test-source").expect("origin");
    let timestamp = Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        timestamp,
        timestamp,
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(ghostrace::GapPayload {
            source: EventSource::Filesystem,
            reason_code: ReasonCode::try_from("authenticated_test").expect("reason"),
            dropped_count: id as u64,
            from_cursor: None,
            to_cursor: None,
            volume_digest: None,
            root_ids: Vec::new(),
            remediation: None,
        }),
        Some(SourceCursor::try_from(cursor).expect("cursor")),
        "authenticated-test-policy",
        1,
        Evidence::Unknown,
        None,
    )
    .expect("event")
}

fn open(path: &Path) -> Journal {
    Journal::open_fixture(path, DeterministicKeyProvider::from_seed("authenticated-state"))
        .expect("journal")
}

fn assert_anomaly(journal: &Journal, expected: AuthenticatedAnomaly) {
    let report = journal.authenticated_state_report().expect("auth report");
    assert!(!report.valid, "tamper must fail: {report:?}");
    assert!(report.anomalies.contains(&expected), "{expected:?}: {report:?}");
    assert!(report.local_key_only);
    assert!(report.origin_authenticity_limit().contains("local journal key"));
}

#[test]
fn fresh_ingest_and_control_state_have_a_valid_local_anchor() {
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("auth-memory")).expect("journal");
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("ingest");
    let report = journal.verify_authenticated_state().expect("valid auth state");
    assert!(report.valid);
    assert_eq!(report.event_count, 1);
    assert_eq!(report.stored_event_count, 1);
    let state = journal.authenticated_state().expect("anchor");
    assert_eq!(state.schema_version, 1);
    assert_eq!(state.chain_epoch, 0);
    assert_eq!(state.deletion_count, 0);
}

#[test]
fn edits_insertions_deletions_reorder_and_truncation_are_detected() {
    let cases = [
        (
            "edit",
            "UPDATE events SET evidence = 'indirect' WHERE ingest_seq = 1",
            AuthenticatedAnomaly::EventEdited,
        ),
        (
            "insert",
            "INSERT INTO events(event_id, schema_version, observed_at, ingested_at, source, kind, collector_instance, source_cursor, provenance_version, policy_profile_id, policy_profile_version, evidence, parent_event_id, payload_ciphertext) SELECT '00000000-0000-4000-8000-000000000099', schema_version, observed_at, ingested_at, source, kind, collector_instance, 'seq-0-99', provenance_version, policy_profile_id, policy_profile_version, evidence, parent_event_id, payload_ciphertext FROM events WHERE ingest_seq = 1",
            AuthenticatedAnomaly::EventInserted,
        ),
        (
            "delete",
            "DELETE FROM events WHERE ingest_seq = 1",
            AuthenticatedAnomaly::EventDeleted,
        ),
        (
            "reorder",
            "UPDATE events SET ingest_seq = ingest_seq + 100 WHERE ingest_seq IN (1, 2); UPDATE events SET ingest_seq = CASE ingest_seq WHEN 101 THEN 2 WHEN 102 THEN 1 ELSE ingest_seq END WHERE ingest_seq IN (101, 102)",
            AuthenticatedAnomaly::EventReordered,
        ),
    ];
    for (name, sql, expected) in cases {
        let (_directory, path) = private_path(&format!("{name}.sqlite3"));
        let journal = open(&path);
        journal
            .ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy())
            .expect("first");
        journal
            .ingest(&IngestionOrigin::fixture(), &event(2, "seq-0-2"), &policy())
            .expect("second");
        let mutation = if sql.starts_with("DELETE") {
            "UPDATE cursors SET last_event_id = NULL; DELETE FROM events WHERE ingest_seq = 1"
        } else {
            sql
        };
        Connection::open(&path)
            .expect("mutation connection")
            .execute_batch(mutation)
            .expect("tamper");
        assert_anomaly(&journal, expected);
    }

    let (_directory, path) = private_path("truncate.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    journal.ingest(&IngestionOrigin::fixture(), &event(2, "seq-0-2"), &policy()).expect("second");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch(
            "UPDATE cursors SET last_event_id = NULL; DELETE FROM events WHERE ingest_seq = 2",
        )
        .expect("truncate");
    assert_anomaly(&journal, AuthenticatedAnomaly::ChainTruncated);
}

#[test]
fn cursor_policy_and_diagnostic_substitution_are_detected() {
    let (_directory, path) = private_path("metadata.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch("UPDATE cursors SET source_cursor = 'seq-0-0'")
        .expect("cursor rollback");
    assert_anomaly(&journal, AuthenticatedAnomaly::CursorRollback);

    let (_directory, path) = private_path("policy.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch("UPDATE policy_metadata SET profile_json = '{\"substituted\":true}'")
        .expect("policy substitution");
    assert_anomaly(&journal, AuthenticatedAnomaly::PolicySubstitution);

    let (_directory, path) = private_path("diagnostic.sqlite3");
    let journal = open(&path);
    journal
        .ingest_batch_with_diagnostics(
            &IngestionOrigin::fixture(),
            &[event(1, "seq-0-1")],
            &policy(),
            &[DiagnosticRecord::new("auth.test", "bounded").expect("diagnostic")],
        )
        .expect("diagnostic ingest");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch("UPDATE diagnostics SET detail = 'substituted'")
        .expect("diagnostic substitution");
    assert_anomaly(&journal, AuthenticatedAnomaly::DiagnosticTampering);
}

#[test]
fn retention_uses_an_authenticated_deletion_boundary() {
    let (_directory, path) = private_path("deletion.sqlite3");
    let journal = open(&path);
    let mut first = event(10, "seq-0-1");
    first.source_cursor = None;
    let mut second = event(11, "seq-0-2");
    second.source_cursor = None;
    journal.ingest(&IngestionOrigin::fixture(), &first, &policy()).expect("first");
    journal.ingest(&IngestionOrigin::fixture(), &second, &policy()).expect("second");
    let before = Utc.timestamp_opt(1_735_689_611, 0).single().expect("time");
    let retention_policy =
        RetentionPolicy { preserve_gaps: false, ..RetentionPolicy::before(before) };
    let plan = journal.retention_plan(&retention_policy).expect("plan");
    let receipt = journal.delete_retention(&plan, &plan.confirm()).expect("delete");
    assert!(receipt.deleted_event_count > 0);
    let report = journal.verify_authenticated_state().expect("valid deletion boundary");
    assert_eq!(report.deletion_count, 1);
    assert_eq!(report.event_count, receipt.remaining_event_count);
}

#[test]
fn deleting_the_anchor_is_not_reseeded_on_reopen() {
    let (_directory, path) = private_path("anchor.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch("DELETE FROM authenticated_state")
        .expect("anchor deletion");
    let report = journal.authenticated_state_report().expect("report");
    assert!(report.anomalies.contains(&AuthenticatedAnomaly::AnchorMissing));
    let reopened =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("authenticated-state"))
            .expect("open preserves missing-anchor reportability");
    assert!(matches!(
        reopened.verify_authenticated_state(),
        Err(GhostraceError::AuthenticatedStateInvalid(_))
    ));
    assert!(matches!(
        reopened.ingest(&IngestionOrigin::fixture(), &event(2, "seq-0-2"), &policy()),
        Err(GhostraceError::AuthenticatedStateInvalid(_))
    ));
}

#[test]
fn cli_authentication_check_is_json_and_fails_closed_on_tamper() {
    let (_directory, path) = private_path("cli.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("fixture-cli-v1"))
            .expect("journal");
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    drop(journal);
    let binary = env!("CARGO_BIN_EXE_ghostrace");
    let output = Command::new(binary)
        .args(["authenticated-check", "--journal"])
        .arg(&path)
        .output()
        .expect("run auth check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("report JSON");
    assert_eq!(json["valid"], true);
    assert_eq!(json["local_key_only"], true);
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch("UPDATE events SET evidence = 'indirect' WHERE ingest_seq = 1")
        .expect("tamper");
    let output = Command::new(binary)
        .args(["authenticated-check", "--journal"])
        .arg(&path)
        .output()
        .expect("run tampered auth check");
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("tamper JSON");
    assert_eq!(json["valid"], false);
    assert!(json["anomalies"]
        .as_array()
        .expect("anomalies")
        .iter()
        .any(|value| { value == "event_edited" || value == "anchor_invalid" }));
    let error = open(&path).verify_authenticated_state().expect_err("must fail closed");
    assert!(matches!(error, GhostraceError::AuthenticatedStateInvalid(_)));
}
