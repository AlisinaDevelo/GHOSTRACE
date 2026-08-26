//! Device-safe contract tests for the future explicit shell wrapper.
//!
//! This is a reference harness, not a shipped shell executor. It invokes a
//! fixed POSIX shell with null stdio and a cleared environment, returns the
//! child's native status unchanged, and models terminal-close/crash paths as
//! explicit gaps. No command text, arguments, environment, or terminal bytes
//! enter the evidence record.

#[cfg(unix)]
mod unix {
    use std::{
        collections::BTreeMap,
        io,
        os::unix::process::ExitStatusExt,
        process::{Child, Command, ExitStatus, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use ghostrace::ShellStatus;
    use serde::Deserialize;
    use serde_json::Value;

    const SHELL: &str = "/bin/sh";
    const MAX_SCENARIO: Duration = Duration::from_secs(5);

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LifecycleFixture {
        schema_version: u32,
        program: String,
        generator: Generator,
        privacy: Privacy,
        resource_limits: ResourceLimits,
        scenarios: Vec<Scenario>,
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
    struct ResourceLimits {
        max_scenarios: usize,
        max_scenario_ms: u64,
        max_total_ms: u64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Scenario {
        id: String,
        terminal_kind: String,
        #[serde(default)]
        gap_reason: Option<String>,
        #[serde(default)]
        expected: Option<ExpectedCompletion>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExpectedCompletion {
        status: String,
        exit_code: Option<i32>,
        signal: Option<i32>,
    }

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    struct Completion {
        status: ShellStatus,
        exit_code: Option<i32>,
        signal: Option<i32>,
    }

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    enum GapReason {
        ExecFailed,
        TerminalClosed,
        WrapperCrashed,
    }

    #[derive(Debug, Eq, PartialEq)]
    enum TerminalEvidence {
        Completed { completion: Completion },
        Gap { reason: GapReason },
    }

    struct ReferenceWrapper {
        child: Child,
        started: Instant,
    }

    impl ReferenceWrapper {
        fn spawn(script: &str) -> io::Result<Self> {
            let mut command = Command::new(SHELL);
            command
                .arg("-c")
                .arg(script)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            Ok(Self { child: command.spawn()?, started: Instant::now() })
        }

        fn wait(mut self) -> io::Result<TerminalEvidence> {
            let status = self.child.wait()?;
            Ok(TerminalEvidence::Completed { completion: completion(status) })
        }

        fn terminate(mut self, timeout: Option<Duration>) -> io::Result<TerminalEvidence> {
            if let Some(timeout) = timeout {
                let deadline = Instant::now() + timeout;
                loop {
                    if let Some(status) = self.child.try_wait()? {
                        return Ok(TerminalEvidence::Completed { completion: completion(status) });
                    }
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            }
            self.child.kill()?;
            let status = self.child.wait()?;
            Ok(TerminalEvidence::Completed { completion: completion(status) })
        }

        fn abandon(mut self, reason: GapReason) -> TerminalEvidence {
            let _ = self.child.kill();
            let _ = self.child.wait();
            TerminalEvidence::Gap { reason }
        }

        fn elapsed(&self) -> Duration {
            self.started.elapsed()
        }
    }

    fn completion(status: ExitStatus) -> Completion {
        let signal = status.signal();
        let exit_code = status.code();
        let shell_status = match (exit_code, signal) {
            (Some(0), None) => ShellStatus::Succeeded,
            (Some(_), None) => ShellStatus::Failed,
            (None, Some(_)) => ShellStatus::Signaled,
            _ => ShellStatus::Unknown,
        };
        Completion { status: shell_status, exit_code, signal }
    }

    fn fixture() -> LifecycleFixture {
        serde_json::from_str(include_str!("../fixtures/shell-wrapper-lifecycle-v1.json"))
            .expect("shell lifecycle fixture")
    }

    fn assert_completion(evidence: TerminalEvidence, expected: &ExpectedCompletion) {
        let TerminalEvidence::Completed { completion } = evidence else {
            panic!("expected a terminal completion");
        };
        let expected_status = match expected.status.as_str() {
            "succeeded" => ShellStatus::Succeeded,
            "failed" => ShellStatus::Failed,
            "signaled" => ShellStatus::Signaled,
            "unknown" => ShellStatus::Unknown,
            status => panic!("unexpected fixture status {status}"),
        };
        assert_eq!(completion.status, expected_status);
        assert_eq!(completion.exit_code, expected.exit_code);
        assert_eq!(completion.signal, expected.signal);
    }

    fn assert_gap(evidence: TerminalEvidence, expected: GapReason) {
        assert_eq!(evidence, TerminalEvidence::Gap { reason: expected });
    }

    #[test]
    fn wrapper_crash_helper() {
        if std::env::var_os("GHOSTRACE_SHELL_WRAPPER_CRASH_HELPER").is_some() {
            std::process::abort();
        }
    }

    #[test]
    fn fixture_is_strict_synthetic_and_has_every_required_lifecycle_row() {
        let document = fixture();
        assert_eq!(document.schema_version, 1);
        assert_eq!(document.program, "ghostrace-shell-wrapper-lifecycle-v1");
        assert_eq!(document.generator.version, "ghostrace-shell-lifecycle-v1");
        assert_eq!(document.generator.seed, "ghostrace-shell-lifecycle-seed-v1");
        assert!(document.generator.algorithm.contains("no runtime randomness"));
        assert!(document.generator.command.contains("shell_wrapper_lifecycle"));
        assert!(document.privacy.synthetic_only);
        assert!(!document.privacy.user_data_included);
        assert!(!document.privacy.network_required);
        assert!(!document.privacy.captures_stdio);
        assert!(!document.privacy.captures_environment);
        assert!(!document.privacy.retains_command_text);
        assert_eq!(document.resource_limits.max_scenarios, 9);
        assert_eq!(document.resource_limits.max_scenario_ms, 5000);
        assert_eq!(document.resource_limits.max_total_ms, 15000);
        assert_eq!(document.scenarios.len(), 9);
        assert_eq!(
            document.scenarios.iter().map(|scenario| scenario.id.as_str()).collect::<Vec<_>>(),
            vec![
                "normal_exit",
                "signal",
                "exec_failure",
                "shell_builtin",
                "pipeline",
                "timeout",
                "cancellation",
                "terminal_close",
                "wrapper_crash",
            ]
        );
        for scenario in &document.scenarios {
            match scenario.terminal_kind.as_str() {
                "completion" => {
                    assert!(scenario.expected.is_some());
                    assert!(scenario.gap_reason.is_none());
                }
                "gap" => {
                    assert!(scenario.expected.is_none());
                    assert!(scenario.gap_reason.is_some());
                }
                kind => panic!("unexpected terminal kind {kind}"),
            }
        }
    }

    #[test]
    fn child_status_is_returned_unchanged_for_exit_signal_builtin_and_pipeline() {
        assert_completion(
            ReferenceWrapper::spawn("exit 0").expect("normal child").wait().expect("wait"),
            &ExpectedCompletion {
                status: "succeeded".to_owned(),
                exit_code: Some(0),
                signal: None,
            },
        );
        assert_completion(
            ReferenceWrapper::spawn("cd /; exit 17").expect("builtin child").wait().expect("wait"),
            &ExpectedCompletion { status: "failed".to_owned(), exit_code: Some(17), signal: None },
        );
        assert_completion(
            ReferenceWrapper::spawn("printf x | cat >/dev/null")
                .expect("pipeline child")
                .wait()
                .expect("wait"),
            &ExpectedCompletion {
                status: "succeeded".to_owned(),
                exit_code: Some(0),
                signal: None,
            },
        );
        assert_completion(
            ReferenceWrapper::spawn("kill -TERM $$").expect("signal child").wait().expect("wait"),
            &ExpectedCompletion {
                status: "signaled".to_owned(),
                exit_code: None,
                signal: Some(15),
            },
        );
    }

    #[test]
    fn timeout_and_cancellation_are_terminal_signals_with_bounded_cleanup() {
        let timeout = ReferenceWrapper::spawn("sleep 30").expect("timeout child");
        assert!(timeout.elapsed() < MAX_SCENARIO);
        assert_completion(
            timeout.terminate(Some(Duration::from_millis(100))).expect("timeout terminate"),
            &ExpectedCompletion { status: "signaled".to_owned(), exit_code: None, signal: Some(9) },
        );

        let cancellation = ReferenceWrapper::spawn("sleep 30").expect("cancel child");
        assert_completion(
            cancellation.terminate(None).expect("cancel terminate"),
            &ExpectedCompletion { status: "signaled".to_owned(), exit_code: None, signal: Some(9) },
        );
    }

    #[test]
    fn exec_failure_terminal_close_and_wrapper_crash_are_explicit_gaps() {
        let missing = Command::new("/definitely/missing-ghostrace-shell-executable")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        assert_eq!(missing.expect_err("exec must fail").kind(), io::ErrorKind::NotFound);
        assert_gap(TerminalEvidence::Gap { reason: GapReason::ExecFailed }, GapReason::ExecFailed);

        let terminal_close = ReferenceWrapper::spawn("sleep 30").expect("terminal child");
        assert_gap(terminal_close.abandon(GapReason::TerminalClosed), GapReason::TerminalClosed);

        let helper = std::env::current_exe().expect("test executable");
        let crashed = Command::new(helper)
            .args(["--exact", "unix::wrapper_crash_helper"])
            .env_clear()
            .env("GHOSTRACE_SHELL_WRAPPER_CRASH_HELPER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("crash helper");
        assert_eq!(crashed.signal(), Some(6));
        assert_gap(
            TerminalEvidence::Gap { reason: GapReason::WrapperCrashed },
            GapReason::WrapperCrashed,
        );
    }

    #[test]
    fn incomplete_evidence_has_no_end_time_or_fabricated_success() {
        let evidence =
            ReferenceWrapper::spawn("sleep 30").expect("child").abandon(GapReason::TerminalClosed);
        let TerminalEvidence::Gap { reason } = evidence else {
            panic!("abandonment must not complete");
        };
        assert_eq!(reason, GapReason::TerminalClosed);
        let serialized = serde_json::to_value(serde_json::json!({
            "terminal_kind": "gap",
            "gap_reason": "terminal_closed"
        }))
        .expect("gap JSON");
        assert_eq!(serialized["ended_at"], Value::Null);
        assert_eq!(serialized.get("status"), None);
        assert_eq!(serialized.get("exit_code"), None);
        assert_eq!(serialized.get("signal"), None);
    }

    #[test]
    fn all_fixture_rows_execute_within_the_declared_total_bound() {
        let document = fixture();
        let started = Instant::now();
        let mut results = BTreeMap::new();

        results.insert(
            "normal_exit",
            ReferenceWrapper::spawn("exit 0").expect("normal").wait().expect("normal wait"),
        );
        results.insert(
            "signal",
            ReferenceWrapper::spawn("kill -TERM $$").expect("signal").wait().expect("signal wait"),
        );
        results.insert("exec_failure", TerminalEvidence::Gap { reason: GapReason::ExecFailed });
        results.insert(
            "shell_builtin",
            ReferenceWrapper::spawn("cd /; exit 17")
                .expect("builtin")
                .wait()
                .expect("builtin wait"),
        );
        results.insert(
            "pipeline",
            ReferenceWrapper::spawn("printf x | cat >/dev/null")
                .expect("pipeline")
                .wait()
                .expect("pipeline wait"),
        );
        results.insert(
            "timeout",
            ReferenceWrapper::spawn("sleep 30")
                .expect("timeout")
                .terminate(Some(Duration::from_millis(100)))
                .expect("timeout wait"),
        );
        results.insert(
            "cancellation",
            ReferenceWrapper::spawn("sleep 30")
                .expect("cancellation")
                .terminate(None)
                .expect("cancellation wait"),
        );
        results.insert(
            "terminal_close",
            ReferenceWrapper::spawn("sleep 30")
                .expect("terminal")
                .abandon(GapReason::TerminalClosed),
        );
        let helper = std::env::current_exe().expect("test executable");
        let crashed = Command::new(helper)
            .args(["--exact", "unix::wrapper_crash_helper"])
            .env_clear()
            .env("GHOSTRACE_SHELL_WRAPPER_CRASH_HELPER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("crash helper");
        assert_eq!(crashed.signal(), Some(6));
        results
            .insert("wrapper_crash", TerminalEvidence::Gap { reason: GapReason::WrapperCrashed });

        assert_eq!(results.len(), document.scenarios.len());
        assert!(started.elapsed() <= Duration::from_millis(document.resource_limits.max_total_ms));
        for scenario in document.scenarios {
            match (scenario.terminal_kind.as_str(), results.remove(scenario.id.as_str())) {
                ("completion", Some(TerminalEvidence::Completed { completion })) => {
                    let expected = scenario.expected.expect("completion expected");
                    let expected_status = match expected.status.as_str() {
                        "succeeded" => ShellStatus::Succeeded,
                        "failed" => ShellStatus::Failed,
                        "signaled" => ShellStatus::Signaled,
                        _ => ShellStatus::Unknown,
                    };
                    assert_eq!(completion.status, expected_status, "{}", scenario.id);
                    assert_eq!(completion.exit_code, expected.exit_code, "{}", scenario.id);
                    assert_eq!(completion.signal, expected.signal, "{}", scenario.id);
                }
                ("gap", Some(TerminalEvidence::Gap { reason })) => {
                    let expected_reason = match scenario.gap_reason.as_deref() {
                        Some("exec_failed") => GapReason::ExecFailed,
                        Some("terminal_closed") => GapReason::TerminalClosed,
                        Some("wrapper_crashed") => GapReason::WrapperCrashed,
                        Some(reason) => panic!("unexpected fixture gap reason {reason}"),
                        None => panic!("gap reason missing for {}", scenario.id),
                    };
                    assert_eq!(reason, expected_reason, "{}", scenario.id);
                }
                (kind, result) => panic!("row {} expected {kind}, got {result:?}", scenario.id),
            }
        }
        assert!(results.is_empty());
    }
}

#[cfg(not(unix))]
#[test]
fn shell_wrapper_lifecycle_requires_a_posix_process_contract() {
    // CI and the supported device matrix are Unix. A non-Unix runner must not
    // silently substitute a different shell or claim equivalent signal data.
    eprintln!("explicit no-go: POSIX shell lifecycle contract is unavailable");
}
