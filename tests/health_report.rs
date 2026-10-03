use std::{fs, process::Command};

use ghostrace::{DeterministicKeyProvider, Journal};
use rusqlite::Connection;
use tempfile::tempdir;

fn private_dir() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn cli(path: &std::path::Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ghostrace"));
    command.args(["health", "--journal"]).arg(path);
    if json {
        command.arg("--json");
    }
    command.output().expect("health command")
}

fn report(output: &[u8]) -> serde_json::Value {
    let report = serde_json::from_slice(output).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/health-report-v1.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&report), "{report}");
    report
}

#[test]
fn report_is_read_only_bounded_and_does_not_claim_unchecked_health() {
    let dir = private_dir();
    let path = dir.path().join("journal.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
    journal.initialize_authenticated_state().unwrap();
    journal.shutdown().unwrap();
    drop(journal);
    let before = fs::read(&path).unwrap();
    let output = cli(&path, true);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report = report(&output.stdout);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["journal_schema_version"], 5);
    assert_eq!(report["counts"]["events"], 0);
    assert_eq!(report["journal"]["status"], "available_unverified");
    assert_eq!(report["coverage"]["status"], "not_checked");
    assert_eq!(report["coverage"]["reason"], "no_events_observed");
    assert_eq!(report["cursors"]["status"], "not_checked");
    assert_eq!(report["cursors"]["reason"], "no_cursor_records");
    for field in ["key", "collectors", "service", "permissions", "updates"] {
        assert_eq!(report[field]["status"], "not_checked", "{field}");
    }
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 4096);
    assert_eq!(fs::read(&path).unwrap(), before, "no migration or checkpoint");
    // A read-only SQLite WAL connection may create coordination sidecars;
    // it must never create event frames or change the database itself.
    if let Ok(wal) = fs::metadata(path.with_extension("sqlite3-wal")) {
        assert_eq!(wal.len(), 0);
    }
}

#[test]
fn prohibited_metadata_and_errors_never_reach_either_output_form() {
    let dir = private_dir();
    let canary = "PRIVATE_origin_command_title_credential";
    let path = dir.path().join(format!("{canary}.sqlite3"));
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
    journal.shutdown().unwrap();
    drop(journal);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("INSERT INTO diagnostics(code, detail, created_at) VALUES (?1, ?1, ?1)", [canary])
        .unwrap();
    connection.execute(
        "INSERT INTO policy_metadata(profile_id, profile_version, profile_json, recorded_at) VALUES (?1, 1, ?1, ?1)", [canary]
    ).unwrap();
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/health-prohibited-v1.json")).unwrap();
    let canaries: Vec<&str> = corpus["canaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    for item in &canaries {
        connection
            .execute(
                "INSERT INTO diagnostics(code, detail, created_at) VALUES (?1, ?1, ?1)",
                [item],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO policy_metadata(profile_id, profile_version, profile_json, recorded_at) VALUES (?1, 1, ?1, ?1)", [item]
        ).unwrap();
        connection.execute(
            "INSERT INTO events(event_id,schema_version,observed_at,ingested_at,source,kind,collector_instance,source_cursor,provenance_version,policy_profile_id,policy_profile_version,evidence,payload_ciphertext) VALUES (?1,1,?1,?1,?1,'gap',?1,?1,?1,?1,1,?1,?1)", [item]
        ).unwrap();
    }
    drop(connection);
    for json in [false, true] {
        let output = cli(&path, json);
        assert!(output.status.success());
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!text.contains(canary), "{text}");
        assert!(!text.contains(dir.path().to_str().unwrap()), "{text}");
        assert!(!text.contains("SQLite"), "raw error text is not a status: {text}");
        for item in &canaries {
            assert!(!text.contains(item), "{item}: {text}");
        }
        if json {
            let report = report(&output.stdout);
            assert_eq!(report["counts"]["events"], canaries.len());
            assert_eq!(report["coverage"]["status"], "needs_attention");
        }
    }
    let missing = dir.path().join(format!("{canary}-absent.sqlite3"));
    let output = cli(&missing, true);
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let report = report(&output.stdout);
    assert_eq!(report["journal"]["status"], "unavailable");
    assert!(report["counts"].is_null(), "unreadable is not zero");
    assert!(!String::from_utf8_lossy(&output.stdout).contains(canary));
    assert!(!missing.exists());
}

#[cfg(unix)]
#[test]
fn symlinks_and_nonprivate_files_are_fixed_refusals() {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let dir = private_dir();
    let path = dir.path().join("journal.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
    journal.shutdown().unwrap();
    drop(journal);
    let linked = dir.path().join("linked.sqlite3");
    symlink(&path, &linked).unwrap();
    for unsafe_path in [&linked, &path] {
        if unsafe_path == &path {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let output = cli(unsafe_path, true);
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        let report = report(&output.stdout);
        assert_eq!(report["journal"]["reason"], "unsafe_storage");
        assert!(report["counts"].is_null());
    }
}

#[test]
fn corrupt_and_foreign_formats_are_fixed_reports_not_raw_errors() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private_dir();
    let path = dir.path().join("credential-canary-corrupt.sqlite3");
    fs::write(&path, b"credential-canary not a database").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let output = cli(&path, true);
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let parsed = report(&output.stdout);
    assert_eq!(parsed["journal"]["reason"], "unreadable_storage");
    assert!(parsed["counts"].is_null());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("credential-canary"));
    fs::remove_file(&path).unwrap();
    let foreign = Connection::open(&path).unwrap();
    foreign.execute_batch("CREATE TABLE unrelated(value TEXT)").unwrap();
    drop(foreign);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let output = cli(&path, true);
    assert!(!output.status.success());
    assert_eq!(report(&output.stdout)["journal"]["reason"], "unsupported_format");
}

#[test]
fn incomplete_modified_downgraded_and_future_schemas_are_refused_without_migration() {
    for (sql, reason) in [
        ("DROP TABLE migration_records", "unreadable_storage"),
        ("UPDATE migration_records SET checksum='modified' WHERE version=1", "unreadable_storage"),
        ("DELETE FROM migration_records WHERE version=3", "unreadable_storage"),
        ("PRAGMA user_version=4", "unsupported_format"),
        ("PRAGMA user_version=999", "unsupported_format"),
        ("DELETE FROM schema_versions WHERE version=3", "unreadable_storage"),
        ("UPDATE migration_state SET state_value='unknown-mode'", "unreadable_storage"),
        ("DROP INDEX events_observed_at_idx; ALTER TABLE events DROP COLUMN observed_at", "unreadable_storage"),
        ("DROP TABLE authenticated_state", "unreadable_storage"),
        ("UPDATE migration_records SET tool_version=printf('%1000000s','credential-canary')", "unreadable_storage"),
        ("UPDATE migration_records SET tool_version=CAST('ok'||char(0)||zeroblob(1000000) AS TEXT)", "unreadable_storage"),
        ("UPDATE migration_records SET version=printf('%1000000s','credential-canary') WHERE version=1", "unreadable_storage"),
        ("UPDATE migration_records SET schema_version=CAST('noninteger'||char(0)||zeroblob(1000000) AS TEXT) WHERE version=1", "unreadable_storage"),
    ] {
        let dir = private_dir();
        let path = dir.path().join("journal.sqlite3");
        let journal =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
        journal.shutdown().unwrap();
        drop(journal);
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(sql).unwrap();
        drop(connection);
        let before = fs::read(&path).unwrap();
        let output = cli(&path, true);
        assert!(!output.status.success(), "accepted incompatible schema: {sql}");
        assert!(output.stderr.is_empty());
        let parsed = report(&output.stdout);
        if reason == "unreadable_storage" {
            assert!(matches!(parsed["journal"]["reason"].as_str(), Some("unreadable_storage" | "storage_budget_exceeded")), "{sql}: {parsed}");
        } else {
            assert_eq!(parsed["journal"]["reason"], reason, "{sql}");
        }
        let remediation = if reason == "unsupported_format" {
            "use_compatible_version"
        } else {
            "preserve_and_inspect_recovery"
        };
        assert_eq!(parsed["journal"]["remediation"], remediation, "{sql}");
        assert!(parsed["counts"].is_null());
        assert!(parsed["journal_schema_version"].is_null());
        assert_eq!(fs::read(&path).unwrap(), before, "must not repair or migrate: {sql}");
    }
}

#[test]
fn a_busy_database_reports_retry_not_incompatibility() {
    let dir = private_dir();
    let path = dir.path().join("journal.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
    journal.shutdown().unwrap();
    drop(journal);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("PRAGMA journal_mode=DELETE; BEGIN EXCLUSIVE").unwrap();
    let started = std::time::Instant::now();
    let output = cli(&path, true);
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let parsed = report(&output.stdout);
    assert_eq!(parsed["journal"]["reason"], "storage_busy");
    assert_eq!(parsed["journal"]["remediation"], "retry_at_quiet_time");
    assert!(parsed["counts"].is_null());
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    writer.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn large_sidecars_are_refused_before_sqlite_can_recover_them() {
    use std::os::unix::fs::PermissionsExt;
    for suffix in ["-wal", "-shm", "-journal"] {
        let dir = private_dir();
        let path = dir.path().join("journal.sqlite3");
        let journal =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
        journal.shutdown().unwrap();
        drop(journal);
        let before = fs::read(&path).unwrap();
        let sidecar = dir.path().join(format!("journal.sqlite3{suffix}"));
        let file = fs::File::create(&sidecar).unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
        file.set_len(64 * 1024 * 1024 + 1).unwrap();
        let output = cli(&path, true);
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        let parsed = report(&output.stdout);
        assert_eq!(parsed["journal"]["reason"], "storage_budget_exceeded", "{suffix}");
        assert_eq!(parsed["journal"]["remediation"], "preserve_and_inspect_recovery");
        assert!(parsed["counts"].is_null());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::metadata(&sidecar).unwrap().len(), 64 * 1024 * 1024 + 1);
    }
}

#[test]
fn oversized_compared_values_are_refused_without_partial_counts() {
    for sql in [
        "UPDATE migration_state SET state_value=printf('%1000000s','credential-canary')",
        "UPDATE journal_metadata SET metadata_value=printf('%1000000s','credential-canary') WHERE metadata_key='format'",
        "INSERT INTO cursors(source,collector_instance,source_cursor,updated_at,state) VALUES ('filesystem','canary','cursor','timestamp',printf('%1000000s','credential-canary'))",
        "INSERT INTO events(event_id,schema_version,observed_at,ingested_at,source,kind,collector_instance,source_cursor,provenance_version,policy_profile_id,policy_profile_version,evidence,payload_ciphertext) VALUES ('event',1,'timestamp','timestamp','filesystem',printf('%1000000s','credential-canary'),'canary','cursor','v1','policy',1,'unknown',X'00')",
    ] {
        let dir = private_dir();
        let path = dir.path().join("journal.sqlite3");
        let journal =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("health")).unwrap();
        journal.shutdown().unwrap();
        drop(journal);
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(sql).unwrap();
        drop(connection);
        let output = cli(&path, true);
        assert!(!output.status.success(), "accepted unbounded compared value: {sql}");
        assert!(output.stderr.is_empty());
        let parsed = report(&output.stdout);
        assert!(parsed["counts"].is_null());
        assert!(parsed["journal_schema_version"].is_null());
        assert_eq!(parsed["journal"]["remediation"], "preserve_and_inspect_recovery");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("credential-canary"));
    }
}

// APFS rejects non-UTF-8 filenames with EILSEQ; the byte-preserving path helper
// is tested on every Unix host, and actual filesystem refusal on Linux.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_sidecar_names_are_checked_without_lossy_conversion() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    };
    let dir = private_dir();
    let path = dir.path().join(OsString::from_vec(b"journal-\xff.sqlite3".to_vec()));
    fs::write(&path, b"unused-before-sidecar-refusal").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut name = path.as_os_str().to_os_string();
    name.push("-wal");
    let sidecar = std::path::PathBuf::from(name);
    let file = fs::File::create(sidecar).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    // Test the library boundary because argument parsing may reject non-UTF-8
    // text before it reaches the filesystem.
    let parsed = serde_json::to_value(ghostrace::health::HealthReport::inspect(&path)).unwrap();
    assert!(parsed["counts"].is_null());
    assert_eq!(parsed["journal"]["reason"], "storage_budget_exceeded");
}
