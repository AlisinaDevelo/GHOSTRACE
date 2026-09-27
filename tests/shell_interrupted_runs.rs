//! Interrupted wrapper runs: forwarded termination signals, crash recovery,
//! and explanations that report a run with no observed end.
//!
//! The helper test re-executes this test binary as a wrapper process against a
//! file-backed journal. Its child signals or aborts the wrapper, and the parent
//! test inspects what the wrapper managed to journal.

#[cfg(unix)]
mod unix {
    use std::{
        ffi::OsStr,
        path::Path,
        process::{Command, Stdio},
    };

    use chrono::{TimeZone, Utc};
    use ghostrace::{
        explain, journal::Journal, ConsentPreview, DeterministicKeyProvider, EventKind,
        EventPayload, EventSource, PolicyDocument, ShellStatus, ShellWrapper, ShellWrapperConfig,
        WriterConfig, SHELL_RUN_INCOMPLETE_REASON,
    };

    const HELPER_MODE: &str = "GHOSTRACE_INTERRUPT_HELPER_MODE";
    const HELPER_JOURNAL: &str = "GHOSTRACE_INTERRUPT_HELPER_JOURNAL";
    const KEY_SEED: &str = "shell-interrupted-runs";

    fn wrapper(journal: &Journal) -> ShellWrapper {
        let document = PolicyDocument::new(
            "shell-interrupt-v1",
            1,
            [EventSource::Shell],
            ["workspace-main"],
            false,
        )
        .expect("policy");
        let confirmation = ConsentPreview::from_policy(
            &document,
            ["executable_id", "working_directory", "timing", "outcome"],
            ["no_arguments", "no_environment", "no_terminal_streams"],
        )
        .expect("preview")
        .confirm();
        ShellWrapper::new(
            confirmation,
            document,
            journal.clone(),
            ShellWrapperConfig {
                writer: WriterConfig::default(),
                collector_instance: "live-shell-interrupt-test".to_owned(),
                consent_at: Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
                actor: "human".to_owned(),
                reason: "explicit_run_opt_in".to_owned(),
                workspace: None,
                home: None,
            },
        )
        .expect("wrapper")
    }

    fn private_directory() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("directory");
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private permissions");
        directory
    }

    fn open(path: &Path) -> Journal {
        Journal::open_fixture(path, DeterministicKeyProvider::from_seed(KEY_SEED)).expect("journal")
    }

    /// Runs only when re-executed by a parent test.
    #[test]
    fn interrupted_run_helper() {
        let (Some(mode), Some(path)) =
            (std::env::var_os(HELPER_MODE), std::env::var_os(HELPER_JOURNAL))
        else {
            return;
        };
        let script = match mode.to_str() {
            Some("term") => "sleep 0.5; kill -TERM $PPID; sleep 5",
            Some("hup") => "sleep 0.5; kill -HUP $PPID; sleep 5",
            Some("abort") => "sleep 0.5; kill -ABRT $PPID; sleep 1",
            _ => panic!("unknown helper mode"),
        };
        let journal = open(Path::new(&path));
        let mut wrapper = wrapper(&journal);
        let report = wrapper
            .run_in(&std::env::temp_dir(), OsStr::new("/bin/sh"), ["-c", script])
            .expect("run");
        std::process::exit(report.exit_code);
    }

    fn run_helper(mode: &str, journal: &Path) -> std::process::ExitStatus {
        Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "unix::interrupted_run_helper", "--nocapture", "--test-threads=1"])
            .env(HELPER_MODE, mode)
            .env(HELPER_JOURNAL, journal)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("helper runs")
    }

    fn assert_forwarded(mode: &str, signal: u8) {
        let directory = private_directory();
        let path = directory.path().join("journal.sqlite3");
        drop(open(&path));
        let status = run_helper(mode, &path);
        assert_eq!(status.code(), Some(128 + i32::from(signal)), "{mode}");

        let journal = open(&path);
        let events = journal.events().expect("events");
        assert_eq!(events.len(), 2, "{mode}");
        let EventPayload::ShellFinished(payload) = &events[1].event.payload else {
            panic!("{mode}: expected shell_finished");
        };
        assert_eq!(payload.status, ShellStatus::Signaled);
        assert_eq!(payload.signal, Some(signal));
        assert_eq!(events[1].event.parent_event_id, Some(events[0].event.event_id));
    }

    #[test]
    fn sigterm_to_the_wrapper_is_forwarded_and_the_outcome_recorded() {
        assert_forwarded("term", 15);
    }

    #[test]
    fn terminal_close_is_forwarded_and_the_outcome_recorded() {
        assert_forwarded("hup", 1);
    }

    #[test]
    fn crashed_wrapper_leaves_an_incomplete_run_that_recovery_closes() {
        let directory = private_directory();
        let path = directory.path().join("journal.sqlite3");
        drop(open(&path));
        let status = run_helper("abort", &path);
        assert!(!status.success());

        let journal = open(&path);
        let events = journal.events().expect("events");
        assert_eq!(events.len(), 1);
        let started = events[0].event.event_id;
        assert_eq!(events[0].event.kind, EventKind::ShellStarted);

        let before = explain(&journal, started).expect("explain");
        assert!(before
            .coverage
            .warnings
            .iter()
            .any(|warning| warning.contains("no terminal observation")
                && warning.contains(&started.to_string())));
        assert!(before.statements.iter().all(|statement| {
            let text = statement.statement.to_ascii_lowercase();
            !text.contains("succeeded") && !text.contains("finished")
        }));

        let mut wrapper = wrapper(&journal);
        // A run younger than the bound may still be live in another process.
        assert_eq!(
            wrapper.recover_incomplete_runs(chrono::Duration::hours(1)).expect("recover"),
            0
        );
        assert_eq!(wrapper.recover_incomplete_runs(chrono::Duration::zero()).expect("recover"), 1);
        assert_eq!(wrapper.recover_incomplete_runs(chrono::Duration::zero()).expect("recover"), 0);

        let events = journal.events().expect("events");
        assert_eq!(events.len(), 2);
        let gap = &events[1].event;
        assert_eq!(gap.kind, EventKind::Gap);
        assert_eq!(gap.parent_event_id, Some(started));
        let EventPayload::Gap(payload) = &gap.payload else { panic!("expected gap") };
        assert_eq!(payload.reason_code.as_str(), SHELL_RUN_INCOMPLETE_REASON);
        assert!(!events.iter().any(|stored| stored.event.kind == EventKind::ShellFinished));

        let after = explain(&journal, started).expect("explain");
        assert!(!after.coverage.warnings.iter().any(|warning| warning.contains("no terminal")));
        let gap_explanation = explain(&journal, gap.event_id).expect("explain gap");
        assert_eq!(gap_explanation.coverage.gap_event_count, 1);
    }

    #[test]
    fn completed_runs_are_not_reported_as_incomplete() {
        let journal =
            Journal::in_memory(DeterministicKeyProvider::from_seed(KEY_SEED)).expect("journal");
        let mut wrapper = wrapper(&journal);
        let report = wrapper
            .run_in(&std::env::temp_dir(), OsStr::new("/bin/sh"), ["-c", "exit 0"])
            .expect("run");
        let explanation = explain(&journal, report.terminal_event_id).expect("explain");
        assert!(!explanation
            .coverage
            .warnings
            .iter()
            .any(|warning| warning.contains("no terminal")));
        assert_eq!(wrapper.recover_incomplete_runs(chrono::Duration::zero()).expect("recover"), 0);
    }
}
