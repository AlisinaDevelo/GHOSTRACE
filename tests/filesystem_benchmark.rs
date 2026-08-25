use serde_json::Value;

const CORPUS: &str = include_str!("../fixtures/filesystem-benchmark-corpus-v1.json");

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("filesystem benchmark corpus JSON")
}

#[test]
fn filesystem_benchmark_contract_names_all_required_synthetic_workloads() {
    let document = corpus();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["program"], "ghostrace-filesystem-benchmark-v1");
    assert_eq!(document["repeat_runs"], 3);
    assert_eq!(
        document["metrics"].as_array().expect("metrics"),
        &vec![
            Value::from("latency_ms"),
            Value::from("coverage_classes"),
            Value::from("duplicate_rate"),
            Value::from("gap_rate"),
            Value::from("cpu_user_ms"),
            Value::from("cpu_system_ms"),
            Value::from("rss_peak_bytes"),
            Value::from("energy_nj"),
            Value::from("disk_growth_bytes"),
        ]
    );
    let expected = [
        "small_tree",
        "deep_tree",
        "wide_tree",
        "unicode_tree",
        "case_variant_tree",
        "git_tree",
        "build_output_tree",
        "event_storm_tree",
    ];
    let scenarios = document["scenarios"].as_array().expect("scenarios");
    assert_eq!(scenarios.len(), expected.len());
    for (scenario, expected_id) in scenarios.iter().zip(expected) {
        assert_eq!(scenario["id"], expected_id);
        assert_eq!(scenario["mode"], "device_safe");
        assert_eq!(scenario["native_test"], "native_safe");
        assert!(scenario["expected_operations_min"].as_u64().is_some());
        assert!(scenario["expected_operations_max"].as_u64().is_some());
        assert!(scenario["resource_budget"]["max_run_ms"].as_u64().is_some());
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn native_filesystem_benchmark_is_an_explicit_no_go_without_substitution() {
    let document = corpus();
    assert_eq!(
        document["platform_no_go"],
        "FSEvents native benchmark requires an authorized macOS device"
    );
}

#[cfg(target_os = "macos")]
mod macos {
    use super::corpus;
    use std::{
        collections::BTreeMap,
        fs, io,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Command,
        time::{Duration, Instant},
    };

    use chrono::{TimeZone, Utc};
    use ghostrace::{
        CollectorCoverageState, ConsentPreview, DeterministicKeyProvider, EventKind, EventSource,
        Evidence, FseventsCollector, FseventsCollectorConfig, FseventsOptions, Journal,
        PolicyDocument, SelectedRoot, WriterConfig,
    };
    use tempfile::tempdir;

    const SCENARIOS: [&str; 8] = [
        "small_tree",
        "deep_tree",
        "wide_tree",
        "unicode_tree",
        "case_variant_tree",
        "git_tree",
        "build_output_tree",
        "event_storm_tree",
    ];

    #[derive(Clone, Copy, Default)]
    struct CpuSnapshot {
        user_micros: u64,
        system_micros: u64,
        max_rss_bytes: u64,
    }

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

    fn confirmation(document: &PolicyDocument) -> ghostrace::ConsentConfirmation {
        ConsentPreview::from_policy(
            document,
            ["path_digest", "operation", "entry_kind"],
            ["fsevents_coalescing", "no_process_attribution", "history_can_be_dropped"],
        )
        .expect("consent preview")
        .confirm()
    }

    fn config(instance: &str) -> FseventsCollectorConfig {
        FseventsCollectorConfig {
            options: FseventsOptions {
                latency: Duration::from_millis(20),
                ..FseventsOptions::default()
            },
            writer: WriterConfig::default(),
            collector_instance: instance.to_owned(),
            instance_label: "filesystem-benchmark".to_owned(),
            consent_at: Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
            actor: "human".to_owned(),
            reason: "root_opt_in".to_owned(),
            history_timeout: Duration::from_secs(5),
            internal_paths: ghostrace::InternalPathPolicy::default(),
        }
    }

    fn private_directory() -> tempfile::TempDir {
        let directory = tempdir().expect("private benchmark directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private benchmark permissions");
        directory
    }

    fn write_fixture(path: &Path, bytes: usize) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut value = Vec::with_capacity(bytes);
        for index in 0..bytes {
            value.push(b'a' + (index % 26) as u8);
        }
        fs::write(path, value)
    }

    fn mutate_file(path: &Path) -> io::Result<()> {
        write_fixture(path, 37)
    }

    fn count_create_files(root: &Path, count: usize, prefix: &str) -> io::Result<Vec<PathBuf>> {
        let mut paths = Vec::with_capacity(count);
        for index in 0..count {
            let path = root.join(format!("{prefix}-{index:03}.dat"));
            write_fixture(&path, 19)?;
            paths.push(path);
        }
        Ok(paths)
    }

    fn tree_entry_count(path: &Path) -> u64 {
        let Ok(entries) = fs::read_dir(path) else { return 0 };
        entries
            .flatten()
            .map(|entry| {
                1 + if entry.path().is_dir() { tree_entry_count(&entry.path()) } else { 0 }
            })
            .sum()
    }

    fn scenario_bounds(id: &str) -> (u64, u64, u64, u64) {
        let document = corpus();
        let scenario = document["scenarios"]
            .as_array()
            .expect("scenarios")
            .iter()
            .find(|scenario| scenario["id"] == id)
            .expect("scenario definition");
        (
            scenario["expected_operations_min"].as_u64().expect("operation minimum"),
            scenario["expected_operations_max"].as_u64().expect("operation maximum"),
            scenario["expected_entry_min"].as_u64().expect("entry minimum"),
            scenario["expected_entry_max"].as_u64().expect("entry maximum"),
        )
    }

    fn generate_workload(root: &Path, scenario: &str) -> io::Result<u64> {
        let scenario_root = root.join(format!("scenario-{scenario}"));
        fs::create_dir_all(&scenario_root)?;
        match scenario {
            "small_tree" => {
                let files = count_create_files(&scenario_root.join("small"), 6, "file")?;
                mutate_file(&files[0])?;
                fs::rename(&files[1], scenario_root.join("small/renamed.dat"))?;
                fs::remove_file(&files[2])?;
                Ok(10)
            }
            "deep_tree" => {
                let mut cursor = scenario_root.clone();
                let mut operations = 0;
                for depth in 0..9 {
                    cursor = cursor.join(format!("level-{depth:02}"));
                    fs::create_dir(&cursor)?;
                    let file = cursor.join("leaf.dat");
                    write_fixture(&file, 23)?;
                    mutate_file(&file)?;
                    operations += 3;
                }
                fs::rename(cursor.join("leaf.dat"), cursor.join("leaf-renamed.dat"))?;
                Ok(operations + 1)
            }
            "wide_tree" => {
                let files = count_create_files(&scenario_root.join("wide"), 64, "wide")?;
                for file in files.iter().step_by(4) {
                    mutate_file(file)?;
                }
                Ok(64 + 16)
            }
            "unicode_tree" => {
                let names = ["café", "e\u{301}", "東京", "данные", "δοκιμή", "🧪"];
                for (index, name) in names.iter().enumerate() {
                    write_fixture(&scenario_root.join(format!("unicode/{name}-{index}.dat")), 29)?;
                }
                let first = scenario_root.join("unicode/café-0.dat");
                mutate_file(&first)?;
                fs::rename(first, scenario_root.join("unicode/renamed-0.dat"))?;
                Ok(names.len() as u64 + 2)
            }
            "case_variant_tree" => {
                let upper = scenario_root.join("Case");
                let lower = scenario_root.join("case");
                fs::create_dir(&upper)?;
                let distinct = fs::create_dir(&lower).is_ok();
                write_fixture(&upper.join("Upper.txt"), 31)?;
                let lower_file =
                    if distinct { lower.join("lower.txt") } else { upper.join("lower.txt") };
                write_fixture(&lower_file, 31)?;
                mutate_file(&lower_file)?;
                fs::rename(&lower_file, lower_file.with_file_name("renamed.txt"))?;
                Ok(if distinct { 6 } else { 5 })
            }
            "git_tree" => {
                fs::create_dir_all(&scenario_root)?;
                let init = Command::new("/usr/bin/git")
                    .args(["init", "--quiet", "--template="])
                    .arg(&scenario_root)
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .status()?;
                if !init.success() {
                    return Err(io::Error::other("git init failed"));
                }
                let files = count_create_files(&scenario_root.join("src"), 24, "module")?;
                let add = Command::new("/usr/bin/git")
                    .args(["-C"])
                    .arg(&scenario_root)
                    .args(["add", "src"])
                    .status()?;
                if !add.success() {
                    return Err(io::Error::other("git add failed"));
                }
                let commit = Command::new("/usr/bin/git")
                    .args(["-C"])
                    .arg(&scenario_root)
                    .args([
                        "-c",
                        "user.name=GHOSTRACE-fixture",
                        "-c",
                        "user.email=fixture@example.invalid",
                        "commit",
                        "--quiet",
                        "--no-gpg-sign",
                        "-m",
                        "fixture",
                    ])
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .status()?;
                if !commit.success() {
                    return Err(io::Error::other("git commit failed"));
                }
                mutate_file(&files[0])?;
                fs::rename(&files[1], files[1].with_file_name("renamed-module.dat"))?;
                Ok(24 + 4)
            }
            "build_output_tree" => {
                let build = scenario_root.join("target");
                let mut files = Vec::new();
                for profile in ["debug", "release"] {
                    for extension in ["o", "d", "rlib", "rmeta"] {
                        let directory = build.join(profile);
                        let path = directory.join(format!("artifact-{extension}.dat"));
                        write_fixture(&path, 43)?;
                        files.push(path);
                    }
                }
                for index in 0..32 {
                    let path = build.join(format!("cache/cache-{index:03}.bin"));
                    write_fixture(&path, 17)?;
                    files.push(path);
                }
                for file in files.iter().step_by(3) {
                    mutate_file(file)?;
                }
                for file in files.iter().skip(2).step_by(7) {
                    fs::remove_file(file)?;
                }
                Ok(files.len() as u64 + files.len() as u64 / 3 + files.len() as u64 / 7)
            }
            "event_storm_tree" => {
                let storm = scenario_root.join("storm");
                let files = count_create_files(&storm, 128, "storm")?;
                for file in &files {
                    mutate_file(file)?;
                }
                for (index, file) in files.iter().take(64).enumerate() {
                    fs::rename(file, file.with_file_name(format!("renamed-{index:03}.dat")))?;
                }
                for file in files.iter().skip(64).step_by(2) {
                    if file.exists() {
                        fs::remove_file(file)?;
                    }
                }
                Ok(128 + 128 + 64 + 32)
            }
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown benchmark scenario")),
        }
    }

    struct DriveOutcome {
        events: Vec<ghostrace::CollectedFilesystemEvent>,
        error: Option<&'static str>,
    }

    fn error_code(error: &ghostrace::FseventsCollectorError) -> &'static str {
        let text = error.to_string();
        if text.contains("cursor regressed") {
            "cursor_regression"
        } else if text.contains("gap") {
            "collector_gap"
        } else {
            "collector_error"
        }
    }

    fn drive_until_quiet(collector: &mut FseventsCollector) -> DriveOutcome {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut quiet_ticks = 0;
        let mut events = Vec::new();
        while Instant::now() < deadline {
            let batch = match collector.run_current_run_loop_for(Duration::from_millis(50)) {
                Ok(batch) => batch,
                Err(error) => {
                    return DriveOutcome { events, error: Some(error_code(&error)) };
                }
            };
            if batch.is_empty() {
                quiet_ticks += 1;
            } else {
                quiet_ticks = 0;
                events.extend(batch);
            }
            if !events.is_empty() && quiet_ticks >= 4 {
                break;
            }
        }
        let _ = quiet_ticks;
        DriveOutcome { events, error: None }
    }

    fn file_tree_bytes(path: &Path) -> u64 {
        let Ok(metadata) = fs::symlink_metadata(path) else { return 0 };
        if metadata.is_file() {
            return metadata.len();
        }
        if !metadata.is_dir() {
            return 0;
        }
        fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| file_tree_bytes(&entry.path()))
            .sum()
    }

    fn cpu_snapshot() -> CpuSnapshot {
        // `ru_maxrss` is bytes on macOS.  Keep the conversion explicit so a
        // future non-macOS test cannot accidentally publish a false unit.
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
        let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
        if result != 0 {
            return CpuSnapshot::default();
        }
        let micros = |value: libc::timeval| -> u64 {
            (value.tv_sec.max(0) as u64)
                .saturating_mul(1_000_000)
                .saturating_add(value.tv_usec.max(0) as u64)
        };
        CpuSnapshot {
            user_micros: micros(usage.ru_utime),
            system_micros: micros(usage.ru_stime),
            max_rss_bytes: usage.ru_maxrss.max(0) as u64,
        }
    }

    fn energy_snapshot() -> Option<u64> {
        let output =
            Command::new("/usr/sbin/ioreg").args(["-r", "-c", "IOPMPowerSource"]).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let marker = "\"AccumulatedSystemEnergyConsumed\"=";
        text.lines().find_map(|line| {
            let start = line.find(marker)? + marker.len();
            let value = line[start..].split([',', '}']).next()?.trim();
            value.parse::<u64>().ok()
        })
    }

    fn command_output(command: &str, arguments: &[&str]) -> String {
        Command::new(command)
            .args(arguments)
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unavailable".to_owned())
    }

    fn source_revision() -> String {
        std::env::var("GHOSTRACE_BENCHMARK_REVISION").unwrap_or_else(|_| "0".repeat(40))
    }

    fn aggregate_coverage(
        events: &[ghostrace::CollectedFilesystemEvent],
    ) -> BTreeMap<&'static str, u64> {
        let mut counts =
            BTreeMap::from([("direct", 0), ("contextual", 0), ("inferred", 0), ("unknown", 0)]);
        for event in events {
            let name = match event.evidence {
                Evidence::Direct => "direct",
                Evidence::Contextual => "contextual",
                Evidence::Inferred => "inferred",
                Evidence::Unknown => "unknown",
            };
            *counts.get_mut(name).expect("coverage class") += 1;
        }
        counts
    }

    fn duplicate_count(events: &[ghostrace::CollectedFilesystemEvent]) -> u64 {
        let mut seen = std::collections::BTreeSet::new();
        events.iter().filter(|event| !seen.insert(event.source_event_id)).count() as u64
    }

    #[test]
    fn native_benchmark_runs_all_synthetic_workloads_and_emits_receipt() {
        let directory = private_directory();
        let selected_root_path = directory.path().join("selected-root");
        fs::create_dir(&selected_root_path).expect("selected root");
        let journal_path = directory.path().join("benchmark.sqlite3");
        let journal = Journal::open_fixture(
            &journal_path,
            DeterministicKeyProvider::from_seed("filesystem-benchmark-v1"),
        )
        .expect("file-backed benchmark journal");
        let root = SelectedRoot::new("root-main", &selected_root_path).expect("selected root");
        let before_cpu = cpu_snapshot();
        let before_energy = energy_snapshot();
        let before_disk = file_tree_bytes(directory.path());
        let mut latency_samples_ms = Vec::new();
        let mut scenario_reports = Vec::new();

        for run in 0..3 {
            for scenario in SCENARIOS {
                let scenario_root = selected_root_path.join(format!("scenario-{scenario}"));
                fs::create_dir_all(&scenario_root).expect("scenario root");
                let instance = format!("live-benchmark-{run}-{scenario}");
                let mut collector = FseventsCollector::new(
                    confirmation(&policy()),
                    policy(),
                    [root.clone()],
                    journal.clone(),
                    config(&instance),
                )
                .expect("benchmark collector");
                collector.start().expect("start benchmark collector");
                assert_eq!(collector.status().coverage_state, CollectorCoverageState::Live);
                let event_count_before = journal.events().expect("journal before").len();
                let started = Instant::now();
                let expected_operations = generate_workload(&selected_root_path, scenario)
                    .expect("generate synthetic workload");
                let (operation_min, operation_max, entry_min, entry_max) =
                    scenario_bounds(scenario);
                let entries = tree_entry_count(&scenario_root);
                assert!(
                    (operation_min..=operation_max).contains(&expected_operations),
                    "{scenario} generated {expected_operations} operations outside {operation_min}..={operation_max}"
                );
                assert!(
                    (entry_min..=entry_max).contains(&entries),
                    "{scenario} generated {entries} entries outside {entry_min}..={entry_max}"
                );
                let mutation_finished = Instant::now();
                let outcome = drive_until_quiet(&mut collector);
                let latency = mutation_finished.elapsed().as_secs_f64() * 1000.0;
                let total_elapsed = started.elapsed().as_millis();
                assert!(total_elapsed <= 30_000, "scenario exceeded bounded run time");
                latency_samples_ms.push(latency);
                let status = collector.status();
                let mut errors = outcome.error.into_iter().collect::<Vec<_>>();
                if let Err(error) = collector.stop() {
                    errors.push(error_code(&error));
                }
                drop(collector);
                let event_count_after = journal.events().expect("journal after");
                let gaps = (event_count_after[event_count_before..]
                    .iter()
                    .filter(|stored| stored.event.kind == EventKind::Gap)
                    .count() as u64)
                    .saturating_add(errors.len() as u64);
                let coverage = aggregate_coverage(&outcome.events);
                let observed_events = outcome.events.len() as u64;
                scenario_reports.push(serde_json::json!({
                    "id": scenario,
                    "coverage": coverage,
                    "expected_operations": expected_operations,
                    "observed_events": observed_events,
                    "duplicates": duplicate_count(&outcome.events).saturating_add(status.transport_duplicates),
                    "gaps": gaps,
                    "errors": errors,
                    "latency_ms": latency,
                }));
                fs::remove_dir_all(&scenario_root).expect("clean synthetic workload");
            }
        }
        let after_cpu = cpu_snapshot();
        let after_energy = energy_snapshot();
        let after_disk = file_tree_bytes(directory.path());
        let disk_growth = after_disk.saturating_sub(before_disk);
        let energy_delta =
            after_energy.zip(before_energy).and_then(|(end, start)| end.checked_sub(start));
        let resource = if let Some(energy_nj) = energy_delta {
            serde_json::json!({
                "cpu_user_ms": after_cpu.user_micros.saturating_sub(before_cpu.user_micros) as f64 / 1000.0,
                "cpu_system_ms": after_cpu.system_micros.saturating_sub(before_cpu.system_micros) as f64 / 1000.0,
                "rss_peak_bytes": after_cpu.max_rss_bytes,
                "disk_growth_bytes": disk_growth,
                "energy_nj": energy_nj,
            })
        } else {
            serde_json::json!({
                "cpu_user_ms": after_cpu.user_micros.saturating_sub(before_cpu.user_micros) as f64 / 1000.0,
                "cpu_system_ms": after_cpu.system_micros.saturating_sub(before_cpu.system_micros) as f64 / 1000.0,
                "rss_peak_bytes": after_cpu.max_rss_bytes,
                "disk_growth_bytes": disk_growth,
                "energy_nj": null,
                "energy_no_go_reason": "IOPMPowerSource accumulated energy telemetry unavailable; privileged powermetrics was not substituted",
            })
        };
        let receipt = serde_json::json!({
            "schema_version": 1,
            "source_revision": source_revision(),
            "device": {
                "model": command_output("/usr/sbin/sysctl", &["-n", "hw.model"]),
                "os": command_output("/usr/bin/sw_vers", &["-productVersion"]),
                "arch": command_output("/usr/bin/uname", &["-m"]),
                "toolchain": command_output("rustc", &["--version"]),
            },
            "latency_samples_ms": latency_samples_ms,
            "scenarios": scenario_reports,
            "resource": resource,
        });
        let rendered = serde_json::to_string(&receipt).expect("benchmark receipt JSON");
        assert!(!rendered.contains(&selected_root_path.to_string_lossy().to_string()));
        assert!(!rendered.contains("fixture@example.invalid"));
        println!("filesystem-benchmark-receipt={rendered}");
    }
}
