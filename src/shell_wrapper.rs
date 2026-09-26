//! Explicit, consent-gated shell run wrapper.
//!
//! The wrapper records metadata only for a command the user deliberately runs
//! through it. It retains a normalized executable basename token, a
//! working-directory class plus anchored digest, timing, and the outcome. The
//! child inherits the caller's terminal and environment so the command behaves
//! normally, but arguments, environment values, standard input, and output are
//! never read, stored, or hashed.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    consent::{ConsentConfirmation, ConsentReceipt, ConsentState, ConsentStateMachine},
    error::GhostraceError,
    journal::Journal,
    model::{
        EventEnvelope, EventKind, EventPayload, EventSource, Evidence, GapPayload, IngestionOrigin,
        PathClass, PathDigest, ReasonCode, RootId, SessionId, ShellFinishedPayload, ShellKind,
        ShellStartedPayload, ShellStatus,
    },
    policy::{PolicyDocument, PolicyProfile},
    shell_metadata::{
        ShellExecutableId, ShellExecutionMetadata, ShellWorkingDirectory, MAX_SHELL_SIGNAL,
    },
    writer::{Writer, WriterConfig, WriterOutcome},
};

/// `shell_kind` recorded for sessions opened by this wrapper.
pub const SHELL_WRAPPER_KIND: &str = "ghostrace-run";
/// Executable token used when a basename is not a safe opaque token.
pub const UNCLASSIFIED_EXECUTABLE_ID: &str = "unclassified";
/// Gap reason when the executable could not be started.
pub const SHELL_EXEC_FAILED_REASON: &str = "shell_exec_failed";
/// Gap reason when the wrapper lost the child before observing its status.
pub const SHELL_WAIT_FAILED_REASON: &str = "shell_wait_failed";

const CWD_DIGEST_DOMAIN: &[u8] = b"ghostrace-shell-cwd-digest-v1\0";
const EXEC_FAILED_EXIT_CODE: i32 = 127;

/// A selected workspace root the working directory may be classified against.
#[derive(Clone, Debug)]
pub struct ShellWorkspaceRoot {
    pub id: RootId,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ShellWrapperConfig {
    pub writer: WriterConfig,
    /// Live collector instance; must start with `live-`.
    pub collector_instance: String,
    pub consent_at: DateTime<Utc>,
    pub actor: String,
    pub reason: String,
    /// Optional selected root; it must be selected and not excluded by policy.
    pub workspace: Option<ShellWorkspaceRoot>,
    /// Home directory used for the `home_relative` class.
    pub home: Option<PathBuf>,
}

/// Terminal evidence for one wrapped run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellRunEvidence {
    Completed(ShellExecutionMetadata),
    /// The run has no observed end. No end time or status is fabricated.
    Gap {
        reason_code: &'static str,
    },
}

#[derive(Clone, Debug)]
pub struct ShellRunReport {
    pub session_id: SessionId,
    pub started_event_id: Uuid,
    pub terminal_event_id: Uuid,
    pub evidence: ShellRunEvidence,
    /// Exit code the wrapper should return: the child's code, `128 + signal`
    /// for a signaled child, or 127 when the executable could not be started.
    pub exit_code: i32,
}

pub struct ShellWrapper {
    policy: PolicyProfile,
    consent: ConsentStateMachine,
    consent_receipt: ConsentReceipt,
    writer: Writer,
    origin: IngestionOrigin,
    workspace: Option<AnchoredScope>,
    home: Option<AnchoredScope>,
}

#[derive(Clone, Debug)]
struct AnchoredScope {
    scope_id: String,
    path: PathBuf,
    anchor: FileAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileAnchor {
    device: u64,
    inode: u64,
}

impl ShellWrapper {
    /// Build a wrapper only after the caller rendered and consumed a consent
    /// preview for a policy that enables the shell source. Construction does
    /// not run anything.
    pub fn new(
        confirmation: ConsentConfirmation,
        document: PolicyDocument,
        journal: Journal,
        config: ShellWrapperConfig,
    ) -> Result<Self, GhostraceError> {
        let policy = PolicyProfile::from_document(&document)?;
        if !policy.is_source_enabled(EventSource::Shell) {
            return Err(wrapper_error("policy does not enable the shell source"));
        }
        let workspace = match config.workspace {
            Some(root) => {
                if !policy.selected_roots.contains(root.id.as_str())
                    || policy.excluded_roots.contains(root.id.as_str())
                {
                    return Err(wrapper_error("workspace root is not selected by policy"));
                }
                Some(AnchoredScope::new(format!("workspace:{}", root.id.as_str()), &root.path)?)
            }
            None => None,
        };
        let home = config
            .home
            .as_deref()
            .and_then(|path| AnchoredScope::new("home".to_owned(), path).ok());

        let mut consent = ConsentStateMachine::new();
        let consent_receipt = consent.grant_preview(
            confirmation,
            config.consent_at,
            &config.actor,
            &config.reason,
        )?;
        if consent_receipt.policy_id.as_str() != document.id
            || consent_receipt.policy_version != document.version
            || consent_receipt.scope_digest != document.scope_digest()?
        {
            return Err(wrapper_error("consent does not match the policy document"));
        }
        let origin = IngestionOrigin::live(config.collector_instance)?;
        let writer = Writer::new(journal, config.writer)?;
        Ok(Self { policy, consent, consent_receipt, writer, origin, workspace, home })
    }

    pub fn consent_receipt(&self) -> &ConsentReceipt {
        &self.consent_receipt
    }

    pub fn consent_state(&self) -> ConsentState {
        self.consent.state()
    }

    /// Revoke consent. Later runs are refused before anything is spawned.
    pub fn revoke(
        &mut self,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        self.consent.revoke(occurred_at, actor, reason)
    }

    /// Run `program` with `args` in the process's current directory.
    pub fn run<I, S>(&mut self, program: &OsStr, args: I) -> Result<ShellRunReport, GhostraceError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cwd = std::env::current_dir()
            .map_err(|_| wrapper_error("current directory is unavailable"))?;
        self.run_in(&cwd, program, args)
    }

    /// Run `program` with `args` in `cwd`. Arguments are passed to the child
    /// and never retained.
    pub fn run_in<I, S>(
        &mut self,
        cwd: &Path,
        program: &OsStr,
        args: I,
    ) -> Result<ShellRunReport, GhostraceError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        if !self.consent.is_capture_allowed() {
            return Err(GhostraceError::PolicyDenied { reason: "consent_not_active".to_owned() });
        }
        let executable_id = normalize_executable(program);
        let working_directory = self.classify_working_directory(cwd);
        let session_id = SessionId::try_from(format!("shell-{}", Uuid::new_v4().simple()))?;

        let started_at = Utc::now();
        let started_event_id = Uuid::new_v4();
        let started = EventPayload::ShellStarted(ShellStartedPayload {
            session_id: session_id.clone(),
            shell_kind: ShellKind::try_from(SHELL_WRAPPER_KIND)?,
            executable_id: Some(executable_id.clone()),
            working_directory: Some(working_directory.clone()),
        });
        self.commit(started_event_id, started_at, started, None)?;

        let mut command = Command::new(program);
        command.args(args).current_dir(cwd);
        let status = match command.spawn() {
            Ok(mut child) => {
                // The terminal delivers SIGINT/SIGQUIT to the whole foreground
                // group; the wrapper must survive them to record the outcome.
                let _guard = InterruptGuard::install();
                child.wait()
            }
            Err(error) => Err(error),
        };
        let ended_at = Utc::now();

        let (terminal_event_id, evidence, exit_code) = match status {
            Ok(status) => {
                let (status_class, code, signal) = classify(status);
                let metadata = ShellExecutionMetadata::new(
                    session_id.clone(),
                    executable_id,
                    working_directory,
                    started_at,
                    ended_at,
                    status_class,
                    code,
                    signal,
                )?;
                let duration_ms =
                    u64::try_from(ended_at.signed_duration_since(started_at).num_milliseconds())
                        .unwrap_or(0);
                let event_id = Uuid::new_v4();
                self.commit(
                    event_id,
                    ended_at,
                    EventPayload::ShellFinished(ShellFinishedPayload {
                        session_id: session_id.clone(),
                        status: status_class,
                        exit_code: code,
                        duration_ms,
                        signal,
                    }),
                    Some(started_event_id),
                )?;
                let exit_code = match (code, signal) {
                    (Some(code), _) => code,
                    (None, Some(signal)) => 128 + i32::from(signal),
                    (None, None) => 1,
                };
                (event_id, ShellRunEvidence::Completed(metadata), exit_code)
            }
            Err(error) => {
                let spawn_failed = error.kind() != std::io::ErrorKind::Interrupted;
                let reason_code =
                    if spawn_failed { SHELL_EXEC_FAILED_REASON } else { SHELL_WAIT_FAILED_REASON };
                let event_id = Uuid::new_v4();
                self.commit(
                    event_id,
                    ended_at,
                    EventPayload::Gap(GapPayload {
                        source: EventSource::Shell,
                        reason_code: ReasonCode::try_from(reason_code)?,
                        dropped_count: 0,
                        from_cursor: None,
                        to_cursor: None,
                        volume_digest: None,
                        root_ids: Vec::new(),
                        remediation: None,
                    }),
                    Some(started_event_id),
                )?;
                (event_id, ShellRunEvidence::Gap { reason_code }, EXEC_FAILED_EXIT_CODE)
            }
        };

        Ok(ShellRunReport { session_id, started_event_id, terminal_event_id, evidence, exit_code })
    }

    fn commit(
        &self,
        event_id: Uuid,
        observed_at: DateTime<Utc>,
        payload: EventPayload,
        parent_event_id: Option<Uuid>,
    ) -> Result<(), GhostraceError> {
        let kind = payload.kind();
        let evidence = if kind == EventKind::Gap { Evidence::Unknown } else { Evidence::Direct };
        let event = EventEnvelope::new(
            &self.origin,
            event_id,
            observed_at,
            Utc::now(),
            EventSource::Shell,
            kind,
            payload,
            None,
            self.policy.id.clone(),
            self.policy.version,
            evidence,
            parent_event_id,
        )?;
        match self.writer.submit(
            self.origin.clone(),
            vec![event],
            self.policy.clone(),
            Vec::new(),
        )? {
            WriterOutcome::Committed(_) => Ok(()),
            WriterOutcome::Gap(_) => Err(wrapper_error("journal writer did not admit the event")),
        }
    }

    fn classify_working_directory(&self, cwd: &Path) -> ShellWorkingDirectory {
        let canonical = match std::fs::canonicalize(cwd) {
            Ok(path) => path,
            Err(_) => return unknown_working_directory(),
        };
        for scope in [self.workspace.as_ref(), self.home.as_ref()].into_iter().flatten() {
            if let Ok(relative) = canonical.strip_prefix(&scope.path) {
                let class = if scope.scope_id == "home" {
                    PathClass::HomeRelative
                } else {
                    PathClass::WorkspaceRelative
                };
                return ShellWorkingDirectory::new(
                    class,
                    anchored_digest(&scope.scope_id, scope.anchor, relative.as_os_str()),
                );
            }
        }
        // Outside every known scope: digest only the directory's own
        // filesystem identity so the digest cannot be matched against a
        // dictionary of path strings.
        match FileAnchor::of(&canonical) {
            Ok(anchor) => ShellWorkingDirectory::new(
                PathClass::AbsoluteRedacted,
                anchored_digest("absolute", anchor, OsStr::new("")),
            ),
            Err(_) => unknown_working_directory(),
        }
    }
}

impl AnchoredScope {
    fn new(scope_id: String, path: &Path) -> Result<Self, GhostraceError> {
        let path = std::fs::canonicalize(path)
            .map_err(|_| wrapper_error("scope directory cannot be resolved"))?;
        if !path.is_dir() {
            return Err(wrapper_error("scope is not a directory"));
        }
        let anchor = FileAnchor::of(&path)?;
        Ok(Self { scope_id, path, anchor })
    }
}

impl FileAnchor {
    #[cfg(unix)]
    fn of(path: &Path) -> Result<Self, GhostraceError> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path)
            .map_err(|_| wrapper_error("directory identity is unavailable"))?;
        Ok(Self { device: metadata.dev(), inode: metadata.ino() })
    }

    #[cfg(not(unix))]
    fn of(_path: &Path) -> Result<Self, GhostraceError> {
        Err(wrapper_error("directory identity is unsupported on this platform"))
    }
}

/// Normalize the program to its lowercase basename when that is a safe opaque
/// token; otherwise record the fixed unclassified token.
pub fn normalize_executable(program: &OsStr) -> ShellExecutableId {
    Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .and_then(|name| ShellExecutableId::new(name).ok())
        .unwrap_or_else(|| {
            ShellExecutableId::new(UNCLASSIFIED_EXECUTABLE_ID).expect("static token is valid")
        })
}

fn anchored_digest(scope_id: &str, anchor: FileAnchor, relative: &OsStr) -> PathDigest {
    let mut hasher = Sha256::new();
    hasher.update(CWD_DIGEST_DOMAIN);
    hasher.update(scope_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(anchor.device.to_le_bytes());
    hasher.update(anchor.inode.to_le_bytes());
    hasher.update(os_bytes(relative));
    let encoded = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    PathDigest::try_from(format!("sha256:{encoded}")).expect("sha256 digest is valid")
}

fn unknown_working_directory() -> ShellWorkingDirectory {
    let mut hasher = Sha256::new();
    hasher.update(CWD_DIGEST_DOMAIN);
    hasher.update(b"unknown");
    let encoded = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    ShellWorkingDirectory::new(
        PathClass::Unknown,
        PathDigest::try_from(format!("sha256:{encoded}")).expect("sha256 digest is valid"),
    )
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> Vec<u8> {
    value.to_string_lossy().as_bytes().to_vec()
}

fn classify(status: ExitStatus) -> (ShellStatus, Option<i32>, Option<u8>) {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status
            .signal()
            .and_then(|signal| u8::try_from(signal).ok())
            .filter(|signal| *signal > 0 && *signal <= MAX_SHELL_SIGNAL)
    };
    #[cfg(not(unix))]
    let signal: Option<u8> = None;
    match (status.code(), signal) {
        (Some(0), _) => (ShellStatus::Succeeded, Some(0), None),
        (Some(code), _) => (ShellStatus::Failed, Some(code), None),
        (None, Some(signal)) => (ShellStatus::Signaled, None, Some(signal)),
        (None, None) => (ShellStatus::Unknown, None, None),
    }
}

/// Ignores SIGINT and SIGQUIT in the wrapper for its lifetime. It is installed
/// after the child is spawned so the child keeps default dispositions.
struct InterruptGuard {
    #[cfg(unix)]
    previous: [libc::sighandler_t; 2],
}

impl InterruptGuard {
    fn install() -> Self {
        #[cfg(unix)]
        {
            // SAFETY: `signal` with SIG_IGN installs no Rust code as a handler.
            let previous = unsafe {
                [
                    libc::signal(libc::SIGINT, libc::SIG_IGN),
                    libc::signal(libc::SIGQUIT, libc::SIG_IGN),
                ]
            };
            Self { previous }
        }
        #[cfg(not(unix))]
        Self {}
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        // SAFETY: restores the dispositions returned by `install`.
        unsafe {
            libc::signal(libc::SIGINT, self.previous[0]);
            libc::signal(libc::SIGQUIT, self.previous[1]);
        }
    }
}

fn wrapper_error(message: &str) -> GhostraceError {
    GhostraceError::InvalidEvent(format!("shell wrapper: {message}"))
}
