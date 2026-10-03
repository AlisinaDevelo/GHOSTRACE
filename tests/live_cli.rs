//! End-to-end live CLI against the real login keychain. Runs only with
//! GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 and always forgets its throwaway home.

#![cfg(target_os = "macos")]

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdout, Command, Output, Stdio},
    time::{Duration, Instant},
};

fn enabled() -> bool {
    std::env::var_os("GHOSTRACE_LOGIN_KEYCHAIN_TEST").is_some_and(|value| value == "1")
}

/// The frontmost application's bundle ID, from LaunchServices.
#[allow(dead_code)]
fn front_bundle() -> String {
    let asn = Command::new("/usr/bin/lsappinfo").arg("front").output().expect("lsappinfo front");
    let asn = String::from_utf8_lossy(&asn.stdout).trim().to_owned();
    assert!(!asn.is_empty(), "no frontmost application");
    let info = Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "bundleid", &asn])
        .output()
        .expect("lsappinfo info");
    String::from_utf8_lossy(&info.stdout)
        .rsplit('=')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .to_owned()
}

fn ghostrace(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ghostrace")).args(args).output().expect("ghostrace runs")
}

fn ghostrace_with_input(args: &[String], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ghostrace starts");
    child
        .stdin
        .take()
        .expect("ghostrace stdin")
        .write_all(input.as_bytes())
        .expect("answer prompt");
    child.wait_with_output().expect("ghostrace ends")
}

struct LiveWatch {
    child: Option<Child>,
    prefix: Vec<u8>,
    reader: Option<std::thread::JoinHandle<()>>,
}

fn read_watch_ready(stdout: ChildStdout) -> io::Result<(Vec<u8>, ChildStdout)> {
    let mut reader = BufReader::new(stdout);
    let mut prefix = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "watch exited before ready"));
        }
        prefix.extend_from_slice(line.as_bytes());
        if prefix.len() > 64 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "watch preview exceeded bound"));
        }
        if line.trim_end() == "Watching. Press Ctrl-C to stop." {
            prefix.extend_from_slice(reader.buffer());
            return Ok((prefix, reader.into_inner()));
        }
    }
}

fn stop_and_reap(mut child: Child) -> io::Result<Output> {
    let stop = (|| {
        if child.try_wait()?.is_none() {
            // SAFETY: this is the still-owned, unreaped child process, not a
            // PID discovered elsewhere. The watch handles SIGINT as Ctrl-C.
            let signalled = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
            if signalled != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait()?.is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
            }
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
        }
        Ok(())
    })();
    if stop.is_err() {
        let _ = child.kill();
    }
    let output = child.wait_with_output();
    stop?;
    output
}

impl LiveWatch {
    fn start(folder: &Path, home: &str) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
            .args(["live", "watch", folder.to_str().expect("utf8"), "--home", home, "--yes"])
            .args(["--seconds", "30"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("watch starts");
        let mut watch = Self { child: Some(child), prefix: Vec::new(), reader: None };
        let stdout = watch.child.as_mut().expect("child").stdout.take().expect("watch stdout");
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        watch.reader = Some(std::thread::spawn(move || {
            let _ = sender.send(read_watch_ready(stdout));
        }));
        let error = match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok((prefix, stdout))) => {
                watch.prefix = prefix;
                watch.child.as_mut().expect("child").stdout = Some(stdout);
                watch.reader.take().expect("reader").join().expect("readiness thread");
                return watch;
            }
            Ok(Err(error)) => error.to_string(),
            Err(error) => error.to_string(),
        };
        let output = watch.into_output();
        panic!("watch readiness failed: {error}; {}", String::from_utf8_lossy(&output.stderr));
    }

    fn into_output(mut self) -> Output {
        let mut output = stop_and_reap(self.child.take().expect("child")).expect("watch ends");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("readiness thread");
        }
        self.prefix.extend_from_slice(&output.stdout);
        output.stdout = std::mem::take(&mut self.prefix);
        output
    }

    fn finish(mut self) -> Output {
        let exited = self.child.as_mut().expect("child").try_wait().expect("watch status");
        let output = self.into_output();
        assert!(exited.is_none(), "watch exited before requested stop: {}", output.status);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("0 lost."));
        output
    }
}

impl Drop for LiveWatch {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            let _ = stop_and_reap(child);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn live_watch_guard_reaps_its_child_during_unwind() {
    let child = Command::new("/bin/sleep").arg("30").spawn().expect("sleep starts");
    let pid = child.id() as libc::pid_t;
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _watch = LiveWatch { child: Some(child), prefix: Vec::new(), reader: None };
        panic!("synthetic assertion failure");
    }));
    assert!(unwind.is_err());
    // SAFETY: waitpid only checks whether the known child remains unreaped.
    let waited = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    assert_eq!(waited, -1, "the guard must reap its child before home cleanup");
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ECHILD));
}

fn assert_private_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path).expect("output metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "{} must be private", path.display());
}

fn assert_no_export_temporary_files(folder: &Path) {
    let leftovers = fs::read_dir(folder)
        .expect("output folder")
        .map(|entry| entry.expect("output entry").file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.starts_with(".ghostrace-export-incomplete-")
                || name.starts_with(".ghostrace-archive-")
        })
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "temporary export artifacts remain: {leftovers:?}");
}

fn preview_value(stdout: &str, prefix: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("missing {prefix:?} line in output:\n{stdout}"))
}

fn preview_event_count(stdout: &str) -> u64 {
    let line = stdout
        .lines()
        .find(|line| line.starts_with("Events:"))
        .unwrap_or_else(|| panic!("missing Events line in output:\n{stdout}"));
    line.strip_prefix("Events:")
        .expect("Events prefix")
        .split_whitespace()
        .next()
        .expect("preview event count")
        .parse()
        .expect("numeric preview event count")
}

fn export_manifest(path: &Path) -> serde_json::Value {
    let text = fs::read_to_string(path).expect("export");
    let line = text.lines().next().expect("manifest line");
    serde_json::from_str(line).expect("manifest JSON")
}

fn assert_no_filesystem_events(home: &str) {
    let timeline = ghostrace(&["live", "timeline", "--home", home, "--json"]);
    assert!(timeline.status.success(), "{}", String::from_utf8_lossy(&timeline.stderr));
    let entries: serde_json::Value = serde_json::from_slice(&timeline.stdout).expect("timeline");
    assert!(
        entries
            .as_array()
            .expect("entries")
            .iter()
            .all(|entry| { entry["kind"].as_str().expect("event kind") != "filesystem_changed" }),
        "internal artifacts must not become filesystem events, including the final drain"
    );
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
    let initialized = ghostrace(&["live", "init", "--home", home]);
    assert!(initialized.status.success());
    let _forget = Forget(home);
    let init_message = String::from_utf8_lossy(&initialized.stdout);
    assert!(init_message.contains("`ghostrace run -- <command>` requires"));
    assert!(init_message.contains("policy-gated without requiring shell consent"));

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
        let child = LiveWatch::start(&folder, &home);
        write();
        let output = child.finish();
        String::from_utf8(output.stdout).expect("utf8")
    };

    let inside = watch(&|| {
        for index in 0..3 {
            std::fs::write(home_path.join(format!("scratch-{index}")), b"x").expect("write");
        }
    });
    assert!(inside.contains(" 0 change(s) recorded"), "{inside}");
    assert_no_filesystem_events(&home);

    let outside = watch(&|| std::fs::write(folder.join("outside.txt"), b"x").expect("write"));
    assert!(!outside.contains(" 0 change(s) recorded"), "{outside}");
}

#[test]
fn live_export_interactive_decline_leaves_no_outputs_or_temporary_files() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to run the live CLI end to end");
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let folder = directory.path().canonicalize().expect("canonical");
    let home = folder.join("home");
    let home_string = home.to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home_string]).status.success());
    let _forget = Forget(home_string.clone());
    assert!(ghostrace(&["live", "consent-shell", "--home", &home_string, "--yes"])
        .status
        .success());
    assert!(ghostrace(&["run", "--home", &home_string, "--", "/usr/bin/true"]).status.success());

    let export = folder.join("declined.jsonl");
    let archive = folder.join("declined.parquet");
    let mut args = vec![
        "live".to_owned(),
        "export".to_owned(),
        "--home".to_owned(),
        home_string,
        "--output".to_owned(),
        export.to_str().expect("utf8").to_owned(),
    ];
    if cfg!(feature = "parquet") {
        args.extend(["--parquet".to_owned(), archive.to_str().expect("utf8").to_owned()]);
    }

    let declined = ghostrace_with_input(&args, "n\n");
    assert!(declined.status.success(), "{}", String::from_utf8_lossy(&declined.stderr));
    assert!(String::from_utf8_lossy(&declined.stdout).contains("No export was written."));
    assert!(!export.exists(), "declining must not publish JSONL");
    assert!(!archive.exists(), "declining must not publish Parquet");
    assert_no_export_temporary_files(&folder);
}

#[test]
fn live_export_interactive_preview_matches_manifest_and_writes_private_outputs() {
    if !enabled() {
        eprintln!("set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 to run the live CLI end to end");
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let folder = directory.path().canonicalize().expect("canonical");
    let home = folder.join("home");
    let home_string = home.to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home_string]).status.success());
    let _forget = Forget(home_string.clone());
    assert!(ghostrace(&["live", "consent-shell", "--home", &home_string, "--yes"])
        .status
        .success());
    assert!(ghostrace(&["run", "--home", &home_string, "--", "/usr/bin/true"]).status.success());

    let export = folder.join("interactive.jsonl");
    let archive = folder.join("interactive.parquet");
    let mut args = vec![
        "live".to_owned(),
        "export".to_owned(),
        "--home".to_owned(),
        home_string.clone(),
        "--output".to_owned(),
        export.to_str().expect("utf8").to_owned(),
    ];
    if cfg!(feature = "parquet") {
        args.extend(["--parquet".to_owned(), archive.to_str().expect("utf8").to_owned()]);
    }

    // Render the preview through the same CLI executable and keychain access
    // path as the affirmative run. Declining does not mutate the journal, so
    // these values are the exact confirmation rendered for the later write.
    let declined = ghostrace_with_input(&args, "n\n");
    assert!(declined.status.success(), "{}", String::from_utf8_lossy(&declined.stderr));
    let declined_stdout = String::from_utf8(declined.stdout).expect("utf8");
    let expected_event_count = preview_event_count(&declined_stdout);
    let expected_plan = preview_value(&declined_stdout, "Plan ");
    let expected_snapshot = preview_value(&declined_stdout, "Snapshot ");
    assert!(!export.exists(), "declining the preview must not publish JSONL");
    assert!(!archive.exists(), "declining the preview must not publish Parquet");
    assert_no_export_temporary_files(&folder);

    let exported = ghostrace_with_input(&args, "y\n");
    assert!(exported.status.success(), "{}", String::from_utf8_lossy(&exported.stderr));
    let stdout = String::from_utf8(exported.stdout).expect("utf8");
    assert_eq!(preview_event_count(&stdout), expected_event_count);
    assert_eq!(preview_value(&stdout, "Plan "), expected_plan);
    assert_eq!(preview_value(&stdout, "Snapshot "), expected_snapshot);

    let manifest = export_manifest(&export);
    assert_eq!(manifest["query_scope"]["kind"], "all_committed");
    assert_eq!(manifest["coverage"]["event_count"].as_u64(), Some(expected_event_count));
    assert_eq!(manifest["record_counts"]["event"].as_u64(), Some(expected_event_count));
    // The default live export includes the complete journal, so its body
    // digest is the snapshot digest confirmed immediately before publication;
    // manifests store that digest as bare hexadecimal while the preview uses
    // the `sha256:`-tagged SnapshotDigest representation.
    let snapshot_hex = expected_snapshot.strip_prefix("sha256:").expect("tagged snapshot digest");
    assert_eq!(manifest["record_digests"]["event"].as_str(), Some(snapshot_hex));
    assert!(ghostrace(&["validate", "--export", export.to_str().expect("utf8")]).status.success());
    assert_private_file(&export);
    if cfg!(feature = "parquet") {
        assert!(ghostrace(&[
            "verify-archive",
            "--archive",
            archive.to_str().expect("utf8"),
            "--export",
            export.to_str().expect("utf8"),
        ])
        .status
        .success());
        assert_private_file(&archive);
    }
    assert_no_export_temporary_files(&folder);

    let duplicate = ghostrace_with_input(&args, "y\n");
    assert!(!duplicate.status.success(), "an existing export must not be replaced");
    let inside = home.join("inside.jsonl");
    let inside_args = vec![
        "live".to_owned(),
        "export".to_owned(),
        "--home".to_owned(),
        home_string,
        "--yes".to_owned(),
        "--output".to_owned(),
        inside.to_str().expect("utf8").to_owned(),
    ];
    let refused = ghostrace_with_input(&inside_args, "y\n");
    assert!(!refused.status.success(), "an export inside the home must be refused");
    assert!(!inside.exists());
    assert_no_export_temporary_files(&folder);
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
    let watch = LiveWatch::start(&folder, &home);

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

    let output = watch.finish();
    let summary = String::from_utf8(output.stdout).expect("utf8");
    assert!(summary.contains(" 0 change(s) recorded"), "{summary}");
    assert_no_filesystem_events(&home);

    assert!(ghostrace(&["validate", "--export", export.to_str().unwrap()]).status.success());
    assert_private_file(&export);
    if cfg!(feature = "parquet") {
        let verify = ["verify-archive", "--archive", archive.to_str().unwrap()];
        let verify = [&verify[..], &["--export", export.to_str().unwrap()]].concat();
        assert!(ghostrace(&verify).status.success());
        assert_private_file(&archive);
    }
    assert_no_export_temporary_files(&folder);
    // A second export to the same destination, or one inside the home, is refused.
    assert!(!ghostrace(&args).status.success());
    let inside = format!("{home}/export.jsonl");
    let inside_args = ["live", "export", "--home", &home, "--yes", "--output", inside.as_str()];
    assert!(!ghostrace(&inside_args).status.success());
}

#[cfg(feature = "frontmost")]
#[test]
fn live_apps_records_a_focus_switch_with_name_version_and_dwell() {
    let focus = std::env::var_os("GHOSTRACE_FRONTMOST_TEST").is_some_and(|value| value == "1");
    if !enabled() || !focus {
        eprintln!(
            "set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 and GHOSTRACE_FRONTMOST_TEST=1 to switch focus"
        );
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let home = directory.path().join("home").to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home]).status.success());
    let _forget = Forget(home.clone());

    let declined = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(["live", "apps", "--home", &home])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("apps");
    assert!(String::from_utf8_lossy(&declined.stdout).contains("Not started"));

    let front = |bundle: &str| {
        Command::new("/usr/bin/open").args(["-b", bundle]).status().expect("open");
    };
    let previous = front_bundle();
    let recorder = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(["live", "apps", "--home", &home, "--yes", "--seconds", "5"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("apps starts");
    std::thread::sleep(std::time::Duration::from_millis(1500));
    front("com.apple.finder");
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if !previous.is_empty() && previous != "com.apple.finder" {
        front(&previous);
    }
    let output = recorder.wait_with_output().expect("apps ends");
    assert!(output.status.success());
    let summary = String::from_utf8_lossy(&output.stdout);
    assert!(summary.contains("activation(s) recorded"), "{summary}");

    let journal = ghostrace(&["live", "timeline", "--home", &home, "--limit", "100", "--json"]);
    let entries: serde_json::Value = serde_json::from_slice(&journal.stdout).expect("timeline");
    let kinds = entries
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| {
            (
                entry["kind"].as_str().unwrap().to_owned(),
                entry["statement"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        kinds.iter().any(
            |(kind, text)| kind == "frontmost_app_changed" && text.contains("com.apple.finder")
        ),
        "{kinds:?}"
    );
    assert!(kinds.iter().any(|(kind, _)| kind == "collector_started"));
    assert!(kinds.iter().any(|(kind, _)| kind == "collector_stopped"));
    let text = String::from_utf8_lossy(&journal.stdout);
    assert!(!text.contains("/Applications") && !text.contains("/System"));

    // The payload keeps the name, version, and dwell, and nothing else of the app.
    let export = directory.path().join("apps.jsonl");
    let args = ["live", "export", "--home", &home, "--yes", "--output", export.to_str().unwrap()];
    assert!(ghostrace(&args).status.success());
    let exported = std::fs::read_to_string(&export).expect("export");
    assert!(exported.contains("\"app_name\":\"Finder\""), "{exported}");
    assert!(exported.contains("\"dwell_ms\":"));
    assert!(!exported.contains("/Applications") && !exported.contains("/System"));
}

#[cfg(feature = "frontmost")]
#[test]
fn live_apps_rapid_switching_termination_and_latency_on_device() {
    let focus = std::env::var_os("GHOSTRACE_FRONTMOST_TEST").is_some_and(|value| value == "1");
    if !enabled() || !focus {
        eprintln!(
            "set GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 and GHOSTRACE_FRONTMOST_TEST=1 to switch focus"
        );
        return;
    }
    let directory = tempfile::tempdir().expect("tempdir");
    let home = directory.path().join("home").to_str().expect("utf8").to_owned();
    struct Forget(String);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = ghostrace(&["live", "forget", "--home", &self.0, "--yes"]);
            let _ = Command::new("/usr/bin/pkill").args(["-x", "Calculator"]).status();
        }
    }
    assert!(ghostrace(&["live", "init", "--home", &home]).status.success());
    let _forget = Forget(home.clone());
    let previous = front_bundle();
    assert!(!previous.is_empty() && previous != "com.apple.finder", "start from another app");

    let recorder = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .args(["live", "apps", "--home", &home, "--yes", "--seconds", "14"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("apps starts");
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let mut switches = Vec::new();
    for index in 0..8 {
        let target = if index % 2 == 0 { "com.apple.finder" } else { previous.as_str() };
        let sent = chrono::Utc::now();
        Command::new("/usr/bin/open").args(["-b", target]).status().expect("open");
        switches.push((sent, target.to_ascii_lowercase()));
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
    // Termination: Calculator comes to the front, then quits.
    let sent = chrono::Utc::now();
    Command::new("/usr/bin/open").args(["-b", "com.apple.calculator"]).status().expect("open");
    switches.push((sent, "com.apple.calculator".to_owned()));
    std::thread::sleep(std::time::Duration::from_millis(1500));
    Command::new("/usr/bin/pkill").args(["-x", "Calculator"]).status().expect("pkill");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    Command::new("/usr/bin/open").args(["-b", &previous]).status().expect("open");
    assert!(recorder.wait_with_output().expect("apps ends").status.success());

    let export = directory.path().join("apps.jsonl");
    let args = ["live", "export", "--home", &home, "--yes", "--output", export.to_str().unwrap()];
    assert!(ghostrace(&args).status.success());
    let text = std::fs::read_to_string(&export).expect("export");
    assert!(
        !text.contains("/Applications") && !text.contains("/System") && !text.contains("/Users")
    );
    let events = text
        .lines()
        .skip(1)
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("record")["event"].clone()
        })
        .filter(|event| event["kind"] == "frontmost_app_changed")
        .collect::<Vec<_>>();
    let time = |event: &serde_json::Value| {
        event["observed_at"].as_str().unwrap().parse::<chrono::DateTime<chrono::Utc>>().unwrap()
    };
    assert!(events.windows(2).all(|pair| time(&pair[0]) <= time(&pair[1])), "records are ordered");
    let activations = events
        .iter()
        .filter(|event| event["payload"]["data"]["change"] == "activated")
        .map(|event| (time(event), event["payload"]["data"]["app_id"].as_str().unwrap().to_owned()))
        .collect::<Vec<_>>();

    // Every switch is attributed, in order, and its latency is measured.
    let mut latencies = Vec::new();
    let mut cursor = 0;
    for (sent, target) in &switches {
        let found = activations[cursor..]
            .iter()
            .position(|(at, app)| at >= sent && app == target)
            .unwrap_or_else(|| {
                panic!("switch to {target} at {sent} not attributed: {activations:?}")
            });
        cursor += found + 1;
        latencies.push((activations[cursor - 1].0 - *sent).num_milliseconds());
    }
    let mut sorted = latencies.clone();
    sorted.sort_unstable();
    println!(
        "attribution latency over {} switches: median {} ms, max {} ms (poll interval 250 ms)",
        sorted.len(),
        sorted[sorted.len() / 2],
        sorted[sorted.len() - 1]
    );
    assert!(sorted[sorted.len() - 1] < 2_000, "latency {latencies:?}");

    // Calculator's session is closed with its dwell once it quits.
    let closed = events.iter().any(|event| {
        let data = &event["payload"]["data"];
        data["app_id"] == "com.apple.calculator"
            && data["change"] == "deactivated"
            && data["dwell_ms"].as_u64().is_some_and(|dwell| dwell > 500)
    });
    assert!(closed, "the terminated application's session was closed");
}
