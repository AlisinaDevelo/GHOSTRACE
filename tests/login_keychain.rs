//! Opt-in login-keychain custody. Touches the real login keychain, so it
//! runs only with GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 on a logged-in macOS
//! session, using a unique throwaway item that it always deletes.

#![cfg(target_os = "macos")]

use ghostrace::{
    fixture::ingest_fixture, Journal, KeyCustody, KeyProvider, MacOsKeychainProvider, PolicyProfile,
};

fn enabled() -> bool {
    std::env::var_os("GHOSTRACE_LOGIN_KEYCHAIN_TEST").is_some_and(|value| value == "1")
}

struct Cleanup(MacOsKeychainProvider);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.delete();
    }
}

#[test]
fn custody_is_explicit_and_data_protection_is_the_default() {
    assert_eq!(MacOsKeychainProvider::new().custody(), KeyCustody::DataProtection);
    assert_eq!(MacOsKeychainProvider::login_keychain().custody(), KeyCustody::LoginKeychain);
}

#[test]
fn login_keychain_round_trips_a_journal_key() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to exercise the real login keychain");
        return;
    }
    let service = format!("com.alisinadevelo.ghostrace.test.{}", uuid::Uuid::new_v4());
    let provider =
        MacOsKeychainProvider::login_keychain_with_identity(&service, "journal-key-test")
            .expect("provider");
    let _cleanup = Cleanup(
        MacOsKeychainProvider::login_keychain_with_identity(&service, "journal-key-test")
            .expect("cleanup provider"),
    );
    assert!(provider.key().is_err(), "reading must never create an item");

    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(37).wrapping_add(11);
    }
    provider.provision(key).expect("provision");
    assert_eq!(provider.key().expect("read back"), key);
    assert!(provider.provision(key).is_err(), "an existing item is never replaced");

    let directory = tempfile::tempdir().expect("tempdir");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private");
    }
    let path = directory.path().join("journal.sqlite3");
    let journal = Journal::open_fixture(
        &path,
        MacOsKeychainProvider::login_keychain_with_identity(&service, "journal-key-test")
            .expect("provider"),
    )
    .expect("journal with login-keychain key");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    ingest_fixture(fixture, &journal, &PolicyProfile::fixture_default()).expect("ingest");
    journal.shutdown().expect("shutdown");

    let reopened = Journal::open_fixture(
        &path,
        MacOsKeychainProvider::login_keychain_with_identity(&service, "journal-key-test")
            .expect("provider"),
    )
    .expect("reopen");
    assert_eq!(reopened.events().expect("decrypted events").len(), 8);

    provider.delete().expect("delete");
    assert!(provider.key().is_err());
}
