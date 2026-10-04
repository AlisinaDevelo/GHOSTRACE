//! Adversarial regression coverage for the v2 authenticated-state boundary.
//!
//! These tests deliberately mutate a fixture SQLite file through an unkeyed
//! connection.  The mutation side must never receive or derive the journal
//! key; the public `Journal` handle is used only to exercise the normal writer
//! and full-report paths.
//!
//! A whole-database rollback to an earlier complete authenticated snapshot is
//! intentionally outside these assertions: the local-key-only contract has no
//! external monotonic witness with which to reject that rollback.

use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use ghostrace::{
    AuthenticatedAnomaly, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, IngestionOrigin, Journal, PolicyProfile, ReasonCode,
    SourceCursor, AUTHENTICATED_STATE_V2_DOMAIN,
};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn private_path(name: &str) -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join(name);
    (directory, path)
}

fn policy() -> PolicyProfile {
    let mut profile = PolicyProfile::deny_by_default("adversarial-authentication-policy");
    profile.enable_source(EventSource::Filesystem);
    profile
}

fn fixture_origin() -> IngestionOrigin {
    IngestionOrigin::fixture_instance("fixture-adversarial-authentication").expect("fixture origin")
}

fn event(origin: &IngestionOrigin, id: u128) -> EventEnvelope {
    let timestamp =
        Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("event timestamp");
    EventEnvelope::new(
        origin,
        uuid::Uuid::from_u128(id),
        timestamp,
        timestamp,
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(ghostrace::GapPayload {
            source: EventSource::Filesystem,
            reason_code: ReasonCode::try_from("adversarial_auth").expect("reason code"),
            dropped_count: id as u64,
            from_cursor: None,
            to_cursor: None,
            volume_digest: None,
            root_ids: Vec::new(),
            remediation: None,
        }),
        Some(SourceCursor::try_from(format!("seq-0-{id}")).expect("cursor")),
        "adversarial-authentication-policy",
        1,
        Evidence::Unknown,
        None,
    )
    .expect("event")
}

fn open(path: &Path) -> Journal {
    Journal::open_fixture(
        path,
        DeterministicKeyProvider::from_seed("adversarial-authentication-test"),
    )
    .expect("fixture journal")
}

fn trigger_sql(connection: &Connection, name: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
            params![name],
            |row| row.get(0),
        )
        .expect("trigger definition")
}

fn restore_trigger(connection: &Connection, sql: &str) {
    connection.execute_batch(sql).expect("restore exact trigger");
}

fn canonical_fields(fields: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for (label, value) in fields {
        put_field(&mut bytes, label, value);
    }
    bytes
}

fn put_field(output: &mut Vec<u8>, label: &str, value: &[u8]) {
    output.extend_from_slice(&(label.len() as u32).to_le_bytes());
    output.extend_from_slice(label.as_bytes());
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}

fn sha_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::from("sha256:");
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn diagnostic_commitment_digest(connection: &Connection, diagnostic_id: i64) -> String {
    let (code, detail, created_at): (String, String, String) = connection
        .query_row(
            "SELECT code, detail, created_at FROM diagnostics WHERE diagnostic_id = ?1",
            params![diagnostic_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("diagnostic row");
    let mut row = Vec::new();
    put_field(&mut row, "id", &diagnostic_id.to_le_bytes());
    put_field(&mut row, "code", code.as_bytes());
    put_field(&mut row, "detail", detail.as_bytes());
    put_field(&mut row, "created", created_at.as_bytes());
    let canonical = canonical_fields(&[("row", &row)]);
    sha_digest(&canonical_fields(&[
        ("domain", AUTHENTICATED_STATE_V2_DOMAIN.as_bytes()),
        ("table", b"diagnostics"),
        ("row", &canonical),
    ]))
}

fn changed_operation_row(connection: &Connection) -> (i64, i64, String) {
    connection
        .query_row(
            "SELECT operation_seq, change_index, row_key_digest
             FROM authenticated_operation_changes
             WHERE operation_seq > 0
             ORDER BY operation_seq, change_index
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("post-bootstrap operation change")
}

fn operation_with_multiple_changes(connection: &Connection) -> (i64, i64, i64) {
    connection
        .query_row(
            "SELECT operation_seq, MIN(change_index), MAX(change_index)
             FROM authenticated_operation_changes
             WHERE operation_seq > 0
             GROUP BY operation_seq
             HAVING COUNT(*) >= 2
             ORDER BY operation_seq
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("operation with multiple changes")
}

fn assert_invalid_report(journal: &Journal, context: &str) {
    let report = journal.authenticated_state_report().expect("full report");
    assert!(!report.valid, "{context}: tampering was accepted: {report:?}");
}

fn seed_operation_history(path: &Path) -> Journal {
    let journal = open(path);
    let origin = fixture_origin();
    journal.ingest(&origin, &event(&origin, 11), &policy()).expect("first event");
    journal
        .ingest_batch(&origin, &[event(&origin, 12), event(&origin, 13)], &policy())
        .expect("second event batch");
    journal
}

#[test]
fn semantically_wrong_events_insert_trigger_refuses_writer_before_commit() {
    let (_directory, path) = private_path("wrong-trigger.sqlite3");
    let journal = open(&path);
    let origin = fixture_origin();
    let initial = event(&origin, 1);
    journal.ingest(&origin, &initial, &policy()).expect("initial event");

    let mutation = Connection::open(&path).expect("unkeyed mutation connection");
    let original_trigger = trigger_sql(&mutation, "authenticated_events_ai");
    mutation
        .execute_batch(
            "DROP TRIGGER authenticated_events_ai;
             CREATE TRIGGER authenticated_events_ai
             AFTER INSERT ON events
             BEGIN
                 INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
                 VALUES ('events', CAST(NEW.ingest_seq + 1000000 AS TEXT), 'insert');
             END;",
        )
        .expect("install semantically wrong trigger");

    let result = journal.ingest(&origin, &event(&origin, 2), &policy());

    mutation.execute_batch("DROP TRIGGER authenticated_events_ai").expect("drop wrong trigger");
    restore_trigger(&mutation, &original_trigger);
    drop(mutation);

    assert!(
        result.is_err(),
        "writer acknowledged a row whose pending trigger targeted a nonexistent key"
    );
    let check = Connection::open(&path).expect("check connection");
    let event_count: i64 =
        check.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0)).expect("event count");
    let pending_count: i64 = check
        .query_row("SELECT COUNT(*) FROM authenticated_pending_changes", [], |row| row.get(0))
        .expect("pending count");
    assert_eq!(event_count, 1, "failed authentication must roll back the new event");
    assert_eq!(pending_count, 0, "failed authentication must roll back pending work");
    assert!(journal.authenticated_state_report().expect("restored report").valid);
}

#[test]
fn an_extra_pending_row_deletion_trigger_refuses_the_writer() {
    let (_directory, path) = private_path("extra-trigger.sqlite3");
    let journal = open(&path);
    let origin = fixture_origin();
    journal.ingest(&origin, &event(&origin, 1), &policy()).expect("initial event");
    let initial_head = journal.authenticated_state().expect("initial anchor").head_mac;
    let mutation = Connection::open(&path).expect("unkeyed mutation connection");
    mutation
        .execute_batch(
            "CREATE TRIGGER adversarial_discard_change
             AFTER INSERT ON authenticated_pending_changes
             BEGIN
                 DELETE FROM authenticated_pending_changes WHERE change_id = NEW.change_id;
             END;",
        )
        .expect("install extra pending-row deletion trigger");

    let result = journal.ingest(&origin, &event(&origin, 2), &policy());
    assert!(result.is_err(), "an unapproved trigger must refuse before any writer acknowledgement");
    let count: i64 = mutation
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .expect("event count");
    assert_eq!(count, 1, "unrecorded event must not commit");
    assert_eq!(journal.authenticated_state().expect("unchanged anchor").head_mac, initial_head);
}

#[test]
fn in_memory_read_snapshot_refuses_sql_mutation_and_preserves_writes() {
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("readonly-authentication"))
            .expect("in-memory journal");
    let origin = fixture_origin();
    journal.ingest(&origin, &event(&origin, 1), &policy()).expect("initial event");
    let initial_head = journal.authenticated_state().expect("initial anchor").head_mac;
    let result = journal.with_read_snapshot(|connection| {
        connection.execute("DELETE FROM authenticated_state", [])?;
        Ok(())
    });
    assert!(result.is_err(), "the documented read-only callback must reject ordinary SQL writes");
    assert_eq!(journal.authenticated_state().expect("preserved anchor").head_mac, initial_head);
    journal.ingest(&origin, &event(&origin, 2), &policy()).expect("writer remains usable");
    assert!(journal.authenticated_state_report().expect("full report").valid);
}

#[test]
fn rewritten_live_row_with_matching_side_commitment_fails_full_report() {
    let (_directory, path) = private_path("matching-commitment.sqlite3");
    let journal = open(&path);
    let origin = fixture_origin();
    let diagnostic =
        DiagnosticRecord::new("adversarial.status", "original-detail").expect("diagnostic");
    journal
        .ingest_batch_with_diagnostics(&origin, &[], &policy(), &[diagnostic])
        .expect("diagnostic seed");

    let mut mutation = Connection::open(&path).expect("unkeyed mutation connection");
    let original_trigger = trigger_sql(&mutation, "authenticated_diagnostics_au");
    let diagnostic_id: i64 = mutation
        .query_row(
            "SELECT diagnostic_id FROM diagnostics ORDER BY diagnostic_id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("diagnostic id");
    let row_key_digest: String = mutation
        .query_row(
            "SELECT row_key_digest FROM authenticated_commitments
             WHERE table_name = 'diagnostics' AND row_locator = ''",
            [],
            |row| row.get(0),
        )
        .expect("diagnostic commitment key");
    let transaction = mutation.transaction().expect("mutation transaction");
    transaction
        .execute_batch("DROP TRIGGER authenticated_diagnostics_au")
        .expect("disable diagnostic pending trigger");
    transaction
        .execute(
            "UPDATE diagnostics SET detail = ?1 WHERE diagnostic_id = ?2",
            params!["rewritten-without-journal-key", diagnostic_id],
        )
        .expect("rewrite diagnostic");
    let replacement_digest = diagnostic_commitment_digest(&transaction, diagnostic_id);
    transaction
        .execute(
            "UPDATE authenticated_commitments
             SET row_digest = ?1
             WHERE table_name = 'diagnostics'
               AND row_key_digest = ?2",
            params![replacement_digest, row_key_digest],
        )
        .expect("rewrite matching side commitment");
    restore_trigger(&transaction, &original_trigger);
    transaction.commit().expect("commit unkeyed rewrite");
    drop(mutation);

    let report = journal.authenticated_state_report().expect("full report");
    assert!(
        !report.valid,
        "full authentication accepted a live-row rewrite paired with its side commitment: {report:?}"
    );
    assert!(
        report.anomalies.contains(&AuthenticatedAnomaly::DiagnosticTampering)
            || report.anomalies.contains(&AuthenticatedAnomaly::AnchorInvalid),
        "tampering must be classified as an authentication failure: {report:?}"
    );
}

#[test]
fn authenticated_operation_change_corruption_fails_full_report() {
    let (_directory, path) = private_path("operation-change-corruption.sqlite3");
    let journal = seed_operation_history(&path);
    let connection = Connection::open(&path).expect("unkeyed mutation connection");
    let (operation_seq, change_index, original_key) = changed_operation_row(&connection);
    let replacement_key = if original_key
        == "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    {
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    } else {
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    };
    connection
        .execute(
            "UPDATE authenticated_operation_changes
             SET row_key_digest = ?1
             WHERE operation_seq = ?2 AND change_index = ?3",
            params![replacement_key, operation_seq, change_index],
        )
        .expect("corrupt operation change");
    assert_invalid_report(&journal, "operation-change corruption");
}

#[test]
fn authenticated_operation_change_reordering_fails_full_report() {
    let (_directory, path) = private_path("operation-change-order.sqlite3");
    let journal = seed_operation_history(&path);
    let mut connection = Connection::open(&path).expect("unkeyed mutation connection");
    let (operation_seq, first_index, last_index) = operation_with_multiple_changes(&connection);
    assert_ne!(first_index, last_index, "test requires distinct change indices");
    let transaction = connection.transaction().expect("reorder transaction");
    transaction
        .execute(
            "UPDATE authenticated_operation_changes
             SET change_index = -1
             WHERE operation_seq = ?1 AND change_index = ?2",
            params![operation_seq, first_index],
        )
        .expect("free first change index");
    transaction
        .execute(
            "UPDATE authenticated_operation_changes
             SET change_index = ?1
             WHERE operation_seq = ?2 AND change_index = ?3",
            params![first_index, operation_seq, last_index],
        )
        .expect("move last change index");
    transaction
        .execute(
            "UPDATE authenticated_operation_changes
             SET change_index = ?1
             WHERE operation_seq = ?2 AND change_index = -1",
            params![last_index, operation_seq],
        )
        .expect("move first change index");
    transaction.commit().expect("commit reordered changes");
    assert_invalid_report(&journal, "operation-change reorder");
}

#[test]
fn authenticated_operation_change_removal_fails_full_report() {
    let (_directory, path) = private_path("operation-change-removal.sqlite3");
    let journal = seed_operation_history(&path);
    let connection = Connection::open(&path).expect("unkeyed mutation connection");
    let (operation_seq, change_index, _original_key) = changed_operation_row(&connection);
    connection
        .execute(
            "DELETE FROM authenticated_operation_changes
             WHERE operation_seq = ?1 AND change_index = ?2",
            params![operation_seq, change_index],
        )
        .expect("remove operation change");
    assert_invalid_report(&journal, "operation-change removal");
}

#[test]
fn bootstrap_operation_map_tampering_fails_full_report() {
    let (_directory, path) = private_path("bootstrap-operation-map.sqlite3");
    let journal = open(&path);

    // Create data before the explicit first anchor so promotion must persist a
    // non-empty operation-0 map. This mutation is setup only and uses no key.
    let connection = Connection::open(&path).expect("unkeyed setup connection");
    connection
        .execute(
            "INSERT INTO diagnostics(code, detail, created_at)
             VALUES (?1, ?2, ?3)",
            params!["bootstrap.status", "pre-anchor", "1970-01-01T00:00:00Z"],
        )
        .expect("pre-anchor diagnostic");
    drop(connection);
    journal.initialize_authenticated_state().expect("promote pre-anchor map");
    assert_eq!(
        journal.authenticated_state().expect("promoted anchor").schema_version,
        2,
        "pre-anchor state must be promoted to v2 before the bootstrap-map check"
    );
    assert!(journal.authenticated_state_report().expect("promoted report").valid);

    let connection = Connection::open(&path).expect("unkeyed mutation connection");
    let (change_index, original_key): (i64, String) = connection
        .query_row(
            "SELECT change_index, row_key_digest
             FROM authenticated_operation_changes
             WHERE operation_seq = 0
             ORDER BY change_index
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("bootstrap operation map row");
    let replacement_key = if original_key
        == "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    {
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    } else {
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    };
    connection
        .execute(
            "UPDATE authenticated_operation_changes
             SET row_key_digest = ?1
             WHERE operation_seq = 0 AND change_index = ?2",
            params![replacement_key, change_index],
        )
        .expect("tamper bootstrap map");
    assert_invalid_report(&journal, "bootstrap map tampering");
}
