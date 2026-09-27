//! Repository-local Git hook lifecycle against real, throwaway repositories.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ghostrace::{
    GitHookAction, GitHookError, GitHookHealth, GitHookManager, GIT_HOOK_RECORD_NAME,
    MANAGED_GIT_HOOKS,
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
    repo: PathBuf,
    delegate: PathBuf,
    log: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("tempdir");
        let base = fs::canonicalize(directory.path()).expect("canonical");
        let repo = base.join("repo");
        fs::create_dir(&repo).expect("repo");
        ok(&repo, &["init", "-q", "-b", "main"]);
        ok(&repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let log = base.join("delegate.log");
        let delegate = base.join("delegate.sh");
        fs::write(&delegate, format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()))
            .expect("delegate");
        fs::set_permissions(&delegate, fs::Permissions::from_mode(0o755)).expect("chmod");
        Self { _directory: directory, repo, delegate, log }
    }

    fn manager(&self) -> GitHookManager {
        GitHookManager::new(GIT, &self.delegate).expect("manager")
    }

    fn hooks(&self) -> PathBuf {
        self.repo.join(".git/hooks")
    }

    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn actions(changes: &[ghostrace::GitHookChange]) -> Vec<GitHookAction> {
    changes.iter().map(|change| change.action).collect()
}

#[test]
fn plan_install_verify_and_uninstall_are_exact_and_idempotent() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    let manager = fixture.manager();

    let plan = manager.plan_install(&fixture.repo).expect("plan");
    assert_eq!(actions(&plan), vec![GitHookAction::Create; 4]);
    assert!(plan.iter().all(|change| change.path.starts_with(fixture.hooks())));
    assert!(!fixture.hooks().join("post-commit").exists(), "planning must not write");

    assert_eq!(
        actions(&manager.install(&fixture.repo).expect("install")),
        vec![GitHookAction::Create; 4]
    );
    assert_eq!(
        actions(&manager.install(&fixture.repo).expect("reinstall")),
        vec![GitHookAction::Unchanged; 4]
    );
    for hook in MANAGED_GIT_HOOKS {
        let metadata = fs::metadata(fixture.hooks().join(hook)).expect("shim");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
    }
    assert!(manager
        .verify(&fixture.repo)
        .expect("verify")
        .iter()
        .all(|item| item.health == GitHookHealth::Intact && item.enabled));

    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "two"]);
    assert_eq!(fixture.calls(), "git-hook post-commit\n");

    let removed = manager.uninstall(&fixture.repo).expect("uninstall");
    assert_eq!(actions(&removed), vec![GitHookAction::Remove; 4]);
    for hook in MANAGED_GIT_HOOKS {
        assert!(!fixture.hooks().join(hook).exists());
    }
    assert!(!fixture.hooks().join(GIT_HOOK_RECORD_NAME).exists());
    assert!(manager.uninstall(&fixture.repo).expect("second uninstall").is_empty());
}

#[test]
fn existing_user_hooks_are_preserved_by_refusing_before_any_write() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    let user_hook = fixture.hooks().join("post-merge");
    fs::write(&user_hook, "#!/bin/sh\necho mine\n").expect("user hook");
    fs::set_permissions(&user_hook, fs::Permissions::from_mode(0o700)).expect("chmod");

    let manager = fixture.manager();
    assert_eq!(
        manager.plan_install(&fixture.repo).unwrap_err(),
        GitHookError::ForeignHook("post-merge")
    );
    assert_eq!(
        manager.install(&fixture.repo).unwrap_err(),
        GitHookError::ForeignHook("post-merge")
    );
    assert_eq!(fs::read_to_string(&user_hook).expect("read"), "#!/bin/sh\necho mine\n");
    assert_eq!(fs::metadata(&user_hook).expect("meta").permissions().mode() & 0o777, 0o700);
    for hook in ["post-checkout", "post-commit", "post-rewrite"] {
        assert!(!fixture.hooks().join(hook).exists(), "{hook} written despite refusal");
    }
}

#[test]
fn a_configured_hooks_path_or_symlinked_hooks_are_refused() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    ok(&fixture.repo, &["config", "core.hooksPath", ".husky"]);
    assert_eq!(
        fixture.manager().install(&fixture.repo).unwrap_err(),
        GitHookError::HooksPathManaged
    );
    ok(&fixture.repo, &["config", "--unset", "core.hooksPath"]);

    let elsewhere = fixture.repo.parent().expect("parent").join("elsewhere");
    fs::create_dir(&elsewhere).expect("elsewhere");
    fs::rename(fixture.hooks(), fixture.repo.join(".git/hooks-original")).expect("move hooks");
    symlink(&elsewhere, fixture.hooks()).expect("symlink");
    assert_eq!(
        fixture.manager().install(&fixture.repo).unwrap_err(),
        GitHookError::UnsafeHooksDirectory
    );
    assert_eq!(fs::read_dir(&elsewhere).expect("read").count(), 0);
    fs::remove_file(fixture.hooks()).expect("unlink");
    fs::rename(fixture.repo.join(".git/hooks-original"), fixture.hooks()).expect("restore");

    let target = elsewhere.join("target");
    fs::write(&target, "#!/bin/sh\n").expect("target");
    symlink(&target, fixture.hooks().join("post-commit")).expect("hook symlink");
    assert_eq!(
        fixture.manager().install(&fixture.repo).unwrap_err(),
        GitHookError::UnsafeHook("post-commit")
    );
    assert_eq!(fs::read_to_string(&target).expect("read"), "#!/bin/sh\n");
}

#[test]
fn drifted_shims_are_reported_and_never_removed() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    let manager = fixture.manager();
    manager.install(&fixture.repo).expect("install");
    let edited = fixture.hooks().join("post-commit");
    let mut content = fs::read_to_string(&edited).expect("read");
    content.push_str("echo user-edit\n");
    fs::write(&edited, &content).expect("edit");

    let health = manager.verify(&fixture.repo).expect("verify");
    assert_eq!(
        health.iter().find(|item| item.hook == "post-commit").expect("entry").health,
        GitHookHealth::Drifted
    );
    assert_eq!(
        manager.uninstall(&fixture.repo).unwrap_err(),
        GitHookError::Drift(vec!["post-commit"])
    );
    assert_eq!(
        manager.disable(&fixture.repo).unwrap_err(),
        GitHookError::Drift(vec!["post-commit"])
    );
    assert_eq!(fs::read_to_string(&edited).expect("read"), content);
    assert!(fixture.hooks().join("post-merge").exists(), "refusal must not partially uninstall");
    assert!(matches!(manager.install(&fixture.repo).unwrap_err(), GitHookError::Drift(_)));
}

#[test]
fn disable_and_enable_are_reversible_and_stop_delegation() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    let manager = fixture.manager();
    manager.install(&fixture.repo).expect("install");

    assert_eq!(
        actions(&manager.disable(&fixture.repo).expect("disable")),
        vec![GitHookAction::Disable; 4]
    );
    assert_eq!(
        actions(&manager.disable(&fixture.repo).expect("disable again")),
        vec![GitHookAction::Unchanged; 4]
    );
    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "while disabled"]);
    assert_eq!(fixture.calls(), "", "a disabled shim must not delegate");
    assert!(manager
        .verify(&fixture.repo)
        .expect("verify")
        .iter()
        .all(|item| item.health == GitHookHealth::Intact && !item.enabled));

    assert_eq!(
        actions(&manager.enable(&fixture.repo).expect("enable")),
        vec![GitHookAction::Enable; 4]
    );
    ok(&fixture.repo, &["commit", "-q", "--allow-empty", "-m", "enabled"]);
    assert_eq!(fixture.calls(), "git-hook post-commit\n");
}

#[test]
fn upgrade_rewrites_only_intact_shims_and_covers_linked_worktrees() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fixture.manager().install(&fixture.repo).expect("install");

    let moved = fixture.delegate.with_file_name("delegate-v2.sh");
    fs::copy(&fixture.delegate, &moved).expect("copy delegate");
    let upgraded = GitHookManager::new(GIT, &moved).expect("manager");
    assert_eq!(
        actions(&upgraded.plan_install(&fixture.repo).expect("plan")),
        vec![GitHookAction::Replace; 4]
    );
    upgraded.install(&fixture.repo).expect("upgrade");
    assert!(fs::read_to_string(fixture.hooks().join("post-commit"))
        .expect("read")
        .contains("delegate-v2.sh"));
    assert!(upgraded
        .verify(&fixture.repo)
        .expect("verify")
        .iter()
        .all(|item| item.health == GitHookHealth::Intact));

    let worktree = fixture.repo.parent().expect("parent").join("linked");
    ok(&fixture.repo, &["worktree", "add", "-q", "-b", "side", worktree.to_str().expect("utf8")]);
    let from_worktree = upgraded.plan_install(&worktree).expect("plan from worktree");
    assert!(from_worktree.iter().all(|change| change.path.starts_with(fixture.hooks())));
    assert_eq!(actions(&from_worktree), vec![GitHookAction::Unchanged; 4]);
    ok(&worktree, &["commit", "-q", "--allow-empty", "-m", "in worktree"]);
    // `worktree add` checks out the new worktree, so post-checkout fires too.
    assert_eq!(fixture.calls(), "git-hook post-checkout\ngit-hook post-commit\n");
}

#[test]
fn concurrent_operations_and_invalid_delegates_are_refused() {
    if !git_available() {
        return;
    }
    let fixture = Fixture::new();
    fs::write(fixture.hooks().join("ghostrace-hooks.lock"), "").expect("lock");
    assert_eq!(fixture.manager().install(&fixture.repo).unwrap_err(), GitHookError::Locked);
    fs::remove_file(fixture.hooks().join("ghostrace-hooks.lock")).expect("unlock");

    let not_executable = fixture.delegate.with_file_name("plain.txt");
    fs::write(&not_executable, "x").expect("plain");
    let quoted = fixture.delegate.with_file_name("it's.sh");
    fs::copy(&fixture.delegate, &quoted).expect("quoted");
    for delegate in [PathBuf::from("relative/ghostrace"), not_executable, quoted] {
        assert_eq!(GitHookManager::new(GIT, &delegate).err(), Some(GitHookError::InvalidDelegate));
    }
}
