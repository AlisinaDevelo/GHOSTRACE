use std::{fs, path::PathBuf};

use chrono::{TimeZone, Utc};
use ghostrace::{
    CollectorState, ConsentPreview, DeterministicKeyProvider, EventKind, EventPayload, EventSource,
    Evidence, FseventsCollector, FseventsCollectorConfig, FseventsCollectorError, FseventsOptions,
    Journal, PolicyDocument, SelectedRoot, WriterConfig,
};
use tempfile::tempdir;

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
