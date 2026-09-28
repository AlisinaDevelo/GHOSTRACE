//! End-to-end live CLI against the real login keychain. Runs only with
//! GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 and always forgets its throwaway home.

#![cfg(target_os = "macos")]

use std::process::Command;

fn enabled() -> bool {
    std::env::var_os("GHOSTRACE_LOGIN_KEYCHAIN_TEST").is_some_and(|value| value == "1")
}

fn ghostrace(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ghostrace")).args(args).output().expect("ghostrace runs")
}

#[test]
fn init_run_timeline_and_forget_round_trip() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to run the live CLI end to end");
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let home = directory.path().join("home");
    let home = home.to_str().expect("utf8");
    struct Forget<'a>(&'a str);
    impl Drop for Forget<'_> {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", home]).status.success());
    let _forget = Forget(home);

    let run = ghostrace(&["run", "--home", home, "--", "/bin/sh", "-c", "exit 7", "SENTINEL-ARG"]);
    assert_eq!(run.status.code(), Some(7), "the wrapped exit code is returned");
    let missing = ghostrace(&["run", "--home", home, "--", "/nonexistent/program"]);
    assert_eq!(missing.status.code(), Some(127));

    let timeline = ghostrace(&["live", "timeline", "--home", home, "--json"]);
    assert!(timeline.status.success());
    let text = String::from_utf8(timeline.stdout).expect("utf8");
    let entries: serde_json::Value = serde_json::from_str(&text).expect("timeline JSON");
    let kinds = entries
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| entry["kind"].as_str().expect("kind").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["shell_started", "shell_finished", "shell_started", "gap"]);
    assert!(!text.contains("SENTINEL-ARG") && !text.contains("nonexistent"));

    let status = ghostrace(&["live", "status", "--home", home, "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status["custody"], "login_keychain");
    assert_eq!(status["gaps"], 1);

    assert!(ghostrace(&["live", "forget", "--home", home, "--yes"]).status.success());
    assert!(!std::path::Path::new(home).exists());
    assert!(!ghostrace(&["live", "status", "--home", home]).status.success());
}
