use std::{
    fs,
    path::Path,
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use chrono::{TimeZone, Utc};
use ghostrace::{
    AuthenticatedAnomaly, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, GhostraceError, IngestionOrigin, Journal, KeyRing,
    PolicyProfile, ReasonCode, RetentionPolicy, SourceCursor,
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
    event_from(&origin, id, cursor)
}

fn event_from(origin: &IngestionOrigin, id: u128, cursor: &str) -> EventEnvelope {
    let timestamp = Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp");
    EventEnvelope::new(
        origin,
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
    assert_eq!(state.schema_version, 2);
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
fn replayed_event_with_a_new_identity_is_detected() {
    let (_directory, path) = private_path("replay.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    Connection::open(&path)
        .expect("mutation connection")
        .execute_batch(
            "UPDATE cursors SET last_event_id = NULL;
             INSERT INTO events(
                 event_id, schema_version, observed_at, ingested_at, source, kind,
                 collector_instance, source_cursor, provenance_version,
                 policy_profile_id, policy_profile_version, evidence, parent_event_id,
                 payload_ciphertext
             )
             SELECT
                 '00000000-0000-4000-8000-000000000099', schema_version, observed_at,
                 ingested_at, source, kind, collector_instance, source_cursor,
                 provenance_version, policy_profile_id, policy_profile_version,
                 evidence, parent_event_id, payload_ciphertext
             FROM events WHERE ingest_seq = 1",
        )
        .expect("replay mutation");
    assert_anomaly(&journal, AuthenticatedAnomaly::EventReplayed);
}

#[test]
fn key_rotation_advances_the_authenticated_chain_boundary() {
    let (_directory, path) = private_path("rotation.sqlite3");
    let first_ring = KeyRing::new(1, [0x31; 32]).expect("first key ring");
    let journal = Journal::open_fixture(&path, first_ring.clone()).expect("journal");
    journal.ingest(&IngestionOrigin::fixture(), &event(1, "seq-0-1"), &policy()).expect("first");
    journal.shutdown().expect("first shutdown");

    let mut rotated_ring = first_ring;
    rotated_ring.stage_generation(2, [0x32; 32]).expect("stage second key");
    rotated_ring.activate_generation(2).expect("activate second key");
    let rotated = Journal::open_fixture(&path, rotated_ring).expect("rotated journal");
    let before = rotated.verify_authenticated_state().expect("old generation verifies");
    assert_eq!(before.key_generation, 1);
    rotated
        .ingest(&IngestionOrigin::fixture(), &event(2, "seq-0-2"), &policy())
        .expect("post-rotation ingest");
    let after = rotated.verify_authenticated_state().expect("new generation verifies");
    assert_eq!(after.key_generation, 2);
    assert_eq!(after.chain_epoch, 1);
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

#[test]
fn two_connections_writing_one_journal_never_see_each_other_as_tampering() {
    // Two handles are two SQLite connections, as two GHOSTRACE processes would
    // be. Before the pre-write check ran under the write lock, one writer
    // could read the event count before the other's commit and the anchor
    // after it, and report a valid journal as tampered with.
    let (_directory, path) = private_path("concurrent.sqlite3");
    open(&path).initialize_authenticated_state().expect("initialize");
    let writers = (0..2u128)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                // Wait for the other writer the way the live CLI does.
                let journal = Journal::open_fixture_with_policy(
                    &path,
                    DeterministicKeyProvider::from_seed("authenticated-state"),
                    ghostrace::WalPolicy { busy_timeout_ms: 10_000, ..Default::default() },
                )
                .expect("journal");
                let origin = IngestionOrigin::fixture_instance(format!(
                    "fixture-concurrent-writer-{writer}"
                ))
                .expect("origin");
                for index in 0..150u128 {
                    let id = 1_000_000 * (writer + 1) + index;
                    let event = event_from(&origin, id, &format!("cursor-{index:06}"));
                    journal.ingest(&origin, &event, &policy()).expect("concurrent ingest");
                    // Paced like a collector's batches, so the test measures
                    // interleaving rather than lock starvation.
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            })
        })
        .collect::<Vec<_>>();
    for writer in writers {
        writer.join().expect("writer");
    }
    let journal = open(&path);
    assert_eq!(journal.events().expect("events").len(), 300);
    assert!(journal.authenticated_state_report().expect("report").valid);
}

/// Device acceptance lane for issue #403.  It is intentionally ignored in the
/// ordinary suite: the seed is large enough to characterize the reference
/// device rather than to act as a CI smoke test.  Seed/setup timing is emitted
/// separately from the hot-write timing; any v1-to-v2 promotion measurement
/// must be collected in the legacy migration lane, while the measured writes
/// here use the retained v2 commitment state.
#[test]
#[ignore = "reference-device 100,000-event acceptance lane; run explicitly"]
fn one_hundred_thousand_events_and_two_default_timeout_writers() {
    const SEED_EVENTS: u128 = 100_000;
    const SEED_BATCH: u128 = 1_000;
    const HOT_WRITES: u128 = 64;
    const CONCURRENT_WRITES: u128 = 32;

    let (_directory, path) = private_path("authenticated-100k-acceptance.sqlite3");
    let journal = open(&path);
    let seed_origin =
        IngestionOrigin::fixture_instance("fixture-authenticated-100k-seed").expect("seed origin");
    let seed_policy = policy();
    let seed_started = Instant::now();
    for batch in 0..(SEED_EVENTS / SEED_BATCH) {
        let first = batch * SEED_BATCH + 1;
        let events = (first..first + SEED_BATCH)
            .map(|id| event_from(&seed_origin, id, &format!("seq-0-{id}")))
            .collect::<Vec<_>>();
        journal.ingest_batch(&seed_origin, &events, &seed_policy).expect("seed batch");
    }
    let seed_elapsed_ms = seed_started.elapsed().as_millis();
    assert_eq!(journal.authenticated_state().expect("seed anchor").event_count, SEED_EVENTS as u64);

    drop(journal);
    let journal = open(&path);
    // This is deliberately one separately timed write: a newly opened connection must
    // perform the full external-commit/startup preflight outside BEGIN
    // IMMEDIATE.  It must not be included in the steady-state bound below.
    let hot_origin =
        IngestionOrigin::fixture_instance("fixture-authenticated-100k-hot").expect("hot origin");
    let startup_started = Instant::now();
    journal
        .ingest(&hot_origin, &event_from(&hot_origin, 200_001, "seq-0-1"), &seed_policy)
        .expect("hot-path warm write");
    let startup_write_ms = startup_started.elapsed().as_millis();
    println!("AUTH_100K_SETUP seed_events={SEED_EVENTS} seed_ms={seed_elapsed_ms} startup_write_ms={startup_write_ms}");
    let mut hot_write_us = Vec::with_capacity(HOT_WRITES as usize);
    for index in 0..HOT_WRITES {
        let started = Instant::now();
        journal
            .ingest(
                &hot_origin,
                &event_from(&hot_origin, 200_002 + index, &format!("seq-0-{}", index + 2)),
                &seed_policy,
            )
            .expect("hot-path write");
        hot_write_us.push(started.elapsed().as_micros());
    }
    let max_hot_write_us = hot_write_us.iter().copied().max().unwrap_or(0);
    println!("AUTH_100K_HOT whole_write_us={hot_write_us:?} max_hot_write_us={max_hot_write_us}");
    let hot_bound_ms = std::env::var("GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS")
        .expect("set GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS to the measured reference-device bound")
        .parse::<u128>()
        .expect("GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS must be an integer");
    assert!(
        max_hot_write_us <= hot_bound_ms * 1_000,
        "steady-state authenticated write exceeded reference bound: max={max_hot_write_us}us bound={hot_bound_ms}ms"
    );

    // Reopen two independent connections after the 100k seed.  The default
    // 250ms timeout is part of this lane; a custom 10s timeout would hide the
    // regression that issue #403 is intended to prevent.
    drop(journal);
    const WRITER_READY_TIMEOUT: Duration = Duration::from_secs(30);
    const WRITER_COMPLETION_TIMEOUT: Duration = Duration::from_secs(300);

    // Finish all fallible setup before spawning. A worker's first operation is
    // a readiness signal; the coordinator releases both workers only after
    // both signals arrive, so setup failures cannot strand a peer at a
    // Barrier. The bounded completion channel covers writer failures and the
    // test's failure path. The acceptance command still runs under an outer
    // process-group watchdog because Rust cannot cancel an uninterruptible
    // SQLite call or force-join a stuck thread.
    let prepared_writers = (0..2_u128)
        .map(|writer| {
            let journal = open(&path);
            assert_eq!(
                journal.busy_timeout_ms().expect("writer busy timeout"),
                250,
                "acceptance lane must use the default 250ms busy timeout"
            );
            let origin = IngestionOrigin::fixture_instance(format!(
                "fixture-authenticated-100k-writer-{writer}"
            ))
            .expect("writer origin");
            (writer, journal, origin, seed_policy.clone())
        })
        .collect::<Vec<_>>();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<u128>(2);
    let (done_tx, done_rx) = mpsc::sync_channel::<(u128, Result<(), String>)>(2);
    let mut starts = Vec::with_capacity(prepared_writers.len());
    let mut writers = Vec::with_capacity(prepared_writers.len());
    for (writer, journal, origin, policy) in prepared_writers {
        let (start_tx, start_rx) = mpsc::channel::<()>();
        starts.push(start_tx);
        let ready_tx = ready_tx.clone();
        let done_tx = done_tx.clone();
        let handle = thread::spawn(move || -> Result<(), String> {
            let result: Result<(), String> = (|| {
                ready_tx
                    .send(writer)
                    .map_err(|_| "acceptance coordinator dropped readiness".to_owned())?;
                start_rx
                    .recv_timeout(WRITER_COMPLETION_TIMEOUT)
                    .map_err(|error| format!("writer {writer} start timeout: {error}"))?;
                for index in 0..CONCURRENT_WRITES {
                    let id = 300_000 + writer * CONCURRENT_WRITES + index;
                    journal
                        .ingest(
                            &origin,
                            &event_from(&origin, id, &format!("seq-0-{}", index + 1)),
                            &policy,
                        )
                        .map_err(|error| error.to_string())?;
                    thread::yield_now();
                }
                Ok(())
            })();
            let completion = result.as_ref().map(|_| ()).map_err(|error| error.clone());
            let _ = done_tx.send((writer, completion));
            result
        });
        writers.push((writer, handle));
    }
    drop(ready_tx);
    drop(done_tx);

    for _ in 0..2 {
        if let Err(error) = ready_rx.recv_timeout(WRITER_READY_TIMEOUT) {
            for start in starts {
                let _ = start.send(());
            }
            drop(writers);
            panic!("authenticated writer readiness timed out: {error}");
        }
    }
    for start in starts {
        if let Err(error) = start.send(()) {
            drop(writers);
            panic!("authenticated writer start failed: {error}");
        }
    }

    let mut completed = 0;
    while completed < 2 {
        match done_rx.recv_timeout(WRITER_COMPLETION_TIMEOUT) {
            Ok((_writer, Ok(()))) => completed += 1,
            Ok((writer, Err(error))) => {
                drop(writers);
                panic!("authenticated writer {writer} failed: {error}");
            }
            Err(error) => {
                drop(writers);
                panic!("authenticated writers did not complete: {error}");
            }
        }
    }
    for (_writer, handle) in writers {
        handle.join().expect("acceptance writer thread").expect("default-timeout write");
    }

    // Exercise separate operating-system processes as well as independent
    // connections. Opening sequentially via readiness files avoids measuring
    // migration setup contention; the shared start marker releases both
    // writers with the original 250ms busy timeout.
    let mut children = Vec::new();
    for writer in 0..2 {
        let child = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "authenticated_acceptance_child_writer", "--ignored", "--nocapture"])
            .env("GHOSTRACE_AUTH_ACCEPTANCE_JOURNAL", &path)
            .env("GHOSTRACE_AUTH_ACCEPTANCE_WRITER", writer.to_string())
            .spawn()
            .expect("spawn acceptance writer");
        children.push(child);
        let ready = path.with_extension(format!("ready-{writer}"));
        let deadline = Instant::now() + WRITER_READY_TIMEOUT;
        while !ready.exists() {
            if Instant::now() >= deadline
                || children.last_mut().unwrap().try_wait().unwrap().is_some()
            {
                for child in &mut children {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                panic!("process writer {writer} failed to become ready");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    fs::write(path.with_extension("start"), b"start").expect("release process writers");
    let process_started = Instant::now();
    let mut statuses = [None, None];
    while statuses.iter().any(Option::is_none) {
        for (index, child) in children.iter_mut().enumerate() {
            if statuses[index].is_none() {
                statuses[index] = child.try_wait().expect("poll acceptance writer");
            }
        }
        if statuses.iter().flatten().any(|status| !status.success())
            || process_started.elapsed() >= WRITER_COMPLETION_TIMEOUT
        {
            for child in &mut children {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!("process acceptance writers failed or timed out: {statuses:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let process_elapsed_ms = process_started.elapsed().as_millis();
    let final_journal = open(&path);
    let final_state = final_journal.authenticated_state().expect("final anchor");
    assert_eq!(
        final_state.event_count,
        SEED_EVENTS as u64 + 1 + HOT_WRITES as u64 + 4 * CONCURRENT_WRITES as u64
    );
    let full_check_started = Instant::now();
    assert!(final_journal.authenticated_state_report().expect("final report").valid);
    let full_check_ms = full_check_started.elapsed().as_millis();
    println!(
        "AUTH_100K_ACCEPTANCE seed_events={SEED_EVENTS} seed_ms={seed_elapsed_ms} startup_write_ms={startup_write_ms} full_check_ms={full_check_ms} hot_writes={HOT_WRITES} max_hot_write_us={max_hot_write_us} hot_bound_ms={hot_bound_ms} concurrent_connections=2 concurrent_processes=2 concurrent_writes_per_writer={CONCURRENT_WRITES} process_writers_ms={process_elapsed_ms} busy_timeout_ms=250 PASS"
    );
}

/// Helper for the process lane; requires the parent's private fixture path.
#[test]
#[ignore = "spawned by the 100,000-event acceptance coordinator"]
fn authenticated_acceptance_child_writer() {
    let path = std::path::PathBuf::from(
        std::env::var_os("GHOSTRACE_AUTH_ACCEPTANCE_JOURNAL").expect("parent journal"),
    );
    let writer: u128 = std::env::var("GHOSTRACE_AUTH_ACCEPTANCE_WRITER")
        .expect("parent writer ID")
        .parse()
        .expect("writer ID");
    assert!(writer < 2);
    let journal = open(&path);
    assert_eq!(journal.busy_timeout_ms().expect("busy timeout"), 250);
    let origin =
        IngestionOrigin::fixture_instance(format!("fixture-authenticated-100k-process-{writer}"))
            .expect("process origin");
    fs::write(path.with_extension(format!("ready-{writer}")), b"ready").expect("signal readiness");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.with_extension("start").exists() {
        assert!(Instant::now() < deadline, "parent start timeout");
        thread::sleep(Duration::from_millis(10));
    }
    let started = Instant::now();
    for index in 0..32_u128 {
        journal
            .ingest(
                &origin,
                &event_from(
                    &origin,
                    400_000 + writer * 32 + index,
                    &format!("seq-0-{}", index + 1),
                ),
                &policy(),
            )
            .expect("process write with default timeout");
        thread::yield_now();
    }
    println!(
        "AUTH_100K_PROCESS writer={writer} writes=32 elapsed_ms={} busy_timeout_ms=250 PASS",
        started.elapsed().as_millis()
    );
}
