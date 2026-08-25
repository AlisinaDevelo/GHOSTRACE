use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use ghostrace::{
    ingest_fixture, read_fixture, CursorIdentity, DeterministicKeyProvider, EventKind, EventSource,
    FaultPlan, FaultPoint, IngestionOrigin, Journal, PolicyProfile, Writer, WriterConfig,
    WriterOutcome,
};
use tempfile::tempdir;

const CHILD_ENV: &str = "GHOSTRACE_REPLAY_CRASH_CHILD";
const CHILD_PATH_ENV: &str = "GHOSTRACE_REPLAY_CRASH_PATH";
const CHILD_MARKER_ENV: &str = "GHOSTRACE_REPLAY_CRASH_MARKER";

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn fixture_event() -> (IngestionOrigin, ghostrace::EventEnvelope, PolicyProfile) {
    let event = read_fixture(fixture_path()).expect("fixture").remove(0);
    (IngestionOrigin::fixture(), event, PolicyProfile::fixture_default())
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

#[test]
fn multi_source_fixture_replays_byte_identically_and_preserves_explicit_gap() {
    let events = read_fixture(fixture_path()).expect("fixture");
    let sources = events.iter().map(|event| event.source).collect::<BTreeSet<_>>();
    assert_eq!(
        sources,
        BTreeSet::from([
            EventSource::Browser,
            EventSource::Filesystem,
            EventSource::Git,
            EventSource::Lifecycle,
            EventSource::Shell,
        ])
    );

    let policy = PolicyProfile::fixture_default();
    let first = Journal::in_memory(DeterministicKeyProvider::from_seed("replay-harness-v1"))
        .expect("first journal");
    let second = Journal::in_memory(DeterministicKeyProvider::from_seed("replay-harness-v1"))
        .expect("second journal");
    let first_report = ingest_fixture(fixture_path(), &first, &policy).expect("first replay");
    let second_report = ingest_fixture(fixture_path(), &second, &policy).expect("second replay");

    assert_eq!(first_report, second_report);
    assert_eq!(first_report.ingest_sequences, (1..=8).collect::<Vec<_>>());
    assert_eq!(first_report.gap_event_ids, vec![events[5].event_id]);
    assert_eq!(
        first
            .events()
            .expect("first events")
            .iter()
            .map(|stored| stored.event.kind)
            .filter(|kind| *kind == EventKind::Gap)
            .count(),
        1
    );

    let first_json = first
        .events()
        .expect("first events")
        .into_iter()
        .map(|stored| serde_json::to_vec(&stored.event).expect("serialize event"))
        .collect::<Vec<_>>();
    let second_json = second
        .events()
        .expect("second events")
        .into_iter()
        .map(|stored| serde_json::to_vec(&stored.event).expect("serialize event"))
        .collect::<Vec<_>>();
    assert_eq!(first_json, second_json);
}

#[test]
fn injected_writer_crash_produces_no_ack_and_replays_after_restart() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_crash_child();
        unreachable!("crash child must abort");
    }

    let directory = private_tempdir();
    let path = directory.path().join("replay-crash.sqlite3");
    let marker = directory.path().join("ack.marker");
    let status = Command::new(std::env::current_exe().expect("test executable"))
        .arg("--exact")
        .arg("injected_writer_crash_produces_no_ack_and_replays_after_restart")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(CHILD_PATH_ENV, &path)
        .env(CHILD_MARKER_ENV, &marker)
        .status()
        .expect("spawn crash child");
    assert!(!status.success(), "fault child must abort before acknowledgement");
    assert!(!marker.exists(), "a crashed writer must not report an acknowledgement");

    let (origin, event, policy) = fixture_event();
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("replay-crash-v1"))
            .expect("reopen journal");
    assert!(journal.events().expect("events after crash").is_empty());
    let identity =
        CursorIdentity::new(event.source, event.collector_instance()).expect("cursor identity");
    assert!(journal.cursor_state(&identity).expect("cursor state").is_none());

    let writer = Writer::new(journal.clone(), WriterConfig::default()).expect("recovery writer");
    let outcome =
        writer.submit(origin, vec![event.clone()], policy, Vec::new()).expect("replay after crash");
    let WriterOutcome::Committed(ack) = outcome else { panic!("unexpected gap") };
    assert_eq!(ack.event_ids, vec![event.event_id]);
    assert_eq!(journal.events().expect("replayed event").len(), 1);
    assert_eq!(
        journal
            .cursor_state(&identity)
            .expect("cursor state")
            .expect("replayed cursor")
            .last_event_id,
        Some(event.event_id.to_string())
    );
}

fn run_crash_child() {
    let path = PathBuf::from(std::env::var(CHILD_PATH_ENV).expect("crash path"));
    let marker = PathBuf::from(std::env::var(CHILD_MARKER_ENV).expect("ack marker"));
    let (origin, event, policy) = fixture_event();
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("replay-crash-v1"))
            .expect("child journal")
            .with_fault_plan(FaultPlan::abort_once(FaultPoint::IngestBeforeCommit));
    let writer = Writer::new(journal, WriterConfig::default()).expect("child writer");
    match writer.submit(origin, vec![event], policy, Vec::new()) {
        Ok(_) => fs::write(marker, b"ack").expect("ack marker"),
        Err(_) => fs::write(marker, b"error").expect("error marker"),
    }
    std::process::abort();
}
