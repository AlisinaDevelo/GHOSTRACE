//! The explicit shell run wrapper: consent gating, retained metadata, outcome
//! propagation, and the absence of arguments, environment, and terminal data.

#[cfg(unix)]
mod unix {
    use std::{ffi::OsStr, fs, path::Path};

    use chrono::{TimeZone, Utc};
    use ghostrace::{
        journal::Journal, normalize_executable, ConsentPreview, DeterministicKeyProvider,
        EventKind, EventPayload, EventSource, PathClass, PolicyDocument, RootId, ShellRunEvidence,
        ShellStatus, ShellWorkspaceRoot, ShellWrapper, ShellWrapperConfig, WriterConfig,
        SHELL_EXEC_FAILED_REASON, SHELL_WRAPPER_KIND, UNCLASSIFIED_EXECUTABLE_ID,
    };

    const SECRET: &str = "sentinel-wrapper-secret-value";

    fn policy(sources: &[EventSource]) -> PolicyDocument {
        PolicyDocument::new(
            "shell-wrapper-v1",
            1,
            sources.iter().copied(),
            ["workspace-main"],
            false,
        )
        .expect("policy")
    }

    fn config(workspace: Option<&Path>, home: Option<&Path>) -> ShellWrapperConfig {
        ShellWrapperConfig {
            writer: WriterConfig::default(),
            collector_instance: "live-shell-wrapper-test".to_owned(),
            consent_at: Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
            actor: "human".to_owned(),
            reason: "explicit_run_opt_in".to_owned(),
            workspace: workspace.map(|path| ShellWorkspaceRoot {
                id: RootId::try_from("workspace-main").expect("root id"),
                path: path.to_path_buf(),
            }),
            home: home.map(Path::to_path_buf),
        }
    }

    fn wrapper(journal: &Journal, workspace: Option<&Path>, home: Option<&Path>) -> ShellWrapper {
        let document = policy(&[EventSource::Shell]);
        let confirmation = ConsentPreview::from_policy(
            &document,
            ["executable_id", "working_directory", "timing", "outcome"],
            ["no_arguments", "no_environment", "no_terminal_streams"],
        )
        .expect("preview")
        .confirm();
        ShellWrapper::new(confirmation, document, journal.clone(), config(workspace, home))
            .expect("wrapper")
    }

    fn journal(seed: &str) -> Journal {
        Journal::in_memory(DeterministicKeyProvider::from_seed(seed)).expect("journal")
    }

    fn journal_json(journal: &Journal) -> String {
        let events = journal.events().expect("events");
        serde_json::to_string(&events.iter().map(|stored| &stored.event).collect::<Vec<_>>())
            .expect("events JSON")
    }

    #[test]
    fn policy_without_the_shell_source_is_refused() {
        let journal = journal("shell-no-source");
        let document = policy(&[EventSource::Filesystem]);
        let confirmation =
            ConsentPreview::from_policy(&document, ["executable_id"], ["no_arguments"])
                .expect("preview")
                .confirm();
        assert!(ShellWrapper::new(confirmation, document, journal, config(None, None)).is_err());
    }

    #[test]
    fn completed_run_records_started_and_finished_metadata_only() {
        let workspace = tempfile::tempdir().expect("workspace");
        let journal = journal("shell-success");
        let mut wrapper = wrapper(&journal, Some(workspace.path()), None);

        let report = wrapper
            .run_in(workspace.path(), OsStr::new("/bin/sh"), ["-c", &format!("exit 0 # {SECRET}")])
            .expect("run");

        assert_eq!(report.exit_code, 0);
        let ShellRunEvidence::Completed(metadata) = &report.evidence else {
            panic!("expected completion");
        };
        assert_eq!(metadata.status, ShellStatus::Succeeded);
        assert_eq!(metadata.executable_id.as_str(), "sh");
        assert_eq!(metadata.working_directory.path_class, PathClass::WorkspaceRelative);
        assert!(metadata.ended_at >= metadata.started_at);

        let events = journal.events().expect("events");
        assert_eq!(events.len(), 2);
        let started = &events[0].event;
        let finished = &events[1].event;
        assert_eq!(started.source, EventSource::Shell);
        assert_eq!(started.kind, EventKind::ShellStarted);
        assert_eq!(started.event_id, report.started_event_id);
        let EventPayload::ShellStarted(payload) = &started.payload else {
            panic!("expected shell_started");
        };
        assert_eq!(payload.shell_kind.as_str(), SHELL_WRAPPER_KIND);
        assert_eq!(payload.executable_id.as_ref().map(|id| id.as_str()), Some("sh"));
        assert_eq!(finished.kind, EventKind::ShellFinished);
        assert_eq!(finished.parent_event_id, Some(report.started_event_id));
        let EventPayload::ShellFinished(payload) = &finished.payload else {
            panic!("expected shell_finished");
        };
        assert_eq!(payload.session_id, report.session_id);
        assert_eq!(payload.exit_code, Some(0));
        assert_eq!(payload.signal, None);

        let json = journal_json(&journal);
        assert!(!json.contains(SECRET));
        assert!(!json.contains("exit 0"));
        assert!(!json.contains(&workspace.path().to_string_lossy().to_string()));
    }

    #[test]
    fn wrapper_events_validate_against_the_published_event_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(ghostrace::EVENT_SCHEMA_JSON).expect("schema JSON");
        let validator = jsonschema::options()
            .should_validate_formats(true)
            .build(&schema)
            .expect("valid JSON Schema");
        let journal = journal("shell-schema");
        let mut wrapper = wrapper(&journal, None, None);
        let cwd = std::env::temp_dir();
        wrapper.run_in(&cwd, OsStr::new("/bin/sh"), ["-c", "exit 0"]).expect("run");
        wrapper.run_in(&cwd, OsStr::new("/bin/sh"), ["-c", "kill -TERM $$"]).expect("run");
        wrapper.run_in(&cwd, OsStr::new("/nonexistent/missing"), [] as [&str; 0]).expect("run");
        let events = journal.events().expect("events");
        assert_eq!(events.len(), 6);
        for stored in events {
            let value = serde_json::to_value(&stored.event).expect("event JSON");
            assert!(validator.is_valid(&value), "{}", stored.event.kind);
        }
    }

    #[test]
    fn failure_and_signal_outcomes_propagate_unchanged() {
        let journal = journal("shell-outcomes");
        let mut wrapper = wrapper(&journal, None, None);
        let cwd = std::env::temp_dir();

        let failed = wrapper.run_in(&cwd, OsStr::new("/bin/sh"), ["-c", "exit 17"]).expect("run");
        assert_eq!(failed.exit_code, 17);
        let ShellRunEvidence::Completed(metadata) = &failed.evidence else {
            panic!("expected completion");
        };
        assert_eq!(metadata.status, ShellStatus::Failed);
        assert_eq!(metadata.exit_code, Some(17));

        let signaled =
            wrapper.run_in(&cwd, OsStr::new("/bin/sh"), ["-c", "kill -TERM $$"]).expect("run");
        assert_eq!(signaled.exit_code, 128 + 15);
        let ShellRunEvidence::Completed(metadata) = &signaled.evidence else {
            panic!("expected completion");
        };
        assert_eq!(metadata.status, ShellStatus::Signaled);
        assert_eq!(metadata.signal, Some(15));
        assert_eq!(metadata.exit_code, None);

        let events = journal.events().expect("events");
        let EventPayload::ShellFinished(payload) = &events[3].event.payload else {
            panic!("expected shell_finished");
        };
        assert_eq!(payload.status, ShellStatus::Signaled);
        assert_eq!(payload.signal, Some(15));
    }

    #[test]
    fn exec_failure_is_a_gap_without_a_fabricated_end_or_status() {
        let journal = journal("shell-exec-failure");
        let mut wrapper = wrapper(&journal, None, None);
        let report = wrapper
            .run_in(
                &std::env::temp_dir(),
                OsStr::new("/nonexistent/ghostrace-missing-executable"),
                [SECRET],
            )
            .expect("run");

        assert_eq!(report.exit_code, 127);
        assert_eq!(
            report.evidence,
            ShellRunEvidence::Gap { reason_code: SHELL_EXEC_FAILED_REASON }
        );
        let events = journal.events().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event.kind, EventKind::Gap);
        assert_eq!(events[1].event.parent_event_id, Some(report.started_event_id));
        assert!(!events.iter().any(|stored| stored.event.kind == EventKind::ShellFinished));
        let json = journal_json(&journal);
        assert!(!json.contains(SECRET));
        assert!(!json.contains("nonexistent"));
    }

    #[test]
    fn revoked_consent_refuses_before_spawning() {
        let scratch = tempfile::tempdir().expect("scratch");
        let marker = scratch.path().join("marker");
        let journal = journal("shell-revoked");
        let mut wrapper = wrapper(&journal, None, None);
        wrapper
            .revoke(
                Utc.timestamp_opt(1_750_000_100, 0).single().expect("ts"),
                "human",
                "user_revoked",
            )
            .expect("revoke");

        let result =
            wrapper.run_in(scratch.path(), OsStr::new("/usr/bin/touch"), [marker.as_os_str()]);
        assert!(result.is_err());
        assert!(!marker.exists());
        assert!(journal.events().expect("events").is_empty());
    }

    #[test]
    fn environment_and_standard_streams_never_reach_the_journal() {
        let journal = journal("shell-env");
        let mut wrapper = wrapper(&journal, None, None);
        std::env::set_var("GHOSTRACE_WRAPPER_TEST_TOKEN", SECRET);
        let report = wrapper
            .run_in(
                &std::env::temp_dir(),
                OsStr::new("/bin/sh"),
                ["-c", "printf '%s' \"$GHOSTRACE_WRAPPER_TEST_TOKEN\" >/dev/null; exit 3"],
            )
            .expect("run");
        std::env::remove_var("GHOSTRACE_WRAPPER_TEST_TOKEN");
        assert_eq!(report.exit_code, 3);
        let json = journal_json(&journal);
        assert!(!json.contains(SECRET));
        assert!(!json.contains("GHOSTRACE_WRAPPER_TEST_TOKEN"));
        assert!(!json.contains("printf"));
    }

    #[test]
    fn working_directory_is_classified_and_digested_without_path_text() {
        let workspace = tempfile::tempdir().expect("workspace");
        let home = tempfile::tempdir().expect("home");
        let outside = tempfile::tempdir().expect("outside");
        fs::create_dir_all(workspace.path().join("alpha")).expect("alpha");
        fs::create_dir_all(workspace.path().join("beta")).expect("beta");
        let journal = journal("shell-cwd");
        let mut wrapper = wrapper(&journal, Some(workspace.path()), Some(home.path()));

        let mut class_and_digest = |cwd: &Path| {
            let report =
                wrapper.run_in(cwd, OsStr::new("/usr/bin/true"), [] as [&str; 0]).expect("run");
            let ShellRunEvidence::Completed(metadata) = report.evidence else {
                panic!("expected completion");
            };
            (metadata.working_directory.path_class, metadata.working_directory.path_digest)
        };

        let (alpha_class, alpha) = class_and_digest(&workspace.path().join("alpha"));
        let (_, alpha_again) = class_and_digest(&workspace.path().join("alpha"));
        let (_, beta) = class_and_digest(&workspace.path().join("beta"));
        let (home_class, _) = class_and_digest(home.path());
        let (outside_class, _) = class_and_digest(outside.path());

        assert_eq!(alpha_class, PathClass::WorkspaceRelative);
        assert_eq!(alpha, alpha_again);
        assert_ne!(alpha, beta);
        assert_eq!(home_class, PathClass::HomeRelative);
        assert_eq!(outside_class, PathClass::AbsoluteRedacted);

        let json = journal_json(&journal);
        for dir in [workspace.path(), home.path(), outside.path()] {
            let text = dir.to_string_lossy().to_string();
            assert!(!json.contains(&text));
        }
        assert!(!json.contains("alpha"));
        assert!(!json.contains("beta"));
    }

    #[test]
    fn workspace_must_be_selected_by_policy() {
        let workspace = tempfile::tempdir().expect("workspace");
        let journal = journal("shell-unselected");
        let document = policy(&[EventSource::Shell]);
        let confirmation =
            ConsentPreview::from_policy(&document, ["executable_id"], ["no_arguments"])
                .expect("preview")
                .confirm();
        let mut config = config(None, None);
        config.workspace = Some(ShellWorkspaceRoot {
            id: RootId::try_from("workspace-other").expect("root id"),
            path: workspace.path().to_path_buf(),
        });
        assert!(ShellWrapper::new(confirmation, document, journal, config).is_err());
    }

    #[test]
    fn executable_identity_is_a_normalized_basename_or_unclassified() {
        assert_eq!(normalize_executable(OsStr::new("/usr/bin/Git")).as_str(), "git");
        assert_eq!(normalize_executable(OsStr::new("cargo")).as_str(), "cargo");
        assert_eq!(normalize_executable(OsStr::new("python3.12")).as_str(), "python3.12");
        for hostile in [
            "my tool",
            "deploy-secret",
            "print-password",
            "caf\u{e9}",
            "-rf",
            "",
            "..",
            "tool\nnext",
        ] {
            assert_eq!(
                normalize_executable(OsStr::new(hostile)).as_str(),
                UNCLASSIFIED_EXECUTABLE_ID,
                "{hostile:?}"
            );
        }
    }
}
