use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use ghostrace::{
    ingest_fixture, CheckpointMode, DeterministicKeyProvider, GhostraceError, Journal,
    PolicyProfile, WalPolicy,
};
use rusqlite::OptionalExtension;
use tempfile::tempdir;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn private_directory() -> tempfile::TempDir {
    let directory = tempdir().expect("temporary directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private temporary directory");
    }
    directory
}

fn policy(max_wal_bytes: u64, reader_limit_ms: u64) -> WalPolicy {
    WalPolicy::new(100_000, 100, reader_limit_ms, max_wal_bytes).expect("valid WAL policy")
}

#[test]
fn wal_policy_is_applied_and_checkpoint_is_observable() {
    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    let configured = policy(64 * 1024, 500);
    let journal = Journal::open_fixture_with_policy(
        &path,
        DeterministicKeyProvider::from_seed("wal-policy-settings"),
        configured,
    )
    .expect("open journal");

    assert_eq!(journal.wal_policy(), configured);
    assert_eq!(journal.wal_autocheckpoint_pages().expect("autocheckpoint"), 100_000);
    assert_eq!(journal.busy_timeout_ms().expect("busy timeout"), 100);
    assert_eq!(journal.journal_size_limit_bytes().expect("journal limit"), 64 * 1024);

    let report = journal.checkpoint(CheckpointMode::Truncate).expect("checkpoint");
    println!("WAL_POLICY_CHECKPOINT {report:?}");
    assert!(!report.busy);
    assert!(report.wal_bytes <= configured.max_wal_bytes);
}

#[test]
fn long_reader_is_refused_and_checkpoint_reports_remaining_frames() {
    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    let configured = policy(4 * 1024, 25);
    let journal = Journal::open_fixture_with_policy(
        &path,
        DeterministicKeyProvider::from_seed("wal-policy-reader"),
        configured,
    )
    .expect("open journal");

    let (started_tx, started_rx) = mpsc::channel();
    let reader = journal.clone();
    let reader_thread = thread::spawn(move || {
        reader.with_read_snapshot(|connection| {
            let _: i64 =
                connection.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
            started_tx.send(()).expect("signal reader");
            thread::sleep(Duration::from_millis(60));
            Ok(())
        })
    });
    started_rx.recv_timeout(Duration::from_secs(2)).expect("reader started");

    ingest_fixture(fixture_path(), &journal, &PolicyProfile::fixture_default())
        .expect("write while reader is active");
    let checkpoint = journal.checkpoint(CheckpointMode::Passive);
    match checkpoint {
        Err(GhostraceError::WalCheckpointRefused {
            frames_remaining,
            wal_bytes,
            max_wal_bytes,
        }) => {
            println!(
                "WAL_POLICY_STARVATION frames_remaining={frames_remaining} wal_bytes={wal_bytes}"
            );
            assert!(
                frames_remaining > 0 || wal_bytes > max_wal_bytes,
                "refusal must identify remaining frames or an oversized WAL"
            );
        }
        Ok(report) => {
            println!("WAL_POLICY_PASSIVE {report:?}");
            assert!(
                report.frames_checkpointed < report.frames_in_wal
                    || report.wal_bytes <= configured.max_wal_bytes,
                "checkpoint must either report starvation or remain within its bound: {report:?}"
            );
        }
        Err(error) => panic!("unexpected checkpoint error: {error}"),
    }

    let reader_result = reader_thread.join().expect("reader thread");
    println!("WAL_POLICY_READER_RESULT {reader_result:?}");
    assert!(matches!(reader_result, Err(GhostraceError::LongReader { .. })));
}

#[test]
fn database_snapshot_rejects_sidecars_and_reopens_after_truncate_checkpoint() {
    let directory = private_directory();
    let source = directory.path().join("journal.sqlite3");
    let destination = directory.path().join("snapshot.sqlite3");
    let journal = Journal::open_fixture_with_policy(
        &source,
        DeterministicKeyProvider::from_seed("wal-policy-backup"),
        policy(64 * 1024, 500),
    )
    .expect("open journal");
    ingest_fixture(fixture_path(), &journal, &PolicyProfile::fixture_default())
        .expect("write fixture");

    let receipt = journal.backup_snapshot(&destination).expect("database snapshot");
    assert!(receipt.bytes > 0);
    assert!(!destination.with_file_name("snapshot.sqlite3-wal").exists());
    assert!(!destination.with_file_name("snapshot.sqlite3-shm").exists());

    let reopened = Journal::open_fixture(
        &destination,
        DeterministicKeyProvider::from_seed("wal-policy-backup"),
    )
    .expect("reopen database-only snapshot");
    assert_eq!(reopened.events().expect("snapshot events").len(), 8);

    let sidecar = directory.path().join("invalid.sqlite3-wal");
    let error = journal.backup_snapshot(&sidecar).expect_err("sidecar backup");
    assert!(matches!(error, GhostraceError::SidecarBackupRefused));
    let shutdown = journal.shutdown().expect("bounded shutdown checkpoint");
    println!("WAL_POLICY_SHUTDOWN {shutdown:?}");
    assert!(shutdown.within_policy());
}

#[test]
fn reader_limit_is_measured_from_begin_to_commit() {
    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    let configured = policy(64 * 1024, 10);
    let journal = Journal::open_fixture_with_policy(
        &path,
        DeterministicKeyProvider::from_seed("wal-policy-timing"),
        configured,
    )
    .expect("open journal");
    let started = Instant::now();
    let result = journal.with_read_snapshot(|_| {
        thread::sleep(Duration::from_millis(30));
        Ok(())
    });
    println!("WAL_POLICY_LONG_READER {result:?}");
    assert!(matches!(result, Err(GhostraceError::LongReader { .. })));
    assert!(started.elapsed() >= Duration::from_millis(30));
}

#[test]
fn invalid_policy_and_in_memory_boundaries_fail_closed() {
    assert!(WalPolicy::new(0, 100, 10, 4096).is_err());
    assert!(WalPolicy::new(1, 30_001, 10, 4096).is_err());
    assert!(WalPolicy::new(1, 100, 0, 4096).is_err());
    assert!(WalPolicy::new(1, 100, 10, 4095).is_err());

    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("wal-policy-memory"))
        .expect("in-memory journal");
    let report = journal.checkpoint(CheckpointMode::Passive).expect("memory checkpoint report");
    assert!(report.within_policy());
    assert!(matches!(
        journal.backup_snapshot("/tmp/ghostrace-invalid-memory-backup.sqlite3"),
        Err(GhostraceError::BackupUnavailable)
    ));
}

#[test]
fn abrupt_child_exit_recovers_without_uncommitted_schema_or_unbounded_wal() {
    if std::env::var_os("GHOSTRACE_WAL_CRASH_CHILD").is_some() {
        let path = std::env::var("GHOSTRACE_WAL_CRASH_PATH").expect("crash path");
        let connection = rusqlite::Connection::open(path).expect("child open");
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE crash_marker(uncommitted INTEGER NOT NULL);",
            )
            .expect("uncommitted transaction");
        std::process::exit(137);
    }

    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    let journal = Journal::open_fixture_with_policy(
        &path,
        DeterministicKeyProvider::from_seed("wal-policy-crash"),
        policy(64 * 1024, 500),
    )
    .expect("open journal");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .arg("--exact")
        .arg("abrupt_child_exit_recovers_without_uncommitted_schema_or_unbounded_wal")
        .arg("--nocapture")
        .env("GHOSTRACE_WAL_CRASH_CHILD", "1")
        .env("GHOSTRACE_WAL_CRASH_PATH", &path)
        .status()
        .expect("spawn abrupt child");
    assert!(!child.success(), "child must terminate before committing");
    drop(journal);

    let reopened = Journal::open_fixture_with_policy(
        &path,
        DeterministicKeyProvider::from_seed("wal-policy-crash"),
        policy(64 * 1024, 500),
    )
    .expect("reopen after abrupt child exit");
    assert!(reopened.shutdown().expect("recovery checkpoint").within_policy());
    let connection = rusqlite::Connection::open(&path).expect("inspect recovered database");
    let marker: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'crash_marker'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("inspect schema");
    assert!(marker.is_none(), "uncommitted table survived abrupt exit");
}
