use std::{fs, path::Path, time::Duration};

use ghostrace::{
    read_fixture, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope, EventSource,
    GhostraceError, IngestionOrigin, Journal, PolicyProfile, QueueFullPolicy, Writer, WriterConfig,
    WriterOutcome, WriterSubmission,
};
use rusqlite::Connection;
use tempfile::tempdir;
use uuid::Uuid;

fn fixture() -> (IngestionOrigin, Vec<EventEnvelope>, PolicyProfile) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    (
        IngestionOrigin::fixture(),
        read_fixture(path).expect("fixture events"),
        PolicyProfile::fixture_default(),
    )
}

fn unique_event(event: &EventEnvelope, id: u128) -> EventEnvelope {
    let mut event = event.clone();
    event.event_id = Uuid::from_u128(id);
    event
}

fn locked_journal(seed: &str) -> (tempfile::TempDir, Journal, Connection) {
    let directory = tempdir().expect("private tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private tempdir permissions");
    }
    let path = directory.path().join("journal.sqlite3");
    let journal = Journal::open_fixture(&path, DeterministicKeyProvider::from_seed(seed))
        .expect("file journal");
    let lock = Connection::open(&path).expect("lock connection");
    lock.execute_batch("BEGIN IMMEDIATE").expect("hold write lock");
    (directory, journal, lock)
}

#[test]
fn defaults_are_bounded_and_source_policy_is_explicit() {
    let defaults = WriterConfig::default();
    defaults.validate().expect("safe defaults");
    assert_eq!(defaults.queue_items, 64);
    assert_eq!(defaults.max_batch_items, 16);
    assert_eq!(defaults.max_memory_bytes, 4 * 1024 * 1024);
    assert_eq!(defaults.max_wait, Duration::from_millis(250));
    assert_eq!(defaults.max_retries, 2);
    assert_eq!(defaults.queue_full_policy(EventSource::Filesystem), QueueFullPolicy::Block);

    let configured = defaults
        .clone()
        .with_source_queue_policy(EventSource::Filesystem, QueueFullPolicy::EmitGap);
    assert_eq!(configured.queue_full_policy(EventSource::Filesystem), QueueFullPolicy::EmitGap);
    assert_eq!(configured.queue_full_policy(EventSource::Shell), QueueFullPolicy::Block);

    for invalid in [
        WriterConfig { queue_items: 0, ..defaults.clone() },
        WriterConfig { max_batch_items: 0, ..defaults.clone() },
        WriterConfig { max_memory_bytes: 0, ..defaults.clone() },
        WriterConfig { max_wait: Duration::ZERO, ..defaults.clone() },
        WriterConfig { max_retries: 9, ..defaults },
    ] {
        assert!(matches!(invalid.validate(), Err(GhostraceError::InvalidWriterConfig(_))));
    }
}

#[test]
fn acknowledgement_follows_one_atomic_event_cursor_policy_and_diagnostic_commit() {
    let (origin, events, policy) = fixture();
    let event = events.first().expect("fixture event").clone();
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("writer-atomic")).expect("journal");
    let writer = Writer::new(journal.clone(), WriterConfig::default()).expect("writer");
    let diagnostic =
        DiagnosticRecord::new("writer.commit", "accepted fixture batch").expect("diagnostic");

    let outcome = writer
        .submit(origin, vec![event.clone()], policy.clone(), vec![diagnostic])
        .expect("submit");
    let WriterOutcome::Committed(ack) = outcome else { panic!("unexpected gap") };
    assert_eq!(ack.source, event.source);
    assert_eq!(ack.ingest_sequences, vec![1]);
    assert_eq!(ack.event_ids, vec![event.event_id]);
    assert_eq!(ack.policy_profile_id, policy.id);
    assert_eq!(ack.policy_profile_version, 1);
    assert_eq!(ack.diagnostic_count, 1);
    assert_eq!(ack.attempts, 1);
    assert!(ack.committed_at.contains('T'));
    assert_eq!(journal.events().expect("events").len(), 1);
    assert_eq!(journal.diagnostic_count().expect("diagnostics"), 1);
    let (events, cursors, policies, diagnostics) = journal
        .with_read_snapshot(|connection| {
            let events: u64 =
                connection.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
            let cursors: u64 =
                connection.query_row("SELECT COUNT(*) FROM cursors", [], |row| row.get(0))?;
            let policies: u64 =
                connection
                    .query_row("SELECT COUNT(*) FROM policy_metadata", [], |row| row.get(0))?;
            let diagnostics: u64 =
                connection.query_row("SELECT COUNT(*) FROM diagnostics", [], |row| row.get(0))?;
            Ok((events, cursors, policies, diagnostics))
        })
        .expect("snapshot");
    assert_eq!((events, cursors, policies, diagnostics), (1, 1, 1, 1));
}

#[test]
fn fifo_acknowledgements_are_ordered_and_memory_is_bounded() {
    let (origin, events, policy) = fixture();
    let first = events.first().expect("fixture event");
    let batch = vec![unique_event(first, 0x6001), unique_event(first, 0x6002)];
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("writer-order")).expect("journal");
    let config = WriterConfig { queue_items: 4, max_batch_items: 4, ..WriterConfig::default() };
    let writer = Writer::new(journal.clone(), config).expect("writer");
    let tickets = [0x6003u128, 0x6004, 0x6005]
        .into_iter()
        .map(|id| {
            match writer
                .enqueue(origin.clone(), vec![unique_event(first, id)], policy.clone(), Vec::new())
                .expect("enqueue")
            {
                WriterSubmission::Queued(ticket) => ticket,
                WriterSubmission::Gap(_) => panic!("unexpected gap"),
            }
        })
        .collect::<Vec<_>>();
    let acknowledgements =
        tickets.into_iter().map(|ticket| ticket.wait().expect("ack")).collect::<Vec<_>>();
    assert_eq!(
        acknowledgements.iter().map(|ack| ack.ingest_sequences[0]).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(journal.events().expect("events").len(), 3);

    let too_small = WriterConfig { max_memory_bytes: 1, ..WriterConfig::default() };
    let writer = Writer::new(journal, too_small).expect("writer");
    let error = writer.enqueue(origin, batch, policy, Vec::new()).expect_err("memory bound");
    assert!(matches!(error, GhostraceError::WriterMemoryBound { .. }));
}

#[test]
fn sqlite_busy_retries_are_bounded_and_reported() {
    let (origin, events, policy) = fixture();
    let event = events.first().expect("fixture event").clone();
    let (_directory, journal, lock) = locked_journal("writer-retry");
    let config = WriterConfig { max_retries: 1, ..WriterConfig::default() };
    let writer = Writer::new(journal, config).expect("writer");
    let submission = writer.enqueue(origin, vec![event], policy, Vec::new()).expect("enqueue");
    let WriterSubmission::Queued(ticket) = submission else { panic!("unexpected gap") };
    let result = ticket.wait();
    drop(lock);
    assert!(matches!(result, Err(GhostraceError::WriterRetryExhausted { attempts: 2 })));
}

#[test]
fn invalid_diagnostics_are_rejected_before_any_write() {
    let (_origin, events, _policy) = fixture();
    let error = DiagnosticRecord::new("../secret", "not stored").expect_err("invalid code");
    assert!(matches!(error, GhostraceError::InvalidWriterDiagnostic(_)));
    assert!(!error.to_string().contains("secret"));
    assert!(fs::metadata(env!("CARGO_MANIFEST_DIR")).is_ok());
    assert!(!events.is_empty());
}
