//! Verifiable, reversible installation of optional repository-local Git hooks.
//!
//! GHOSTRACE never takes silent control of Git behavior. The hook manager
//! writes a small shim per managed hook into the repository's own hooks
//! directory, records each written file's SHA-256 digest and mode in a record
//! beside it, and removes only files whose content still matches that record.
//! It refuses rather than guesses when:
//!
//! - a hook it would write already exists and is not a GHOSTRACE shim;
//! - `core.hooksPath` is configured, so another hook manager owns hooks;
//! - the hooks directory or a hook is a symlink, or is owned by another user;
//! - a recorded file changed since it was written (verify reports drift);
//! - another hook operation holds the lock.
//!
//! Every operation returns the exact files it would or did affect. Operations
//! are idempotent: repeating one that already holds is a no-op.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::git_adapter::GitRunner;

/// Hooks a GHOSTRACE shim may occupy. Each only signals that a snapshot may be
/// due; none receives hook arguments or standard input.
pub const MANAGED_GIT_HOOKS: [&str; 4] =
    ["post-checkout", "post-commit", "post-merge", "post-rewrite"];
/// Version written into every shim; bumping it makes `upgrade` rewrite shims.
pub const GIT_HOOK_SHIM_VERSION: u32 = 1;
/// Record of installed shims, kept in the hooks directory.
pub const GIT_HOOK_RECORD_NAME: &str = "ghostrace-hooks.json";
const LOCK_NAME: &str = "ghostrace-hooks.lock";
const SHIM_MARKER: &str = "# ghostrace-hook-shim";
/// Suffix of a user hook preserved behind a chaining shim.
pub const GIT_HOOK_PRESERVED_SUFFIX: &str = ".ghostrace-preserved";
use GIT_HOOK_PRESERVED_SUFFIX as PRESERVED_SUFFIX;

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum GitHookError {
    #[error("the Git executable could not be started")]
    GitUnavailable,
    #[error("the path is not a Git repository with a worktree")]
    NotARepository,
    #[error("core.hooksPath is configured; another hook manager owns this repository's hooks")]
    HooksPathManaged,
    #[error("the hooks directory is a symlink, not a directory, or not owned by the current user")]
    UnsafeHooksDirectory,
    #[error("hook {0} exists and is not a GHOSTRACE shim")]
    ForeignHook(&'static str),
    #[error("hook {0} is a symlink or not a regular file")]
    UnsafeHook(&'static str),
    #[error("installed files changed since they were written: {0:?}")]
    Drift(Vec<&'static str>),
    #[error("another GHOSTRACE hook operation is in progress")]
    Locked,
    #[error("a hook file appeared while it was being created")]
    ConcurrentEdit,
    #[error("the delegate command must be an absolute path to an executable file")]
    InvalidDelegate,
    #[error("the hook record is unreadable")]
    InvalidRecord,
    #[error("the confirmed plan no longer matches the repository")]
    ConfirmationMismatch,
    #[error("a preserved copy of hook {0} already exists")]
    PreservedHookExists(&'static str),
    #[error("a filesystem operation on the hooks directory failed")]
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHookAction {
    Create,
    /// Preserve an existing user hook and install a shim that runs it first.
    Chain,
    /// Put a preserved user hook back in place.
    Restore,
    Replace,
    Enable,
    Disable,
    Remove,
    Unchanged,
}

/// One file an operation affects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitHookChange {
    pub hook: &'static str,
    pub path: PathBuf,
    pub action: GitHookAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHookHealth {
    /// Present and byte-identical to what was written.
    Intact,
    Missing,
    /// Present but changed, replaced, or no longer a regular file.
    Drifted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitHookVerification {
    pub hook: &'static str,
    pub health: GitHookHealth,
    /// Health of the preserved user hook the shim chains to, if any.
    pub preserved: Option<GitHookHealth>,
    pub enabled: bool,
    pub shim_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordEntry {
    sha256: String,
    shim_version: u32,
    enabled: bool,
    /// Digest of the user hook moved aside when this shim chains to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preserved_sha256: Option<String>,
}

impl RecordEntry {
    /// A chained shim stays executable while disabled so the user's hook
    /// keeps running; only a plain shim is disabled by clearing execute bits.
    fn expected_executable(&self) -> bool {
        self.enabled || self.preserved_sha256.is_some()
    }
}

/// A chained-install plan and the digest a caller must confirm to apply it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GitHookPlan {
    pub changes: Vec<GitHookChange>,
    pub digest: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    hooks: BTreeMap<String, RecordEntry>,
}

/// Plans and applies hook changes for one repository.
pub struct GitHookManager {
    runner: GitRunner,
    delegate: PathBuf,
}

impl GitHookManager {
    /// `delegate` is the absolute path each shim executes, for example the
    /// installed `ghostrace` binary.
    pub fn new(
        git: impl Into<PathBuf>,
        delegate: impl Into<PathBuf>,
    ) -> Result<Self, GitHookError> {
        let delegate = delegate.into();
        let executable = fs::metadata(&delegate)
            .map(|metadata| metadata.is_file() && is_executable(&metadata))
            .unwrap_or(false);
        let printable = delegate
            .to_str()
            .is_some_and(|text| !text.chars().any(|c| c.is_control() || c == '\''));
        if !delegate.is_absolute() || !executable || !printable {
            return Err(GitHookError::InvalidDelegate);
        }
        Ok(Self { runner: GitRunner::new(git), delegate })
    }

    /// Show exactly what `install` would do without changing anything.
    pub fn plan_install(&self, repository: &Path) -> Result<Vec<GitHookChange>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let record = read_record(&hooks)?;
        MANAGED_GIT_HOOKS
            .iter()
            .map(|hook| {
                let path = hooks.join(hook);
                let action = match (inspect(&path, hook)?, record.hooks.get(*hook)) {
                    (FileState::Absent, _) => GitHookAction::Create,
                    (FileState::Regular { content, .. }, Some(entry))
                        if digest(&content) == entry.sha256 =>
                    {
                        if content == self.shim_for(hook, entry.preserved_sha256.is_some(), true)
                            && entry.enabled
                        {
                            GitHookAction::Unchanged
                        } else {
                            GitHookAction::Replace
                        }
                    }
                    (FileState::Regular { content, .. }, _) if is_shim(&content) => {
                        return Err(GitHookError::Drift(vec![*hook]))
                    }
                    (FileState::Regular { .. }, _) => return Err(GitHookError::ForeignHook(hook)),
                };
                Ok(GitHookChange { hook, path, action })
            })
            .collect()
    }

    /// Install or upgrade every managed shim. Existing foreign hooks, drift,
    /// or unsafe indirection refuse the whole operation before any write.
    pub fn install(&self, repository: &Path) -> Result<Vec<GitHookChange>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let _lock = Lock::acquire(&hooks)?;
        let plan = self.plan_install(repository)?;
        let mut record = read_record(&hooks)?;
        for change in &plan {
            let chained =
                record.hooks.get(change.hook).is_some_and(|entry| entry.preserved_sha256.is_some());
            let content = self.shim_for(change.hook, chained, true);
            match change.action {
                GitHookAction::Create => write_new(&change.path, &content)?,
                GitHookAction::Replace => replace(&change.path, &content)?,
                _ => {}
            }
            record.hooks.insert(
                change.hook.to_owned(),
                RecordEntry {
                    sha256: digest(&content),
                    shim_version: GIT_HOOK_SHIM_VERSION,
                    enabled: true,
                    preserved_sha256: record
                        .hooks
                        .get(change.hook)
                        .and_then(|entry| entry.preserved_sha256.clone()),
                },
            );
        }
        write_record(&hooks, &record)?;
        Ok(plan)
    }

    /// Report whether each recorded shim is intact, missing, or drifted.
    pub fn verify(&self, repository: &Path) -> Result<Vec<GitHookVerification>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let record = read_record(&hooks)?;
        MANAGED_GIT_HOOKS
            .iter()
            .filter_map(|hook| record.hooks.get(*hook).map(|entry| (hook, entry)))
            .map(|(hook, entry)| {
                let preserved = entry.preserved_sha256.as_ref().map(|expected| {
                    match inspect(&preserved_path(&hooks, hook), hook) {
                        Ok(FileState::Absent) => GitHookHealth::Missing,
                        Ok(FileState::Regular { content, .. }) if &digest(&content) == expected => {
                            GitHookHealth::Intact
                        }
                        _ => GitHookHealth::Drifted,
                    }
                });
                let health = match inspect(&hooks.join(hook), hook) {
                    Ok(FileState::Absent) => GitHookHealth::Missing,
                    Ok(FileState::Regular { content, executable }) => {
                        if digest(&content) == entry.sha256
                            && executable == entry.expected_executable()
                        {
                            GitHookHealth::Intact
                        } else {
                            GitHookHealth::Drifted
                        }
                    }
                    Err(_) => GitHookHealth::Drifted,
                };
                Ok(GitHookVerification {
                    hook,
                    health,
                    preserved,
                    enabled: entry.enabled,
                    shim_version: entry.shim_version,
                })
            })
            .collect()
    }

    /// Make every intact shim inert by clearing its execute bits; Git skips
    /// non-executable hooks. Content is untouched, so `enable` restores it.
    pub fn disable(&self, repository: &Path) -> Result<Vec<GitHookChange>, GitHookError> {
        self.set_enabled(repository, false)
    }

    pub fn enable(&self, repository: &Path) -> Result<Vec<GitHookChange>, GitHookError> {
        self.set_enabled(repository, true)
    }

    /// Remove every recorded shim whose content still matches the record,
    /// then the record itself. Any drift refuses the whole operation.
    pub fn uninstall(&self, repository: &Path) -> Result<Vec<GitHookChange>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let _lock = Lock::acquire(&hooks)?;
        let record = read_record(&hooks)?;
        let checks = self.intact_or_refuse(&hooks, &record)?;
        let mut changes = Vec::new();
        for (hook, present) in checks {
            let path = hooks.join(hook);
            if present {
                // Re-check immediately before removal so a concurrent edit
                // since the drift check is preserved, not deleted.
                match inspect(&path, hook)? {
                    FileState::Regular { content, .. }
                        if digest(&content) == record.hooks[hook].sha256 =>
                    {
                        fs::remove_file(&path).map_err(|_| GitHookError::Io)?;
                    }
                    _ => return Err(GitHookError::Drift(vec![hook])),
                }
            }
            if record.hooks[hook].preserved_sha256.is_some() {
                // The preserved hook was verified intact above; put it back
                // with its original content and mode.
                fs::rename(preserved_path(&hooks, hook), &path).map_err(|_| GitHookError::Io)?;
                changes.push(GitHookChange { hook, path, action: GitHookAction::Restore });
                continue;
            }
            changes.push(GitHookChange {
                hook,
                path,
                action: if present { GitHookAction::Remove } else { GitHookAction::Unchanged },
            });
        }
        match fs::remove_file(hooks.join(GIT_HOOK_RECORD_NAME)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(GitHookError::Io),
        }
        Ok(changes)
    }

    fn set_enabled(
        &self,
        repository: &Path,
        enabled: bool,
    ) -> Result<Vec<GitHookChange>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let _lock = Lock::acquire(&hooks)?;
        let mut record = read_record(&hooks)?;
        let checks = self.intact_or_refuse(&hooks, &record)?;
        let mut changes = Vec::new();
        for (hook, present) in checks {
            let path = hooks.join(hook);
            let entry = record.hooks.get_mut(hook).expect("recorded hook");
            let action = if !present || entry.enabled == enabled {
                GitHookAction::Unchanged
            } else {
                if entry.preserved_sha256.is_some() {
                    let content = self.shim_for(hook, true, enabled);
                    replace(&path, &content)?;
                    entry.sha256 = digest(&content);
                } else {
                    set_mode(&path, if enabled { 0o755 } else { 0o644 })?;
                }
                entry.enabled = enabled;
                if enabled {
                    GitHookAction::Enable
                } else {
                    GitHookAction::Disable
                }
            };
            changes.push(GitHookChange { hook, path, action });
        }
        write_record(&hooks, &record)?;
        Ok(changes)
    }

    /// Returns each recorded hook and whether its file is present, or refuses
    /// if any recorded file drifted.
    fn intact_or_refuse(
        &self,
        hooks: &Path,
        record: &Record,
    ) -> Result<Vec<(&'static str, bool)>, GitHookError> {
        let mut drifted = Vec::new();
        let mut checks = Vec::new();
        for hook in MANAGED_GIT_HOOKS {
            let Some(entry) = record.hooks.get(hook) else { continue };
            match inspect(&hooks.join(hook), hook) {
                Ok(FileState::Absent) => checks.push((hook, false)),
                Ok(FileState::Regular { content, executable })
                    if digest(&content) == entry.sha256
                        && executable == entry.expected_executable() =>
                {
                    let preserved_intact = entry.preserved_sha256.as_ref().is_none_or(|expected| {
                        matches!(
                            inspect(&preserved_path(hooks, hook), hook),
                            Ok(FileState::Regular { content, .. }) if &digest(&content) == expected
                        )
                    });
                    if preserved_intact {
                        checks.push((hook, true))
                    } else {
                        drifted.push(hook)
                    }
                }
                _ => drifted.push(hook),
            }
        }
        if drifted.is_empty() {
            Ok(checks)
        } else {
            Err(GitHookError::Drift(drifted))
        }
    }

    fn hooks_dir(&self, repository: &Path) -> Result<PathBuf, GitHookError> {
        let bare = self.runner.flag(repository, "--is-bare-repository").map_err(map_git)?;
        if bare {
            return Err(GitHookError::NotARepository);
        }
        let configured = self
            .runner
            .run(repository, &["config", "--null", "--show-scope", "--get-all", "core.hooksPath"])
            .map_err(map_git)?;
        let mut fields = configured.stdout.split(|byte| *byte == 0);
        while let (Some(scope), Some(_value)) = (fields.next(), fields.next()) {
            // The runner's own `-c core.hooksPath=/dev/null` has command scope.
            if !scope.is_empty() && scope != b"command" {
                return Err(GitHookError::HooksPathManaged);
            }
        }
        // Hooks live in the common directory, so they apply to every linked
        // worktree of the repository.
        let common = self.runner.path_line(repository, "--git-common-dir").map_err(map_git)?;
        let hooks = common.join("hooks");
        match fs::symlink_metadata(&hooks) {
            Ok(metadata) if metadata.is_dir() && owned_by_current_user(&metadata) => Ok(hooks),
            Ok(_) => Err(GitHookError::UnsafeHooksDirectory),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&hooks).map_err(|_| GitHookError::Io)?;
                Ok(hooks)
            }
            Err(_) => Err(GitHookError::Io),
        }
    }

    /// The shim for `hook`. A chained shim runs the preserved user hook
    /// first with the original arguments and standard input and exits with
    /// its status; the delegate never sees either. A disabled chained shim
    /// runs only the user hook.
    fn shim_for(&self, hook: &str, chained: bool, enabled: bool) -> Vec<u8> {
        let delegate = self.delegate.to_str().expect("delegate validated as text");
        let header = format!(
            "#!/bin/sh\n{SHIM_MARKER} v{GIT_HOOK_SHIM_VERSION} hook={hook}\n\
             # Installed by GHOSTRACE. Remove it with the GHOSTRACE hook uninstaller;\n\
             # hand edits are reported as drift and block automatic removal.\n"
        );
        let body = match (chained, enabled) {
            (false, _) => format!("exec '{delegate}' git-hook {hook} </dev/null\n"),
            (true, true) => format!(
                "\"$(dirname \"$0\")/{hook}{PRESERVED_SUFFIX}\" \"$@\"\n\
                 status=$?\n\
                 '{delegate}' git-hook {hook} </dev/null || true\n\
                 exit $status\n"
            ),
            (true, false) => format!(
                "# GHOSTRACE delegation is disabled; only the preserved hook runs.\n\
                 exec \"$(dirname \"$0\")/{hook}{PRESERVED_SUFFIX}\" \"$@\"\n"
            ),
        };
        format!("{header}{body}").into_bytes()
    }

    /// Plan an install that chains existing user hooks instead of refusing.
    /// The returned digest covers every change, the content of every hook
    /// that would be preserved, the delegate, and the shim version; the
    /// caller must pass it back to [`Self::install_chained`].
    pub fn plan_chained_install(&self, repository: &Path) -> Result<GitHookPlan, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let record = read_record(&hooks)?;
        let mut changes = Vec::new();
        let mut hasher = Sha256::new();
        hasher.update(b"ghostrace-hook-plan-v1\0");
        hasher.update(self.delegate.to_string_lossy().as_bytes());
        hasher.update(GIT_HOOK_SHIM_VERSION.to_le_bytes());
        for hook in MANAGED_GIT_HOOKS {
            let path = hooks.join(hook);
            let (action, foreign) = match (inspect(&path, hook)?, record.hooks.get(hook)) {
                (FileState::Absent, _) => (GitHookAction::Create, None),
                (FileState::Regular { content, .. }, Some(entry))
                    if digest(&content) == entry.sha256 =>
                {
                    let current = self.shim_for(hook, entry.preserved_sha256.is_some(), true);
                    if content == current && entry.enabled {
                        (GitHookAction::Unchanged, None)
                    } else {
                        (GitHookAction::Replace, None)
                    }
                }
                (FileState::Regular { content, .. }, _) if is_shim(&content) => {
                    return Err(GitHookError::Drift(vec![hook]))
                }
                (FileState::Regular { content, .. }, _) => {
                    if fs::symlink_metadata(preserved_path(&hooks, hook)).is_ok() {
                        return Err(GitHookError::PreservedHookExists(hook));
                    }
                    (GitHookAction::Chain, Some(digest(&content)))
                }
            };
            hasher.update(hook.as_bytes());
            hasher.update([action as u8]);
            hasher.update(foreign.as_deref().unwrap_or("").as_bytes());
            changes.push(GitHookChange { hook, path, action });
        }
        let digest = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(GitHookPlan { changes, digest })
    }

    /// Apply a chained-install plan the caller confirmed by digest. The plan
    /// is recomputed under the lock and refused if anything changed.
    pub fn install_chained(
        &self,
        repository: &Path,
        confirmed_digest: &str,
    ) -> Result<Vec<GitHookChange>, GitHookError> {
        let hooks = self.hooks_dir(repository)?;
        let _lock = Lock::acquire(&hooks)?;
        let plan = self.plan_chained_install(repository)?;
        if plan.digest != confirmed_digest {
            return Err(GitHookError::ConfirmationMismatch);
        }
        let mut record = read_record(&hooks)?;
        for change in &plan.changes {
            let existing = record.hooks.get(change.hook).cloned();
            let preserved_sha256 = match change.action {
                GitHookAction::Chain => {
                    let FileState::Regular { content, .. } = inspect(&change.path, change.hook)?
                    else {
                        return Err(GitHookError::ConfirmationMismatch);
                    };
                    let preserved = preserved_path(&hooks, change.hook);
                    if fs::symlink_metadata(&preserved).is_ok() {
                        return Err(GitHookError::PreservedHookExists(change.hook));
                    }
                    fs::rename(&change.path, &preserved).map_err(|_| GitHookError::Io)?;
                    Some(digest(&content))
                }
                _ => existing.as_ref().and_then(|entry| entry.preserved_sha256.clone()),
            };
            let content = self.shim_for(change.hook, preserved_sha256.is_some(), true);
            match change.action {
                GitHookAction::Create | GitHookAction::Chain => write_new(&change.path, &content)?,
                GitHookAction::Replace => replace(&change.path, &content)?,
                _ => {}
            }
            record.hooks.insert(
                change.hook.to_owned(),
                RecordEntry {
                    sha256: digest(&content),
                    shim_version: GIT_HOOK_SHIM_VERSION,
                    enabled: true,
                    preserved_sha256,
                },
            );
        }
        write_record(&hooks, &record)?;
        Ok(plan.changes)
    }
}

enum FileState {
    Absent,
    Regular { content: Vec<u8>, executable: bool },
}

fn inspect(path: &Path, hook: &'static str) -> Result<FileState, GitHookError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && owned_by_current_user(&metadata) => {
            let content = fs::read(path).map_err(|_| GitHookError::Io)?;
            Ok(FileState::Regular { content, executable: is_executable(&metadata) })
        }
        Ok(_) => Err(GitHookError::UnsafeHook(hook)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileState::Absent),
        Err(_) => Err(GitHookError::Io),
    }
}

fn preserved_path(hooks: &Path, hook: &str) -> PathBuf {
    hooks.join(format!("{hook}{PRESERVED_SUFFIX}"))
}

fn is_shim(content: &[u8]) -> bool {
    content.windows(SHIM_MARKER.len()).any(|window| window == SHIM_MARKER.as_bytes())
}

fn digest(content: &[u8]) -> String {
    Sha256::digest(content).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_record(hooks: &Path) -> Result<Record, GitHookError> {
    let path = hooks.join(GIT_HOOK_RECORD_NAME);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            let record: Record =
                serde_json::from_slice(&fs::read(&path).map_err(|_| GitHookError::Io)?)
                    .map_err(|_| GitHookError::InvalidRecord)?;
            if record.schema_version != 1 {
                return Err(GitHookError::InvalidRecord);
            }
            Ok(record)
        }
        Ok(_) => Err(GitHookError::InvalidRecord),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Record { schema_version: 1, hooks: BTreeMap::new() })
        }
        Err(_) => Err(GitHookError::Io),
    }
}

fn write_record(hooks: &Path, record: &Record) -> Result<(), GitHookError> {
    let path = hooks.join(GIT_HOOK_RECORD_NAME);
    if record.hooks.is_empty() {
        return Ok(());
    }
    let content = serde_json::to_vec_pretty(record).map_err(|_| GitHookError::InvalidRecord)?;
    replace_with_mode(&path, &content, 0o644)
}

/// Create a file that must not already exist, so a concurrent creation wins
/// and this operation refuses instead of overwriting it.
fn write_new(path: &Path, content: &[u8]) -> Result<(), GitHookError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_create_mode(&mut options, 0o755);
    let mut file = options.open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            GitHookError::ConcurrentEdit
        } else {
            GitHookError::Io
        }
    })?;
    file.write_all(content).map_err(|_| GitHookError::Io)?;
    file.sync_all().map_err(|_| GitHookError::Io)
}

fn replace(path: &Path, content: &[u8]) -> Result<(), GitHookError> {
    replace_with_mode(path, content, 0o755)
}

/// Write beside the target and rename over it, so readers see either the
/// old or the new file and never a partial one.
fn replace_with_mode(path: &Path, content: &[u8], mode: u32) -> Result<(), GitHookError> {
    let temporary = path.with_extension(format!("ghostrace-tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_create_mode(&mut options, mode);
    let mut file = options.open(&temporary).map_err(|_| GitHookError::Io)?;
    file.write_all(content).map_err(|_| GitHookError::Io)?;
    file.sync_all().map_err(|_| GitHookError::Io)?;
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        GitHookError::Io
    })
}

struct Lock(PathBuf);

impl Lock {
    fn acquire(hooks: &Path) -> Result<Self, GitHookError> {
        let path = hooks.join(LOCK_NAME);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(Self(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(GitHookError::Locked)
            }
            Err(_) => Err(GitHookError::Io),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn map_git(error: crate::git_adapter::GitAdapterError) -> GitHookError {
    match error {
        crate::git_adapter::GitAdapterError::GitUnavailable => GitHookError::GitUnavailable,
        _ => GitHookError::NotARepository,
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn owned_by_current_user(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions.
    metadata.uid() == unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn owned_by_current_user(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), GitHookError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|_| GitHookError::Io)
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), GitHookError> {
    Ok(())
}

#[cfg(unix)]
fn set_create_mode(options: &mut OpenOptions, mode: u32) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(mode);
}

#[cfg(not(unix))]
fn set_create_mode(_options: &mut OpenOptions, _mode: u32) {}
