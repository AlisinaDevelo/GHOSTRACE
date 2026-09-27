//! The explicit Git snapshot adapter against real, throwaway repositories.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ghostrace::{
    GitAdapterError, GitAlternateObjectDatabaseState, GitAncestryProbe, GitBranchClass,
    GitHistoryGapReason, GitHistoryTransition, GitObjectFormat, GitOperation, GitPartialCloneState,
    GitReplaceRefsState, GitRepositoryKind, GitShallowHistoryState, GitSnapshotAdapter,
    GitSnapshotMetadata, GitSubmoduleState, GitWorktreeState, PolicyDocument, PolicyProfile,
    RootId,
};

const GIT: &str = "/usr/bin/git";
const ROOT: &str = "root-main";

fn git_available() -> bool {
    Command::new(GIT).arg("--version").stdout(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Setup commands run with a private HOME and fixed identity.
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
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .stdin(Stdio::null())
        .output()
        .expect("git runs")
}

fn ok(dir: &Path, args: &[&str]) {
    let output = git(dir, args);
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

fn init(dir: &Path) {
    ok(dir, &["init", "-q", "-b", "main"]);
}

fn commit_file(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write");
    ok(dir, &["add", "--", name]);
    ok(dir, &["commit", "-q", "-m", "commit"]);
}

fn policy(sources: &[ghostrace::EventSource]) -> PolicyProfile {
    PolicyProfile::from_document(
        &PolicyDocument::new("git-snapshot-v1", 1, sources.iter().copied(), [ROOT], false)
            .expect("policy"),
    )
    .expect("profile")
}

fn adapter() -> GitSnapshotAdapter {
    GitSnapshotAdapter::new(GIT, policy(&[ghostrace::EventSource::Git])).expect("adapter")
}

fn root() -> RootId {
    RootId::try_from(ROOT).expect("root id")
}

fn snapshot(dir: &Path) -> GitSnapshotMetadata {
    adapter().snapshot(dir, root()).expect("snapshot")
}

fn assert_no_text(snapshot: &GitSnapshotMetadata, forbidden: &[&str]) {
    let json = serde_json::to_string(snapshot).expect("JSON");
    for text in forbidden {
        assert!(!json.contains(text), "snapshot leaked {text:?}");
    }
    assert!(!json.contains('/'), "snapshot leaked a path separator");
}

fn repo() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = fs::canonicalize(directory.path()).expect("canonical").join("work");
    fs::create_dir(&path).expect("work dir");
    (directory, path)
}

#[test]
fn counts_status_classes_and_retains_no_names() {
    if !git_available() {
        return;
    }
    let (_directory, dir) = repo();
    init(&dir);
    commit_file(&dir, "tracked-secret-name.txt", "one");
    commit_file(&dir, "second.txt", "two");
    ok(&dir, &["switch", "-q", "-c", "customer-acme-launch"]);
    fs::write(dir.join("staged-file.txt"), "staged").expect("write");
    ok(&dir, &["add", "staged-file.txt"]);
    fs::write(dir.join("tracked-secret-name.txt"), "changed").expect("write");
    fs::write(dir.join("untracked-one.txt"), "u").expect("write");
    fs::write(dir.join("untracked-two.txt"), "u").expect("write");

    let snapshot = snapshot(&dir);
    assert_eq!(snapshot.repository_kind, GitRepositoryKind::Standard);
    assert_eq!(snapshot.object_format, GitObjectFormat::Sha1);
    assert_eq!(snapshot.branch_class, GitBranchClass::Local);
    assert_eq!(snapshot.worktree_state, GitWorktreeState::Modified);
    assert_eq!(snapshot.operation, GitOperation::Idle);
    assert_eq!(
        (snapshot.status.staged, snapshot.status.unstaged, snapshot.status.untracked),
        (1, 1, 2)
    );
    assert!(snapshot.head.is_some() && snapshot.tree.is_some());
    assert_eq!(snapshot.limitations.shallow_history, GitShallowHistoryState::Complete);
    assert_eq!(snapshot.limitations.replace_refs, GitReplaceRefsState::None);
    assert_eq!(snapshot.limitations.partial_clone, GitPartialCloneState::Full);
    assert_eq!(snapshot.limitations.submodules, GitSubmoduleState::None);
    snapshot.validate().expect("valid snapshot");
    assert_no_text(
        &snapshot,
        &["customer-acme-launch", "tracked-secret-name", "untracked-one", "staged-file", "main"],
    );
}

#[test]
fn hostile_branch_and_file_names_are_counted_but_never_parsed_as_text() {
    if !git_available() {
        return;
    }
    let (_directory, dir) = repo();
    init(&dir);
    commit_file(&dir, "base.txt", "base");
    ok(&dir, &["switch", "-q", "-c", "evil-$(touch_pwned)-\u{202e}txt.exe"]);
    for name in [
        "line\nbreak.txt",
        "tab\tname.txt",
        "semi;rm -rf.txt",
        "\u{fe0f}emoji-\u{1f600}.txt",
        "-dash.txt",
    ] {
        fs::write(dir.join(name), "x").expect("write hostile file");
    }
    fs::write(dir.join("renamed-from.txt"), "r").expect("write");
    ok(&dir, &["add", "renamed-from.txt"]);
    ok(&dir, &["commit", "-q", "-m", "add"]);
    ok(&dir, &["mv", "renamed-from.txt", "renamed\nto.txt"]);

    let snapshot = snapshot(&dir);
    assert_eq!(snapshot.branch_class, GitBranchClass::Local);
    assert_eq!(snapshot.status.untracked, 5);
    assert_eq!(snapshot.status.staged, 1, "a rename is one staged record");
    assert_no_text(&snapshot, &["evil", "touch_pwned", "line", "semi", "emoji", "renamed", "dash"]);
    assert!(!dir.join("touch_pwned").exists());
}

#[test]
fn repository_configuration_cannot_execute_programs_during_a_snapshot() {
    if !git_available() {
        return;
    }
    let (directory, dir) = repo();
    init(&dir);
    commit_file(&dir, "tracked.txt", "one");
    let markers = directory.path().join("markers");
    fs::create_dir(&markers).expect("markers");
    let script = |name: &str| {
        let path = directory.path().join(format!("{name}.sh"));
        fs::write(&path, format!("#!/bin/sh\ntouch '{}'\ncat\n", markers.join(name).display()))
            .expect("script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    };
    let fsmonitor = script("fsmonitor");
    let filter = script("filter");
    let hooks = dir.join(".git/hooks");
    fs::write(
        hooks.join("post-index-change"),
        format!("#!/bin/sh\ntouch '{}'\n", markers.join("hook").display()),
    )
    .expect("hook");
    fs::set_permissions(hooks.join("post-index-change"), fs::Permissions::from_mode(0o755))
        .expect("chmod");
    ok(&dir, &["config", "core.fsmonitor", fsmonitor.to_str().expect("utf8")]);
    ok(&dir, &["config", "filter.evil.clean", filter.to_str().expect("utf8")]);
    ok(&dir, &["config", "filter.evil.required", "true"]);
    ok(&dir, &["config", "core.pager", &format!("touch '{}'", markers.join("pager").display())]);
    fs::write(dir.join(".gitattributes"), "*.txt filter=evil\n").expect("attributes");
    fs::write(dir.join("tracked.txt"), "changed").expect("modify");

    let snapshot = snapshot(&dir);
    assert_eq!(snapshot.worktree_state, GitWorktreeState::Modified);
    let fired = fs::read_dir(&markers).expect("markers").count();
    assert_eq!(fired, 0, "a repository-configured program ran during the snapshot");

    // Control: the same repository does run these programs under plain Git,
    // so the absence above is the adapter's hardening, not an inert setup.
    let _ = git(&dir, &["status"]);
    assert!(fs::read_dir(&markers).expect("markers").count() > 0);
}

#[test]
fn detached_unborn_bare_and_conflicted_states_are_classified() {
    if !git_available() {
        return;
    }
    let (directory, dir) = repo();
    init(&dir);
    let unborn = snapshot(&dir);
    assert_eq!(unborn.branch_class, GitBranchClass::Unborn);
    assert!(unborn.head.is_none());

    commit_file(&dir, "file.txt", "base\n");
    ok(&dir, &["checkout", "-q", "--detach"]);
    assert_eq!(snapshot(&dir).branch_class, GitBranchClass::DetachedHead);
    ok(&dir, &["switch", "-q", "main"]);

    ok(&dir, &["switch", "-q", "-c", "other"]);
    commit_file(&dir, "file.txt", "other\n");
    ok(&dir, &["switch", "-q", "main"]);
    commit_file(&dir, "file.txt", "main\n");
    let _ = git(&dir, &["merge", "-q", "other"]);
    let conflicted = snapshot(&dir);
    assert_eq!(conflicted.worktree_state, GitWorktreeState::Conflicted);
    assert_eq!(conflicted.operation, GitOperation::Merge);
    assert_eq!(conflicted.status.conflicted, 1);
    ok(&dir, &["merge", "--abort"]);

    let bare = directory.path().join("bare.git");
    ok(
        directory.path(),
        &["clone", "-q", "--bare", dir.to_str().expect("utf8"), bare.to_str().expect("utf8")],
    );
    let bare_snapshot = snapshot(&bare);
    assert_eq!(bare_snapshot.repository_kind, GitRepositoryKind::Bare);
    assert_eq!(bare_snapshot.branch_class, GitBranchClass::NoWorktree);
    assert_eq!(bare_snapshot.worktree_state, GitWorktreeState::NotApplicable);
}

#[test]
fn source_limitations_are_observed_from_the_repository() {
    if !git_available() {
        return;
    }
    let (directory, dir) = repo();
    init(&dir);
    commit_file(&dir, "one.txt", "1");
    commit_file(&dir, "two.txt", "2");
    ok(&dir, &["config", "uploadpack.allowFilter", "true"]);
    let url = format!("file://{}", dir.display());

    let shallow = directory.path().join("shallow");
    ok(directory.path(), &["clone", "-q", "--depth", "1", &url, shallow.to_str().expect("utf8")]);
    assert_eq!(snapshot(&shallow).limitations.shallow_history, GitShallowHistoryState::Shallow);

    let partial = directory.path().join("partial");
    ok(
        directory.path(),
        &["clone", "-q", "--filter=blob:none", &url, partial.to_str().expect("utf8")],
    );
    assert_eq!(snapshot(&partial).limitations.partial_clone, GitPartialCloneState::Partial);

    let shared = directory.path().join("shared");
    ok(
        directory.path(),
        &["clone", "-q", "--shared", dir.to_str().expect("utf8"), shared.to_str().expect("utf8")],
    );
    assert_eq!(
        snapshot(&shared).limitations.alternate_object_database,
        GitAlternateObjectDatabaseState::Present
    );

    ok(&dir, &["replace", "-f", "HEAD", "HEAD~1"]);
    assert_eq!(snapshot(&dir).limitations.replace_refs, GitReplaceRefsState::Active);
    ok(&dir, &["replace", "-d", "HEAD"]);

    let superproject = directory.path().join("super");
    fs::create_dir(&superproject).expect("super");
    init(&superproject);
    commit_file(&superproject, "readme.txt", "r");
    ok(
        &superproject,
        &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "child-module"],
    );
    ok(&superproject, &["commit", "-q", "-m", "submodule"]);
    let parent = snapshot(&superproject);
    assert_eq!(parent.limitations.submodules, GitSubmoduleState::Present);
    let child = snapshot(&superproject.join("child-module"));
    assert_eq!(child.repository_kind, GitRepositoryKind::Submodule);
    assert_no_text(&parent, &["child-module"]);
}

#[test]
fn ancestry_probe_feeds_the_history_transition_contract() {
    if !git_available() {
        return;
    }
    let (_directory, dir) = repo();
    init(&dir);
    commit_file(&dir, "a.txt", "a");
    let first = snapshot(&dir);
    commit_file(&dir, "b.txt", "b");
    let second = snapshot(&dir);
    let adapter = adapter();
    let forward = adapter
        .probe_ancestry(
            &dir,
            first.head.as_ref().expect("head"),
            second.head.as_ref().expect("head"),
        )
        .expect("probe");
    assert_eq!(forward, GitAncestryProbe::PreviousIsAncestor);
    assert!(GitHistoryTransition::classify(&first, &second, forward)
        .expect("classify")
        .gap
        .is_none());

    ok(&dir, &["commit", "-q", "--amend", "-m", "amended"]);
    let amended = snapshot(&dir);
    let probe = adapter
        .probe_ancestry(
            &dir,
            second.head.as_ref().expect("head"),
            amended.head.as_ref().expect("head"),
        )
        .expect("probe");
    assert_eq!(probe, GitAncestryProbe::PreviousNotAncestor);
    let transition = GitHistoryTransition::classify(&second, &amended, probe).expect("classify");
    assert_eq!(transition.gap.map(|gap| gap.reason), Some(GitHistoryGapReason::HistoryRewritten));
}

#[test]
fn policy_missing_repositories_and_missing_git_fail_closed() {
    let (_directory, dir) = repo();
    assert!(matches!(
        GitSnapshotAdapter::new(GIT, policy(&[ghostrace::EventSource::Filesystem])),
        Err(GitAdapterError::PolicyDenied)
    ));
    assert_eq!(
        adapter().snapshot(&dir, RootId::try_from("root-other").expect("root")).unwrap_err(),
        GitAdapterError::PolicyDenied
    );
    assert_eq!(
        GitSnapshotAdapter::new("/nonexistent/git", policy(&[ghostrace::EventSource::Git]))
            .expect("adapter")
            .snapshot(&dir, root())
            .unwrap_err(),
        GitAdapterError::GitUnavailable
    );
    if git_available() {
        assert_eq!(adapter().snapshot(&dir, root()).unwrap_err(), GitAdapterError::NotARepository);
    }
}
