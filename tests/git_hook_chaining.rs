//! Chaining existing user hooks behind GHOSTRACE shims, with confirmation,
//! preservation, and exact restoration.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ghostrace::{
    GitHookAction, GitHookError, GitHookHealth, GitHookManager, GIT_HOOK_PRESERVED_SUFFIX,
};

const GIT: &str = "/usr/bin/git";

fn git_available() -> bool {
    Command::new(GIT).arg("--version").stdout(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(GIT)
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .stdin(Stdio::null())
        .output()
        .expect("git runs")
}

fn ok(dir: &Path, args: &[&str]) {
    let output = git(dir, args);
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

struct Fixture {
    _directory: tempfile::TempDir,
    base: PathBuf,
    repo: PathBuf,
    manager: GitHookManager,
}

const USER_HOOK: &str =
    "#!/bin/sh\necho \"user $*\" >> \"$(git rev-parse --git-dir)/../user.log\"\n";

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("tempdir");
        let base = fs::canonicalize(directory.path()).expect("canonical");
        let repo = base.join("repo");
        fs::create_dir(&repo).expect("repo");
        ok(&repo, &["init", "-q", "-b", "main"]);
        ok(&repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let delegate = base.join("delegate.sh");
        fs::write(
            &delegate,
            format!("#!/bin/sh\necho \"$@\" >> '{}'\n", base.join("delegate.log").display()),
        )
        .expect("delegate");
        fs::set_permissions(&delegate, fs::Permissions::from_mode(0o755)).expect("chmod");
        let manager = GitHookManager::new(GIT, &delegate).expect("manager");
        Self { _directory: directory, base, repo, manager }
    }

    fn hooks(&self) -> PathBuf {
        self.repo.join(".git/hooks")
    }

    fn user_hook(&self, name: &str) -> PathBuf {
        let path = self.hooks().join(name);
        fs::write(&path, USER_HOOK).expect("user hook");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o750)).expect("chmod");
        path
    }

    fn log(&self, name: &str) -> String {
        fs::read_to_string(self.base.join(name)).unwrap_or_default()
    }

    fn user_log(&self) -> String {
        fs::read_to_string(self.repo.join("user.log")).unwrap_or_default()
    }
}

#[test]
fn chained_install_requires_the_confirmed_plan_digest() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fixture.user_hook("post-commit");
    let plan = fixture.manager.plan_chained_install(&fixture.repo).expect("plan");
    let chain = plan.changes.iter().find(|change| change.hook == "post-commit").expect("change");
    assert_eq!(chain.action, GitHookAction::Chain);
    assert_eq!(
        fixture.manager.install_chained(&fixture.repo, &"0".repeat(64)).unwrap_err(),
        GitHookError::ConfirmationMismatch
    );
    assert_eq!(fs::read_to_string(fixture.hooks().join("post-commit")).expect("read"), USER_HOOK);

    // A change after confirmation invalidates the digest.
    fs::write(fixture.hooks().join("post-commit"), format!("{USER_HOOK}# edited\n")).expect("edit");
    assert_eq!(
        fixture.manager.install_chained(&fixture.repo, &plan.digest).unwrap_err(),
        GitHookError::ConfirmationMismatch
    );
}

#[test]
fn a_chained_user_hook_still_runs_first_with_its_arguments() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fixture.user_hook("post-checkout");
    fixture.user_hook("post-commit");
    let plan = fixture.manager.plan_chained_install(&fixture.repo).expect("plan");
    fixture.manager.install_chained(&fixture.repo, &plan.digest).expect("install");

    let preserved = fixture.hooks().join(format!("post-commit{GIT_HOOK_PRESERVED_SUFFIX}"));
    assert_eq!(fs::read_to_string(&preserved).expect("preserved"), USER_HOOK);
    assert_eq!(fs::metadata(&preserved).expect("meta").permissions().mode() & 0o777, 0o750);

    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "two"]);
    assert_eq!(fixture.user_log(), "user \n");
    assert_eq!(fixture.log("delegate.log"), "git-hook post-commit\n");

    ok(&fixture.repo, &["checkout", "-q", "-b", "side"]);
    let user = fixture.user_log();
    let checkout_line = user.lines().last().expect("user checkout call");
    assert!(checkout_line.starts_with("user ") && checkout_line.ends_with(" 1"), "{checkout_line}");

    let verification = fixture.manager.verify(&fixture.repo).expect("verify");
    let commit = verification.iter().find(|item| item.hook == "post-commit").expect("entry");
    assert_eq!(commit.health, GitHookHealth::Intact);
    assert_eq!(commit.preserved, Some(GitHookHealth::Intact));
}

#[test]
fn disabling_keeps_the_user_hook_running_and_enable_restores_delegation() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fixture.user_hook("post-commit");
    let plan = fixture.manager.plan_chained_install(&fixture.repo).expect("plan");
    fixture.manager.install_chained(&fixture.repo, &plan.digest).expect("install");

    fixture.manager.disable(&fixture.repo).expect("disable");
    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "disabled"]);
    assert_eq!(fixture.user_log(), "user \n", "the user hook must keep running");
    assert_eq!(fixture.log("delegate.log"), "", "GHOSTRACE must not delegate while disabled");
    assert!(fixture
        .manager
        .verify(&fixture.repo)
        .expect("verify")
        .iter()
        .all(|item| item.health == GitHookHealth::Intact));

    fixture.manager.enable(&fixture.repo).expect("enable");
    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "enabled"]);
    assert_eq!(fixture.log("delegate.log"), "git-hook post-commit\n");
}

#[test]
fn uninstall_restores_the_original_hook_exactly_and_refuses_if_it_changed() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    let user_hook = fixture.user_hook("post-merge");
    let plan = fixture.manager.plan_chained_install(&fixture.repo).expect("plan");
    fixture.manager.install_chained(&fixture.repo, &plan.digest).expect("install");

    let preserved = fixture.hooks().join(format!("post-merge{GIT_HOOK_PRESERVED_SUFFIX}"));
    fs::write(&preserved, "#!/bin/sh\necho changed\n").expect("edit preserved");
    assert_eq!(
        fixture.manager.uninstall(&fixture.repo).unwrap_err(),
        GitHookError::Drift(vec!["post-merge"])
    );
    fs::write(&preserved, USER_HOOK).expect("restore preserved");

    let changes = fixture.manager.uninstall(&fixture.repo).expect("uninstall");
    let merge = changes.iter().find(|change| change.hook == "post-merge").expect("entry");
    assert_eq!(merge.action, GitHookAction::Restore);
    assert_eq!(fs::read_to_string(&user_hook).expect("restored"), USER_HOOK);
    assert_eq!(fs::metadata(&user_hook).expect("meta").permissions().mode() & 0o777, 0o750);
    assert!(!preserved.exists());
    for hook in ["post-checkout", "post-commit", "post-rewrite"] {
        assert!(!fixture.hooks().join(hook).exists(), "{hook} left behind");
    }
}

#[test]
fn an_existing_preserved_copy_is_never_overwritten_and_global_config_is_untouched() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fixture.user_hook("post-commit");
    let stale = fixture.hooks().join(format!("post-commit{GIT_HOOK_PRESERVED_SUFFIX}"));
    fs::write(&stale, "#!/bin/sh\necho older\n").expect("stale copy");
    assert_eq!(
        fixture.manager.plan_chained_install(&fixture.repo).unwrap_err(),
        GitHookError::PreservedHookExists("post-commit")
    );
    assert_eq!(fs::read_to_string(&stale).expect("stale"), "#!/bin/sh\necho older\n");

    fs::remove_file(&stale).expect("remove stale");
    let global = fixture.base.join("global.gitconfig");
    fs::write(&global, "[user]\n\tname = untouched\n").expect("global config");
    let before = fs::read(&global).expect("read");
    let plan = fixture.manager.plan_chained_install(&fixture.repo).expect("plan");
    fixture.manager.install_chained(&fixture.repo, &plan.digest).expect("install");
    fixture.manager.uninstall(&fixture.repo).expect("uninstall");
    assert_eq!(fs::read(&global).expect("read"), before);
    let config = fs::read_to_string(fixture.repo.join(".git/config")).expect("repo config");
    assert!(!config.contains("hooksPath"));
}
