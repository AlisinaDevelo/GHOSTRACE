use std::{fs, path::Path};

use chrono::{TimeZone, Utc};
use ghostrace::{
    DeterministicKeyProvider, EventEnvelope, EventKind, EventPayload, EventSource, Evidence,
    GhostraceError, IngestionOrigin, Journal, PolicyProfile, ReasonCode, RepairInterval,
    MAX_REPAIR_INTERVAL_EVENTS,
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
    let mut policy = PolicyProfile::deny_by_default("recovery-test-policy");
    policy.enable_source(EventSource::Filesystem);
    policy
}

fn event(number: u128) -> EventEnvelope {
    let origin = IngestionOrigin::fixture_instance("fixture-recovery-test-source").expect("origin");
    let timestamp = Utc.timestamp_opt(1_735_700_000 + number as i64, 0).single().expect("time");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(number),
        timestamp,
        timestamp,
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(ghostrace::GapPayload {
            source: EventSource::Filesystem,
            reason_code: ReasonCode::try_from("recovery_test").expect("reason"),
            dropped_count: number as u64,
            from_cursor: None,
            to_cursor: None,
            volume_digest: None,
            root_ids: Vec::new(),
            remediation: None,
        }),
        None,
        "recovery-test-policy",
        1,
        Evidence::Direct,
        None,
    )
    .expect("event")
}

fn open(path: &Path) -> Journal {
    Journal::open_fixture(path, DeterministicKeyProvider::from_seed("recovery-test-key"))
        .expect("journal")
}

#[test]
fn checkpoint_binds_state_and_rejects_tampering_or_mutation() {
    let (_directory, path) = private_path("checkpoint.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1), &policy()).expect("first");
    let checkpoint = journal.create_checkpoint().expect("checkpoint");
    checkpoint.validate().expect("valid checkpoint");
    journal.verify_checkpoint(&checkpoint).expect("checkpoint verifies");
    let json = serde_json::to_string(&checkpoint).expect("checkpoint JSON");
    assert!(!json.contains(path.to_string_lossy().as_ref()));
    assert!(!json.contains("recovery-test-key"));

    let mut tampered = checkpoint.clone();
    tampered.head_mac.replace_range(0..1, "0");
    assert!(matches!(
        journal.verify_checkpoint(&tampered),
        Err(GhostraceError::CheckpointMismatch(_)) | Err(GhostraceError::CheckpointInvalid(_))
    ));

    journal.ingest(&IngestionOrigin::fixture(), &event(2), &policy()).expect("second");
    assert!(matches!(
        journal.verify_checkpoint(&checkpoint),
        Err(GhostraceError::CheckpointMismatch(_))
    ));
}

#[test]
fn repair_is_copy_only_emits_reconciled_manifest_and_keeps_source() {
    let (directory, source) = private_path("source.sqlite3");
    let destination = directory.path().join("repaired.sqlite3");
    let journal = open(&source);
    journal.ingest(&IngestionOrigin::fixture(), &event(1), &policy()).expect("first");
    journal.ingest(&IngestionOrigin::fixture(), &event(2), &policy()).expect("second");
    let interval = RepairInterval::new(EventSource::Filesystem, 1, 1).expect("interval");

    let manifest = journal.repair_verified_copy(&destination, &[interval]).expect("repair copy");
    manifest.validate().expect("manifest validates");
    assert!(manifest.verified_copy);
    assert_eq!(manifest.before.event_count, 2);
    assert_eq!(manifest.after.event_count, 2);
    assert_eq!(manifest.dropped_event_count, 1);
    assert_eq!(manifest.reconstructed_event_count, 0);
    assert_eq!(manifest.gap_event_count, 1);
    assert_eq!(journal.events().expect("source events").len(), 2);
    assert_eq!(journal.gap_event_count().expect("source gaps"), 0);

    let repaired = open(&destination);
    repaired.verify_authenticated_state().expect("repaired auth");
    assert_eq!(repaired.events().expect("repaired events").len(), 2);
    assert_eq!(repaired.gap_event_count().expect("repair gap"), 1);
    let json = serde_json::to_string(&manifest).expect("manifest JSON");
    assert!(!json.contains(source.to_string_lossy().as_ref()));
    assert!(!json.contains("recovery-test-key"));
    assert!(!json.contains("00000000-0000-4000-8000-000000000001"));
}

#[test]
fn integrity_failure_stops_normal_writer() {
    let (_directory, path) = private_path("integrity.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1), &policy()).expect("first");
    Connection::open(&path)
        .expect("tamper connection")
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             UPDATE events SET parent_event_id = '00000000-0000-4000-8000-000000000099'
             WHERE ingest_seq = 1;",
        )
        .expect("tamper foreign key");
    let report = journal.integrity_check().expect("integrity report");
    assert!(!report.integrity_ok);
    assert!(matches!(
        journal.ingest(&IngestionOrigin::fixture(), &event(2), &policy()),
        Err(GhostraceError::IntegrityReportInvalid(_))
    ));
}

#[test]
fn repair_intervals_are_bounded_and_non_overlapping() {
    assert!(
        RepairInterval::new(EventSource::Filesystem, 1, MAX_REPAIR_INTERVAL_EVENTS + 1).is_err()
    );
    assert!(RepairInterval::new(EventSource::Fixture, 1, 1).is_err());
    let (_directory, path) = private_path("overlap.sqlite3");
    let journal = open(&path);
    journal.ingest(&IngestionOrigin::fixture(), &event(1), &policy()).expect("first");
    journal.ingest(&IngestionOrigin::fixture(), &event(2), &policy()).expect("second");
    let first = RepairInterval::new(EventSource::Filesystem, 1, 1).expect("first interval");
    let overlap = RepairInterval::new(EventSource::Filesystem, 1, 2).expect("overlap interval");
    let destination = path.with_file_name("overlap-repaired.sqlite3");
    assert!(matches!(
        journal.repair_verified_copy(&destination, &[first, overlap]),
        Err(GhostraceError::RepairRefused(_))
    ));
}
