use std::fs;

use chrono::{TimeZone, Utc};
use ghostrace::{
    CursorIdentity, CursorStreamMode, DeterministicKeyProvider, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, FaultPlan, FaultPoint, GapPayload, GhostraceError,
    IngestionOrigin, Journal, PolicyProfile, ReasonCode, ReplayBoundary, ReplayConfiguration,
    SnapshotDigest, SourceCursor, VolumeIdentity,
};
use tempfile::tempdir;
use uuid::Uuid;

fn digest(hex: char) -> SnapshotDigest {
    SnapshotDigest::try_from(format!("sha256:{}", hex.to_string().repeat(64))).expect("digest")
}

fn policy() -> PolicyProfile {
    let mut policy = PolicyProfile::deny_by_default("replay-boundary-policy");
    policy.enable_source(EventSource::Filesystem);
    policy
}

fn origin() -> IngestionOrigin {
    IngestionOrigin::fixture_instance("fixture-boundary-fs").expect("origin")
}

fn identity() -> CursorIdentity {
    CursorIdentity::for_volume(
        EventSource::Filesystem,
        "fixture-boundary-fs",
        CursorStreamMode::PerHost,
        VolumeIdentity::synthetic("volume-a"),
    )
    .expect("identity")
}

fn boundary() -> ReplayBoundary {
    ReplayBoundary::new(
        identity(),
        ReplayConfiguration::new(
            digest('a'),
            digest('b'),
            41,
            std::time::Duration::from_millis(250),
            true,
        )
        .expect("configuration"),
    )
    .expect("boundary")
}

fn event(id: u128, cursor: &str) -> EventEnvelope {
    let timestamp = Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp");
    EventEnvelope::new(
        &origin(),
        Uuid::from_u128(id),
        timestamp,
        timestamp,
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(GapPayload {
            source: EventSource::Filesystem,
            reason_code: ReasonCode::try_from("replay_boundary").expect("reason"),
            dropped_count: id as u64,
            from_cursor: None,
            to_cursor: None,
            volume_digest: None,
            root_ids: Vec::new(),
            remediation: None,
        }),
        Some(SourceCursor::try_from(cursor).expect("cursor")),
        "replay-boundary-policy",
        1,
        Evidence::Unknown,
        None,
    )
    .expect("event")
}

fn private_tempdir() -> tempfile::TempDir {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private tempdir");
    }
    directory
}

#[test]
fn every_replay_setting_change_refuses_without_writing() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("boundary-settings"))
        .expect("journal");
    let policy = policy();
    let base = boundary();
    let first = event(1, "cursor-1");
    assert_eq!(journal.ingest_with_boundary(&origin(), &first, &policy, &base).expect("first"), 1);

    let mut changed = [
        ("root", base.clone()),
        ("exclusions", base.clone()),
        ("since-when", base.clone()),
        ("latency", base.clone()),
        ("file-events", base.clone()),
    ];
    changed[0].1.configuration.root_scope_digest = digest('c');
    changed[1].1.configuration.exclusions_digest = digest('d');
    changed[2].1.configuration.since_when += 1;
    changed[3].1.configuration.latency_millis += 1;
    changed[4].1.configuration.file_events = false;

    for (name, candidate) in changed {
        let error = journal
            .ingest_with_boundary(
                &origin(),
                &event(10 + u128::from(name.len() as u8), "cursor-2"),
                &policy,
                &candidate,
            )
            .expect_err(name);
        assert!(matches!(error, GhostraceError::CursorBoundaryMismatch { .. }), "{name}: {error}");
        assert_eq!(journal.events().expect("events").len(), 1, "{name} wrote an event");
    }
    let state = journal.cursor_state(&identity()).expect("state").expect("stored boundary");
    assert_eq!(state.boundary, Some(base));
}

#[test]
fn restart_replays_idempotently_and_advances_atomically_with_boundary() {
    let directory = private_tempdir();
    let path = directory.path().join("replay-boundary.sqlite3");
    let base = boundary();
    let policy = policy();
    let first = event(20, "cursor-1");
    let second = event(21, "cursor-2");
    let first_sequence = {
        let journal =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("boundary-restart"))
                .expect("open");
        journal.ingest_with_boundary(&origin(), &first, &policy, &base).expect("first")
    };
    let reopened =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("boundary-restart"))
            .expect("reopen");
    assert_eq!(
        reopened.ingest_with_boundary(&origin(), &first, &policy, &base).expect("duplicate"),
        first_sequence
    );
    assert_eq!(
        reopened.ingest_with_boundary(&origin(), &second, &policy, &base).expect("second"),
        2
    );
    let state = reopened.cursor_state(&identity()).expect("state").expect("cursor");
    assert_eq!(state.token.raw().as_str(), "cursor-2");
    assert_eq!(state.boundary, Some(base));

    let atomic = Journal::in_memory(DeterministicKeyProvider::from_seed("boundary-atomic"))
        .expect("journal")
        .with_fault_plan(FaultPlan::fail_once(FaultPoint::CursorBeforeUpdate));
    let error = atomic
        .ingest_with_boundary(&origin(), &event(30, "cursor-1"), &policy, &boundary())
        .expect_err("cursor fault");
    assert!(matches!(error, GhostraceError::InjectedFault { .. }));
    assert!(atomic.events().expect("rolled back events").is_empty());
    assert!(atomic.cursor_state(&identity()).expect("rolled back cursor").is_none());
    let recovered = atomic.with_fault_plan(FaultPlan::none());
    assert_eq!(
        recovered
            .ingest_with_boundary(&origin(), &event(30, "cursor-1"), &policy, &boundary())
            .expect("retry"),
        1
    );
}

#[test]
fn boundary_serialization_is_path_free_and_digest_stable() {
    let boundary = boundary();
    let json = serde_json::to_string(&boundary).expect("json");
    assert!(!json.contains("/"));
    assert!(!json.contains("volume-a"));
    assert_eq!(boundary.digest().expect("digest"), boundary.digest().expect("digest"));
    let decoded: ReplayBoundary = serde_json::from_str(&json).expect("round trip");
    assert_eq!(decoded, boundary);
}
