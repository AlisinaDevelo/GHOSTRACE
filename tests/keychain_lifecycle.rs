#![cfg(target_os = "macos")]

//! Device-only Keychain lifecycle reproduction.
//!
//! This test deliberately uses an isolated temporary keychain so it can lock
//! and unlock the real Security.framework boundary without touching the user's
//! login keychain. It is ignored in ordinary CI and run explicitly on a device.

use std::{path::PathBuf, process::Command, sync::Arc};

use ghostrace::{
    read_fixture, EventEnvelope, IngestionOrigin, Journal, KeyProvider, KeyUnavailablePolicy,
    MacOsKeychainProvider, PolicyProfile, Writer, WriterConfig, WriterGapReason, WriterOutcome,
};
use tempfile::tempdir;
use uuid::Uuid;

fn security(args: &[&str]) -> std::process::Output {
    Command::new("/usr/bin/security").args(args).output().expect("security command")
}

fn security_ok(args: &[&str]) -> std::process::Output {
    let output = security(args);
    assert!(
        output.status.success(),
        "security {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn keychain_list() -> Vec<String> {
    let output = security_ok(&["list-keychains", "-d", "user"]);
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"')?.strip_suffix('"'))
        .map(str::to_owned)
        .collect()
}

fn default_keychain() -> String {
    let output = security_ok(&["default-keychain", "-d", "user"]);
    String::from_utf8_lossy(&output.stdout).trim().trim_matches('"').to_owned()
}

struct IsolatedKeychain {
    path: PathBuf,
    password: String,
    original_search_list: Vec<String>,
    original_default: String,
    service: String,
    account: String,
}

impl IsolatedKeychain {
    fn create() -> Self {
        let directory = tempdir().expect("temporary keychain directory");
        let path = directory.keep().join("lifecycle.keychain-db");
        let password = format!("ghostrace-test-{}", Uuid::new_v4().simple());
        let original_search_list = keychain_list();
        let original_default = default_keychain();
        security_ok(&["create-keychain", "-p", &password, path.to_str().expect("path")]);
        security_ok(&["unlock-keychain", "-p", &password, path.to_str().expect("path")]);
        security_ok(&["set-keychain-settings", "-lut", "3600", path.to_str().expect("path")]);
        security_ok(&["list-keychains", "-d", "user", "-s", path.to_str().expect("path")]);
        security_ok(&["default-keychain", "-d", "user", "-s", path.to_str().expect("path")]);
        let suffix = Uuid::new_v4().simple();
        Self {
            path,
            password,
            original_search_list,
            original_default,
            service: format!("com.alisinadevelo.ghostrace.lifecycle-{suffix}"),
            account: format!("journal-key-{suffix}"),
        }
    }

    fn path_str(&self) -> &str {
        self.path.to_str().expect("keychain path")
    }

    fn provider(&self) -> MacOsKeychainProvider {
        MacOsKeychainProvider::with_identity_in_keychain(
            &self.service,
            &self.account,
            Option::<String>::None,
            &self.path,
        )
        .expect("provider identity")
    }
}

impl Drop for IsolatedKeychain {
    fn drop(&mut self) {
        let _ = security(&["unlock-keychain", "-p", &self.password, self.path_str()]);
        let _ = security(&[
            "delete-generic-password",
            "-a",
            &self.account,
            "-s",
            &self.service,
            self.path_str(),
        ]);
        let mut args = vec!["list-keychains", "-d", "user", "-s"];
        args.extend(self.original_search_list.iter().map(String::as_str));
        let _ = security(&args);
        let _ = security(&["default-keychain", "-d", "user", "-s", &self.original_default]);
        let _ = security(&["delete-keychain", self.path_str()]);
        if let Some(directory) = self.path.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

fn fixture() -> (IngestionOrigin, EventEnvelope, PolicyProfile) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    (
        IngestionOrigin::fixture(),
        read_fixture(path).expect("fixture events").remove(0),
        PolicyProfile::fixture_default(),
    )
}

#[test]
#[ignore = "requires explicit device Keychain lifecycle authorization"]
fn locked_keychain_emits_a_gap_and_recovers_after_unlock_without_plaintext() {
    let isolated = IsolatedKeychain::create();
    let provider = Arc::new(isolated.provider());
    provider.provision([7_u8; 32]).expect("provision isolated key");
    assert_eq!(provider.key().expect("unlocked key read"), [7_u8; 32]);

    let journal = Journal::in_memory(Arc::clone(&provider)).expect("journal");
    let (origin, event, policy) = fixture();
    let writer = Writer::new(
        journal.clone(),
        WriterConfig {
            key_unavailable_policy: KeyUnavailablePolicy::EmitGap,
            ..WriterConfig::default()
        },
    )
    .expect("writer");

    security_ok(&["lock-keychain", isolated.path_str()]);
    let locked_error = provider.key().expect_err("locked keychain must fail closed");
    assert!(!locked_error.to_string().contains(&isolated.service));
    assert!(!locked_error.to_string().contains(&isolated.account));

    let gap = writer
        .submit(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
        .expect("locked key should produce an explicit gap");
    assert!(matches!(
        gap,
        WriterOutcome::Gap(ghostrace::WriterGap {
            reason: WriterGapReason::KeyUnavailable,
            event_count: 1,
            ..
        })
    ));
    assert!(journal.events().expect("events after gap").is_empty());
    assert_eq!(writer.outstanding(), (0, 0));

    security_ok(&["unlock-keychain", "-p", &isolated.password, isolated.path_str()]);
    assert_eq!(provider.key().expect("unlocked key recovery"), [7_u8; 32]);
    let committed =
        writer.submit(origin, vec![event], policy, Vec::new()).expect("post-unlock commit");
    assert!(matches!(committed, WriterOutcome::Committed(_)));
    assert_eq!(journal.events().expect("events after recovery").len(), 1);
}
