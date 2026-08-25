use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{TimeZone, Utc};
use ghostrace::{
    ConsentPreview, CursorIdentity, DeterministicKeyProvider, EventSource, FseventsCollector,
    FseventsCollectorConfig, FseventsCollectorError, FseventsOptions, Journal, PolicyDocument,
    SelectedRoot, StartupCursor, StartupCursorDecision, WriterConfig,
};
use tempfile::tempdir;

#[cfg(target_os = "macos")]
use ghostrace::{
    CollectorCoverageState, CollectorState, EventKind, EventPayload, Evidence, FseventsEvent,
    FseventsStream, GapRemediation,
};

#[cfg(not(target_os = "macos"))]
use ghostrace::FseventsError;

fn policy() -> PolicyDocument {
    PolicyDocument::new(
        "live-filesystem-v1",
        1,
        [EventSource::Filesystem, EventSource::Lifecycle],
        ["root-main"],
        false,
    )
    .expect("policy")
}

fn config() -> FseventsCollectorConfig {
    FseventsCollectorConfig {
        options: FseventsOptions {
            latency: std::time::Duration::from_millis(20),
            ..FseventsOptions::default()
        },
        writer: WriterConfig::default(),
        collector_instance: "live-fsevents-test".to_owned(),
        instance_label: "selected-root-test".to_owned(),
        consent_at: Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
        actor: "human".to_owned(),
        reason: "root_opt_in".to_owned(),
        history_timeout: std::time::Duration::from_millis(50),
    }
}

fn confirmation(document: &PolicyDocument) -> ghostrace::ConsentConfirmation {
    ConsentPreview::from_policy(
        document,
        ["path_digest", "operation", "entry_kind"],
        ["fsevents_coalescing", "no_process_attribution", "history_can_be_dropped"],
    )
    .expect("preview")
    .confirm()
}

#[cfg(target_os = "macos")]
fn has_operation(
    events: &[ghostrace::CollectedFilesystemEvent],
    operation: ghostrace::FileOperation,
) -> bool {
    events.iter().any(|event| event.operation == operation)
}

#[cfg(not(target_os = "macos"))]
#[test]
fn collector_is_an_explicit_non_macos_no_go() {
    let directory = tempdir().expect("tempdir");
    let root = SelectedRoot::new("root-main", directory.path()).expect("selected root");
    let result = FseventsCollector::new(
        confirmation(&policy()),
        policy(),
        [root],
        Journal::in_memory(DeterministicKeyProvider::from_seed("collector-no-go"))
            .expect("journal"),
        config(),
    );
    assert!(matches!(
        result,
        Err(FseventsCollectorError::Stream(FseventsError::UnsupportedPlatform))
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn selected_root_collector_captures_controlled_file_lifecycle_without_content() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("collector-device"))
        .expect("journal");
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, config())
            .expect("collector");

    assert_eq!(collector.status().state, CollectorState::Created);
    collector.start().expect("start collector");
    assert_eq!(collector.status().state, ghostrace::CollectorState::Running);

    let tracked = root_path.join("tracked.txt");
    fs::write(&tracked, b"collector-test-secret").expect("create file");
    let mut operations = Vec::new();
    for _ in 0..40 {
        operations.extend(
            collector
                .run_current_run_loop_for(std::time::Duration::from_millis(100))
                .expect("drive create"),
        );
        if operations.iter().any(|event| event.operation == ghostrace::FileOperation::Created) {
            break;
        }
    }

    fs::write(&tracked, b"collector-test-secret-updated").expect("modify file");
    for _ in 0..40 {
        operations.extend(
            collector
                .run_current_run_loop_for(std::time::Duration::from_millis(100))
                .expect("drive modify"),
        );
        if has_operation(&operations, ghostrace::FileOperation::Modified) {
            break;
        }
    }
    let moved = root_path.join("moved.txt");
    fs::rename(&tracked, &moved).expect("move file");
    for _ in 0..40 {
        operations.extend(
            collector
                .run_current_run_loop_for(std::time::Duration::from_millis(100))
                .expect("drive move"),
        );
        if has_operation(&operations, ghostrace::FileOperation::Renamed) {
            break;
        }
    }
    fs::remove_file(&moved).expect("delete file");
    for _ in 0..80 {
        operations.extend(
            collector
                .run_current_run_loop_for(std::time::Duration::from_millis(100))
                .expect("drive lifecycle"),
        );
        if has_operation(&operations, ghostrace::FileOperation::Created)
            && has_operation(&operations, ghostrace::FileOperation::Modified)
            && has_operation(&operations, ghostrace::FileOperation::Renamed)
            && has_operation(&operations, ghostrace::FileOperation::Deleted)
        {
            break;
        }
    }

    collector.stop().expect("stop collector");
    assert_eq!(collector.status().state, ghostrace::CollectorState::Stopped);
    assert!(collector.status().callback_health.delivered_events >= 1);
    assert!(has_operation(&operations, ghostrace::FileOperation::Created));
    assert!(has_operation(&operations, ghostrace::FileOperation::Modified));
    assert!(has_operation(&operations, ghostrace::FileOperation::Renamed));
    assert!(has_operation(&operations, ghostrace::FileOperation::Deleted));

    let rendered = serde_json::to_string(&operations).expect("records JSON");
    assert!(!rendered.contains("collector-test-secret"));
    assert!(!rendered.contains(&root_path.to_string_lossy().to_string()));
    assert!(operations.iter().all(|event| event.path_digest.as_str().starts_with("sha256:")));

    let events = collector.journal().events().expect("journal events");
    assert!(events.iter().any(|event| event.event.kind == EventKind::CollectorStarted));
    assert!(events.iter().any(|event| event.event.kind == EventKind::CollectorStopped));
    assert!(events.iter().any(|event| {
        matches!(event.event.payload, EventPayload::FilesystemChanged(_))
            && event.event.evidence != Evidence::Unknown
    }));
}

#[cfg(target_os = "macos")]
#[test]
fn root_replacement_emits_a_bounded_gap_before_any_resume_claim() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    let moved_path = directory.path().join("selected-root-moved");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("collector-root-gap"))
        .expect("journal");
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, config())
            .expect("collector");
    collector.start().expect("start");

    fs::rename(&root_path, &moved_path).expect("replace selected root");
    let mut observed_gap = false;
    for _ in 0..80 {
        collector
            .run_current_run_loop_for(std::time::Duration::from_millis(50))
            .expect("drive root replacement");
        observed_gap = collector.journal().events().expect("events").iter().any(|stored| {
            matches!(
                &stored.event.payload,
                EventPayload::Gap(payload)
                    if payload.reason_code.as_str() == "fsevents_root_changed"
                        && payload.root_ids.iter().any(|root| root.as_str() == "root-main")
                        && payload.volume_digest.is_some()
                        && payload.remediation == Some(GapRemediation::ReconcileSelectedRoot)
                        && payload.from_cursor.is_none()
                        && payload.to_cursor.is_none()
            )
        });
        if observed_gap {
            break;
        }
    }

    fs::create_dir(&root_path).expect("replacement path");
    fs::write(root_path.join("must-wait-for-reconciliation.txt"), b"not admitted")
        .expect("replacement event");
    for _ in 0..20 {
        collector
            .run_current_run_loop_for(std::time::Duration::from_millis(50))
            .expect("drive after root replacement");
    }

    collector.stop().expect("stop");
    assert!(observed_gap, "WatchRoot must make RootChanged a durable gap");
    assert!(collector.status().recovery_required);
    let events = collector.journal().events().expect("events");
    assert!(!events.iter().any(|stored| stored.event.kind == EventKind::FilesystemChanged));
    assert!(events.iter().any(|stored| {
        stored.event.kind == EventKind::Gap
            && !serde_json::to_string(&stored.event).expect("json").contains("selected-root")
    }));
}

#[cfg(target_os = "macos")]
#[test]
fn history_done_transitions_replaying_to_live_without_user_event() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let observed = Arc::new(Mutex::new(Vec::<FseventsEvent>::new()));
    let observed_callback = Arc::clone(&observed);
    let mut seed_stream =
        FseventsStream::new([root_path.clone()], FseventsOptions::default(), move |batch| {
            observed_callback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend_from_slice(batch);
        })
        .expect("seed stream");
    seed_stream.schedule_on_current_run_loop().expect("schedule seed stream");
    seed_stream.start().expect("start seed stream");
    fs::write(root_path.join("history-marker.txt"), b"history-marker").expect("create marker");
    for _ in 0..80 {
        seed_stream.run_current_run_loop_for(Duration::from_millis(50)).expect("drive seed stream");
        if !observed.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_empty() {
            break;
        }
    }
    seed_stream.flush().expect("flush seed stream");
    seed_stream.stop().expect("stop seed stream");
    seed_stream.invalidate().expect("invalidate seed stream");
    let since_when = observed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .map(|event| event.event_id)
        .max()
        .filter(|event_id| *event_id > 0)
        .expect("seed stream event ID");

    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("collector-history-done"))
        .expect("journal");
    let mut replay_config = config();
    replay_config.options.since_when = since_when;
    replay_config.history_timeout = Duration::from_secs(5);
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, replay_config)
            .expect("collector");
    collector.start().expect("start replay collector");
    assert_eq!(collector.status().coverage_state, CollectorCoverageState::Replaying);

    for _ in 0..120 {
        collector
            .run_current_run_loop_for(Duration::from_millis(50))
            .expect("drive replay collector");
        if collector.status().coverage_state == CollectorCoverageState::Live {
            break;
        }
    }
    assert_eq!(collector.status().coverage_state, CollectorCoverageState::Live);
    assert!(!collector.status().recovery_required);
    assert!(collector.status().coverage_boundaries >= 1, "HistoryDone boundary was not consumed");
    let events = collector.journal().events().expect("journal events");
    assert!(!events.iter().any(|stored| stored.event.kind == EventKind::Gap));
    collector.stop().expect("stop replay collector");
}

#[cfg(target_os = "macos")]
#[test]
fn incomplete_history_emits_a_gap_and_never_reports_live() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("collector-history-gap"))
        .expect("journal");
    let mut replay_config = config();
    replay_config.options.since_when = 1;
    replay_config.history_timeout = Duration::from_secs(5);
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, replay_config)
            .expect("collector");
    collector.start().expect("start replay collector");
    assert_eq!(collector.status().coverage_state, CollectorCoverageState::Replaying);
    collector.stop().expect("stop before history completion");
    let status = collector.status();
    assert_eq!(status.coverage_state, CollectorCoverageState::HistoryUnavailable);
    assert!(status.recovery_required);
    let events = collector.journal().events().expect("journal events");
    assert!(!events.iter().any(|stored| stored.event.kind == EventKind::FilesystemChanged));
    assert!(events.iter().any(|stored| {
        stored.event.kind == EventKind::Gap
            && matches!(
                &stored.event.payload,
                EventPayload::Gap(payload)
                    if payload.reason_code.as_str() == "fsevents_history_incomplete"
                        && payload.remediation == Some(GapRemediation::ReinitializeStream)
            )
    }));
}

#[cfg(target_os = "macos")]
#[test]
fn restart_resumes_from_committed_cursor_and_persists_invalidated_gap() {
    let directory = tempdir().expect("tempdir");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let volume = root.volume_identity().clone();
    let journal_path = directory.path().join("restart.sqlite3");

    let mut first = FseventsCollector::new(
        confirmation(&policy()),
        policy(),
        [root],
        Journal::open_fixture(
            &journal_path,
            DeterministicKeyProvider::from_seed("collector-restart"),
        )
        .expect("first journal"),
        config(),
    )
    .expect("first collector");
    first.start().expect("start first collector");
    fs::write(root_path.join("committed.txt"), b"restart-evidence").expect("create file");
    for _ in 0..100 {
        first.run_current_run_loop_for(Duration::from_millis(50)).expect("drive first collector");
        if first
            .journal()
            .events()
            .expect("first events")
            .iter()
            .any(|stored| stored.event.kind == EventKind::FilesystemChanged)
        {
            break;
        }
    }
    first.stop().expect("stop first collector");
    drop(first);

    let resumed_journal = Journal::open_fixture(
        &journal_path,
        DeterministicKeyProvider::from_seed("collector-restart"),
    )
    .expect("reopen journal");
    let identity = CursorIdentity::for_volume(
        EventSource::Filesystem,
        "live-fsevents-test",
        config().options.stream_mode,
        volume.clone(),
    )
    .expect("cursor identity");
    let committed = resumed_journal
        .cursor_state(&identity)
        .expect("read committed cursor")
        .expect("committed cursor");
    let committed_event_id = match StartupCursor::from_source_cursor(committed.token.raw())
        .expect("ordered committed cursor")
    {
        StartupCursor::EventId { event_id } => event_id,
        StartupCursor::SinceNow => panic!("a committed cursor cannot be SinceNow"),
    };
    let resumed = FseventsCollector::new(
        confirmation(&policy()),
        policy(),
        [SelectedRoot::new("root-main", &root_path).expect("reopened selected root")],
        resumed_journal,
        config(),
    )
    .expect("resumed collector");
    assert_eq!(
        resumed.startup_cursor_decision(),
        StartupCursorDecision::Replay { event_id: committed_event_id }
    );
    drop(resumed);

    let invalidated_journal = Journal::open_fixture(
        &journal_path,
        DeterministicKeyProvider::from_seed("collector-restart"),
    )
    .expect("reopen for invalidation");
    invalidated_journal.invalidate_cursor(&identity).expect("invalidate persisted cursor");
    let mut blocked = FseventsCollector::new(
        confirmation(&policy()),
        policy(),
        [SelectedRoot::new("root-main", &root_path).expect("selected root after invalidation")],
        invalidated_journal,
        config(),
    )
    .expect("collector records recovery gap");
    assert!(blocked.status().recovery_required);
    assert_eq!(blocked.status().coverage_state, CollectorCoverageState::HistoryUnavailable);
    assert!(blocked.start().is_err(), "invalidated history must not auto-resume");
    assert!(blocked.journal().events().expect("recovery events").iter().any(|stored| {
        matches!(
            &stored.event.payload,
            EventPayload::Gap(payload)
                if payload.reason_code.as_str() == "fsevents_cursor_invalidated"
                    && payload.remediation == Some(GapRemediation::ReinitializeStream)
                    && payload.from_cursor.is_none()
                    && payload.to_cursor.is_none()
        )
    }));
}

#[cfg(target_os = "macos")]
#[test]
fn history_timeout_emits_a_gap_before_native_flush_can_claim_live() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("collector-history-timeout"))
            .expect("journal");
    let mut replay_config = config();
    replay_config.options.since_when = 1;
    replay_config.history_timeout = Duration::from_millis(1);
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, replay_config)
            .expect("collector");
    collector.start().expect("start replay collector");
    assert_eq!(collector.status().coverage_state, CollectorCoverageState::Replaying);
    std::thread::sleep(Duration::from_millis(10));
    collector.run_current_run_loop_for(Duration::ZERO).expect("drive timed-out replay");
    let status = collector.status();
    assert_eq!(status.coverage_state, CollectorCoverageState::HistoryUnavailable);
    assert!(status.recovery_required);
    let events = collector.journal().events().expect("journal events");
    assert!(!events.iter().any(|stored| stored.event.kind == EventKind::FilesystemChanged));
    assert!(events.iter().any(|stored| {
        stored.event.kind == EventKind::Gap
            && matches!(
                &stored.event.payload,
                EventPayload::Gap(payload)
                    if payload.reason_code.as_str() == "fsevents_history_timeout"
                        && payload.remediation == Some(GapRemediation::ReinitializeStream)
            )
    }));
    collector.stop().expect("stop timed-out replay");
}

#[cfg(target_os = "macos")]
#[test]
fn revocation_stops_capture_before_pending_events_can_commit() {
    let directory = tempdir().expect("tempdir");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("collector-revoke"))
        .expect("journal");
    let mut collector =
        FseventsCollector::new(confirmation(&policy()), policy(), [root], journal, config())
            .expect("collector");
    collector.start().expect("start");
    fs::write(root_path.join("revoked.txt"), b"never-persist-content").expect("create");
    collector
        .revoke(
            Utc.timestamp_opt(1_750_000_001, 0).single().expect("timestamp"),
            "human",
            "user_revoked",
        )
        .expect("revoke");
    assert_eq!(collector.status().state, ghostrace::CollectorState::Revoked);
    assert!(collector.start().is_err());
    assert!(collector.journal().events().expect("events").iter().all(|event| {
        !serde_json::to_string(&event.event).unwrap().contains("never-persist-content")
    }));
}

#[test]
fn selected_root_rejects_relative_paths_and_keeps_ids_opaque() {
    let result = SelectedRoot::new("root-main", PathBuf::from("relative-root"));
    assert!(matches!(result, Err(FseventsCollectorError::InvalidRoot { .. })));
}
