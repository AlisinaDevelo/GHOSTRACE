use std::{fs, path::PathBuf};

use chrono::{TimeZone, Utc};
use ghostrace::{
    ingest_fixture, CollectorLifecyclePayload, DeterministicKeyProvider, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, GhostraceError, IngestionOrigin, Journal, PolicyProfile,
    QueryRequest,
};
use rusqlite::Connection;
use tempfile::tempdir;
use uuid::Uuid;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn private_tempdir() -> tempfile::TempDir {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    directory
}

fn lifecycle_event(id: u128, observed_seconds: i64, policy: &PolicyProfile) -> EventEnvelope {
    let origin =
        IngestionOrigin::fixture_instance("fixture-query-pagination").expect("fixture origin");
    let observed_at = Utc.timestamp_opt(observed_seconds, 0).single().expect("timestamp");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        observed_at,
        observed_at,
        EventSource::Lifecycle,
        EventKind::CollectorStarted,
        EventPayload::CollectorStarted(CollectorLifecyclePayload {
            collector: EventSource::Lifecycle,
            instance_label: "query-test".try_into().expect("instance label"),
        }),
        None,
        policy.id.clone(),
        policy.version,
        Evidence::Direct,
        None,
    )
    .expect("lifecycle event")
}

#[test]
fn snapshot_pagination_has_stable_order_and_excludes_concurrent_ingest() {
    let directory = private_tempdir();
    let path = directory.path().join("journal.sqlite3");
    let policy = PolicyProfile::fixture_default();
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("query-pagination-order"))
            .expect("journal");
    let fixture = ingest_fixture(fixture_path(), &journal, &policy).expect("fixture ingest");
    let mut request = QueryRequest::for_policy(&policy).expect("query request");
    request.page_size = 2;

    let first = journal.query_page(&request, None).expect("first page");
    assert_eq!(first.events.len(), 2);
    assert!(first.next_page_token.is_some());
    assert!(first.snapshot_boundary >= 2);
    let first_ids = first.events.iter().map(|event| event.event.event_id).collect::<Vec<_>>();

    let new_event = lifecycle_event(0x9_900, 1_735_689_500, &policy);
    journal
        .ingest(
            &IngestionOrigin::fixture_instance("fixture-query-pagination").expect("origin"),
            &new_event,
            &policy,
        )
        .expect("concurrent ingest");

    let mut seen = first_ids.clone();
    let mut token = first.next_page_token;
    while let Some(next) = token {
        let page = journal.query_page(&request, Some(&next)).expect("next page");
        assert!(page.events.len() <= request.page_size);
        assert!(!page.events.iter().any(|event| event.event.event_id == new_event.event_id));
        assert!(!page.events.iter().any(|event| seen.contains(&event.event.event_id)));
        seen.extend(page.events.iter().map(|event| event.event.event_id));
        token = page.next_page_token;
    }

    let expected = fixture.ingest_sequences.len();
    assert_eq!(seen.len(), expected);
    assert!(!seen.contains(&new_event.event_id));
    assert_eq!(journal.events().expect("events").len(), expected + 1);
}

#[test]
fn forged_cross_profile_and_page_parameter_tokens_fail_closed() {
    let policy = PolicyProfile::fixture_default();
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("query-pagination-token"))
        .expect("journal");
    ingest_fixture(fixture_path(), &journal, &policy).expect("fixture ingest");
    let mut request = QueryRequest::for_policy(&policy).expect("query request");
    request.page_size = 2;
    let page = journal.query_page(&request, None).expect("first page");
    let token = page.next_page_token.expect("next token");

    let mut forged = token.clone();
    let replacement = if forged.as_bytes()[0] == b'0' { '1' } else { '0' };
    forged.replace_range(0..1, &replacement.to_string());
    assert!(matches!(
        journal.query_page(&request, Some(&forged)),
        Err(GhostraceError::QueryTokenInvalid)
    ));

    let mut cross_profile = policy.clone();
    cross_profile.version += 1;
    let other_request = QueryRequest::for_policy(&cross_profile).expect("other request");
    assert!(matches!(
        journal.query_page(&other_request, Some(&token)),
        Err(GhostraceError::QueryTokenMismatch)
    ));

    let mut changed_page_size = request.clone();
    changed_page_size.page_size = 3;
    assert!(matches!(
        journal.query_page(&changed_page_size, Some(&token)),
        Err(GhostraceError::QueryTokenMismatch)
    ));
}

#[test]
fn deletion_after_snapshot_is_not_resurrected_and_schema_change_invalidates_token() {
    let directory = private_tempdir();
    let path = directory.path().join("journal.sqlite3");
    let policy = PolicyProfile::fixture_default();
    let journal = Journal::open_fixture(
        &path,
        DeterministicKeyProvider::from_seed("query-pagination-retention"),
    )
    .expect("journal");
    ingest_fixture(fixture_path(), &journal, &policy).expect("fixture ingest");
    let mut request = QueryRequest::for_policy(&policy).expect("query request");
    request.page_size = 2;
    let first = journal.query_page(&request, None).expect("first page");
    let token = first.next_page_token.clone().expect("next token");

    let deleted_id: String = {
        let connection = Connection::open(&path).expect("retention connection");
        connection.execute_batch("PRAGMA foreign_keys = OFF").expect("retention mode");
        let id = connection
            .query_row("SELECT event_id FROM events ORDER BY ingest_seq DESC LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("tail event");
        connection
            .execute("DELETE FROM events WHERE event_id = ?1", [&id])
            .expect("retention deletion");
        id
    };

    let second = journal.query_page(&request, Some(&token)).expect("second page");
    assert!(second.events.iter().all(|event| event.event.event_id.to_string() != deleted_id));
    assert!(second.events.iter().all(|event| !first
        .events
        .iter()
        .any(|previous| previous.event.event_id == event.event.event_id)));

    let connection = Connection::open(&path).expect("migration connection");
    connection.execute_batch("PRAGMA user_version = 999").expect("future migration marker");
    assert!(matches!(
        journal.query_page(&request, Some(&token)),
        Err(GhostraceError::QuerySchemaChanged)
    ));
}
