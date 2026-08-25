use ghostrace::{
    normalize_fsevents_event, FseventsCompleteness, FseventsEvent, FseventsEventFlag,
    FseventsEvidenceStatus, FseventsRescanReason, NormalizedFseventsEvent,
    EVENT_FLAG_EVENT_IDS_WRAPPED, EVENT_FLAG_HISTORY_DONE, EVENT_FLAG_ITEM_CLONED,
    EVENT_FLAG_ITEM_CREATED, EVENT_FLAG_ITEM_IS_DIR, EVENT_FLAG_ITEM_IS_FILE,
    EVENT_FLAG_ITEM_MODIFIED, EVENT_FLAG_ITEM_RENAMED, EVENT_FLAG_KERNEL_DROPPED,
    EVENT_FLAG_MUST_SCAN_SUB_DIRS, EVENT_FLAG_USER_DROPPED, FSEVENTS_NORMALIZED_SCHEMA_JSON,
};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn golden_callback_batches_preserve_compound_and_loss_status() {
    let observed = normalize_fsevents_event(
        0x100,
        EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_CLONED,
    );
    assert_eq!(
        serde_json::to_value(&observed).expect("observed JSON"),
        json!({
            "schema_version": 1,
            "event_id": 256,
            "flags": {
                "raw_flags": 4260096,
                "known_flags": ["item_created", "item_is_file", "item_cloned"],
                "unknown_bits": 0
            },
            "status": {"status": "observed"},
            "completeness": "complete"
        })
    );

    let dropped = normalize_fsevents_event(
        0x101,
        EVENT_FLAG_MUST_SCAN_SUB_DIRS
            | EVENT_FLAG_USER_DROPPED
            | EVENT_FLAG_KERNEL_DROPPED
            | EVENT_FLAG_ITEM_IS_DIR,
    );
    assert_eq!(
        dropped.status,
        FseventsEvidenceStatus::RescanRequired { reason: FseventsRescanReason::BothDropped }
    );
    assert_eq!(dropped.completeness, FseventsCompleteness::Lowered);
    assert!(dropped.flags.contains(FseventsEventFlag::MustScanSubDirs));
}

#[test]
fn golden_callback_batches_refuse_contradictions_and_unknown_future_bits() {
    let contradictory = normalize_fsevents_event(
        0x102,
        EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_IS_DIR | EVENT_FLAG_ITEM_RENAMED,
    );
    assert_eq!(
        serde_json::to_value(&contradictory).expect("contradictory JSON")["status"],
        json!({"status": "contradictory", "reason": "multiple_entry_kinds"})
    );
    assert_eq!(contradictory.flags.unknown_bits, 0);

    let future = normalize_fsevents_event(0x103, EVENT_FLAG_ITEM_MODIFIED | 0x8000_0000);
    assert_eq!(future.flags.unknown_bits, 0x8000_0000);
    assert_eq!(future.status_code(), "unsupported");
    assert!(!future.is_complete());

    let boundary =
        normalize_fsevents_event(0x104, EVENT_FLAG_HISTORY_DONE | EVENT_FLAG_EVENT_IDS_WRAPPED);
    assert_eq!(boundary.status_code(), "rescan_required");
    assert!(matches!(
        boundary.status,
        FseventsEvidenceStatus::RescanRequired { reason: FseventsRescanReason::EventIdsWrapped }
    ));
}

#[test]
fn stream_event_exposes_path_free_normalization() {
    let event = FseventsEvent {
        path: PathBuf::from("/private/tmp/synthetic-event"),
        event_id: 55,
        flags: EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE,
    };
    let normalized = event.normalize_flags();
    assert_eq!(normalized.event_id, 55);
    assert!(!serde_json::to_string(&normalized)
        .expect("normalized JSON")
        .contains("synthetic-event"));
}

#[test]
fn normalized_schema_is_strict_and_accepts_every_golden_batch() {
    let schema: serde_json::Value =
        serde_json::from_str(FSEVENTS_NORMALIZED_SCHEMA_JSON).expect("normalized schema JSON");
    let validator = jsonschema::options().build(&schema).expect("valid normalized schema");
    for raw in [
        EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_CLONED,
        EVENT_FLAG_MUST_SCAN_SUB_DIRS | EVENT_FLAG_USER_DROPPED | EVENT_FLAG_KERNEL_DROPPED,
        EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_IS_DIR | EVENT_FLAG_ITEM_RENAMED,
        EVENT_FLAG_ITEM_MODIFIED | 0x8000_0000,
    ] {
        let value =
            serde_json::to_value(normalize_fsevents_event(1, raw)).expect("normalized value");
        assert!(validator.is_valid(&value), "schema rejected raw flag word {raw:#x}");
        let _: NormalizedFseventsEvent =
            serde_json::from_value(value).expect("typed normalized value");
    }
    let invalid = json!({
        "schema_version": 1,
        "event_id": 1,
        "flags": {"raw_flags": 1, "known_flags": [], "unknown_bits": 0},
        "status": {"status": "observed"},
        "completeness": "complete",
        "private_path": "/Users/example"
    });
    assert!(!validator.is_valid(&invalid));
    assert!(serde_json::from_value::<NormalizedFseventsEvent>(invalid).is_err());
}
