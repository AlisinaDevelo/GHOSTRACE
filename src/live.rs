//! The live journal a person runs on their own Mac.
//!
//! A GHOSTRACE home is a private directory holding the encrypted journal and
//! a small configuration file. Its key lives in the user's login keychain
//! (explicit custody for unsigned builds, see [`MacOsKeychainProvider`]).
//! Every recording command is itself the explicit request: `run` wraps one
//! command, `watch` observes one named folder for a bounded time after the
//! user confirms the preview, and `git-snapshot` records one repository.
//! Nothing records in the background.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Utc;
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    claims::{render_claim, ClaimLocale},
    consent::ConsentPreview,
    crypto::KeyProvider,
    fsevents::FseventsOptions,
    fsevents_collector::{
        FseventsCollector, FseventsCollectorConfig, InternalPathPolicy, SelectedRoot,
    },
    git_adapter::GitSnapshotAdapter,
    git_history::GitHistoryTransition,
    git_snapshot::GitSnapshotMetadata,
    journal::Journal,
    keychain::{KeyCustody, MacOsKeychainProvider, JOURNAL_KEYCHAIN_SERVICE},
    model::{
        EventEnvelope, EventKind, EventPayload, EventSource, Evidence, GapPayload,
        GitSnapshotPayload, IngestionOrigin, ReasonCode, RootId,
    },
    policy::{PolicyDocument, PolicyProfile},
    shell_wrapper::{ShellRunEvidence, ShellWrapper, ShellWrapperConfig},
    writer::{Writer, WriterConfig, WriterOutcome},
};

const CONFIG_NAME: &str = "config.json";
const JOURNAL_NAME: &str = "journal.sqlite3";
const GIT_STATE_DIR: &str = "git-state";
const POLICY_ID: &str = "live-v1";
/// Policy root that Git snapshots are recorded under.
pub const GIT_ROOT_ID: &str = "git-explicit";

#[derive(Debug, Error)]
pub enum LiveError {
    #[error("no GHOSTRACE home here; run `ghostrace live init` first")]
    NotInitialized,
    #[error("a GHOSTRACE home already exists here")]
    AlreadyInitialized,
    #[error("the home directory is not private to this user")]
    UnsafeHome,
    #[error("`ghostrace run` needs your consent first; run `ghostrace live consent-shell`")]
    ShellConsentRequired,
    #[error("the configuration is unreadable")]
    Config,
    #[error("{0} failed: {1}")]
    Operation(&'static str, String),
}

fn op<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> LiveError {
    move |error| LiveError::Operation(what, error.to_string())
}

/// Persisted configuration. Folder paths the user chose to watch are kept
/// here, in the private home, so repeat watches reuse their root identity;
/// they never enter the journal.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveConfig {
    pub schema_version: u32,
    pub custody: KeyCustody,
    pub keychain_service: String,
    pub keychain_account: String,
    pub policy_version: u32,
    pub watched_roots: BTreeMap<String, PathBuf>,
    /// Present only while the user's consent to `ghostrace run` stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_consent: Option<ShellConsentReceipt>,
}

/// The persisted record of consent to `ghostrace run`. It is bound to the
/// digest of the exact preview the user accepted, so a changed preview
/// requires consent again.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellConsentReceipt {
    pub granted_at: chrono::DateTime<Utc>,
    pub preview_sha256: String,
}

/// What `ghostrace run` records, shown before consent is given.
pub const SHELL_CONSENT_PREVIEW: &str = "`ghostrace run -- <command>` will record, for each \
command you run through it:\n\
  - the program's name as a normalized token, and the kind of working folder\n\
    (as a salted digest, not a path)\n\
  - when it started and finished, how it ended, and its exit code or signal\n\
Never recorded: arguments, environment variables, input or output, or anything\n\
you run without `ghostrace run`. Consent lasts until `ghostrace live revoke-shell`.";

fn shell_preview_digest() -> String {
    Sha256::digest(SHELL_CONSENT_PREVIEW.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub struct LiveHome {
    dir: PathBuf,
    config: LiveConfig,
}

/// A summary line for `timeline`.
#[derive(Clone, Debug, Serialize)]
pub struct TimelineEntry {
    pub event_id: Uuid,
    pub observed_at: chrono::DateTime<Utc>,
    pub source: String,
    pub kind: String,
    pub evidence: Evidence,
    pub statement: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveStatus {
    pub custody: KeyCustody,
    pub events: usize,
    pub gaps: usize,
    pub by_source: BTreeMap<String, usize>,
    pub watched_roots: usize,
    pub shell_consent: bool,
    pub last_event_at: Option<chrono::DateTime<Utc>>,
}

/// What a `watch` session recorded.
#[derive(Clone, Debug, Serialize)]
pub struct WatchSummary {
    pub root_id: String,
    pub accepted_events: u64,
    pub blocked_events: u64,
    pub dropped_events: u64,
    pub seconds: u64,
}

/// What a `git-snapshot` recorded.
#[derive(Clone, Debug, Serialize)]
pub struct GitSnapshotSummary {
    pub repository_id: String,
    pub branch_class: String,
    pub worktree_state: String,
    pub changed: u64,
    pub transition: Option<String>,
    pub gap: Option<String>,
}

impl LiveHome {
    /// The default home: `~/Library/Application Support/GHOSTRACE`.
    pub fn default_dir() -> Result<PathBuf, LiveError> {
        let home = std::env::var_os("HOME").ok_or(LiveError::NotInitialized)?;
        Ok(PathBuf::from(home).join("Library/Application Support/GHOSTRACE"))
    }

    /// Create a home with a fresh key in the login keychain.
    pub fn init(dir: &Path) -> Result<Self, LiveError> {
        if dir.join(CONFIG_NAME).exists() {
            return Err(LiveError::AlreadyInitialized);
        }
        if !dir.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(op("create home"))?;
        }
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(op("secure home"))?;
        let canonical = fs::canonicalize(dir).map_err(op("resolve home"))?;
        let digest = Sha256::digest(canonical.as_os_str().as_encoded_bytes());
        let account = format!(
            "journal-{}",
            digest[..8].iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        let provider =
            MacOsKeychainProvider::login_keychain_with_identity(JOURNAL_KEYCHAIN_SERVICE, &account)
                .map_err(op("configure keychain"))?;
        let mut key = [0u8; 32];
        rand_core::OsRng.try_fill_bytes(&mut key).map_err(op("generate key"))?;
        provider.provision(key).map_err(op("store key in login keychain"))?;
        let home = Self {
            dir: canonical,
            config: LiveConfig {
                schema_version: 1,
                custody: KeyCustody::LoginKeychain,
                keychain_service: JOURNAL_KEYCHAIN_SERVICE.to_owned(),
                keychain_account: account,
                policy_version: 1,
                watched_roots: BTreeMap::new(),
                shell_consent: None,
            },
        };
        home.save()?;
        let journal = home.journal()?;
        journal.initialize_authenticated_state().map_err(op("initialize journal"))?;
        journal.shutdown().map_err(op("close journal"))?;
        Ok(home)
    }

    pub fn open(dir: &Path) -> Result<Self, LiveError> {
        let metadata = fs::symlink_metadata(dir).map_err(|_| LiveError::NotInitialized)?;
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        use std::os::unix::fs::MetadataExt;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(LiveError::UnsafeHome);
        }
        let bytes = fs::read(dir.join(CONFIG_NAME)).map_err(|_| LiveError::NotInitialized)?;
        let config: LiveConfig = serde_json::from_slice(&bytes).map_err(|_| LiveError::Config)?;
        // Compare against the resolved path: watch and report refusals and
        // the watch exclusion would miss a home reached through a symlink
        // such as /var -> /private/var.
        let dir = fs::canonicalize(dir).map_err(|_| LiveError::NotInitialized)?;
        Ok(Self { dir, config })
    }

    pub fn config(&self) -> &LiveConfig {
        &self.config
    }

    fn save(&self) -> Result<(), LiveError> {
        let path = self.dir.join(CONFIG_NAME);
        let temporary = self.dir.join(format!("{CONFIG_NAME}.tmp"));
        let _ = fs::remove_file(&temporary);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(op("write configuration"))?;
        file.write_all(&serde_json::to_vec_pretty(&self.config).map_err(|_| LiveError::Config)?)
            .map_err(op("write configuration"))?;
        file.sync_all().map_err(op("write configuration"))?;
        fs::rename(&temporary, &path).map_err(op("write configuration"))
    }

    fn provider(&self) -> Result<MacOsKeychainProvider, LiveError> {
        MacOsKeychainProvider::login_keychain_with_identity(
            &self.config.keychain_service,
            &self.config.keychain_account,
        )
        .map_err(op("configure keychain"))
    }

    /// Read the journal key once. On an unsigned build macOS may ask the user
    /// to allow access here; later reads in the same command reuse approval.
    pub fn unlock(&self) -> Result<(), LiveError> {
        self.provider()?.key().map(|_| ()).map_err(op("read key from login keychain"))
    }

    pub fn journal(&self) -> Result<Journal, LiveError> {
        let provider = self.provider()?;
        provider.key().map_err(op("read key from login keychain"))?;
        Journal::open_fixture(self.dir.join(JOURNAL_NAME), provider).map_err(op("open journal"))
    }

    /// The policy that authorizes live recording: shell, Git, filesystem, and
    /// lifecycle sources for the folders the user has chosen to watch.
    pub fn policy_document(&self) -> Result<PolicyDocument, LiveError> {
        let mut roots = self.config.watched_roots.keys().cloned().collect::<BTreeSet<_>>();
        roots.insert(GIT_ROOT_ID.to_owned());
        PolicyDocument::new(
            POLICY_ID,
            self.config.policy_version,
            [EventSource::Shell, EventSource::Git, EventSource::Filesystem, EventSource::Lifecycle],
            roots,
            false,
        )
        .map_err(op("build policy"))
    }

    /// Run one command through the explicit wrapper and return its exit code.
    pub fn run(
        &self,
        program: &std::ffi::OsStr,
        args: &[std::ffi::OsString],
    ) -> Result<i32, LiveError> {
        let consent_at = match &self.config.shell_consent {
            Some(receipt) if receipt.preview_sha256 == shell_preview_digest() => receipt.granted_at,
            _ => return Err(LiveError::ShellConsentRequired),
        };
        let document = self.policy_document()?;
        let confirmation = ConsentPreview::from_policy(
            &document,
            ["executable_id", "working_directory", "timing", "outcome"],
            ["no_arguments", "no_environment", "no_terminal_streams"],
        )
        .map_err(op("prepare consent"))?
        .confirm();
        let journal = self.journal()?;
        let mut wrapper = ShellWrapper::new(
            confirmation,
            document,
            journal.clone(),
            ShellWrapperConfig {
                writer: WriterConfig::default(),
                collector_instance: "live-shell".to_owned(),
                consent_at,
                actor: "human".to_owned(),
                reason: "explicit_run".to_owned(),
                workspace: None,
                home: std::env::var_os("HOME").map(PathBuf::from),
            },
        )
        .map_err(op("start wrapper"))?;
        let report = wrapper.run(program, args).map_err(op("run"))?;
        journal.shutdown().map_err(op("close journal"))?;
        if let ShellRunEvidence::Gap { reason_code } = report.evidence {
            eprintln!("ghostrace: the command did not start ({reason_code}); recorded as a gap");
        }
        Ok(report.exit_code)
    }

    /// Persist consent to `ghostrace run` for the current preview.
    pub fn grant_shell_consent(&mut self) -> Result<(), LiveError> {
        self.config.shell_consent = Some(ShellConsentReceipt {
            granted_at: Utc::now(),
            preview_sha256: shell_preview_digest(),
        });
        self.save()
    }

    /// Withdraw consent; later `ghostrace run` calls refuse before spawning.
    pub fn revoke_shell_consent(&mut self) -> Result<bool, LiveError> {
        let had = self.config.shell_consent.take().is_some();
        self.save()?;
        Ok(had)
    }

    pub fn shell_consent_granted(&self) -> bool {
        self.config
            .shell_consent
            .as_ref()
            .is_some_and(|receipt| receipt.preview_sha256 == shell_preview_digest())
    }

    /// The preview a person confirms before `watch` starts.
    pub fn watch_preview(&self, folder: &Path) -> Result<String, LiveError> {
        let canonical = fs::canonicalize(folder).map_err(op("resolve folder"))?;
        if !canonical.is_dir() {
            return Err(LiveError::Operation("watch", "not a directory".to_owned()));
        }
        Ok(format!(
            "GHOSTRACE will watch this folder until you stop it:\n  {}\n\n\
             Recorded: which files change (as salted digests, not names), the kind of change\n\
             (created, modified, renamed, deleted), file or folder, and when.\n\
             Not recorded: file contents, file names in readable form, which app made the change.\n\
             Limits: macOS may coalesce rapid changes; missed history is recorded as a gap.",
            canonical.display()
        ))
    }

    /// Watch `folder` until `deadline` or `stop` returns true.
    pub fn watch(
        &mut self,
        folder: &Path,
        deadline: Option<Duration>,
        stop: &dyn Fn() -> bool,
    ) -> Result<WatchSummary, LiveError> {
        let canonical = fs::canonicalize(folder).map_err(op("resolve folder"))?;
        if canonical.starts_with(&self.dir) {
            return Err(LiveError::Operation(
                "watch",
                "the GHOSTRACE home cannot be watched".to_owned(),
            ));
        }
        let root_id = match self
            .config
            .watched_roots
            .iter()
            .find(|(_, path)| **path == canonical)
            .map(|(id, _)| id.clone())
        {
            Some(id) => id,
            None => {
                let digest = Sha256::digest(canonical.as_os_str().as_encoded_bytes());
                let id = format!(
                    "root-{}",
                    digest[..6].iter().map(|b| format!("{b:02x}")).collect::<String>()
                );
                self.config.watched_roots.insert(id.clone(), canonical.clone());
                self.config.policy_version += 1;
                self.save()?;
                id
            }
        };
        // Each watched folder has its own policy and collector instance: the
        // collector requires its policy scope to be exactly its roots, and a
        // per-folder instance keeps each folder's FSEvents cursor separate.
        let document = PolicyDocument::new(
            format!("watch-{root_id}"),
            1,
            [EventSource::Filesystem, EventSource::Lifecycle],
            [root_id.clone()],
            false,
        )
        .map_err(op("build policy"))?;
        let confirmation = ConsentPreview::from_policy(
            &document,
            ["path_digest", "operation", "entry_kind"],
            ["fsevents_coalescing", "no_process_attribution", "history_can_be_dropped"],
        )
        .map_err(op("prepare consent"))?
        .confirm();
        let root = SelectedRoot::new(root_id.as_str(), &canonical).map_err(op("select folder"))?;
        let journal = self.journal()?;
        let mut collector = FseventsCollector::new(
            confirmation,
            document,
            [root],
            journal.clone(),
            FseventsCollectorConfig {
                options: FseventsOptions {
                    latency: Duration::from_millis(200),
                    ..FseventsOptions::default()
                },
                writer: WriterConfig::default(),
                collector_instance: format!("live-watch-{root_id}"),
                instance_label: "ghostrace-watch".to_owned(),
                consent_at: Utc::now(),
                actor: "human".to_owned(),
                reason: "explicit_watch".to_owned(),
                history_timeout: Duration::from_secs(30),
                // The home may sit inside the watched folder; nothing in it
                // (journal, configuration, Git state) is a user change.
                internal_paths: InternalPathPolicy::new()
                    .with_directory(self.dir.clone())
                    .map_err(op("exclude the GHOSTRACE home"))?,
            },
        )
        .map_err(op("start watcher"))?;
        let started = Instant::now();
        collector.start().map_err(op("start watcher"))?;
        while !stop() && deadline.is_none_or(|limit| started.elapsed() < limit) {
            collector.run_current_run_loop_for(Duration::from_millis(250)).map_err(op("watch"))?;
        }
        let status = collector.status();
        collector.stop().map_err(op("stop watcher"))?;
        drop(collector);
        journal.shutdown().map_err(op("close journal"))?;
        Ok(WatchSummary {
            root_id,
            accepted_events: status.accepted_events,
            blocked_events: status.blocked_events,
            dropped_events: status.dropped_events,
            seconds: started.elapsed().as_secs(),
        })
    }

    /// Record a metadata-only snapshot of the repository containing `path`,
    /// and the history transition since the previous snapshot of it.
    pub fn git_snapshot(&self, path: &Path) -> Result<GitSnapshotSummary, LiveError> {
        let document = self.policy_document()?;
        let policy = PolicyProfile::from_document(&document).map_err(op("build policy"))?;
        let adapter =
            GitSnapshotAdapter::new("/usr/bin/git", policy.clone()).map_err(op("prepare Git"))?;
        let root = RootId::try_from(GIT_ROOT_ID).map_err(op("prepare Git"))?;
        let snapshot = adapter.snapshot(path, root).map_err(op("Git snapshot"))?;
        let state_dir = self.dir.join(GIT_STATE_DIR);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&state_dir)
            .map_err(op("store Git state"))?;
        let state_file = state_dir.join(format!("{}.json", snapshot.repository_id.as_str()));
        let previous = fs::read_to_string(&state_file)
            .ok()
            .and_then(|text| GitSnapshotMetadata::parse(&text).ok());
        let transition =
            match (&previous, &snapshot.head, previous.as_ref().and_then(|p| p.head.clone())) {
                (Some(previous), Some(current), Some(before)) => {
                    let probe = adapter
                        .probe_ancestry(path, &before, current)
                        .map_err(op("Git ancestry"))?;
                    Some(
                        GitHistoryTransition::classify(previous, &snapshot, probe)
                            .map_err(op("Git history"))?,
                    )
                }
                _ => None,
            };

        let journal = self.journal()?;
        let origin = IngestionOrigin::live("live-git").map_err(op("prepare Git event"))?;
        let writer =
            Writer::new(journal.clone(), WriterConfig::default()).map_err(op("prepare writer"))?;
        let branch_class = serde_json::to_value(snapshot.branch_class)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        let worktree_state = serde_json::to_value(snapshot.worktree_state)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        let changed = u64::from(snapshot.status.staged)
            + u64::from(snapshot.status.unstaged)
            + u64::from(snapshot.status.untracked)
            + u64::from(snapshot.status.conflicted);
        let mut events = Vec::new();
        if let Some(head) = &snapshot.head {
            events.push(
                EventEnvelope::new(
                    &origin,
                    Uuid::new_v4(),
                    Utc::now(),
                    Utc::now(),
                    EventSource::Git,
                    EventKind::GitSnapshot,
                    EventPayload::GitSnapshot(GitSnapshotPayload {
                        repository_id: snapshot.repository_id.clone(),
                        branch: crate::model::BranchName::try_from(branch_class.replace('_', "-"))
                            .map_err(op("record Git snapshot"))?,
                        head_oid: crate::model::GitObjectId::try_from(head.hex().to_owned())
                            .map_err(op("record Git snapshot"))?,
                        dirty: changed > 0,
                        changed_file_count: changed,
                        snapshot_digest: Some(snapshot.snapshot_digest.clone()),
                    }),
                    None,
                    policy.id.clone(),
                    policy.version,
                    Evidence::Direct,
                    None,
                )
                .map_err(op("record Git snapshot"))?,
            );
        }
        let gap_reason = transition.as_ref().and_then(|t| t.gap.as_ref()).map(|gap| {
            serde_json::to_value(gap.reason)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_owned())
        });
        if let Some(reason) = &gap_reason {
            events.push(
                EventEnvelope::new(
                    &origin,
                    Uuid::new_v4(),
                    Utc::now(),
                    Utc::now(),
                    EventSource::Git,
                    EventKind::Gap,
                    EventPayload::Gap(GapPayload {
                        source: EventSource::Git,
                        reason_code: ReasonCode::try_from(format!("git_{reason}")).map_err(op("record Git gap"))?,
                        dropped_count: 0,
                        from_cursor: None,
                        to_cursor: None,
                        volume_digest: None,
                        root_ids: vec![RootId::try_from(GIT_ROOT_ID).map_err(op("record Git gap"))?],
                        remediation: None,
                    }),
                    None,
                    policy.id.clone(),
                    policy.version,
                    Evidence::Unknown,
                    None,
                )
                .map_err(op("record Git gap"))?,
            );
        }
        for event in events {
            match writer
                .submit(origin.clone(), vec![event], policy.clone(), Vec::new())
                .map_err(op("write"))?
            {
                WriterOutcome::Committed(_) => {}
                WriterOutcome::Gap(_) => {
                    return Err(LiveError::Operation(
                        "write",
                        "journal refused the event".to_owned(),
                    ))
                }
            }
        }
        drop(writer);
        journal.shutdown().map_err(op("close journal"))?;
        let serialized = serde_json::to_string(&snapshot).map_err(|_| LiveError::Config)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&state_file)
            .map_err(op("store Git state"))?;
        file.write_all(serialized.as_bytes()).map_err(op("store Git state"))?;
        Ok(GitSnapshotSummary {
            repository_id: snapshot.repository_id.as_str().to_owned(),
            branch_class,
            worktree_state,
            changed,
            transition: transition.as_ref().map(|t| {
                serde_json::to_value(t.ref_movement)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default()
            }),
            gap: gap_reason,
        })
    }

    pub fn timeline(&self, limit: usize) -> Result<Vec<TimelineEntry>, LiveError> {
        let journal = self.journal()?;
        let events = journal.events().map_err(op("read journal"))?;
        // Gaps are listed as their own entries; whether a gap limits a
        // particular claim is decided by `explain` for that evidence chain.
        let gap_present = false;
        let start = events.len().saturating_sub(limit);
        let entries = events[start..]
            .iter()
            .map(|stored| {
                let event = &stored.event;
                let statement = render_claim(event, ClaimLocale::En, gap_present)
                    .map(|claim| claim.text)
                    .unwrap_or_else(|_| event.payload.summary());
                TimelineEntry {
                    event_id: event.event_id,
                    observed_at: event.observed_at,
                    source: event.source.to_string(),
                    kind: event.kind.to_string(),
                    evidence: event.evidence,
                    statement,
                }
            })
            .collect();
        journal.shutdown().map_err(op("close journal"))?;
        Ok(entries)
    }

    pub fn status(&self) -> Result<LiveStatus, LiveError> {
        let journal = self.journal()?;
        let events = journal.events().map_err(op("read journal"))?;
        let mut by_source = BTreeMap::new();
        for stored in &events {
            *by_source.entry(stored.event.source.to_string()).or_insert(0) += 1;
        }
        let status = LiveStatus {
            custody: self.config.custody,
            events: events.len(),
            gaps: events.iter().filter(|stored| stored.event.kind == EventKind::Gap).count(),
            by_source,
            watched_roots: self.config.watched_roots.len(),
            shell_consent: self.shell_consent_granted(),
            last_event_at: events.iter().map(|stored| stored.event.observed_at).max(),
        };
        journal.shutdown().map_err(op("close journal"))?;
        Ok(status)
    }

    /// Delete the journal key from the login keychain and remove the home.
    /// The journal cannot be decrypted afterwards; this is the reversal of
    /// `init` and cannot be undone.
    pub fn forget(self) -> Result<(), LiveError> {
        match self.provider()?.delete() {
            Ok(()) => {}
            Err(error) if error.to_string().contains("missing") => {}
            Err(error) => return Err(LiveError::Operation("delete key", error.to_string())),
        }
        fs::remove_dir_all(&self.dir).map_err(op("remove home"))
    }

    /// Write the offline HTML timeline to `output`, which must not exist and
    /// must be outside the home. The file is created 0600 beside `output`
    /// and renamed into place. Returns the number of events rendered.
    pub fn write_report(&self, output: &Path) -> Result<usize, LiveError> {
        let parent = match output.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        let parent = fs::canonicalize(parent).map_err(op("resolve report folder"))?;
        if parent.starts_with(&self.dir) {
            return Err(LiveError::Operation(
                "report",
                "the report must be written outside the GHOSTRACE home".to_owned(),
            ));
        }
        if output.exists() {
            return Err(LiveError::Operation(
                "report",
                "the destination already exists".to_owned(),
            ));
        }
        let journal = self.journal()?;
        let events = journal
            .events()
            .map_err(op("read journal"))?
            .into_iter()
            .map(|stored| stored.event)
            .collect::<Vec<_>>();
        journal.shutdown().map_err(op("close journal"))?;
        let html = crate::report::render_timeline_html(&events, Utc::now());
        let mut temporary = tempfile::Builder::new()
            .prefix(".ghostrace-report-")
            .tempfile_in(&parent)
            .map_err(op("write report"))?;
        temporary.write_all(html.as_bytes()).map_err(op("write report"))?;
        temporary.as_file().sync_all().map_err(op("write report"))?;
        temporary.persist_noclobber(output).map_err(|error| op("write report")(error.error))?;
        Ok(events.len())
    }

    pub fn explain(&self, event: Uuid) -> Result<String, LiveError> {
        let journal = self.journal()?;
        let explanation = crate::explain::explain(&journal, event).map_err(op("explain"))?;
        let json = explanation.to_pretty_json().map_err(op("explain"))?;
        journal.shutdown().map_err(op("close journal"))?;
        Ok(json)
    }
}
