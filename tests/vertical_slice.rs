use std::{fs, path::PathBuf, process::Command};

use chrono::{TimeZone, Utc};
use ghostrace::{
    capture, decrypt_payload, encrypt_payload, explain, export_fixture, ingest_fixture, AppChange,
    BookmarkChange, BrowserBookmarkChangedPayload, BrowserNavigationPayload,
    DeterministicKeyProvider, EntryKind, EventEnvelope, EventKind, EventPayload, EventSource,
    Evidence, ExportPolicyProfile, FileOperation, FilesystemChangedPayload,
    FrontmostAppChangedPayload, Journal, PathClass, PolicyDecision, PolicyProfile, SanitizedUrl,
    ShellFinishedPayload, ShellStatus, SourceErrorPayload, EVENT_SCHEMA_JSON,
};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

fn timestamp(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().expect("valid timestamp")
}

fn make_private(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private directory");
    }
}

fn filesystem_event(id: u128, parent_event_id: Option<Uuid>) -> EventEnvelope {
    EventEnvelope::new(
        Uuid::from_u128(id),
        timestamp(1_735_689_600),
        timestamp(1_735_689_600),
        EventSource::Filesystem,
        EventKind::FilesystemChanged,
        EventPayload::FilesystemChanged(FilesystemChangedPayload {
            root_id: "root-a".to_owned(),
            path_class: PathClass::WorkspaceRelative,
            operation: FileOperation::Modified,
            entry_kind: EntryKind::File,
            path_digest: Some("fixture_secret_not_plaintext".to_owned()),
            size_bytes: Some(42),
        }),
        "fixture-fs",
        Some(format!("cursor-{id}")),
        "fixture-v1",
        "fixture-default-v1",
        1,
        Evidence::Direct,
        parent_event_id,
    )
    .expect("valid event")
}

fn filesystem_test_policy() -> PolicyProfile {
    let mut policy = PolicyProfile::deny_by_default("fixture-default-v1");
    policy.enable_source(EventSource::Filesystem);
    policy.select_root("root-a");
    policy
}

fn fixture_event(
    id: u128,
    source: EventSource,
    kind: EventKind,
    payload: EventPayload,
) -> EventEnvelope {
    EventEnvelope::new(
        Uuid::from_u128(id),
        timestamp(1_735_689_600),
        timestamp(1_735_689_600),
        source,
        kind,
        payload,
        "fixture-schema",
        None,
        "fixture-v1",
        "fixture-default-v1",
        1,
        Evidence::Direct,
        None,
    )
    .expect("valid fixture event")
}

#[test]
fn crypto_round_trip_and_authentication_failure() {
    let provider = DeterministicKeyProvider::from_seed("crypto-test");
    let encrypted = encrypt_payload(&provider, b"event-aad", b"fixture secret").expect("encrypt");
    assert_ne!(encrypted, b"fixture secret");
    assert_eq!(
        decrypt_payload(&provider, b"event-aad", &encrypted).expect("decrypt"),
        b"fixture secret"
    );
    let mut tampered = encrypted.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(decrypt_payload(&provider, b"event-aad", &tampered).is_err());
}

#[test]
fn encrypted_payload_is_not_plaintext_at_rest() {
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("at-rest-test")).expect("journal");
    let event = filesystem_event(1, None);
    let ciphertext = journal.ingest(&event, &filesystem_test_policy()).expect("ingest");
    assert_eq!(ciphertext, 1);
    let bytes = journal.raw_payload_ciphertext(event.event_id).expect("ciphertext");
    assert!(!String::from_utf8_lossy(&bytes).contains("fixture_secret_not_plaintext"));
    assert_eq!(journal.event(event.event_id).expect("read").payload, event.payload);
}

#[test]
fn plaintext_metadata_is_authenticated_with_the_payload() {
    let directory = tempdir().expect("tempdir");
    make_private(directory.path());
    let path = directory.path().join("journal.sqlite3");
    let key = DeterministicKeyProvider::from_seed("metadata-auth-test");
    let event = filesystem_event(1, None);
    let journal = Journal::open_fixture(&path, key.clone()).expect("open");
    journal.ingest(&event, &filesystem_test_policy()).expect("ingest");
    drop(journal);

    let connection = rusqlite::Connection::open(&path).expect("tamper connection");
    connection
        .execute(
            "UPDATE events SET evidence = '\"unknown\"' WHERE event_id = ?1",
            [event.event_id.to_string()],
        )
        .expect("tamper metadata");
    drop(connection);

    let reopened = Journal::open_fixture(&path, key).expect("reopen");
    assert!(reopened.event(event.event_id).is_err());
}

#[test]
fn migration_is_idempotent_and_sqlite_safety_pragmas_are_set() {
    let directory = tempdir().expect("tempdir");
    make_private(directory.path());
    let path = directory.path().join("journal.sqlite3");
    let key = DeterministicKeyProvider::from_seed("migration-test");
    let journal = Journal::open_fixture(&path, key.clone()).expect("open");
    assert_eq!(journal.journal_mode().expect("pragma").to_ascii_lowercase(), "wal");
    assert_eq!(journal.synchronous_mode().expect("pragma"), "FULL");
    assert!(journal.foreign_keys_enabled().expect("pragma"));
    assert_eq!(journal.schema_version_count().expect("schema"), 1);
    drop(journal);
    let reopened = Journal::open_fixture(&path, key).expect("reopen");
    assert_eq!(reopened.schema_version_count().expect("schema"), 1);
    assert_eq!(reopened.journal_mode().expect("pragma").to_ascii_lowercase(), "wal");
}

#[test]
fn policy_is_deny_by_default_and_rejects_unselected_boundaries() {
    let mut profile = PolicyProfile::deny_by_default("policy-test-v1");
    assert!(matches!(
        profile.decide(EventSource::Filesystem, Some("root-a"), false),
        PolicyDecision::Denied { reason: ghostrace::PolicyReason::SourceNotEnabled, .. }
    ));
    profile.enable_source(EventSource::Filesystem);
    assert!(matches!(
        profile.decide(EventSource::Filesystem, Some("root-a"), false),
        PolicyDecision::Denied { reason: ghostrace::PolicyReason::RootNotSelected, .. }
    ));
    profile.select_root("root-a");
    assert!(profile.decide(EventSource::Filesystem, Some("root-a"), false).is_allowed());
    assert!(matches!(
        profile.decide(EventSource::Filesystem, None, true),
        PolicyDecision::Denied { reason: ghostrace::PolicyReason::PrivateContext, .. }
    ));

    let event = filesystem_event(9, None);
    assert!(PolicyProfile::deny_by_default("different-policy").authorize(&event).is_err());

    let mut wrong_version = filesystem_test_policy();
    wrong_version.version = 2;
    let error = wrong_version.authorize(&event).expect_err("version mismatch must fail");
    assert!(error.to_string().contains("policy_profile_mismatch"));
}

#[test]
fn policy_versions_are_immutable() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("policy-version-test"))
        .expect("journal");
    let first = filesystem_event(1, None);
    let second = filesystem_event(2, None);
    let policy = filesystem_test_policy();
    journal.ingest(&first, &policy).expect("first policy use");

    let mut changed_policy = policy;
    changed_policy.enable_source(EventSource::Browser);
    assert!(journal.ingest(&second, &changed_policy).is_err());
}

#[test]
fn browser_url_sanitizes_sensitive_components_and_private_context_is_rejected() {
    let url = SanitizedUrl::parse("https://alice:password@example.test/a?secret=value#fragment")
        .expect("URL");
    assert_eq!(url.as_str(), "https://example.test/a");
    let private =
        BrowserNavigationPayload::new("fixture-browser", "https://example.test/private", true);
    assert!(private.is_err());

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let mut browser =
        serde_json::to_value(ghostrace::read_fixture(&fixture).expect("fixture")[4].clone())
            .expect("JSON");
    browser["payload"]["data"]["private_context"] = json!(true);
    assert!(serde_json::from_value::<EventEnvelope>(browser).is_err());
}

#[test]
fn raw_shell_fields_are_not_deserializable_into_typed_payloads() {
    let payload = json!({
        "type": "shell_started",
        "data": {
            "session_id": "session-1",
            "shell_kind": "zsh",
            "argv": ["--secret"],
            "env": {"TOKEN": "secret"},
            "stdin": "secret",
            "output": "secret"
        }
    });
    assert!(serde_json::from_value::<EventPayload>(payload).is_err());
}

#[test]
fn invalid_schema_and_parent_are_rejected() {
    let event = filesystem_event(1, Some(Uuid::from_u128(99)));
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("parent-test")).expect("journal");
    assert!(journal.ingest(&event, &filesystem_test_policy()).is_err());

    let mut value = serde_json::to_value(filesystem_event(2, None)).expect("JSON");
    value["schema_version"] = json!(99);
    assert!(serde_json::from_value::<EventEnvelope>(value).is_err());

    let mut value = serde_json::to_value(filesystem_event(3, None)).expect("JSON");
    value["raw_collector_blob"] = json!("must fail closed");
    assert!(serde_json::from_value::<EventEnvelope>(value).is_err());

    let mut mutated = filesystem_event(4, None);
    mutated.kind = EventKind::GitSnapshot;
    assert!(mutated.to_json_line().is_err());

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let gap = ghostrace::read_fixture(&fixture).expect("fixture")[5].clone();
    let mut value = serde_json::to_value(gap).expect("JSON");
    value["payload"]["data"]["source"] = json!("browser");
    assert!(serde_json::from_value::<EventEnvelope>(value).is_err());
}

#[test]
fn fixture_and_event_size_limits_fail_closed() {
    let directory = tempdir().expect("tempdir");
    let fixture = directory.path().join("oversized.jsonl");
    fs::write(&fixture, "x".repeat(ghostrace::fixture::MAX_FIXTURE_LINE_BYTES + 1))
        .expect("write oversized fixture");
    assert!(ghostrace::read_fixture(&fixture).is_err());

    let oversized = EventEnvelope::new(
        Uuid::from_u128(88),
        timestamp(1_735_689_600),
        timestamp(1_735_689_600),
        EventSource::Filesystem,
        EventKind::FilesystemChanged,
        EventPayload::FilesystemChanged(FilesystemChangedPayload {
            root_id: "root-a".to_owned(),
            path_class: PathClass::WorkspaceRelative,
            operation: FileOperation::Modified,
            entry_kind: EntryKind::File,
            path_digest: Some("x".repeat(ghostrace::model::MAX_EVENT_PAYLOAD_BYTES)),
            size_bytes: None,
        }),
        "fixture-fs",
        None,
        "fixture-v1",
        "fixture-default-v1",
        1,
        Evidence::Direct,
        None,
    );
    assert!(oversized.is_err());
}

#[test]
fn fixture_ingest_rejects_spoofed_provenance_without_echoing_it() {
    let directory = tempdir().expect("tempdir");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let first_line = fs::read_to_string(fixture)
        .expect("fixture")
        .lines()
        .next()
        .expect("first line")
        .to_owned();
    let mut event: serde_json::Value = serde_json::from_str(&first_line).expect("JSON");
    event["provenance_version"] = json!("endpoint-security-sensitive-value");
    let spoofed = directory.path().join("spoofed.jsonl");
    fs::write(&spoofed, serde_json::to_string(&event).expect("serialize")).expect("write");

    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("spoof-test")).expect("journal");
    let error = ingest_fixture(&spoofed, &journal, &PolicyProfile::fixture_default())
        .expect_err("spoofed provenance must fail");
    let display = error.to_string();
    assert_eq!(display, "fixture event provenance is invalid");
    assert!(!display.contains("sensitive"));
    assert!(journal.events().expect("events").is_empty());
}

#[test]
fn fixture_errors_do_not_echo_untrusted_content_or_paths() {
    let directory = tempdir().expect("tempdir");
    let fixture = directory.path().join("secret-customer-path.jsonl");
    fs::write(&fixture, r#"{"secret_control":"do-not-echo"}"#).expect("write");
    let display = ghostrace::read_fixture(&fixture).expect_err("invalid fixture").to_string();
    assert!(!display.contains("do-not-echo"));
    assert!(!display.contains("secret_control"));
    assert!(!display.contains("secret-customer-path"));

    let missing = directory.path().join("secret-missing-path.jsonl");
    let display = ghostrace::read_fixture(&missing).expect_err("missing fixture").to_string();
    assert_eq!(display, "I/O operation failed");
    assert!(!display.contains("secret-missing-path"));
}

#[test]
fn published_json_schema_compiles_and_matches_fixture_envelopes() {
    let schema: serde_json::Value = serde_json::from_str(EVENT_SCHEMA_JSON).expect("schema JSON");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("valid JSON Schema");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let mut events = ghostrace::read_fixture(&fixture).expect("fixture");
    events.extend([
        fixture_event(
            43,
            EventSource::FrontmostApp,
            EventKind::FrontmostAppChanged,
            EventPayload::FrontmostAppChanged(FrontmostAppChangedPayload {
                app_id: "com.example.fixture".to_owned(),
                change: AppChange::Activated,
                previous_app_id: None,
            }),
        ),
        fixture_event(
            44,
            EventSource::Shell,
            EventKind::ShellFinished,
            EventPayload::ShellFinished(ShellFinishedPayload {
                session_id: "session-schema".to_owned(),
                status: ShellStatus::Succeeded,
                exit_code: Some(0),
                duration_ms: 10,
            }),
        ),
        fixture_event(
            45,
            EventSource::Browser,
            EventKind::BrowserBookmarkChanged,
            EventPayload::BrowserBookmarkChanged(BrowserBookmarkChangedPayload {
                browser: "fixture-browser".to_owned(),
                bookmark_id: "bookmark-schema".to_owned(),
                change: BookmarkChange::Created,
                url: SanitizedUrl::parse("https://example.test/schema?discard=true").expect("URL"),
                folder_id: None,
                private_context: false,
            }),
        ),
        fixture_event(
            46,
            EventSource::Git,
            EventKind::SourceError,
            EventPayload::SourceError(SourceErrorPayload {
                source: EventSource::Git,
                reason_code: "fixture_source_error".to_owned(),
                retryable: true,
            }),
        ),
    ]);
    assert_eq!(events.len(), 12, "every payload variant must be represented");
    for event in events {
        let value = serde_json::to_value(event).expect("event JSON");
        assert!(validator.is_valid(&value), "schema rejected {value}");
    }

    let mut prohibited = serde_json::to_value(filesystem_event(41, None)).expect("event JSON");
    prohibited["payload"]["data"]["raw_path"] = json!("/Users/private");
    assert!(!validator.is_valid(&prohibited));

    let mut mismatched = serde_json::to_value(filesystem_event(42, None)).expect("event JSON");
    mismatched["kind"] = json!("git_snapshot");
    assert!(!validator.is_valid(&mismatched));

    let fixture_events = ghostrace::read_fixture(&fixture).expect("fixture");
    let mut mismatched_status = serde_json::to_value(&fixture_events[5]).expect("event JSON");
    mismatched_status["payload"]["data"]["source"] = json!("browser");
    assert!(!validator.is_valid(&mismatched_status));
}

#[test]
fn fixture_explanation_is_deterministic_and_cites_the_complete_chain() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("explain-test")).expect("journal");
    ingest_fixture(&fixture, &journal, &PolicyProfile::fixture_default()).expect("ingest");
    let target = Uuid::parse_str("00000000-0000-4000-8000-000000000008").expect("target");
    let first = explain(&journal, target).expect("explain");
    let second = explain(&journal, target).expect("explain");
    assert_eq!(first, second);
    assert_eq!(first.chain_event_ids.len(), 8);
    assert_eq!(first.statements.len(), 8);
    assert_eq!(first.coverage.gap_event_count, 1);
    assert!(first.coverage.warnings[0].contains("00000000-0000-4000-8000-000000000006"));
    for statement in &first.statements {
        assert_eq!(statement.citations, vec![statement.event_id]);
    }
}

#[test]
fn event_envelope_serialization_matches_checked_in_golden() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let event = ghostrace::read_fixture(&fixture).expect("fixture")[0].clone();
    let golden_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/event-envelope-v1.golden.json");
    let golden = fs::read_to_string(golden_path).expect("golden");
    assert_eq!(event.to_json_line().expect("serialize"), golden.trim());
}

#[test]
fn export_refuses_overwrite_without_force_and_writes_manifest() {
    let directory = tempdir().expect("tempdir");
    let output = directory.path().join("export.jsonl");
    fs::write(&output, "do not overwrite").expect("seed output");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    assert!(export_fixture(&fixture, &output, false).is_err());
    assert_eq!(fs::read_to_string(&output).expect("read"), "do not overwrite");
    let manifest = export_fixture(&fixture, &output, true).expect("force export");
    assert_eq!(manifest.export_version, 1);
    assert_eq!(manifest.coverage.gap_count, 1);
    assert_eq!(
        manifest.policy_profiles,
        vec![ExportPolicyProfile { id: "fixture-default-v1".to_owned(), version: 1 }]
    );
    let output_text = fs::read_to_string(&output).expect("read");
    let first_line = output_text.lines().next().expect("manifest");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(first_line).expect("JSON")["record_type"],
        "manifest"
    );
}

#[test]
fn capture_is_explicitly_refused() {
    let error = capture().expect_err("capture must be disabled");
    assert!(error.to_string().contains("intentionally disabled"));
    let output = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .arg("capture")
        .output()
        .expect("capture command");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("intentionally disabled"));
}

#[test]
fn cli_exposes_help_and_version() {
    let binary = env!("CARGO_BIN_EXE_ghostrace");
    let help = Command::new(binary).arg("--help").output().expect("help");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).to_ascii_lowercase().contains("fixture-only"));
    let version = Command::new(binary).arg("--version").output().expect("version");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("ghostrace 0.0.1"));
}
