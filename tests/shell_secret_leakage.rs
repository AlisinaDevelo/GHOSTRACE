//! Synthetic red-team tests for the explicit shell metadata boundary.
//!
//! The future wrapper must reject secret-bearing shell channels before they can
//! reach a journal, diagnostic, export, or crash message. Process inspection and
//! operating-system crash reporting are deliberately tested as external
//! exposure: they are documented, never retained, and never treated as a
//! GHOSTRACE privacy guarantee.

#[cfg(unix)]
mod unix {
    use std::{
        collections::BTreeSet,
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };

    use ghostrace::{
        export_fixture, ingest_fixture, read_fixture, validate_shell_metadata,
        DeterministicKeyProvider, Journal, PolicyProfile, SHELL_METADATA_GOLDEN_JSON,
    };
    use serde::Deserialize;
    use serde_json::{json, Value};
    use tempfile::TempDir;

    const CORPUS: &str = include_str!("../fixtures/shell-secret-leakage-v1.json");
    const CRASH_HELPER_ENV: &str = "GHOSTRACE_SHELL_SECRET_CRASH_HELPER";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LeakageFixture {
        schema_version: u32,
        program: String,
        generator: Generator,
        privacy: Privacy,
        channels: Vec<Channel>,
        external_exposure: Vec<ExternalExposure>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Generator {
        version: String,
        seed: String,
        algorithm: String,
        command: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Privacy {
        synthetic_only: bool,
        user_data_included: bool,
        network_required: bool,
        captures_stdio: bool,
        captures_environment: bool,
        retains_command_text: bool,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Channel {
        id: String,
        field: String,
        surface: String,
        sentinel: String,
        expected: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExternalExposure {
        channel: String,
        reason: String,
        retained_by_ghostrace: bool,
    }

    fn fixture() -> LeakageFixture {
        serde_json::from_str(CORPUS).expect("shell secret leakage fixture")
    }

    fn assert_not_contained(label: &str, sentinel: &str, value: impl AsRef<str>) {
        assert!(!value.as_ref().contains(sentinel), "{label} retained {sentinel}");
    }

    fn metadata_candidate(channel: &Channel) -> Value {
        let mut candidate: Value =
            serde_json::from_str(SHELL_METADATA_GOLDEN_JSON).expect("shell metadata golden");
        match channel.id.as_str() {
            "executable_name" => candidate["executable_id"] = json!(channel.sentinel),
            "working_path" => candidate["working_directory"]["path"] = json!(channel.sentinel),
            "environment" => candidate[&channel.field] = json!({"TOKEN": channel.sentinel}),
            _ => candidate[&channel.field] = json!(channel.sentinel),
        }
        candidate
    }

    fn malicious_fixture(channel: &Channel) -> (TempDir, PathBuf) {
        let base = include_str!("../fixtures/causal-chain.jsonl")
            .lines()
            .nth(1)
            .expect("shell event fixture");
        let mut event: Value = serde_json::from_str(base).expect("shell event JSON");
        event["payload"]["data"][&channel.field] = json!(channel.sentinel);
        let directory = tempfile::tempdir().expect("case tempdir");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private case tempdir");
        let path = directory.path().join("shell-secret-case.jsonl");
        fs::write(&path, serde_json::to_string(&event).expect("case JSON")).expect("case fixture");
        (directory, path)
    }

    fn output_for_command(path: &Path) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ghostrace"))
            .args([
                "demo",
                "--fixture",
                path.to_str().expect("fixture path"),
                "--event",
                "00000000-0000-4000-8000-000000000002",
            ])
            .output()
            .expect("run fixture CLI")
    }

    #[test]
    fn corpus_is_strict_synthetic_and_covers_every_required_channel() {
        let document = fixture();
        assert_eq!(document.schema_version, 1);
        assert_eq!(document.program, "ghostrace-shell-secret-leakage-v1");
        assert_eq!(document.generator.version, "ghostrace-shell-secrets-v1");
        assert_eq!(document.generator.seed, "ghostrace-shell-secrets-seed-v1");
        assert!(document.generator.algorithm.contains("no runtime randomness"));
        assert!(document.generator.command.contains("shell_secret_leakage"));
        assert!(document.privacy.synthetic_only);
        assert!(!document.privacy.user_data_included);
        assert!(!document.privacy.network_required);
        assert!(!document.privacy.captures_stdio);
        assert!(!document.privacy.captures_environment);
        assert!(!document.privacy.retains_command_text);

        let expected_ids = [
            "arguments",
            "environment",
            "stdin",
            "stdout",
            "stderr",
            "executable_name",
            "working_path",
            "failure_message",
            "prompt",
            "process_title",
            "diagnostic",
            "crash_report",
            "command_text",
        ];
        assert_eq!(document.channels.len(), expected_ids.len());
        assert_eq!(
            document.channels.iter().map(|channel| channel.id.as_str()).collect::<Vec<_>>(),
            expected_ids
        );
        let sentinels = document
            .channels
            .iter()
            .map(|channel| channel.sentinel.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(sentinels.len(), document.channels.len());
        for channel in &document.channels {
            assert!(channel.sentinel.starts_with("GHOSTRACE_SECRET_"));
            let expected = match channel.id.as_str() {
                "process_title" | "crash_report" => "os_visible_not_retained",
                _ => "rejected",
            };
            assert_eq!(channel.expected, expected);
            assert!(!channel.field.is_empty());
            assert!(!channel.surface.is_empty());
        }

        assert_eq!(document.external_exposure.len(), 2);
        assert_eq!(
            document
                .external_exposure
                .iter()
                .map(|exposure| exposure.channel.as_str())
                .collect::<Vec<_>>(),
            vec!["process_inspection", "crash_report"]
        );
        assert!(document
            .external_exposure
            .iter()
            .all(|exposure| { !exposure.reason.is_empty() && !exposure.retained_by_ghostrace }));
    }

    #[test]
    fn metadata_validator_rejects_every_secret_channel_without_echo() {
        for channel in fixture().channels {
            let candidate = metadata_candidate(&channel);
            let encoded = serde_json::to_string(&candidate).expect("candidate JSON");
            let error = validate_shell_metadata(&encoded).expect_err(&channel.id);
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert_not_contained(
                &format!("{} display error", channel.id),
                &channel.sentinel,
                display,
            );
            assert_not_contained(&format!("{} debug error", channel.id), &channel.sentinel, debug);
        }
    }

    #[test]
    fn journal_logs_errors_exports_and_cli_outputs_never_retain_sentinels() {
        for channel in fixture().channels {
            let (directory, path) = malicious_fixture(&channel);
            let read_error = read_fixture(&path).expect_err(&channel.id);
            let read_display = read_error.to_string();
            let read_debug = format!("{read_error:?}");
            assert_not_contained(
                &format!("{} read error", channel.id),
                &channel.sentinel,
                read_display,
            );
            assert_not_contained(
                &format!("{} read debug", channel.id),
                &channel.sentinel,
                read_debug,
            );

            let journal_path = directory.path().join("journal.sqlite3");
            let journal_seed = format!("shell-secret-{}", channel.id);
            let journal = Journal::open_fixture(
                &journal_path,
                DeterministicKeyProvider::from_seed(&journal_seed),
            )
            .expect("journal");
            let ingest_error = ingest_fixture(&path, &journal, &PolicyProfile::fixture_default())
                .expect_err(&channel.id);
            let ingest_display = ingest_error.to_string();
            let ingest_debug = format!("{ingest_error:?}");
            assert_not_contained(
                &format!("{} ingest error", channel.id),
                &channel.sentinel,
                ingest_display,
            );
            assert_not_contained(
                &format!("{} ingest debug", channel.id),
                &channel.sentinel,
                ingest_debug,
            );
            assert!(journal.events().expect("journal events").is_empty(), "{} journal", channel.id);
            drop(journal);

            let export_path = directory.path().join("export.jsonl");
            let export_error = export_fixture(&path, &export_path, false).expect_err(&channel.id);
            let export_display = export_error.to_string();
            let export_debug = format!("{export_error:?}");
            assert_not_contained(
                &format!("{} export error", channel.id),
                &channel.sentinel,
                export_display,
            );
            assert_not_contained(
                &format!("{} export debug", channel.id),
                &channel.sentinel,
                export_debug,
            );
            assert!(!export_path.exists(), "{} export was published", channel.id);

            let journal_bytes = fs::read(&journal_path).expect("journal bytes");
            assert_not_contained(
                &format!("{} journal bytes", channel.id),
                &channel.sentinel,
                String::from_utf8_lossy(&journal_bytes),
            );
            let diagnostic_log =
                format!("read={read_error:?}; ingest={ingest_error:?}; export={export_error:?}");
            assert_not_contained(
                &format!("{} diagnostic log", channel.id),
                &channel.sentinel,
                diagnostic_log,
            );

            let cli = output_for_command(&path);
            assert!(!cli.status.success(), "{} CLI unexpectedly succeeded", channel.id);
            assert_not_contained(
                &format!("{} CLI stdout", channel.id),
                &channel.sentinel,
                String::from_utf8_lossy(&cli.stdout),
            );
            assert_not_contained(
                &format!("{} CLI stderr", channel.id),
                &channel.sentinel,
                String::from_utf8_lossy(&cli.stderr),
            );
        }
    }

    #[test]
    fn crash_report_helper() {
        if env::var_os(CRASH_HELPER_ENV).is_some() {
            panic!("shell metadata rejected");
        }
    }

    #[test]
    fn panic_output_excludes_the_untrusted_sentinel() {
        let channel = fixture()
            .channels
            .into_iter()
            .find(|channel| channel.id == "crash_report")
            .expect("crash channel");
        let output = Command::new(env::current_exe().expect("test executable"))
            .args(["--exact", "unix::crash_report_helper"])
            .env_clear()
            .env(CRASH_HELPER_ENV, "1")
            .env("GHOSTRACE_SHELL_SECRET_SENTINEL", &channel.sentinel)
            .env("RUST_BACKTRACE", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("crash helper");
        assert!(!output.status.success());
        assert_not_contained(
            "panic stdout",
            &channel.sentinel,
            String::from_utf8_lossy(&output.stdout),
        );
        assert_not_contained(
            "panic stderr",
            &channel.sentinel,
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn process_inspection_exposure_is_external_and_not_retained() {
        let channel = fixture()
            .channels
            .into_iter()
            .find(|channel| channel.id == "process_title")
            .expect("process channel");
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg("while :; do sleep 1; done")
            .arg(&channel.sentinel)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("process-inspection child");
        let pid = child.id().to_string();
        let inspection = Command::new("/bin/ps")
            .args(["-ww", "-p", &pid, "-o", "command="])
            .output()
            .expect("process inspection");
        let _ = child.kill();
        let _ = child.wait();
        assert!(inspection.status.success());
        let inspection_text = String::from_utf8_lossy(&inspection.stdout);
        assert!(
            inspection_text.contains(&channel.sentinel),
            "OS process inspection did not expose the synthetic argv sentinel"
        );

        let retained_summary = json!({
            "channel": "process_inspection",
            "observed": true,
            "retained_by_ghostrace": false
        });
        assert_not_contained(
            "process exposure summary",
            &channel.sentinel,
            retained_summary.to_string(),
        );
    }
}

#[cfg(not(unix))]
#[test]
fn shell_secret_leakage_requires_a_posix_process_contract() {
    eprintln!("explicit no-go: POSIX process inspection and crash-output contract is unavailable");
}
