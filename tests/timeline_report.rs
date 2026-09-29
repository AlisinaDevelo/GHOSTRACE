//! The offline HTML timeline over the checked-in causal chain.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use ghostrace::{
    report::{render_timeline_html, REPORT_PLAINTEXT_NOTICE},
    DeterministicKeyProvider, Journal, PolicyProfile,
};

fn fixture_html() -> String {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("timeline-report"))
        .expect("journal");
    ghostrace::ingest_fixture(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
        &journal,
        &PolicyProfile::fixture_default(),
    )
    .expect("ingest");
    let events = journal
        .events()
        .expect("events")
        .into_iter()
        .map(|stored| stored.event)
        .collect::<Vec<_>>();
    render_timeline_html(&events, Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap())
}

#[test]
fn the_report_is_offline_and_discloses_that_it_is_plaintext() {
    let html = fixture_html();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("default-src 'none'"), "a CSP forbids every load");
    assert!(html.contains(REPORT_PLAINTEXT_NOTICE));
    for forbidden in ["<script", "<link", "<img", "<iframe", "src=", "href=", "@import", "url("] {
        assert!(!html.contains(forbidden), "report contains {forbidden}");
    }
}

#[test]
fn gaps_abstentions_and_evidence_levels_are_distinct_and_coverage_is_never_complete() {
    let html = fixture_html();
    assert!(html.contains("class=\"row gap\""));
    assert!(html.contains("Coverage is incomplete."));
    assert!(html.contains("no statement fills it"));
    assert!(html.contains("(3 event(s) missing)"));
    assert!(html.contains("class=\"row direct\""));
    assert!(html.contains("class=\"badge contextual\""));
    assert!(!html.to_lowercase().contains("complete coverage"));

    let empty = render_timeline_html(&[], Utc::now());
    assert!(empty.contains("No gaps were recorded.</strong> That does not mean"));
    assert!(empty.contains("The journal has no events."));
}

#[test]
fn rendering_is_deterministic_for_a_given_journal_and_time() {
    assert_eq!(fixture_html(), fixture_html());
}
