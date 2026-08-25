use std::{fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use chrono::{TimeZone, Utc};
use ghostrace::{
    CursorIdentity, CursorKind, CursorOrder, CursorStatus, CursorToken, DeterministicKeyProvider,
    EventEnvelope, EventKind, EventPayload, EventSource, Evidence, GapPayload, GhostraceError,
    IngestionOrigin, Journal, PolicyProfile, ReasonCode, SourceCursor, SourceErrorPayload,
};
use tempfile::tempdir;
use uuid::Uuid;

fn policy(version: u32) -> PolicyProfile {
    let mut profile = PolicyProfile::deny_by_default("cursor-policy");
    profile.version = version;
    profile.enable_source(EventSource::Filesystem);
    profile
}

fn event(instance: &str, id: u128, cursor: &str, version: u32) -> EventEnvelope {
    let origin = IngestionOrigin::fixture_instance(instance).expect("fixture origin");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp"),
        Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp"),
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(GapPayload {
            source: EventSource::Filesystem,
            reason_code: ReasonCode::try_from("dropped").expect("reason"),
            dropped_count: id as u64,
            from_cursor: None,
            to_cursor: None,
            volume_digest: None,
            root_ids: Vec::new(),
            remediation: None,
        }),
        Some(SourceCursor::try_from(cursor).expect("cursor")),
        "cursor-policy",
        version,
        Evidence::Direct,
        None,
    )
    .expect("event")
}

fn open(path: &Path) -> Journal {
    Journal::open_fixture(path, DeterministicKeyProvider::from_seed("cursor-contract"))
        .expect("journal")
}

fn private_tempdir() -> tempfile::TempDir {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private tempdir");
    directory
}

#[test]
fn cursor_types_make_identity_ordering_reset_wrap_and_invalidation_explicit() {
    let identity =
        CursorIdentity::new(EventSource::Filesystem, "fixture-cursor").expect("identity");
    assert_eq!(identity.collector_instance(), "fixture-cursor");

    let first = CursorToken::new(SourceCursor::try_from("cursor-1").expect("cursor"));
    let second = CursorToken::new(SourceCursor::try_from("cursor-2").expect("cursor"));
    let reset = CursorToken::new(SourceCursor::try_from("reset-1-0").expect("cursor"));
    let wrapped = CursorToken::new(SourceCursor::try_from("wrap-2-0").expect("cursor"));
    assert_eq!(first.kind(), CursorKind::Sequence);
    assert_eq!(first.compare(&second), CursorOrder::Advance);
    assert_eq!(second.compare(&first), CursorOrder::Regression);
    assert_eq!(second.compare(&reset), CursorOrder::Advance);
    assert_eq!(reset.kind(), CursorKind::Reset);
    assert_eq!(wrapped.kind(), CursorKind::Wrap);

    let opaque = CursorToken::new(SourceCursor::try_from("opaque-a").expect("cursor"));
    let opaque_next = CursorToken::new(SourceCursor::try_from("opaque-b").expect("cursor"));
    assert_eq!(opaque.compare(&opaque_next), CursorOrder::Unknown);
    assert!(!identity.collector_instance().is_empty());
}

#[test]
fn cursor_ordering_property_covers_reordering_replay_and_unknown_streams() {
    let positions: Vec<CursorToken> = (0..128)
        .map(|position| {
            CursorToken::new(
                SourceCursor::try_from(format!("seq-4-{position}")).expect("generated cursor"),
            )
        })
        .collect();
    for window in positions.windows(2) {
        assert_eq!(window[0].compare(&window[1]), CursorOrder::Advance);
        assert_eq!(window[1].compare(&window[0]), CursorOrder::Regression);
    }
    for (index, candidate) in positions.iter().enumerate() {
        assert_eq!(candidate.compare(&positions[index]), CursorOrder::Equal);
    }
    for index in (1..positions.len()).rev() {
        assert_eq!(positions[index].compare(&positions[index - 1]), CursorOrder::Regression);
    }
}

#[test]
fn journal_replay_is_idempotent_and_conflicts_fail_closed() {
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("cursor-journal")).expect("journal");
    let origin = IngestionOrigin::fixture();
    let p = policy(1);
    let first = event("fixture-cursor", 1, "cursor-1", 1);
    let second = event("fixture-cursor", 2, "cursor-2", 1);
    let first_seq = journal.ingest(&origin, &first, &p).expect("first");
    let second_seq = journal.ingest(&origin, &second, &p).expect("second");
    assert_eq!(journal.ingest(&origin, &first, &p).expect("replay"), first_seq);
    assert_eq!(journal.events().expect("events").len(), 2);

    let divergent_same_cursor = event("fixture-cursor", 3, "cursor-2", 1);
    assert!(matches!(
        journal.ingest(&origin, &divergent_same_cursor, &p),
        Err(GhostraceError::CursorConflict { event_source: EventSource::Filesystem })
    ));
    let regressed = event("fixture-cursor", 4, "cursor-0", 1);
    assert!(matches!(
        journal.ingest(&origin, &regressed, &p),
        Err(GhostraceError::CursorRegression { event_source: EventSource::Filesystem })
    ));
    let mut skipped = event("fixture-cursor", 6, "cursor-4", 1);
    skipped.kind = EventKind::SourceError;
    skipped.payload = EventPayload::SourceError(SourceErrorPayload {
        source: EventSource::Filesystem,
        reason_code: ReasonCode::try_from("source_unavailable").expect("reason"),
        retryable: true,
    });
    assert!(matches!(
        journal.ingest(&origin, &skipped, &p),
        Err(GhostraceError::CursorSkipped { event_source: EventSource::Filesystem })
    ));
    let unknown = event("fixture-cursor", 5, "opaque-next", 1);
    assert!(matches!(
        journal.ingest(&origin, &unknown, &p),
        Err(GhostraceError::CursorOrderingUnknown { event_source: EventSource::Filesystem })
    ));
    assert_eq!(journal.events().expect("events").len(), 2);
    assert!(second_seq > first_seq);
    let state = journal
        .cursor_state(
            &CursorIdentity::new(EventSource::Filesystem, "fixture-cursor").expect("identity"),
        )
        .expect("state")
        .expect("cursor state");
    assert_eq!(state.status, CursorStatus::Active);
    assert_eq!(state.token.raw().as_str(), "cursor-2");
}

#[test]
fn source_replacement_policy_change_and_crash_recovery_are_separate_gates() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("cursor-replacement"))
        .expect("journal");
    let origin = IngestionOrigin::fixture();
    let p1 = policy(1);
    journal
        .ingest(&origin, &event("fixture-cursor-a", 10, "cursor-1", 1), &p1)
        .expect("first source");
    journal
        .ingest(&origin, &event("fixture-cursor-b", 11, "cursor-1", 1), &p1)
        .expect("replacement source has independent identity");
    let p2 = policy(2);
    assert!(matches!(
        journal.ingest(&origin, &event("fixture-cursor-a", 12, "cursor-2", 2), &p2),
        Err(GhostraceError::CursorPolicyMismatch { event_source: EventSource::Filesystem })
    ));
    assert_eq!(journal.events().expect("events").len(), 2);

    let directory = private_tempdir();
    let path = directory.path().join("cursor.sqlite3");
    let first = event("fixture-crash", 20, "cursor-1", 1);
    let first_seq = {
        let disk = open(&path);
        disk.ingest(&IngestionOrigin::fixture(), &first, &p1).expect("disk first")
    };
    let reopened = open(&path);
    assert_eq!(
        reopened.ingest(&IngestionOrigin::fixture(), &first, &p1).expect("replay after reopen"),
        first_seq
    );
    assert_eq!(reopened.events().expect("reopened events").len(), 1);
    fs::metadata(&path).expect("journal exists");
}

#[test]
fn reset_wrap_and_invalidate_controls_require_typed_epochs() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("cursor-controls"))
        .expect("journal");
    let origin = IngestionOrigin::fixture();
    let p = policy(1);
    let identity =
        CursorIdentity::new(EventSource::Filesystem, "fixture-control").expect("identity");
    journal.ingest(&origin, &event("fixture-control", 30, "cursor-1", 1), &p).expect("initial");
    journal.invalidate_cursor(&identity).expect("invalidate");
    assert!(matches!(
        journal.ingest(&origin, &event("fixture-control", 31, "cursor-2", 1), &p),
        Err(GhostraceError::CursorInvalidated { event_source: EventSource::Filesystem })
    ));
    journal
        .reset_cursor(&identity, &SourceCursor::try_from("reset-1-0").expect("reset cursor"), &p)
        .expect("reset");
    journal
        .ingest(&origin, &event("fixture-control", 32, "seq-1-1", 1), &p)
        .expect("first event after reset");
    journal
        .wrap_cursor(&identity, &SourceCursor::try_from("wrap-2-0").expect("wrap cursor"), &p)
        .expect("wrap");
    journal
        .ingest(&origin, &event("fixture-control", 33, "seq-2-1", 1), &p)
        .expect("first event after wrap");
    let state = journal.cursor_state(&identity).expect("state").expect("cursor state");
    assert_eq!(state.status, CursorStatus::Active);
    assert_eq!(state.epoch, 2);
    assert!(Journal::in_memory(DeterministicKeyProvider::from_seed("unused"))
        .expect("journal")
        .reset_cursor(&identity, &SourceCursor::try_from("opaque-reset").expect("cursor"), &p)
        .is_err());
}
