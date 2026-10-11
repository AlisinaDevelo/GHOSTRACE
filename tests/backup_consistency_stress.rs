use std::{
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use ghostrace::{DeterministicKeyProvider, Journal};
use rusqlite::Connection;

#[test]
fn database_backup_never_contains_mixed_committed_generations() {
    let directory = tempfile::tempdir().expect("synthetic fixture directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private fixture directory");
    }
    let source = directory.path().join("journal.sqlite3");
    let journal = Journal::open_fixture(
        &source,
        DeterministicKeyProvider::from_seed("backup-consistency-synthetic"),
    )
    .expect("fixture journal");
    let mut peer = Connection::open(&source).expect("synthetic peer");
    peer.busy_timeout(Duration::from_millis(100)).expect("bounded peer wait");
    peer.execute_batch(
        "PRAGMA wal_autocheckpoint=0;
         CREATE TABLE backup_generation(id INTEGER PRIMARY KEY, generation INTEGER, filler BLOB);",
    )
    .expect("synthetic generation table");
    let transaction = peer.transaction().expect("fixture transaction");
    {
        let mut insert = transaction
            .prepare("INSERT INTO backup_generation VALUES(?1, 0, zeroblob(8000))")
            .expect("fixture insert");
        for id in 0..4096 {
            insert.execute([id]).expect("synthetic row");
        }
    }
    transaction.commit().expect("initial committed generation");
    journal.shutdown().expect("initial checkpoint");

    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = Arc::clone(&stop);
    let commits = Arc::new(AtomicUsize::new(0));
    let writer_commits = Arc::clone(&commits);
    let (started_tx, started_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        started_tx.send(()).expect("peer started");
        let mut generation = 1;
        let started = Instant::now();
        while !writer_stop.load(Ordering::Acquire) && started.elapsed() < Duration::from_secs(5) {
            match peer.execute(
                "UPDATE backup_generation SET generation=?1 WHERE id=0 OR id=4095",
                [generation],
            ) {
                Ok(_) => {
                    writer_commits.fetch_add(1, Ordering::AcqRel);
                    peer.execute_batch("PRAGMA wal_checkpoint(PASSIVE)").expect("peer checkpoint");
                    generation = 1 - generation;
                }
                Err(rusqlite::Error::SqliteFailure(error, _))
                    if error.code == rusqlite::ErrorCode::DatabaseBusy => {}
                Err(error) => panic!("peer update failed: {error}"),
            }
        }
    });
    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        started_rx.recv_timeout(Duration::from_secs(2))?;
        let mut mixed = None;
        let mut successful = 0;
        let mut bytes = 0;
        for attempt in 0..8 {
            let destination = directory.path().join(format!("snapshot-{attempt}.sqlite3"));
            let receipt = match journal.backup_snapshot(&destination) {
                Ok(receipt) => receipt,
                Err(ghostrace::GhostraceError::WalCheckpointRefused { .. }) => continue,
                Err(ghostrace::GhostraceError::Database(rusqlite::Error::SqliteFailure(
                    error,
                    _,
                ))) if matches!(
                    error.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            bytes = receipt.bytes;
            successful += 1;
            let snapshot = Connection::open(&destination)?;
            let range: (i64, i64) = snapshot.query_row(
                "SELECT MIN(generation), MAX(generation) FROM backup_generation WHERE id=0 OR id=4095",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if range.0 != range.1 {
                mixed = Some((attempt, range));
            }
            drop(snapshot);
            fs::remove_file(destination)?;
            if mixed.is_some() {
                break;
            }
        }
        Ok((successful, bytes, mixed))
    })();
    stop.store(true, Ordering::Release);
    writer.join().expect("peer finished");
    let (successful, bytes, mixed) = result.expect("synthetic snapshot checks");
    assert!(successful > 0, "fixture must complete at least one database backup");
    assert!(bytes >= 32 * 1024 * 1024, "fixture must exercise a large database");
    assert!(commits.load(Ordering::Acquire) > 0, "peer must commit during the run");
    assert_eq!(
        mixed, None,
        "a single transaction updates both distant rows, so a backup must contain one committed generation"
    );
}
