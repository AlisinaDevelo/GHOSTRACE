use std::{fs, path::PathBuf};

use ghostrace::{
    DeletionMode, DeterministicKeyProvider, Journal, ResidueArtifactKind, ResidueReport,
};
use rusqlite::Connection;
use tempfile::tempdir;

fn private(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private directory");
    }
}

fn private_file(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private file");
    }
}

fn fixture_journal(directory: &std::path::Path) -> (Journal, PathBuf) {
    let path = directory.join("journal.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("residue-report"))
            .expect("journal");
    (journal, path)
}

#[test]
fn mode_contract_distinguishes_guarantees_costs_and_external_copies() {
    let modes = ResidueReport::mode_descriptions();
    assert_eq!(modes.len(), 4);
    assert_eq!(modes[0].mode, DeletionMode::Logical);
    assert_eq!(modes[1].mode, DeletionMode::Compaction);
    assert_eq!(modes[2].mode, DeletionMode::CryptographicErasure);
    assert_eq!(modes[3].mode, DeletionMode::ExternalCopy);
    assert!(modes[0].sqlite_behavior.contains("secure_delete"));
    assert!(modes[1].sqlite_behavior.contains("VACUUM"));
    assert!(modes[2].guarantee.contains("key material"));
    assert!(modes[3].external_copy_responsibility.contains("user"));
    assert!(modes.iter().all(|mode| !mode.unsupported_media.is_empty()));
}

#[test]
fn report_is_path_free_and_aggregates_known_external_backups() {
    let directory = tempdir().expect("directory");
    private(directory.path());
    let (journal, database) = fixture_journal(directory.path());
    let backup = directory.path().join("backup.sqlite3");
    journal.backup_snapshot(&backup).expect("backup");

    let report = journal.residue_report(std::slice::from_ref(&backup)).expect("report");
    report.validate().expect("valid report");
    assert_eq!(report.external_backup_count, 1);
    assert_eq!(report.fts_shadow_table_count, 0);
    assert_eq!(report.archive_shadow_table_count, 0);
    assert!(report.artifacts.iter().any(|artifact| artifact.kind == ResidueArtifactKind::Database
        && artifact.regular_file_count == 1
        && artifact.bytes > 0));
    assert!(report.artifacts.iter().any(|artifact| artifact.kind == ResidueArtifactKind::Backup
        && artifact.regular_file_count == 1
        && artifact.bytes > 0));
    let encoded = serde_json::to_string(&report).expect("JSON");
    assert!(!encoded.contains("journal.sqlite3"));
    assert!(!encoded.contains(directory.path().to_str().expect("directory text")));
    journal.shutdown().expect("shutdown");
    assert!(database.is_file());
}

#[test]
fn sentinel_matrix_inspects_database_wal_shm_temp_fts_archive_and_backup() {
    let directory = tempdir().expect("directory");
    private(directory.path());
    let database = directory.path().join("sentinel.sqlite3");
    let sentinel = "GHOSTRACE-0087-RESIDUE-SENTINEL-v1";
    let connection = Connection::open(&database).expect("database");
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
         PRAGMA secure_delete = ON;
         CREATE TABLE deletion_marker(value TEXT NOT NULL);
         CREATE VIRTUAL TABLE search USING fts5(content);
         CREATE VIRTUAL TABLE archive_index USING fts5(content);
         INSERT INTO deletion_marker(value) VALUES ('GHOSTRACE-0087-RESIDUE-SENTINEL-v1');
         INSERT INTO search(content) VALUES ('GHOSTRACE-0087-RESIDUE-SENTINEL-v1');
         INSERT INTO archive_index(content) VALUES ('GHOSTRACE-0087-RESIDUE-SENTINEL-v1');",
        )
        .expect("sentinel schema");
    let secure_delete: i64 =
        connection.query_row("PRAGMA secure_delete", [], |row| row.get(0)).expect("secure_delete");
    assert_eq!(secure_delete, 1);

    let shadow_names: Vec<String> = {
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_master WHERE name LIKE '%_data' OR name LIKE 'archive_%'",
            )
            .expect("shadow query");
        statement
            .query_map([], |row| row.get(0))
            .expect("shadow rows")
            .collect::<Result<_, _>>()
            .expect("shadow names")
    };
    assert!(shadow_names.iter().any(|name| name == "search_data"));
    assert!(shadow_names.iter().any(|name| name == "archive_index_data"));
    let marker: String = connection
        .query_row("SELECT value FROM deletion_marker", [], |row| row.get(0))
        .expect("database sentinel");
    assert_eq!(marker, sentinel);
    let fts_marker: String = connection
        .query_row("SELECT content FROM search WHERE rowid = 1", [], |row| row.get(0))
        .expect("FTS sentinel");
    assert_eq!(fts_marker, sentinel);
    let archive_marker: String = connection
        .query_row("SELECT content FROM archive_index WHERE rowid = 1", [], |row| row.get(0))
        .expect("archive sentinel");
    assert_eq!(archive_marker, sentinel);

    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").expect("checkpoint");
    drop(connection);

    let backup = directory.path().join("sentinel-backup.sqlite3");
    fs::copy(&database, &backup).expect("backup");
    private_file(&backup);
    assert!(fs::read(&backup)
        .expect("backup bytes")
        .windows(sentinel.len())
        .any(|bytes| bytes == sentinel.as_bytes()));

    // The test records the observable post-delete bytes rather than claiming
    // that one SQLite build or filesystem has a universal erasure guarantee.
    let connection = Connection::open(&database).expect("reopen");
    connection
        .execute("DELETE FROM deletion_marker WHERE value = ?1", [sentinel])
        .expect("logical deletion");
    connection.execute("DELETE FROM search WHERE rowid = 1", []).expect("FTS deletion");
    connection.execute("DELETE FROM archive_index WHERE rowid = 1", []).expect("archive deletion");
    drop(connection);
    let _database_still_contains_sentinel = fs::read(&database)
        .expect("database after deletion")
        .windows(sentinel.len())
        .any(|bytes| bytes == sentinel.as_bytes());
    assert!(database.is_file());

    for suffix in ["-wal", "-shm", "-tmp", "-backup"] {
        let artifact = database.with_file_name(format!("sentinel.sqlite3{suffix}"));
        fs::write(&artifact, sentinel.as_bytes()).expect("artifact sentinel");
        private_file(&artifact);
        let bytes = fs::read(&artifact).expect("artifact bytes");
        assert!(bytes.windows(sentinel.len()).any(|window| window == sentinel.as_bytes()));
    }
}
