//! Schema and export compatibility matrix: every retained v1 format accepts
//! its golden artifact and refuses forward, backward, unknown-field, mixed,
//! corrupted, and partially migrated inputs, and a legacy journal survives the
//! full upgrade, query, explanation, export, verification, and deletion path.

use std::{fs, path::Path};

use chrono::{TimeZone, Utc};
use ghostrace::{
    explain, export_fixture, export_journal_with_confirmation, fixture::ingest_fixture,
    preview_export, validate_export, validate_shell_metadata, CollectorLifecyclePayload,
    DeterministicKeyProvider, EventEnvelope, EventKind, EventPayload, EventSource, Evidence,
    ExportRequest, GitSnapshotMetadata, IngestionOrigin, Journal, PolicyProfile, QueryRequest,
    RetentionPolicy, GIT_SNAPSHOT_GOLDEN_JSON, SHELL_METADATA_GOLDEN_JSON,
};
use serde_json::Value;
use uuid::Uuid;

const EVENT_GOLDEN: &str = include_str!("../fixtures/event-envelope-v1.golden.json");

fn private_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    directory
}

fn root(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// The version-, field-, and corruption-mutations applied to a JSON contract.
fn variants(golden: &str, version_field: &str) -> Vec<(&'static str, String)> {
    let base: Value = serde_json::from_str(golden).expect("golden JSON");
    let with = |mutate: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        mutate(&mut value);
        serde_json::to_string(&value).expect("JSON")
    };
    vec![
        ("forward", with(&|value| value[version_field] = Value::from(2))),
        ("backward", with(&|value| value[version_field] = Value::from(0))),
        ("unknown_field", with(&|value| value["future_field"] = Value::from(true))),
        ("corrupted", golden[..golden.len() / 2].to_owned()),
    ]
}

#[test]
fn event_envelope_v1_accepts_its_golden_and_refuses_every_other_variant() {
    assert!(serde_json::from_str::<EventEnvelope>(EVENT_GOLDEN).is_ok());
    for (case, input) in variants(EVENT_GOLDEN, "schema_version") {
        assert!(serde_json::from_str::<EventEnvelope>(&input).is_err(), "event {case} accepted");
    }
}

#[test]
fn git_snapshot_v1_accepts_its_golden_and_refuses_every_other_variant() {
    assert!(GitSnapshotMetadata::parse(GIT_SNAPSHOT_GOLDEN_JSON).is_ok());
    for (case, input) in variants(GIT_SNAPSHOT_GOLDEN_JSON, "schema_version") {
        assert!(GitSnapshotMetadata::parse(&input).is_err(), "git snapshot {case} accepted");
    }
}

#[test]
fn shell_metadata_v1_accepts_its_golden_and_refuses_every_other_variant() {
    assert!(validate_shell_metadata(SHELL_METADATA_GOLDEN_JSON).is_ok());
    for (case, input) in variants(SHELL_METADATA_GOLDEN_JSON, "schema_version") {
        assert!(validate_shell_metadata(&input).is_err(), "shell metadata {case} accepted");
    }
}

#[test]
fn export_stream_refuses_forward_backward_unknown_mixed_corrupted_and_truncated_input() {
    let directory = private_directory();
    let output = directory.path().join("export.jsonl");
    export_fixture(root("fixtures/causal-chain.jsonl"), &output, false).expect("export");
    validate_export(&output).expect("current export is accepted");
    let original = fs::read_to_string(&output).expect("export");
    let lines = original.lines().map(str::to_owned).collect::<Vec<_>>();
    let manifest: Value = serde_json::from_str(&lines[0]).expect("manifest");
    let version_field = ["schema_version", "manifest_version", "registry_version"]
        .into_iter()
        .find(|field| manifest.get(*field).is_some_and(Value::is_u64))
        .expect("manifest carries a numeric version");

    let rewrite = |index: usize, mutate: &dyn Fn(&mut Value)| {
        let mut changed = lines.clone();
        let mut value: Value = serde_json::from_str(&changed[index]).expect("line");
        mutate(&mut value);
        changed[index] = serde_json::to_string(&value).expect("line");
        format!("{}\n", changed.join("\n"))
    };
    let cases = [
        ("forward", rewrite(0, &|value| value[version_field] = Value::from(2))),
        ("backward", rewrite(0, &|value| value[version_field] = Value::from(0))),
        ("unknown_field", rewrite(1, &|value| value["future_field"] = Value::from(true))),
        (
            "mixed",
            rewrite(1, &|value| {
                let id = value["schema_id"].as_str().unwrap_or("ghostrace.event-envelope");
                value["schema_id"] = Value::from(format!("{id}-v2"));
            }),
        ),
        ("corrupted", original.replacen('{', "{\u{0}", 1)),
        ("truncated", format!("{}\n", lines[..lines.len() - 1].join("\n"))),
    ];
    for (case, content) in cases {
        fs::write(&output, content).expect("mutated export");
        assert!(validate_export(&output).is_err(), "export {case} accepted");
    }
}

#[test]
fn legacy_journal_survives_upgrade_query_explain_export_verify_and_delete() {
    let directory = private_directory();
    let path = directory.path().join("legacy.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("legacy database");
    connection.execute_batch(include_str!("../migrations/0001_init.sql")).expect("legacy schema");
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode");
    }
    let provider = || DeterministicKeyProvider::from_seed("compatibility-matrix");

    // Upgrade.
    let journal = Journal::open_fixture(&path, provider()).expect("upgrade legacy journal");
    assert_eq!(journal.schema_version().expect("schema"), 6);
    let policy = PolicyProfile::fixture_default();
    let report = ingest_fixture(root("fixtures/causal-chain.jsonl"), &journal, &policy)
        .expect("ingest golden fixture");
    assert_eq!(report.event_ids.len(), 8);

    // Query.
    let request = QueryRequest::for_policy(&policy).expect("query request");
    let page = journal.query_page(&request, None).expect("query");
    assert!(!page.events.is_empty());

    // Explanation.
    let target = Uuid::parse_str("00000000-0000-4000-8000-000000000008").expect("uuid");
    let explanation = explain(&journal, target).expect("explain");
    assert_eq!(explanation.chain_event_ids.len(), 8);

    // Export.
    let output = directory.path().join("export.jsonl");
    let preview =
        preview_export(&journal, &policy, &ExportRequest::default(), &output).expect("preview");
    export_journal_with_confirmation(&journal, &output, preview.confirm(), &policy)
        .expect("export");
    validate_export(&output).expect("export validates");

    // Verification.
    assert!(journal.integrity_check().expect("integrity").integrity_ok);

    // Deletion. Every causal-chain event is a parent or a cursor tail, which
    // retention protects, so two independent lifecycle events give the
    // deletion path an eligible target: the older one, which is neither.
    let origin = IngestionOrigin::fixture_instance("fixture-compat").expect("origin");
    for (id, seconds, cursor) in [(0xc0, 100, "seq-0-1"), (0xc1, 300, "seq-0-2")] {
        let event = EventEnvelope::new(
            &origin,
            Uuid::from_u128(id),
            Utc.timestamp_opt(seconds, 0).single().expect("ts"),
            Utc.timestamp_opt(seconds, 0).single().expect("ts"),
            EventSource::Lifecycle,
            EventKind::CollectorStarted,
            EventPayload::CollectorStarted(CollectorLifecyclePayload {
                collector: EventSource::Lifecycle,
                instance_label: "compat".try_into().expect("label"),
            }),
            Some(cursor.try_into().expect("cursor")),
            policy.id.clone(),
            policy.version,
            Evidence::Direct,
            None,
        )
        .expect("event");
        journal.ingest(&origin, &event, &policy).expect("ingest lifecycle event");
    }
    let plan = journal
        .retention_plan(&RetentionPolicy::before(Utc.timestamp_opt(250, 0).single().expect("ts")))
        .expect("retention plan");
    let receipt = journal.delete_retention(&plan, &plan.confirm()).expect("retention delete");
    assert!(receipt.deleted_event_count > 0);
    assert!(journal.integrity_check().expect("integrity after deletion").integrity_ok);
    journal.shutdown().expect("shutdown");

    let reopened = Journal::open_fixture(&path, provider()).expect("reopen after lifecycle");
    assert!(reopened.integrity_check().expect("integrity after reopen").integrity_ok);
}

#[test]
fn partially_migrated_and_future_journals_are_refused() {
    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    drop(Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("compat")).expect("new"));
    let connection = rusqlite::Connection::open(&path).expect("inspect");
    connection
        .execute_batch("DELETE FROM migration_records WHERE version = 3;")
        .expect("partial migration");
    drop(connection);
    assert!(Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("compat")).is_err());

    let path = directory.path().join("future.sqlite3");
    drop(Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("compat")).expect("new"));
    let connection = rusqlite::Connection::open(&path).expect("inspect");
    connection
        .execute_batch(
            "INSERT INTO migration_records(migration_id, version, checksum, schema_version, tool_version, applied_at)
             VALUES ('0099_future', 99, 'future', 99, 'future-tool', '2026-01-01T00:00:00Z');",
        )
        .expect("future migration");
    drop(connection);
    assert!(Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("compat")).is_err());
}
