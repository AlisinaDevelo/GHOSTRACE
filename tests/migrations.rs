use std::{fs, path::Path, process::Command, time::Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use ghostrace::{DeterministicKeyProvider, GhostraceError, Journal};
use rusqlite::Connection;
use tempfile::tempdir;

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

fn open(path: &Path) -> Journal {
    Journal::open_fixture(path, DeterministicKeyProvider::from_seed("migration-tests"))
        .expect("open journal")
}

fn mutate(path: &Path, sql: &str) {
    let connection = Connection::open(path).expect("open database for mutation");
    connection.execute_batch(sql).expect("mutate migration fixture");
}

#[test]
fn migration_ledger_records_stable_identity_checksum_and_tool_version() {
    let directory = private_directory();
    let path = directory.path().join("journal.sqlite3");
    let journal = open(&path);
    let records = journal.applied_migrations().expect("migration ledger");

    assert_eq!(
        records.iter().map(|record| record.migration_id.as_str()).collect::<Vec<_>>(),
        [
            "0000_migration_ledger",
            "0001_init",
            "0002_journal_metadata",
            "0003_cursor_contract",
            "0004_replay_boundary",
            "0005_authenticated_state",
            "0006_incremental_authenticated_state",
        ]
    );
    assert_eq!(
        records.iter().map(|record| record.version).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert!(records.iter().all(|record| record.checksum.len() == 64));
    assert!(records.iter().all(|record| {
        record.checksum.chars().all(|character| character.is_ascii_hexdigit())
            && record.tool_version == "ghostrace/0.0.1"
            && !record.applied_at.is_empty()
    }));
    assert_eq!(journal.schema_version().expect("schema version"), 6);
    println!("MIGRATION_LEDGER {records:?}");
}

#[test]
fn legacy_v1_database_upgrades_and_reopens_idempotently() {
    let directory = private_directory();
    let path = directory.path().join("legacy.sqlite3");
    let connection = Connection::open(&path).expect("legacy database");
    connection
        .execute_batch(include_str!("../migrations/0001_init.sql"))
        .expect("legacy migration");
    drop(connection);
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("legacy permissions");

    let journal = open(&path);
    assert_eq!(journal.schema_version().expect("upgraded schema"), 6);
    assert_eq!(journal.applied_migrations().expect("upgraded ledger").len(), 7);
    drop(journal);

    let reopened = open(&path);
    assert_eq!(reopened.schema_version().expect("reopened schema"), 6);
    assert_eq!(reopened.applied_migrations().expect("reopened ledger").len(), 7);
    let connection = Connection::open(&path).expect("inspect upgraded database");
    let format: String = connection
        .query_row(
            "SELECT metadata_value FROM journal_metadata WHERE metadata_key = 'format'",
            [],
            |row| row.get(0),
        )
        .expect("metadata format");
    assert_eq!(format, "ghostrace-journal-v1");
}

#[test]
fn modified_migration_refuses_startup_without_echoing_path() {
    let directory = private_directory();
    let path = directory.path().join("modified.sqlite3");
    drop(open(&path));
    mutate(&path, "UPDATE migration_records SET checksum = 'deadbeef' WHERE version = 1;");

    let error =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("migration-tests"))
            .err()
            .expect("modified migration must refuse");
    assert!(matches!(error, GhostraceError::MigrationChecksumMismatch { .. }));
    assert!(!error.to_string().contains(path.to_string_lossy().as_ref()));
}

#[test]
fn missing_reordered_partial_and_future_migrations_refuse_startup() {
    let cases = [
        ("missing", "DELETE FROM migration_records WHERE version = 1;", "missing"),
        (
            "reordered",
            "UPDATE migration_records SET migration_id = 'temporary' WHERE version = 1;
             UPDATE migration_records SET migration_id = '0001_init' WHERE version = 2;
             UPDATE migration_records SET migration_id = '0002_journal_metadata' WHERE migration_id = 'temporary';",
            "order",
        ),
        (
            "partial",
            "DELETE FROM migration_records WHERE version = 4;",
            "partial",
        ),
        (
            "future",
            "INSERT INTO migration_records(migration_id, version, checksum, schema_version, tool_version, applied_at)
             VALUES ('0099_future', 99, 'future', 99, 'future-tool', '2026-01-01T00:00:00Z');",
            "future",
        ),
    ];

    for (name, mutation, expected) in cases {
        let directory = private_directory();
        let path = directory.path().join(format!("{name}.sqlite3"));
        drop(open(&path));
        mutate(&path, mutation);
        let error =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("migration-tests"))
                .err()
                .expect("unsafe migration state must refuse");
        match expected {
            "missing" => assert!(matches!(error, GhostraceError::MigrationRecordMissing { .. })),
            "order" => assert!(matches!(error, GhostraceError::MigrationOrder { .. })),
            "partial" => assert!(matches!(error, GhostraceError::MigrationRecordMissing { .. })),
            "future" => assert!(matches!(error, GhostraceError::FutureMigration { version: 99 })),
            _ => unreachable!(),
        }
        println!("MIGRATION_REFUSAL {name} {error}");
    }
}

#[test]
fn unsupported_downgrade_refuses_startup() {
    let directory = private_directory();
    let path = directory.path().join("downgrade.sqlite3");
    drop(open(&path));
    mutate(&path, "PRAGMA user_version = 1;");

    let error =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("migration-tests"))
            .err()
            .expect("downgrade must refuse");
    assert!(matches!(error, GhostraceError::UnsupportedDowngrade { recorded: 6, database: 1 }));
    println!("MIGRATION_DOWNGRADE_REFUSAL {error}");
}

/// This is run with a binary built from the pre-0006 source in a separate
/// target directory.  Keeping it opt-in makes the compatibility claim honest:
/// the current test binary cannot prove what an older executable would do.
#[test]
#[ignore = "requires GHOSTRACE_OLD_BINARY built before migration 0006"]
fn old_binary_refuses_the_incremental_schema() {
    let old_binary = std::env::var_os("GHOSTRACE_OLD_BINARY")
        .expect("set GHOSTRACE_OLD_BINARY to a binary built before migration 0006");
    let directory = private_directory();
    let path = directory.path().join("v2-schema.sqlite3");
    let current = open(&path);
    assert_eq!(current.schema_version().expect("current schema"), 6);
    drop(current);

    let output = Command::new(old_binary)
        .args(["authenticated-check", "--journal"])
        .arg(&path)
        .output()
        .expect("run old binary");
    assert!(!output.status.success(), "old binary must refuse schema 6");
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        diagnostics.contains("future migration") || diagnostics.contains("migration"),
        "old-binary refusal should identify the migration boundary: {diagnostics}"
    );
    println!("OLD_BINARY_SCHEMA_REFUSAL schema=6 PASS");
}

/// The same pre-0006 binary creates a real v1 anchor.  The current binary
/// must verify that anchor before promoting it; a tampered legacy row must
/// remain v1 and refuse the promotion path.
#[test]
#[ignore = "requires GHOSTRACE_OLD_BINARY built before migration 0006"]
fn legacy_v1_anchor_promotes_only_after_full_verification() {
    let old_binary = std::env::var_os("GHOSTRACE_OLD_BINARY")
        .expect("set GHOSTRACE_OLD_BINARY to a binary built before migration 0006");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let directory = private_directory();
    let valid_path = directory.path().join("legacy-valid.sqlite3");
    let output = Command::new(&old_binary)
        .args(["init", "--journal"])
        .arg(&valid_path)
        .output()
        .expect("run old initializer");
    assert!(output.status.success(), "old initializer failed: {output:?}");
    let output = Command::new(&old_binary)
        .args(["ingest", "--journal"])
        .arg(&valid_path)
        .args(["--fixture"])
        .arg(&fixture)
        .output()
        .expect("run old fixture ingest");
    assert!(output.status.success(), "old fixture ingest failed: {output:?}");

    let current =
        Journal::open_fixture(&valid_path, DeterministicKeyProvider::from_seed("fixture-cli-v1"))
            .expect("open legacy v1 journal");
    assert_eq!(current.authenticated_state().expect("v1 state").schema_version, 1);
    assert!(current.verify_authenticated_state().expect("v1 verification").valid);
    let promotion_started = Instant::now();
    current.initialize_authenticated_state().expect("verified v1 promotion");
    let promotion_us = promotion_started.elapsed().as_micros();
    assert_eq!(current.authenticated_state().expect("v2 state").schema_version, 2);
    assert!(current.verify_authenticated_state().expect("v2 verification").valid);

    let tampered_path = directory.path().join("legacy-tampered.sqlite3");
    let output = Command::new(&old_binary)
        .args(["init", "--journal"])
        .arg(&tampered_path)
        .output()
        .expect("run old initializer for tamper case");
    assert!(output.status.success(), "old initializer failed: {output:?}");
    let output = Command::new(&old_binary)
        .args(["ingest", "--journal"])
        .arg(&tampered_path)
        .args(["--fixture"])
        .arg(&fixture)
        .output()
        .expect("run old fixture ingest for tamper case");
    assert!(output.status.success(), "old fixture ingest failed: {output:?}");
    mutate(&tampered_path, "UPDATE events SET evidence = 'indirect' WHERE ingest_seq = 1;");
    let tampered = Journal::open_fixture(
        &tampered_path,
        DeterministicKeyProvider::from_seed("fixture-cli-v1"),
    )
    .expect("open tampered legacy v1 journal");
    assert!(matches!(
        tampered.initialize_authenticated_state(),
        Err(GhostraceError::AuthenticatedStateInvalid(_))
    ));
    assert_eq!(tampered.authenticated_state().expect("tampered v1 state").schema_version, 1);
    let events = current.authenticated_state().expect("promoted anchor").event_count;
    println!("LEGACY_V1_VERIFIED_PROMOTION events={events} promotion_us={promotion_us} valid=PASS tampered=REFUSED");
}

#[test]
fn backup_restore_preserves_migration_ledger() {
    let directory = private_directory();
    let source = directory.path().join("source.sqlite3");
    let destination = directory.path().join("restored.sqlite3");
    let journal = open(&source);
    let source_records = journal.applied_migrations().expect("source ledger");
    let receipt = journal.backup_snapshot(&destination).expect("backup snapshot");
    assert!(receipt.bytes > 0);

    let restored = open(&destination);
    assert_eq!(restored.applied_migrations().expect("restored ledger"), source_records);
    assert_eq!(restored.schema_version().expect("restored schema"), 6);
    println!("MIGRATION_BACKUP bytes={} records={}", receipt.bytes, source_records.len());
}

#[test]
fn crash_after_each_migration_step_recovers_transactionally() {
    if std::env::var_os("GHOSTRACE_MIGRATION_CRASH_CHILD").is_some() {
        let path = std::env::var("GHOSTRACE_MIGRATION_CRASH_PATH").expect("crash path");
        let _ = open(Path::new(&path));
        unreachable!("migration crash child should abort inside the runner");
    }

    for migration_id in [
        "0001_init",
        "0002_journal_metadata",
        "0003_cursor_contract",
        "0004_replay_boundary",
        "0005_authenticated_state",
        "0006_incremental_authenticated_state",
    ] {
        let directory = private_directory();
        let path = directory.path().join(format!("crash-{migration_id}.sqlite3"));
        let status = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("crash_after_each_migration_step_recovers_transactionally")
            .arg("--nocapture")
            .env("GHOSTRACE_MIGRATION_CRASH_CHILD", "1")
            .env("GHOSTRACE_MIGRATION_CRASH_PATH", &path)
            .env("GHOSTRACE_TEST_MIGRATION_CRASH", migration_id)
            .status()
            .expect("spawn migration crash child");
        assert!(!status.success(), "migration child must abort at {migration_id}");

        let recovered = open(&path);
        assert_eq!(recovered.schema_version().expect("recovered schema"), 6);
        assert_eq!(recovered.applied_migrations().expect("recovered ledger").len(), 7);
        println!("MIGRATION_CRASH_RECOVERY {migration_id} PASS");
    }
}
