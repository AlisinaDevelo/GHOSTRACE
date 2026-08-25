use std::{fs, path::Path, time::Duration};

use ghostrace::{
    read_fixture, CryptoError, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope,
    EventSource, FaultPlan, FaultPoint, GhostraceError, IngestionOrigin, Journal, KeyProvider,
    PolicyProfile, QueueFullPolicy, SourceCursor, Writer, WriterConfig, WriterOutcome,
    WriterSubmission,
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

struct LockedKeyProvider;

impl KeyProvider for LockedKeyProvider {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        Err(CryptoError::KeyProvider("keychain is locked".to_owned()))
    }
}

fn unique_event(event: &EventEnvelope, id: u128) -> EventEnvelope {
    let mut event = event.clone();
    event.event_id = Uuid::from_u128(id);
    let position = id.saturating_sub(0x6000);
    event.source_cursor =
        Some(SourceCursor::try_from(format!("seq-0-{position}")).expect("typed cursor"));
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
fn key_unavailable_can_emit_a_bounded_gap_without_plaintext_or_silent_loss() {
    let (origin, events, policy) = fixture();
    let event = events.first().expect("fixture event").clone();
    let journal = Journal::in_memory(LockedKeyProvider).expect("journal");
    let writer = Writer::new(
        journal.clone(),
        WriterConfig {
            key_unavailable_policy: ghostrace::KeyUnavailablePolicy::EmitGap,
            ..WriterConfig::default()
        },
    )
    .expect("writer");

    let outcome = writer
        .submit(origin, vec![event], policy, Vec::new())
        .expect("key-unavailable policy should produce an explicit outcome");
    assert!(matches!(
        outcome,
        WriterOutcome::Gap(ghostrace::WriterGap {
            reason: ghostrace::WriterGapReason::KeyUnavailable,
            event_count: 1,
            ..
        })
    ));
    assert!(journal.events().expect("events").is_empty());
    assert_eq!(writer.outstanding(), (0, 0));
}

#[test]
fn key_unavailable_rejects_by_default_without_plaintext_or_silent_loss() {
    let (origin, events, policy) = fixture();
    let event = events.first().expect("fixture event").clone();
    let journal = Journal::in_memory(LockedKeyProvider).expect("journal");
    let writer = Writer::new(journal.clone(), WriterConfig::default()).expect("writer");

    let error = writer
        .submit(origin, vec![event], policy, Vec::new())
        .expect_err("the safe default must reject unavailable keys");
    assert!(matches!(error, GhostraceError::Crypto(CryptoError::KeyProvider(_))));
    assert!(journal.events().expect("events").is_empty());
    assert_eq!(writer.outstanding(), (0, 0));
}

#[test]
fn key_unavailable_gap_is_bounded_to_the_admitted_batch() {
    let (origin, events, policy) = fixture();
    let first = events.first().expect("fixture event");
    let batch = vec![unique_event(first, 0x7001), unique_event(first, 0x7002)];
    let journal = Journal::in_memory(LockedKeyProvider).expect("journal");
    let writer = Writer::new(
        journal.clone(),
        WriterConfig {
            max_batch_items: 2,
            max_memory_bytes: 32 * 1024,
            key_unavailable_policy: ghostrace::KeyUnavailablePolicy::EmitGap,
            ..WriterConfig::default()
        },
    )
    .expect("writer");

    let outcome = writer
        .submit(origin, batch, policy, Vec::new())
        .expect("key-unavailable policy should produce an explicit outcome");
    assert!(matches!(
        outcome,
        WriterOutcome::Gap(ghostrace::WriterGap {
            reason: ghostrace::WriterGapReason::KeyUnavailable,
            event_count: 2,
            ..
        })
    ));
    assert!(journal.events().expect("events").is_empty());
    assert_eq!(writer.outstanding(), (0, 0));
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
            let events: i64 =
                connection.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
            let cursors: i64 =
                connection.query_row("SELECT COUNT(*) FROM cursors", [], |row| row.get(0))?;
            let policies: i64 =
                connection
                    .query_row("SELECT COUNT(*) FROM policy_metadata", [], |row| row.get(0))?;
            let diagnostics: i64 =
                connection.query_row("SELECT COUNT(*) FROM diagnostics", [], |row| row.get(0))?;
            Ok((events, cursors, policies, diagnostics))
        })
        .expect("snapshot");
    assert_eq!((events, cursors, policies, diagnostics), (1, 1, 1, 1));
}

#[test]
fn precommit_failure_returns_no_ack_and_rolls_back_event_and_cursor_together() {
    let (origin, events, policy) = fixture();
    let event = unique_event(events.first().expect("fixture event"), 0x7101);
    let journal = Journal::in_memory_with_fault_plan(
        DeterministicKeyProvider::from_seed("writer-atomic-fault"),
        FaultPlan::fail_once(FaultPoint::CursorBeforeUpdate),
    )
    .expect("journal");
    let writer = Writer::new(journal.clone(), WriterConfig::default()).expect("writer");

    let error = writer
        .submit(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
        .expect_err("pre-commit fault must not acknowledge");
    assert!(matches!(error, GhostraceError::InjectedFault { .. }));
    assert!(journal.events().expect("rolled-back events").is_empty());
    let identity = ghostrace::CursorIdentity::new(event.source, event.collector_instance())
        .expect("cursor identity");
    assert!(journal.cursor_state(&identity).expect("cursor state").is_none());

    drop(writer);
    let recovered = journal.with_fault_plan(FaultPlan::none());
    let retry_writer = Writer::new(recovered.clone(), WriterConfig::default()).expect("writer");
    let outcome = retry_writer
        .submit(origin, vec![event.clone()], policy, Vec::new())
        .expect("replay after rollback");
    let WriterOutcome::Committed(ack) = outcome else { panic!("unexpected gap") };
    assert_eq!(ack.event_ids, vec![event.event_id]);
    assert_eq!(recovered.events().expect("committed event").len(), 1);
    let state = recovered.cursor_state(&identity).expect("cursor state").expect("committed cursor");
    assert_eq!(state.last_event_id, Some(event.event_id.to_string()));
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
