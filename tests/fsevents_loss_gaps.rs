use ghostrace::{
    normalize_fsevents_event, EventSource, FseventsOptions, GapPayload, GapRemediation, RootId,
    SourceCursor, VolumeIdentity, EVENT_FLAG_EVENT_IDS_WRAPPED, EVENT_FLAG_KERNEL_DROPPED,
    EVENT_FLAG_MUST_SCAN_SUB_DIRS, EVENT_FLAG_ROOT_CHANGED, EVENT_FLAG_USER_DROPPED,
    FLAG_WATCH_ROOT,
};

#[test]
fn coverage_flags_have_distinct_gap_reason_codes() {
    let cases = [
        (EVENT_FLAG_USER_DROPPED, "fsevents_user_dropped"),
        (EVENT_FLAG_KERNEL_DROPPED, "fsevents_kernel_dropped"),
        (EVENT_FLAG_EVENT_IDS_WRAPPED, "fsevents_event_ids_wrapped"),
        (EVENT_FLAG_ROOT_CHANGED, "fsevents_root_changed"),
        (EVENT_FLAG_MUST_SCAN_SUB_DIRS, "fsevents_must_scan_sub_dirs"),
    ];

    for (flags, expected) in cases {
        let normalized = normalize_fsevents_event(42, flags);
        assert_eq!(normalized.gap_reason_code(), Some(expected), "flags {flags:#x}");
    }
}

#[test]
fn selected_root_defaults_request_watch_root() {
    assert_ne!(FseventsOptions::default().flags & FLAG_WATCH_ROOT, 0);
}

#[test]
fn gap_payload_retains_bounded_recovery_context_without_paths() {
    let payload = GapPayload {
        source: EventSource::Filesystem,
        reason_code: "fsevents_root_changed".try_into().expect("reason"),
        dropped_count: 0,
        from_cursor: Some(SourceCursor::try_from("cursor-41").expect("cursor")),
        to_cursor: Some(SourceCursor::try_from("cursor-42").expect("cursor")),
        volume_digest: Some(VolumeIdentity::synthetic("loss-gap-volume").fingerprint()),
        root_ids: vec![RootId::try_from("root-main").expect("root")],
        remediation: Some(GapRemediation::ReconcileSelectedRoot),
    };

    let json = serde_json::to_string(&payload).expect("serialize gap");
    assert!(!json.contains("loss-gap-volume"));
    assert!(!json.contains("/"));
    let round_trip: GapPayload = serde_json::from_str(&json).expect("deserialize gap");
    assert_eq!(round_trip, payload);
    assert!(json.contains("reconcile_selected_root"));
    assert!(json.contains("sha256:"));
}
