use std::{fs, path::PathBuf};

use chrono::{DateTime, Utc};
use ghostrace::{
    ingest_fixture, CoverageInterval, CoverageStatusKind, DeterministicKeyProvider, EventSource,
    Journal, PolicyProfile, QueryRequest,
};
use rusqlite::Connection;
use serde::Deserialize;
use tempfile::tempdir;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn time(value: &str) -> DateTime<Utc> {
    value.parse().expect("timestamp")
}

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    schema_version: u32,
    cases: Vec<GoldenCase>,
}

#[derive(Debug, Deserialize)]
struct GoldenCase {
    id: String,
    interval: CoverageInterval,
    #[serde(default)]
    window_source: Option<EventSource>,
    window_start: Option<DateTime<Utc>>,
    window_end: Option<DateTime<Utc>>,
    expected: bool,
}

fn private_journal(seed: &str) -> (tempfile::TempDir, Journal, PolicyProfile) {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join("journal.sqlite3");
    let policy = PolicyProfile::fixture_default();
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed(seed)).expect("journal");
    ingest_fixture(fixture_path(), &journal, &policy).expect("fixture ingest");
    (directory, journal, policy)
}

#[test]
fn golden_gap_intervals_are_versioned_and_deterministic() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/query-gap-coverage-v1.json");
    let fixture: GoldenFixture =
        serde_json::from_str(&fs::read_to_string(path).expect("golden coverage fixture"))
            .expect("golden JSON");
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(
        fixture.cases.iter().map(|case| case.id.as_str()).collect::<Vec<_>>(),
        vec!["nested", "adjacent", "open_ended", "cross_source"]
    );
    for case in fixture.cases {
        assert_eq!(
            case.interval.intersects_source(case.window_source, case.window_start, case.window_end),
            case.expected,
            "golden case {}",
            case.id
        );
    }
}

#[test]
fn intervals_cover_nested_adjacent_open_ended_and_cross_source_cases() {
    let nested = CoverageInterval {
        source: EventSource::Filesystem,
        start: Some(time("2026-01-01T01:00:00Z")),
        end: Some(time("2026-01-01T03:00:00Z")),
    };
    let inside = CoverageInterval {
        source: EventSource::Filesystem,
        start: Some(time("2026-01-01T01:30:00Z")),
        end: Some(time("2026-01-01T02:00:00Z")),
    };
    let adjacent = CoverageInterval {
        source: EventSource::Filesystem,
        start: Some(time("2026-01-01T03:00:00Z")),
        end: Some(time("2026-01-01T04:00:00Z")),
    };
    let open = CoverageInterval {
        source: EventSource::Filesystem,
        start: Some(time("2026-01-01T05:00:00Z")),
        end: None,
    };
    let other_source = CoverageInterval {
        source: EventSource::Browser,
        start: Some(time("2026-01-01T01:30:00Z")),
        end: Some(time("2026-01-01T02:00:00Z")),
    };
    assert!(
        nested.intersects(Some(time("2026-01-01T01:30:00Z")), Some(time("2026-01-01T02:00:00Z")))
    );
    assert!(
        inside.intersects(Some(time("2026-01-01T01:00:00Z")), Some(time("2026-01-01T03:00:00Z")))
    );
    assert!(
        adjacent.intersects(Some(time("2026-01-01T03:00:00Z")), Some(time("2026-01-01T03:00:00Z")))
    );
    assert!(open.intersects(Some(time("2026-01-01T06:00:00Z")), None));
    assert!(!nested.intersects(Some(time("2026-01-01T04:00:01Z")), None));
    assert!(!nested.intersects(None, Some(time("2025-12-31T23:59:59Z"))));
    assert!(nested.intersects_source(
        Some(EventSource::Filesystem),
        Some(time("2026-01-01T01:30:00Z")),
        Some(time("2026-01-01T02:00:00Z")),
    ));
    assert!(!nested.intersects_source(
        Some(EventSource::Browser),
        Some(time("2026-01-01T01:30:00Z")),
        Some(time("2026-01-01T02:00:00Z")),
    ));
    assert_ne!(nested.source, other_source.source);
}

#[test]
fn query_reports_markers_even_when_kind_filter_would_hide_them() {
    let (_directory, journal, policy) = private_journal("0079-markers");
    let mut request = QueryRequest::for_policy(&policy).expect("request");
    request.source = Some(EventSource::Filesystem);
    request.kind = Some(ghostrace::EventKind::FilesystemChanged);
    request.observed_from = Some(time("2026-01-01T00:00:06Z"));
    request.page_size = 1;
    let page = journal.query_page(&request, None).expect("query");
    assert!(page.coverage.marker_filter_ignored);
    assert!(page.coverage.statuses.iter().any(|status| {
        status.kind == CoverageStatusKind::SourceGap
            && status.source == Some(EventSource::Filesystem)
    }));
    assert!(!page.coverage.gaps.is_empty());
}

#[test]
fn query_distinguishes_no_events_unknown_history_and_explicit_markers() {
    let (_directory, journal, policy) = private_journal("0079-statuses");
    let mut empty = QueryRequest::for_policy(&policy).expect("request");
    empty.source = Some(EventSource::Browser);
    empty.observed_from = Some(time("2025-01-01T00:00:00Z"));
    empty.observed_until = Some(time("2025-12-31T23:59:59Z"));
    let empty_page = journal.query_page(&empty, None).expect("empty query");
    assert_eq!(empty_page.coverage.observed_event_count, 0);
    assert!(empty_page
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::NoEventsObserved));
    assert!(empty_page
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::UnknownHistory));

    let all = QueryRequest::for_policy(&policy).expect("request");
    let page = journal.query_page(&all, None).expect("all query");
    assert!(page
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::SourceDisabled));
    assert!(page
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::PolicyDenied));
    assert!(page
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::SourceGap));
}

#[test]
fn continuation_detects_retention_deletion_and_opt_out_is_explicit() {
    let (directory, journal, policy) = private_journal("0079-retention");
    let mut request = QueryRequest::for_policy(&policy).expect("request");
    request.page_size = 2;
    let first = journal.query_page(&request, None).expect("first page");
    let token = first.next_page_token.expect("continuation token");
    let connection =
        Connection::open(directory.path().join("journal.sqlite3")).expect("connection");
    connection.execute_batch("PRAGMA foreign_keys = OFF").expect("retention mode");
    let deleted_id: String = connection
        .query_row("SELECT event_id FROM events ORDER BY ingest_seq DESC LIMIT 1", [], |row| {
            row.get(0)
        })
        .expect("tail event");
    connection.execute("DELETE FROM events WHERE event_id = ?1", [&deleted_id]).expect("delete");

    let second = journal.query_page(&request, Some(&token)).expect("second page");
    assert!(second.coverage.retention_deletion_detected);
    assert!(second
        .coverage
        .statuses
        .iter()
        .any(|status| status.kind == CoverageStatusKind::RetentionDeletion));

    request.include_coverage = false;
    let opted_out = journal.query_page(&request, None).expect("opted-out query");
    assert!(opted_out.coverage.opted_out);
    assert!(opted_out.coverage.statuses.is_empty());
}
