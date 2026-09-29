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

    // Without consent, `run` refuses before spawning anything.
    let marker = directory.path().join("spawned");
    let touch = format!("touch {}", marker.display());
    let refused = ghostrace(&["run", "--home", home, "--", "/bin/sh", "-c", &touch]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("consent-shell"));
    assert!(!marker.exists(), "a refused run must not spawn the command");
    assert!(ghostrace(&["live", "consent-shell", "--home", home, "--yes"]).status.success());

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
    assert_eq!(status["shell_consent"], true);

    // The HTML report carries no argument, environment, or path content.
    let report = directory.path().join("report.html");
    let written = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(["run", "--home", home, "--", "/bin/sh", "-c", "exit 0", "SENTINEL-ARG"])
        .env("SENTINEL_ENV", "SENTINEL-ENV-VALUE")
        .status()
        .expect("run");
    assert!(written.success());
    let args = ["live", "report", "--home", home, "--yes", "--output", report.to_str().unwrap()];
    assert!(ghostrace(&args).status.success());
    let html = std::fs::read_to_string(&report).expect("report");
    for sentinel in ["SENTINEL", directory.path().to_str().unwrap(), "/bin/sh", "exit 0"] {
        assert!(!html.contains(sentinel), "report leaked {sentinel}");
    }
    assert!(html.contains("shell"));
    assert!(!ghostrace(&args).status.success(), "an existing report is never replaced");
    // Through the /var symlink, the home is still recognized.
    let inside = format!("{home}/report.html");
    assert!(home.starts_with("/var/") || home.starts_with("/private/"));
    let inside_args = ["live", "report", "--home", home, "--yes", "--output", inside.as_str()];
    assert!(!ghostrace(&inside_args).status.success(), "reports stay outside the home");
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&report).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    // Revoking consent makes the next run refuse before spawning.
    assert!(ghostrace(&["live", "revoke-shell", "--home", home]).status.success());
    let refused = ghostrace(&["run", "--home", home, "--", "/bin/sh", "-c", &touch]);
    assert!(!refused.status.success());
    assert!(!marker.exists());

    assert!(ghostrace(&["live", "forget", "--home", home, "--yes"]).status.success());
    assert!(!std::path::Path::new(home).exists());
    assert!(!ghostrace(&["live", "status", "--home", home]).status.success());
}

#[test]
fn watching_a_folder_that_contains_the_home_never_records_the_home() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to run the live CLI end to end");
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let folder = directory.path().canonicalize().expect("canonical");
    let home_path = folder.join("home");
    let home = home_path.to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home]).status.success());
    let _forget = Forget(home.clone());

    let watch = |write: &dyn Fn()| {
        let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
            .args(["live", "watch", folder.to_str().unwrap(), "--home", &home, "--yes"])
            .args(["--seconds", "4"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("watch starts");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        write();
        let output = child.wait_with_output().expect("watch ends");
        assert!(output.status.success());
        String::from_utf8(output.stdout).expect("utf8")
    };

    let inside = watch(&|| {
        for index in 0..3 {
            std::fs::write(home_path.join(format!("scratch-{index}")), b"x").expect("write");
        }
    });
    assert!(inside.contains(" 0 change(s) recorded"), "{inside}");

    let outside = watch(&|| std::fs::write(folder.join("outside.txt"), b"x").expect("write"));
    assert!(!outside.contains(" 0 change(s) recorded"), "{outside}");
}

#[test]
fn live_export_validates_and_is_never_seen_by_a_running_watch() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to run the live CLI end to end");
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let folder = directory.path().canonicalize().expect("canonical");
    let home = folder.join("home").to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home]).status.success());
    let _forget = Forget(home.clone());
    assert!(ghostrace(&["live", "consent-shell", "--home", &home, "--yes"]).status.success());
    assert!(ghostrace(&["run", "--home", &home, "--", "/usr/bin/true"]).status.success());

    let export = folder.join("export.jsonl");
    let archive = folder.join("archive.parquet");
    let report = folder.join("report.html");
    let watch = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(["live", "watch", folder.to_str().unwrap(), "--home", &home, "--yes"])
        .args(["--seconds", "6"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("watch starts");
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let mut args = vec!["live", "export", "--home", &home, "--yes", "--output"];
    args.push(export.to_str().unwrap());
    if cfg!(feature = "parquet") {
        args.extend(["--parquet", archive.to_str().unwrap()]);
    }
    let exported = ghostrace(&args);
    assert!(exported.status.success(), "{}", String::from_utf8_lossy(&exported.stderr));
    let stdout = String::from_utf8_lossy(&exported.stdout);
    assert!(stdout.contains("Exported "), "{stdout}");
    let report_args = ["live", "report", "--home", &home, "--yes", "--output"];
    assert!(ghostrace(&[&report_args[..], &[report.to_str().unwrap()]].concat()).status.success());

    let output = watch.wait_with_output().expect("watch ends");
    let summary = String::from_utf8(output.stdout).expect("utf8");
    assert!(summary.contains(" 0 change(s) recorded"), "{summary}");

    assert!(ghostrace(&["validate", "--export", export.to_str().unwrap()]).status.success());
    if cfg!(feature = "parquet") {
        let verify = ["verify-archive", "--archive", archive.to_str().unwrap()];
        let verify = [&verify[..], &["--export", export.to_str().unwrap()]].concat();
        assert!(ghostrace(&verify).status.success());
    }
    // A second export to the same destination, or one inside the home, is refused.
    assert!(!ghostrace(&args).status.success());
    let inside = format!("{home}/export.jsonl");
    let inside_args = ["live", "export", "--home", &home, "--yes", "--output", inside.as_str()];
    assert!(!ghostrace(&inside_args).status.success());
}
