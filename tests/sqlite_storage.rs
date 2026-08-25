use std::{fs, path::PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use ghostrace::{
    read_fixture, CursorIdentity, DeterministicKeyProvider, IngestionOrigin, Journal, PolicyProfile,
};
use tempfile::tempdir;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[test]
fn file_backed_journal_enforces_schema_pragmas_permissions_and_durable_state() {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let path = directory.path().join("journal.sqlite3");
    let provider = DeterministicKeyProvider::from_seed("0009-sqlite-contract");
    let journal = Journal::open_fixture(&path, provider).expect("open journal");

    assert_eq!(journal.journal_mode().expect("journal mode").to_ascii_lowercase(), "wal");
    assert_eq!(journal.synchronous_mode().expect("synchronous mode"), "FULL");
    assert!(journal.foreign_keys_enabled().expect("foreign keys"));
    assert_eq!(journal.schema_version_count().expect("schema versions"), 4);
    assert_eq!(journal.schema_version().expect("user version"), 4);
    assert_eq!(journal.applied_migrations().expect("migration ledger").len(), 5);
    #[cfg(unix)]
    {
        assert_eq!(mode(directory.path()), 0o700);
        assert_eq!(mode(&path), 0o600);
    }

    let event = read_fixture(fixture_path()).expect("fixture").remove(0);
    journal
        .ingest(&IngestionOrigin::fixture(), &event, &PolicyProfile::fixture_default())
        .expect("ingest event");
    assert_eq!(journal.events().expect("events").len(), 1);
    assert_eq!(journal.diagnostic_count().expect("diagnostics"), 0);
    let identity = CursorIdentity::new(event.source, event.collector_instance()).expect("cursor");
    let cursor = journal.cursor_state(&identity).expect("cursor state").expect("stored cursor");
    assert_eq!(cursor.last_event_id, Some(event.event_id.to_string()));
}

#[test]
fn reopening_the_file_journal_reuses_the_same_migration_ledger() {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private directory");
    let path = directory.path().join("journal.sqlite3");
    let provider = DeterministicKeyProvider::from_seed("0009-migration-reopen");

    let first = Journal::open_fixture(&path, provider.clone()).expect("first open");
    let first_ledger = first.applied_migrations().expect("first ledger");
    assert_eq!(first_ledger.len(), 5);
    drop(first);

    let reopened = Journal::open_fixture(&path, provider).expect("reopen");
    assert_eq!(reopened.applied_migrations().expect("reopened ledger"), first_ledger);
    assert_eq!(reopened.schema_version().expect("reopened version"), 4);
    assert_eq!(reopened.schema_version_count().expect("reopened schema"), 4);
}
