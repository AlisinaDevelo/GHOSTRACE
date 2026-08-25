use ghostrace::{
    FseventsOptions, HistoryCursorRange, SourceCursor, StartupCursor, StartupCursorDecision,
    StartupCursorError, StartupCursorRejection, EVENT_ID_SINCE_NOW, FSEVENTS_STARTUP_SCHEMA_JSON,
};
use serde_json::json;

#[test]
fn startup_cursor_decisions_make_since_now_and_replay_explicit() {
    let since_now =
        FseventsOptions { since_when: EVENT_ID_SINCE_NOW, ..FseventsOptions::default() };
    assert_eq!(
        since_now.startup_decision(None).expect("since now"),
        StartupCursorDecision::SinceNow
    );

    let replay = FseventsOptions { since_when: 150, ..FseventsOptions::default() };
    let range = HistoryCursorRange::new(100, 200).expect("history range");
    assert_eq!(
        replay.startup_decision(Some(range)).expect("replay"),
        StartupCursorDecision::Replay { event_id: 150 }
    );
}

#[test]
fn startup_cursor_refuses_zero_stale_future_wrapped_and_corrupt_inputs() {
    let range = HistoryCursorRange::new(100, 200).expect("history range");
    for (since_when, reason) in [
        (0, StartupCursorRejection::Zero),
        (99, StartupCursorRejection::Stale),
        (201, StartupCursorRejection::Future),
    ] {
        let options = FseventsOptions { since_when, ..FseventsOptions::default() };
        assert_eq!(
            options.startup_decision(Some(range)).expect_err("refused cursor"),
            StartupCursorError::Refused(reason)
        );
    }

    let wrapped = SourceCursor::try_from("wrap-2-1").expect("wrapped cursor");
    assert_eq!(
        StartupCursor::from_source_cursor(&wrapped).expect_err("wrapped cursor"),
        StartupCursorError::Refused(StartupCursorRejection::Wrapped)
    );
    let corrupt = SourceCursor::try_from("opaque-token").expect("opaque cursor");
    assert_eq!(
        StartupCursor::from_source_cursor(&corrupt).expect_err("corrupt cursor"),
        StartupCursorError::Refused(StartupCursorRejection::Corrupted)
    );
}

#[test]
fn startup_cursor_ranges_are_bounded_and_zero_is_not_a_history_boundary() {
    assert_eq!(
        HistoryCursorRange::new(0, 10).expect_err("zero lower bound"),
        StartupCursorError::InvalidRange
    );
    assert_eq!(
        HistoryCursorRange::new(10, 9).expect_err("reversed range"),
        StartupCursorError::InvalidRange
    );
}

#[test]
fn startup_schema_is_strict_at_the_public_boundary() {
    let schema: serde_json::Value =
        serde_json::from_str(FSEVENTS_STARTUP_SCHEMA_JSON).expect("startup schema JSON");
    let validator = jsonschema::options().build(&schema).expect("valid startup schema");
    assert!(validator.is_valid(&json!({
        "schema_version": 1,
        "cursor": {"kind": "since_now"},
        "decision": {"mode": "since_now"}
    })));
    assert!(validator.is_valid(&json!({
        "schema_version": 1,
        "cursor": {"kind": "event_id", "event_id": 42},
        "decision": {"mode": "replay", "event_id": 42}
    })));
    assert!(!validator.is_valid(&json!({
        "schema_version": 1,
        "cursor": {"kind": "event_id", "event_id": 0},
        "decision": {"mode": "replay", "event_id": 0}
    })));
    assert!(!validator.is_valid(&json!({
        "schema_version": 1,
        "cursor": {"kind": "since_now", "extra": true},
        "decision": {"mode": "since_now"}
    })));
}
