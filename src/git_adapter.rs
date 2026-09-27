//! Explicit, policy-gated Git snapshot adapter.
//!
//! [`GitSnapshotAdapter::snapshot`] runs a fixed set of read-only Git plumbing
//! commands for one repository the caller explicitly asked about, reduces their
//! output to the metadata-only [`GitSnapshotMetadata`] contract, and discards
//! every name: ref names, paths, filenames, remotes, and messages are parsed
//! only to classify or count and never leave this module.
//!
//! Repository configuration is attacker-controlled input. Every invocation
//! runs with a cleared environment, no system or global configuration, and
//! command-line overrides that disable hooks, fsmonitor, the untracked cache,
//! pagers, prompts, optional index locks, and lazy fetches from promisor
//! remotes, so taking a snapshot cannot execute repository-supplied programs or
//! touch the network. Output is size- and time-bounded.

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::{
    git_history::GitAncestryProbe,
    git_identity::{GitIdentity, GitRepositoryKind, GitSourceScope},
    git_snapshot::{
        GitAlternateObjectDatabaseState, GitBranchClass, GitObjectFormat, GitObjectIdRef,
        GitOperation, GitPartialCloneState, GitReplaceRefsState, GitShallowHistoryState,
        GitSnapshotMetadata, GitSourceLimitations, GitStatusCounts, GitSubmoduleState,
        GitWorktreeState, MAX_GIT_STATUS_COUNT,
    },
    model::{EventSource, RootId},
    policy::PolicyProfile,
};

/// Maximum bytes read from any one Git command.
pub const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum wall time for any one Git command.
pub const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

/// Configuration overrides applied to every invocation. Later `-c` values win
/// over repository configuration.
const HARDENING: &[&str] = &[
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.untrackedCache=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.pager=cat",
    "-c",
    "core.askPass=",
    "-c",
    "credential.helper=",
    "-c",
    "diff.external=",
    "-c",
    "status.showUntrackedFiles=normal",
    "-c",
    "status.submoduleSummary=false",
    "-c",
    "protocol.allow=never",
    "--no-optional-locks",
    "--no-pager",
];

/// Errors never carry Git output, paths, or names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum GitAdapterError {
    #[error("policy does not allow a Git snapshot of this root")]
    PolicyDenied,
    #[error("the Git executable could not be started")]
    GitUnavailable,
    #[error("the path is not a Git repository Git will open")]
    NotARepository,
    #[error("Git output was not in the expected form")]
    UnexpectedOutput,
    #[error("Git output exceeded its bound")]
    OutputTooLarge,
    #[error("a Git command exceeded its time bound")]
    Timeout,
    #[error("the repository identity could not be established")]
    IdentityUnavailable,
    #[error("the snapshot violates the metadata contract")]
    InvalidSnapshot,
}

pub(crate) struct Output {
    pub(crate) status: Option<i32>,
    pub(crate) stdout: Vec<u8>,
}

/// Runs Git on behalf of an explicit snapshot request.
pub struct GitSnapshotAdapter {
    runner: GitRunner,
    policy: PolicyProfile,
}

impl GitSnapshotAdapter {
    /// `git` is the executable to run, normally `/usr/bin/git`. The policy
    /// must enable the Git source; each snapshot must name a selected root.
    pub fn new(git: impl Into<PathBuf>, policy: PolicyProfile) -> Result<Self, GitAdapterError> {
        if !policy.is_source_enabled(EventSource::Git) {
            return Err(GitAdapterError::PolicyDenied);
        }
        Ok(Self { runner: GitRunner::new(git), policy })
    }

    /// Take a metadata-only snapshot of the repository containing `path`.
    pub fn snapshot(
        &self,
        path: &Path,
        selected_root_id: RootId,
    ) -> Result<GitSnapshotMetadata, GitAdapterError> {
        if !self
            .policy
            .decide(EventSource::Git, Some(selected_root_id.as_str()), false)
            .is_allowed()
        {
            return Err(GitAdapterError::PolicyDenied);
        }
        let bare = self.runner.flag(path, "--is-bare-repository")?;
        let shallow = self.runner.flag(path, "--is-shallow-repository")?;
        let object_format =
            match self.runner.single_line(path, &["rev-parse", "--show-object-format"])? {
                line if line == b"sha1" => GitObjectFormat::Sha1,
                line if line == b"sha256" => GitObjectFormat::Sha256,
                _ => return Err(GitAdapterError::UnexpectedOutput),
            };
        let common_dir = self.runner.path_line(path, "--git-common-dir")?;
        let git_dir = self.runner.path_line(path, "--git-dir")?;
        let (worktree, superproject) = if bare {
            (None, false)
        } else {
            let toplevel = self.runner.path_line(path, "--show-toplevel")?;
            let superproject = !self
                .runner
                .run(path, &["rev-parse", "--show-superproject-working-tree"])?
                .stdout
                .is_empty();
            (Some(toplevel), superproject)
        };
        let kind = if bare {
            GitRepositoryKind::Bare
        } else if superproject {
            GitRepositoryKind::Submodule
        } else {
            GitRepositoryKind::Standard
        };
        let scope =
            if superproject { GitSourceScope::Submodule } else { GitSourceScope::SelectedRoot };
        let identity = GitIdentity::from_paths(
            &common_dir,
            worktree.as_deref(),
            selected_root_id,
            scope,
            kind,
        )
        .map_err(|_| GitAdapterError::IdentityUnavailable)?;

        let head = self.object_id(path, "HEAD^{commit}", object_format)?;
        let tree = match head {
            Some(_) => self.object_id(path, "HEAD^{tree}", object_format)?,
            None => None,
        };
        let branch_class = if bare {
            GitBranchClass::NoWorktree
        } else {
            self.branch_class(path, head.is_some())?
        };
        let (status, worktree_state) = if bare {
            (
                GitStatusCounts::new(0, 0, 0, 0).expect("zero counts"),
                GitWorktreeState::NotApplicable,
            )
        } else {
            let counts = self.status_counts(path)?;
            (counts, worktree_state(&counts))
        };
        let limitations = GitSourceLimitations {
            partial_clone: self.partial_clone(path)?,
            replace_refs: self.replace_refs(path)?,
            shallow_history: if shallow {
                GitShallowHistoryState::Shallow
            } else {
                GitShallowHistoryState::Complete
            },
            submodules: match (kind, worktree.as_deref()) {
                (GitRepositoryKind::Submodule, _) => GitSubmoduleState::Present,
                (_, Some(top)) if top.join(".gitmodules").is_file() => GitSubmoduleState::Present,
                (_, Some(_)) => GitSubmoduleState::None,
                (_, None) => GitSubmoduleState::Unknown,
            },
            alternate_object_database: alternates(&common_dir),
        };
        GitSnapshotMetadata::from_identity(
            &identity,
            object_format,
            head,
            tree,
            None,
            worktree_state,
            branch_class,
            operation(&git_dir),
            status,
            limitations,
        )
        .map_err(|_| GitAdapterError::InvalidSnapshot)
    }

    /// Ask whether `previous` is an ancestor of `current` in the repository
    /// containing `path`. Only the exit status is used.
    pub fn probe_ancestry(
        &self,
        path: &Path,
        previous: &GitObjectIdRef,
        current: &GitObjectIdRef,
    ) -> Result<GitAncestryProbe, GitAdapterError> {
        let previous = previous.hex().to_owned();
        let current = current.hex().to_owned();
        let exists =
            self.runner.run(path, &["cat-file", "-e", &format!("{previous}^{{commit}}")])?;
        if exists.status != Some(0) {
            return Ok(GitAncestryProbe::PreviousObjectMissing);
        }
        let shallow = self.runner.flag(path, "--is-shallow-repository")?;
        let answer =
            self.runner.run(path, &["merge-base", "--is-ancestor", &previous, &current])?;
        Ok(match answer.status {
            Some(0) => GitAncestryProbe::PreviousIsAncestor,
            Some(1) if shallow => GitAncestryProbe::ShallowBoundaryReached,
            Some(1) => GitAncestryProbe::PreviousNotAncestor,
            _ => GitAncestryProbe::NotProbed,
        })
    }

    fn object_id(
        &self,
        path: &Path,
        revision: &str,
        format: GitObjectFormat,
    ) -> Result<Option<GitObjectIdRef>, GitAdapterError> {
        let output = self.runner.run(path, &["rev-parse", "--verify", "--quiet", revision])?;
        match output.status {
            Some(0) => {
                let text = std::str::from_utf8(&output.stdout)
                    .map_err(|_| GitAdapterError::UnexpectedOutput)?;
                GitObjectIdRef::new(format, text.trim_end_matches('\n'))
                    .map(Some)
                    .map_err(|_| GitAdapterError::UnexpectedOutput)
            }
            Some(1) => Ok(None),
            Some(128) => Err(GitAdapterError::NotARepository),
            _ => Err(GitAdapterError::UnexpectedOutput),
        }
    }

    fn branch_class(&self, path: &Path, has_head: bool) -> Result<GitBranchClass, GitAdapterError> {
        let output = self.runner.run(path, &["symbolic-ref", "--quiet", "HEAD"])?;
        // Only the namespace prefix is inspected; the ref name is discarded.
        let class = match output.status {
            Some(0) if !has_head => GitBranchClass::Unborn,
            Some(0) if output.stdout.starts_with(b"refs/heads/") => GitBranchClass::Local,
            Some(0) if output.stdout.starts_with(b"refs/remotes/") => {
                GitBranchClass::RemoteTracking
            }
            Some(0) if output.stdout.starts_with(b"refs/tags/") => GitBranchClass::Tag,
            Some(0) => GitBranchClass::Unknown,
            Some(1) if has_head => GitBranchClass::DetachedHead,
            Some(1) => GitBranchClass::Unknown,
            _ => return Err(GitAdapterError::UnexpectedOutput),
        };
        Ok(class)
    }

    /// `git status` may run clean filters to compare racily-modified files.
    /// Filter drivers are repository configuration, so each configured driver
    /// is overridden with an empty command, which Git treats as no filter.
    fn filter_overrides(&self, path: &Path) -> Result<Vec<String>, GitAdapterError> {
        let output = self.runner.run(
            path,
            &[
                "config",
                "--null",
                "--name-only",
                "--get-regexp",
                r"^filter\..+\.(clean|smudge|process)$",
            ],
        )?;
        if output.status == Some(1) {
            return Ok(Vec::new());
        }
        if output.status != Some(0) {
            return Err(GitAdapterError::UnexpectedOutput);
        }
        let mut overrides = Vec::new();
        for key in output.stdout.split(|byte| *byte == 0).filter(|key| !key.is_empty()) {
            let key = std::str::from_utf8(key).map_err(|_| GitAdapterError::UnexpectedOutput)?;
            // `-c` splits at the first '='; a key containing one cannot be
            // overridden reliably, so the snapshot is refused.
            if key.contains('=') {
                return Err(GitAdapterError::UnexpectedOutput);
            }
            overrides.push("-c".to_owned());
            overrides.push(format!("{key}="));
        }
        Ok(overrides)
    }

    fn status_counts(&self, path: &Path) -> Result<GitStatusCounts, GitAdapterError> {
        let mut args = self.filter_overrides(path)?;
        args.extend(
            [
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=normal",
                "--ignore-submodules=none",
            ]
            .map(str::to_owned),
        );
        let args = args.iter().map(String::as_str).collect::<Vec<_>>();
        let stdout = self.runner.checked(path, &args)?;
        let (mut staged, mut unstaged, mut untracked, mut conflicted) = (0u64, 0u64, 0u64, 0u64);
        let mut records = stdout.split(|byte| *byte == 0).filter(|record| !record.is_empty());
        while let Some(record) = records.next() {
            match record.first() {
                Some(b'1') | Some(b'2') => {
                    let xy = record.get(2..4).ok_or(GitAdapterError::UnexpectedOutput)?;
                    if xy[0] != b'.' {
                        staged += 1;
                    }
                    if xy[1] != b'.' {
                        unstaged += 1;
                    }
                    if record[0] == b'2' {
                        // A rename or copy is followed by its original path.
                        records.next().ok_or(GitAdapterError::UnexpectedOutput)?;
                    }
                }
                Some(b'u') => conflicted += 1,
                Some(b'?') => untracked += 1,
                Some(b'!') | Some(b'#') => {}
                _ => return Err(GitAdapterError::UnexpectedOutput),
            }
        }
        let bound = |value: u64| {
            u32::try_from(value)
                .ok()
                .filter(|value| *value <= MAX_GIT_STATUS_COUNT)
                .ok_or(GitAdapterError::OutputTooLarge)
        };
        GitStatusCounts::new(
            bound(staged)?,
            bound(unstaged)?,
            bound(untracked)?,
            bound(conflicted)?,
        )
        .map_err(|_| GitAdapterError::OutputTooLarge)
    }

    fn replace_refs(&self, path: &Path) -> Result<GitReplaceRefsState, GitAdapterError> {
        let output =
            self.runner.run(path, &["for-each-ref", "--count=1", "--format=x", "refs/replace/"])?;
        Ok(match output.status {
            Some(0) if output.stdout.is_empty() => GitReplaceRefsState::None,
            Some(0) => GitReplaceRefsState::Active,
            _ => GitReplaceRefsState::Unknown,
        })
    }

    fn partial_clone(&self, path: &Path) -> Result<GitPartialCloneState, GitAdapterError> {
        let promisor = self.runner.run(
            path,
            &[
                "config",
                "--null",
                "--get-regexp",
                r"^(remote\..*\.promisor|extensions\.partialclone)$",
            ],
        )?;
        Ok(match promisor.status {
            Some(0) => GitPartialCloneState::Partial,
            Some(1) => GitPartialCloneState::Full,
            _ => GitPartialCloneState::Unknown,
        })
    }
}

/// Runs Git with the hardening above. Shared by the snapshot adapter and the
/// hook manager so every invocation uses the same environment and overrides.
pub(crate) struct GitRunner {
    git: PathBuf,
}

impl GitRunner {
    pub(crate) fn new(git: impl Into<PathBuf>) -> Self {
        Self { git: git.into() }
    }

    fn command(&self, path: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.git);
        command
            .args(HARDENING)
            .args(args)
            .current_dir(path)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", "/nonexistent")
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_PROTOCOL_FROM_USER", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }

    pub(crate) fn run(&self, path: &Path, args: &[&str]) -> Result<Output, GitAdapterError> {
        let mut child =
            self.command(path, args).spawn().map_err(|_| GitAdapterError::GitUnavailable)?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let reader = thread::spawn(move || {
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 64 * 1024];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => return Ok(buffer),
                    Ok(read) => {
                        if buffer.len() + read > MAX_GIT_OUTPUT_BYTES {
                            return Err(GitAdapterError::OutputTooLarge);
                        }
                        buffer.extend_from_slice(&chunk[..read]);
                    }
                    Err(_) => return Err(GitAdapterError::UnexpectedOutput),
                }
            }
        });
        let deadline = Instant::now() + GIT_COMMAND_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(GitAdapterError::Timeout);
                }
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => return Err(GitAdapterError::GitUnavailable),
            }
        };
        let stdout = match reader.join() {
            Ok(result) => result?,
            Err(_) => return Err(GitAdapterError::UnexpectedOutput),
        };
        Ok(Output { status: status.code(), stdout })
    }

    pub(crate) fn checked(&self, path: &Path, args: &[&str]) -> Result<Vec<u8>, GitAdapterError> {
        let output = self.run(path, args)?;
        match output.status {
            Some(0) => Ok(output.stdout),
            Some(128) => Err(GitAdapterError::NotARepository),
            _ => Err(GitAdapterError::UnexpectedOutput),
        }
    }

    pub(crate) fn single_line(
        &self,
        path: &Path,
        args: &[&str],
    ) -> Result<Vec<u8>, GitAdapterError> {
        let mut stdout = self.checked(path, args)?;
        if stdout.last() != Some(&b'\n') {
            return Err(GitAdapterError::UnexpectedOutput);
        }
        stdout.pop();
        if stdout.contains(&b'\n') {
            return Err(GitAdapterError::UnexpectedOutput);
        }
        Ok(stdout)
    }

    pub(crate) fn flag(&self, path: &Path, flag: &str) -> Result<bool, GitAdapterError> {
        match self.single_line(path, &["rev-parse", flag])?.as_slice() {
            b"true" => Ok(true),
            b"false" => Ok(false),
            _ => Err(GitAdapterError::UnexpectedOutput),
        }
    }

    /// A directory Git reports, used only to read filesystem identity. A path
    /// containing a newline cannot be told apart from two lines and is refused.
    pub(crate) fn path_line(&self, path: &Path, flag: &str) -> Result<PathBuf, GitAdapterError> {
        let line = self.single_line(path, &["rev-parse", "--path-format=absolute", flag])?;
        let resolved = os_path(line);
        if !resolved.is_dir() {
            return Err(GitAdapterError::UnexpectedOutput);
        }
        Ok(resolved)
    }
}

fn worktree_state(counts: &GitStatusCounts) -> GitWorktreeState {
    if counts.conflicted > 0 {
        GitWorktreeState::Conflicted
    } else if counts.staged > 0 || counts.unstaged > 0 {
        GitWorktreeState::Modified
    } else if counts.untracked > 0 {
        GitWorktreeState::UntrackedOnly
    } else {
        GitWorktreeState::Clean
    }
}

fn operation(git_dir: &Path) -> GitOperation {
    let exists = |name: &str| std::fs::symlink_metadata(git_dir.join(name)).is_ok();
    if exists("rebase-merge") {
        GitOperation::Rebase
    } else if exists("rebase-apply/applying") {
        GitOperation::Apply
    } else if exists("rebase-apply") {
        GitOperation::Rebase
    } else if exists("MERGE_HEAD") {
        GitOperation::Merge
    } else if exists("CHERRY_PICK_HEAD") {
        GitOperation::CherryPick
    } else if exists("REVERT_HEAD") {
        GitOperation::Revert
    } else if exists("sequencer") {
        GitOperation::Sequencer
    } else if exists("BISECT_LOG") {
        GitOperation::Bisect
    } else {
        GitOperation::Idle
    }
}

fn alternates(common_dir: &Path) -> GitAlternateObjectDatabaseState {
    match std::fs::metadata(common_dir.join("objects/info/alternates")) {
        Ok(metadata) if metadata.len() > 0 => GitAlternateObjectDatabaseState::Present,
        Ok(_) => GitAlternateObjectDatabaseState::None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            GitAlternateObjectDatabaseState::None
        }
        Err(_) => GitAlternateObjectDatabaseState::Unknown,
    }
}

#[cfg(unix)]
fn os_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_path(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}
