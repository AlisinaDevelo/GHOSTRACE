use serde_json::Value;

const CORPUS: &str = include_str!("../fixtures/fsevents-lifecycle-corpus-v1.json");

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("lifecycle corpus JSON")
}

fn scenario_ids(document: &Value) -> Vec<&str> {
    document["scenarios"]
        .as_array()
        .expect("scenarios")
        .iter()
        .map(|scenario| scenario["id"].as_str().expect("scenario id"))
        .collect()
}

#[test]
fn lifecycle_corpus_contract_is_complete_and_explicit_about_no_go_rows() {
    let document = corpus();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["repeat_runs"], 32);
    assert_eq!(
        scenario_ids(&document),
        vec![
            "bulk_checkout",
            "package_install",
            "rename_storm",
            "directory_deletion",
            "sleep_wake",
            "logout",
            "volume_detach",
            "process_kill",
            "restart",
        ]
    );
    for scenario in document["scenarios"].as_array().expect("scenarios") {
        assert!(scenario["ground_truth"].as_array().is_some_and(|entries| !entries.is_empty()));
        assert!(scenario["resource_budget"]["max_operations"].as_u64().is_some());
        if scenario["mode"] == "device_guarded" {
            assert_eq!(scenario["native_test"], "guarded_no_go");
            assert!(scenario["no_go_reason"].as_str().is_some_and(|reason| !reason.is_empty()));
            assert!(scenario["required_gaps"].as_array().is_some_and(|gaps| !gaps.is_empty()));
        } else {
            assert_eq!(scenario["native_test"], "native_safe");
            assert!(scenario.get("no_go_reason").is_none());
            assert!(scenario["required_gaps"].as_array().is_some_and(|gaps| gaps.is_empty()));
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn native_lifecycle_rows_are_an_explicit_no_go_without_substitution() {
    let document = corpus();
    let native = document["scenarios"]
        .as_array()
        .expect("scenarios")
        .iter()
        .filter(|scenario| scenario["mode"] == "device_safe")
        .count();
    let guarded = document["scenarios"]
        .as_array()
        .expect("scenarios")
        .iter()
        .filter(|scenario| scenario["mode"] == "device_guarded")
        .count();
    assert_eq!(native, 6);
    assert_eq!(guarded, 3);
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::{
        collections::{HashMap, HashSet},
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        time::Duration,
    };

    use chrono::{TimeZone, Utc};
    use ghostrace::{
        ConsentPreview, DeterministicKeyProvider, EventSource, FileOperation, FseventsCollector,
        FseventsCollectorConfig, FseventsOptions, Journal, SelectedRoot, WriterConfig,
    };
    use tempfile::tempdir;

    fn policy() -> ghostrace::PolicyDocument {
        ghostrace::PolicyDocument::new(
            "live-filesystem-v1",
            1,
            [EventSource::Filesystem, EventSource::Lifecycle],
            ["root-main"],
            false,
        )
        .expect("policy")
    }

    fn confirmation(document: &ghostrace::PolicyDocument) -> ghostrace::ConsentConfirmation {
        ConsentPreview::from_policy(
            document,
            ["path_digest", "operation", "entry_kind"],
            ["fsevents_coalescing", "no_process_attribution", "history_can_be_dropped"],
        )
        .expect("consent preview")
        .confirm()
    }

    fn config() -> FseventsCollectorConfig {
        FseventsCollectorConfig {
            options: FseventsOptions {
                latency: Duration::from_millis(20),
                ..FseventsOptions::default()
            },
            writer: WriterConfig::default(),
            collector_instance: "live-lifecycle-corpus".to_owned(),
            instance_label: "lifecycle-corpus".to_owned(),
            consent_at: Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
            actor: "human".to_owned(),
            reason: "root_opt_in".to_owned(),
            history_timeout: Duration::from_secs(5),
            internal_paths: ghostrace::InternalPathPolicy::default(),
        }
    }

    fn drive(collector: &mut FseventsCollector) -> Vec<ghostrace::CollectedFilesystemEvent> {
        let mut collected = Vec::new();
        for _ in 0..20 {
            let batch = collector
                .run_current_run_loop_for(Duration::from_millis(50))
                .expect("drive native callback");
            let had_events = !batch.is_empty();
            collected.extend(batch);
            if had_events {
                break;
            }
        }
        collected
    }

    fn add_stage(
        stages: &mut HashMap<String, Vec<ghostrace::CollectedFilesystemEvent>>,
        id: &str,
        events: Vec<ghostrace::CollectedFilesystemEvent>,
    ) {
        stages.entry(id.to_owned()).or_default().extend(events);
    }

    fn operation_name(operation: FileOperation) -> &'static str {
        match operation {
            FileOperation::Created => "created",
            FileOperation::Modified => "modified",
            FileOperation::Deleted => "deleted",
            FileOperation::Renamed => "renamed",
        }
    }

    fn stage_report(events: &[ghostrace::CollectedFilesystemEvent]) -> Value {
        let mut counts = HashMap::<&str, usize>::new();
        for event in events {
            *counts.entry(operation_name(event.operation)).or_default() += 1;
        }
        serde_json::json!({
            "observed_events": events.len(),
            "operation_counts": counts,
            "path_digests_only": events.iter().all(|event| event.path_digest.as_str().starts_with("sha256:")),
        })
    }

    #[test]
    fn native_safe_storm_lifecycle_runs_publish_loss_order_recovery_and_resource_receipt() {
        let directory = tempdir().expect("private fixture root");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private fixture permissions");
        let root_path = directory.path().join("selected-root");
        fs::create_dir(&root_path).expect("selected root");
        let root = SelectedRoot::new("root-main", &root_path).expect("selected root");
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("lifecycle-corpus"))
            .expect("journal");
        let document = policy();
        let mut collector =
            FseventsCollector::new(confirmation(&document), document, [root], journal, config())
                .expect("collector");
        collector.start().expect("start collector");

        let mut stages = HashMap::<String, Vec<ghostrace::CollectedFilesystemEvent>>::new();
        let mut all_events = Vec::<ghostrace::CollectedFilesystemEvent>::new();
        let mut restart_successes = 0_u64;
        const RUNS: u64 = 3;
        for run in 0..RUNS {
            let prefix = format!("run-{run}");

            let checkout = root_path.join(format!("{prefix}-checkout"));
            fs::create_dir(&checkout).expect("checkout directory");
            for index in 0..16 {
                fs::write(checkout.join(format!("file-{index:02}")), b"fixture")
                    .expect("checkout file");
            }
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "bulk_checkout", events);

            let package = root_path.join(format!("{prefix}-package"));
            fs::create_dir(&package).expect("package directory");
            let temporary = package.join("install.tmp");
            let final_path = package.join("install.pkg");
            fs::write(&temporary, b"fixture").expect("package temporary");
            fs::write(&temporary, b"fixture-updated").expect("package update");
            fs::rename(&temporary, &final_path).expect("atomic package rename");
            fs::write(package.join("manifest"), b"fixture").expect("package manifest");
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "package_install", events);

            let rename_root = root_path.join(format!("{prefix}-rename"));
            fs::create_dir(&rename_root).expect("rename directory");
            for index in 0..8 {
                fs::write(rename_root.join(format!("before-{index:02}")), b"fixture")
                    .expect("rename source");
            }
            for index in 0..8 {
                fs::rename(
                    rename_root.join(format!("before-{index:02}")),
                    rename_root.join(format!("after-{index:02}")),
                )
                .expect("rename storm item");
            }
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "rename_storm", events);

            let deletion = root_path.join(format!("{prefix}-delete"));
            let nested = deletion.join("nested");
            fs::create_dir_all(&nested).expect("deletion tree");
            for index in 0..4 {
                fs::write(nested.join(format!("file-{index:02}")), b"fixture")
                    .expect("deletion file");
            }
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "directory_deletion", events);
            fs::remove_dir_all(&deletion).expect("recursive deletion");
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "directory_deletion", events);

            let child_marker = root_path.join(format!("{prefix}-child-marker"));
            let mut child = Command::new("/bin/sh")
                .arg("-c")
                .arg("printf x > \"$1\"; sleep 30")
                .arg("ghostrace-child")
                .arg(&child_marker)
                .spawn()
                .expect("spawn child process");
            std::thread::sleep(Duration::from_millis(20));
            let _ = child.kill();
            child.wait().expect("wait child process");
            let events = drive(&mut collector);
            all_events.extend(events.iter().cloned());
            add_stage(&mut stages, "process_kill", events);

            collector.stop().expect("stop before restart");
            collector.start().expect("restart collector");
            let restart_marker = root_path.join(format!("{prefix}-restart-marker"));
            fs::write(&restart_marker, b"fixture").expect("post-restart marker");
            let restart_events = drive(&mut collector);
            if restart_events.iter().any(|event| event.operation == FileOperation::Created) {
                restart_successes += 1;
            }
            all_events.extend(restart_events.iter().cloned());
            add_stage(&mut stages, "restart", restart_events);
        }

        collector.stop().expect("stop collector");
        let status = collector.status();
        assert!(all_events.iter().any(|event| event.operation == FileOperation::Created));
        assert!(all_events.iter().any(|event| event.operation == FileOperation::Renamed));
        assert!(all_events.iter().any(|event| event.operation == FileOperation::Deleted));
        assert!(all_events.iter().all(|event| event.path_digest.as_str().starts_with("sha256:")));
        assert_eq!(restart_successes, RUNS, "restart must resume post-stop observations");
        assert!(!status.recovery_required);
        assert!(status.callback_health.delivered_events > 0);

        let mut source_ids = HashSet::new();
        let duplicate_source_events =
            all_events.iter().filter(|event| !source_ids.insert(event.source_event_id)).count();
        let ordering_inversions = all_events
            .windows(2)
            .filter(|pair| pair[1].source_event_id < pair[0].source_event_id)
            .count();
        let stage_reports = stages
            .iter()
            .map(|(id, events)| (id.clone(), stage_report(events)))
            .collect::<HashMap<_, _>>();
        let receipt = serde_json::json!({
            "schema_version": 1,
            "runs": RUNS,
            "native_device_scenarios": ["bulk_checkout", "package_install", "rename_storm", "directory_deletion", "process_kill", "restart"],
            "guarded_no_go_scenarios": ["sleep_wake", "logout", "volume_detach"],
            "observed_events": all_events.len(),
            "duplicate_source_events": duplicate_source_events,
            "ordering_inversions": ordering_inversions,
            "recovery_successes": restart_successes,
            "resource": {
                "max_observed_events_per_scenario": stages.values().map(Vec::len).max().unwrap_or(0),
                "callback_delivered_events": status.callback_health.delivered_events,
                "callback_delivered_batches": status.callback_health.delivered_batches,
                "collector_dropped_events": status.dropped_events,
                "collector_transport_duplicates": status.transport_duplicates,
            },
            "scenarios": stage_reports,
            "interpretation": "macos_native_device_safe_rows_only; guarded_rows_are_explicit_no_go",
        });
        let rendered = serde_json::to_string(&receipt).expect("receipt JSON");
        assert!(!rendered.contains(&root_path.to_string_lossy().to_string()));
        assert!(!rendered.contains("fixture-updated"));
        println!("lifecycle-corpus-receipt={rendered}");
    }
}
