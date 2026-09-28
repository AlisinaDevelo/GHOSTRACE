use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use clap::{Parser, Subcommand};
use ghostrace::{
    capture, checked_in_profile, explain, export_journal_with_confirmation,
    fixture::ingest_fixture, journal::Journal, policy::PolicyProfile, preview_export,
    validate_export, DeterministicKeyProvider, EventEnvelope, EventKind, EventPayload, EventSource,
    Evidence, ExportRequest, GhostraceError, IngestionOrigin, ReasonCode, RepairInterval,
    RetentionConfirmation, RetentionPolicy, RootId, SnapshotDigest, EVENT_SCHEMA_JSON,
    PARQUET_ARCHIVE_PROFILE_JSON, SHELL_METADATA_SCHEMA_JSON,
};
use uuid::Uuid;

const FIXTURE_CLI_KEY_SEED: &str = "fixture-cli-v1";

#[derive(Debug, Parser)]
#[command(name = "ghostrace", version, about = "Local macOS event provenance journal")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run one command and record its metadata (never its arguments, output, or environment).
    Run {
        /// GHOSTRACE home (default: ~/Library/Application Support/GHOSTRACE).
        #[arg(long)]
        home: Option<PathBuf>,
        /// The command and its arguments, after `--`.
        #[arg(trailing_var_arg = true, required = true, allow_hyphen_values = true)]
        command: Vec<std::ffi::OsString>,
    },
    /// Live journal on this Mac: init, status, timeline, explain, watch, git-snapshot.
    Live {
        #[command(subcommand)]
        command: LiveCommand,
    },
    /// Create or open the durable fixture-only journal.
    Init {
        #[arg(long)]
        journal: PathBuf,
    },
    /// Ingest a checked-in JSONL fixture into a durable fixture-only journal.
    Ingest {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        fixture: PathBuf,
    },
    /// Explain one event from a durable fixture-only journal.
    Explain {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        event: Uuid,
    },
    /// Ingest a checked-in JSONL fixture in memory and explain one event.
    Demo {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        event: Uuid,
    },
    /// Preview the exact query, policy, fields, snapshot, and destination class
    /// that a plaintext export would disclose.
    Preview {
        #[arg(long, conflicts_with = "journal", required_unless_present = "journal")]
        fixture: Option<PathBuf>,
        #[arg(long, conflicts_with = "fixture", required_unless_present = "fixture")]
        journal: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Ingest a fixture and stream a versioned JSONL export.
    Export {
        #[arg(long, conflicts_with = "journal", required_unless_present = "journal")]
        fixture: Option<PathBuf>,
        #[arg(long, conflicts_with = "fixture", required_unless_present = "fixture")]
        journal: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Plan digest printed by the matching `preview` command.
        #[arg(long)]
        confirm_plan: Option<String>,
        /// Journal snapshot digest printed by the matching `preview` command.
        #[arg(long)]
        confirm_snapshot: Option<String>,
    },
    /// Print a deterministic, read-only retention plan for the journal.
    RetentionPlan {
        #[arg(long)]
        journal: PathBuf,
        /// RFC3339 cutoff; observations before it are selected. Without this
        /// flag and without a size/count limit, the documented 90-day default
        /// is anchored at the current UTC time.
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        root_id: Option<String>,
        #[arg(long)]
        retain_at_most_events: Option<u64>,
        #[arg(long)]
        retain_at_most_bytes: Option<u64>,
    },
    /// Apply one previously previewed retention scope as a transactional
    /// logical deletion. Compaction and external-copy handling remain separate.
    RetentionDelete {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        root_id: Option<String>,
        #[arg(long)]
        retain_at_most_events: Option<u64>,
        #[arg(long)]
        retain_at_most_bytes: Option<u64>,
        /// Plan digest printed by the matching `retention-plan` command.
        #[arg(long)]
        confirm_plan: String,
        /// Candidate-set digest printed by the matching `retention-plan` command.
        #[arg(long)]
        confirm_candidate_set: String,
        /// Snapshot boundary printed by the matching `retention-plan` command.
        #[arg(long)]
        confirm_snapshot_boundary: u64,
    },
    /// Print a read-only, path-free inventory of retention residue classes.
    ResidueReport {
        #[arg(long)]
        journal: PathBuf,
        /// Known external backup files; their paths are aggregated and never
        /// printed in the report.
        #[arg(long = "backup")]
        backups: Vec<PathBuf>,
    },
    /// Run bounded SQLite integrity and foreign-key checks without repair.
    IntegrityCheck {
        #[arg(long)]
        journal: PathBuf,
    },
    /// Verify keyed event, cursor, policy, diagnostic, and deletion state.
    AuthenticatedCheck {
        #[arg(long)]
        journal: PathBuf,
    },
    /// Create and print a signed, path-free verification checkpoint.
    Checkpoint {
        #[arg(long)]
        journal: PathBuf,
    },
    /// Repair bounded ingest intervals on a verified database copy.
    Repair {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        /// Inclusive interval in source:start:end form; repeat for multiple
        /// intervals. The source must not be fixture.
        #[arg(long = "interval", required = true)]
        intervals: Vec<String>,
    },
    /// Run the bounded checkpoint/repair MVP against two synthetic,
    /// unreferenced filesystem gaps and print the path-free manifest.
    RecoveryDemo,
    /// Validate a JSONL export before consuming its records.
    Validate {
        #[arg(long)]
        export: PathBuf,
    },
    /// Print the checked-in event envelope JSON Schema.
    Schema,
    /// Print the checked-in strict v1 profile for a Parquet-derived archive.
    ParquetProfile,
    /// Write a plaintext Parquet cold archive from a JSONL export. Needs a
    /// build with `--features parquet` and an explicit `--yes`.
    Archive {
        #[arg(long)]
        export: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Confirm that the archive is an unencrypted copy.
        #[arg(long)]
        yes: bool,
    },
    /// Check a Parquet archive against its footer and its source JSONL export.
    VerifyArchive {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        export: PathBuf,
    },
    /// Print the strict v1 metadata schema for a future explicit shell wrapper.
    ShellSchema,
    /// Live capture is intentionally unavailable in this vertical slice.
    Capture,
}

#[derive(Debug, Subcommand)]
enum LiveCommand {
    /// Create a private GHOSTRACE home with its key in your login keychain.
    Init {
        #[arg(long)]
        home: Option<PathBuf>,
    },
    /// Show key custody, event counts by source, and gaps.
    Status {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// List recent events as evidence-labelled statements.
    Timeline {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long, default_value_t = 30)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Explain one event with its cited evidence and coverage warnings.
    Explain {
        #[arg(long)]
        home: Option<PathBuf>,
        event: Uuid,
    },
    /// Watch one folder you choose, after confirming what is recorded.
    Watch {
        folder: PathBuf,
        #[arg(long)]
        home: Option<PathBuf>,
        /// Stop after this many seconds (default: until Ctrl-C).
        #[arg(long)]
        seconds: Option<u64>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Delete the journal key and the home. The journal becomes unreadable.
    Forget {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
    /// Record a metadata-only snapshot of a Git repository.
    GitSnapshot {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        home: Option<PathBuf>,
    },
}

#[cfg(feature = "parquet")]
fn archive(export: PathBuf, output: PathBuf, yes: bool) -> Result<(), GhostraceError> {
    use ghostrace::parquet_archive::{write_parquet_archive, PARQUET_ARCHIVE_PLAINTEXT_WARNING};
    eprintln!("{PARQUET_ARCHIVE_PLAINTEXT_WARNING}");
    if !yes {
        return Err(GhostraceError::ArchiveInvalid(
            "rerun with --yes to write the unencrypted archive".to_owned(),
        ));
    }
    let receipt = write_parquet_archive(export, output)?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

#[cfg(feature = "parquet")]
fn verify_archive(archive: PathBuf, export: PathBuf) -> Result<(), GhostraceError> {
    let receipt = ghostrace::parquet_archive::verify_parquet_archive(archive, export)?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

#[cfg(not(feature = "parquet"))]
fn archive(_: PathBuf, _: PathBuf, _: bool) -> Result<(), GhostraceError> {
    Err(parquet_unavailable())
}

#[cfg(not(feature = "parquet"))]
fn verify_archive(_: PathBuf, _: PathBuf) -> Result<(), GhostraceError> {
    Err(parquet_unavailable())
}

#[cfg(not(feature = "parquet"))]
fn parquet_unavailable() -> GhostraceError {
    GhostraceError::ArchiveInvalid(
        "this build has no Parquet writer; rebuild with --features parquet".to_owned(),
    )
}

fn run(cli: Cli) -> Result<(), GhostraceError> {
    match cli.command {
        Command::Run { home, command } => live::run(home, command),
        Command::Live { command } => live::dispatch(command),
        Command::Init { journal } => {
            let journal = open_fixture_journal(journal)?;
            journal.initialize_authenticated_state()?;
            journal.shutdown()?;
            println!("initialized fixture journal");
            Ok(())
        }
        Command::Ingest { journal, fixture } => {
            let journal = open_fixture_journal(journal)?;
            let report = ingest_fixture(fixture, &journal, &fixture_policy())?;
            journal.shutdown()?;
            println!("ingested {} event(s)", report.event_ids.len());
            Ok(())
        }
        Command::Explain { journal, event } => {
            let journal = open_fixture_journal(journal)?;
            let explanation = explain(&journal, event)?;
            println!("{}", explanation.to_pretty_json()?);
            journal.shutdown()?;
            Ok(())
        }
        Command::Demo { fixture, event } => {
            let journal =
                Journal::in_memory(DeterministicKeyProvider::from_seed("fixture-demo-v1"))?;
            let policy = fixture_policy();
            ingest_fixture(fixture, &journal, &policy)?;
            let explanation = explain(&journal, event)?;
            println!("{}", explanation.to_pretty_json()?);
            Ok(())
        }
        Command::Preview { fixture, journal, output, force } => {
            let (journal, policy, durable) = open_export_input(fixture, journal)?;
            let request = ExportRequest { force, ..ExportRequest::default() };
            let preview = preview_export(&journal, &policy, &request, &output)?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
            if durable {
                journal.shutdown()?;
            }
            Ok(())
        }
        Command::Export { fixture, journal, output, force, confirm_plan, confirm_snapshot } => {
            let (journal, policy, durable) = open_export_input(fixture, journal)?;
            let request = ExportRequest { force, ..ExportRequest::default() };
            let preview = preview_export(&journal, &policy, &request, &output)?;
            let Some(confirm_plan) = confirm_plan else {
                return Err(GhostraceError::ExportConfirmationRequired);
            };
            let Some(confirm_snapshot) = confirm_snapshot else {
                return Err(GhostraceError::ExportConfirmationRequired);
            };
            if preview.plan_digest().as_str() != confirm_plan
                || preview.snapshot_digest().as_str() != confirm_snapshot
            {
                return Err(GhostraceError::ExportConfirmationMismatch);
            }
            let result =
                export_journal_with_confirmation(&journal, &output, preview.confirm(), &policy)?;
            if durable {
                journal.shutdown()?;
            }
            println!(
                "exported {} event(s); plan {}; manifest {}; destination {:?}",
                result.manifest.coverage.event_count,
                result.receipt.plan_digest,
                result.receipt.manifest_digest,
                result.receipt.destination_class,
            );
            Ok(())
        }
        Command::RetentionPlan {
            journal,
            before,
            source,
            root_id,
            retain_at_most_events,
            retain_at_most_bytes,
        } => {
            let mut policy = if let Some(before) = before {
                RetentionPolicy::before(parse_timestamp(&before)?)
            } else if retain_at_most_events.is_some() || retain_at_most_bytes.is_some() {
                RetentionPolicy::default()
            } else {
                RetentionPolicy::default_at(Utc::now())
            };
            policy.source = source.map(|value| parse_source(&value)).transpose()?;
            policy.root_id = root_id.map(RootId::try_from).transpose().map_err(|_| {
                GhostraceError::RetentionPolicyInvalid("root ID is invalid".to_owned())
            })?;
            policy.retain_at_most_events = retain_at_most_events;
            policy.retain_at_most_bytes = retain_at_most_bytes;
            let journal = open_fixture_journal(journal)?;
            let plan = journal.retention_plan(&policy)?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::RetentionDelete {
            journal,
            before,
            source,
            root_id,
            retain_at_most_events,
            retain_at_most_bytes,
            confirm_plan,
            confirm_candidate_set,
            confirm_snapshot_boundary,
        } => {
            let mut policy = if let Some(before) = before {
                RetentionPolicy::before(parse_timestamp(&before)?)
            } else if retain_at_most_events.is_some() || retain_at_most_bytes.is_some() {
                RetentionPolicy::default()
            } else {
                RetentionPolicy::default_at(Utc::now())
            };
            policy.source = source.map(|value| parse_source(&value)).transpose()?;
            policy.root_id = root_id.map(RootId::try_from).transpose().map_err(|_| {
                GhostraceError::RetentionPolicyInvalid("root ID is invalid".to_owned())
            })?;
            policy.retain_at_most_events = retain_at_most_events;
            policy.retain_at_most_bytes = retain_at_most_bytes;
            let journal = open_fixture_journal(journal)?;
            let plan = journal.retention_plan(&policy)?;
            let confirmation = RetentionConfirmation {
                schema_version: plan.schema_version,
                plan_digest: SnapshotDigest::try_from(confirm_plan)
                    .map_err(|_| GhostraceError::RetentionConfirmationMismatch)?,
                candidate_set_digest: SnapshotDigest::try_from(confirm_candidate_set)
                    .map_err(|_| GhostraceError::RetentionConfirmationMismatch)?,
                snapshot_boundary: confirm_snapshot_boundary,
                confirmed: true,
            };
            let receipt = journal.delete_retention(&plan, &confirmation)?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::ResidueReport { journal, backups } => {
            let journal = open_fixture_journal(journal)?;
            let report = journal.residue_report(&backups)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::IntegrityCheck { journal } => {
            let journal = open_fixture_journal(journal)?;
            let report = journal.integrity_check()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            journal.shutdown()?;
            if report.integrity_ok {
                Ok(())
            } else {
                Err(GhostraceError::IntegrityReportInvalid(
                    "integrity check failed; follow the recovery guidance".to_owned(),
                ))
            }
        }
        Command::AuthenticatedCheck { journal } => {
            let journal = open_fixture_journal(journal)?;
            let report = journal.authenticated_state_report()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            journal.shutdown()?;
            if report.valid {
                Ok(())
            } else {
                Err(GhostraceError::AuthenticatedStateInvalid(report.message))
            }
        }
        Command::Checkpoint { journal } => {
            let journal = open_fixture_journal(journal)?;
            let checkpoint = journal.create_checkpoint()?;
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::Repair { journal, destination, intervals } => {
            let journal = open_fixture_journal(journal)?;
            let intervals = intervals
                .iter()
                .map(|value| parse_repair_interval(value))
                .collect::<Result<Vec<_>, _>>()?;
            let manifest = journal.repair_verified_copy(destination, &intervals)?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::RecoveryDemo => {
            let directory = tempfile::tempdir()
                .map_err(|source| GhostraceError::Io { path: std::env::temp_dir(), source })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                    .map_err(|source| GhostraceError::Io {
                        path: directory.path().to_path_buf(),
                        source,
                    })?;
            }
            let source = directory.path().join("recovery-demo.sqlite3");
            let destination = directory.path().join("recovery-demo-repaired.sqlite3");
            let journal = Journal::open_fixture(
                &source,
                DeterministicKeyProvider::from_seed("recovery-demo-v1"),
            )?;
            let policy = {
                let mut policy = PolicyProfile::deny_by_default("recovery-demo-policy");
                policy.enable_source(EventSource::Filesystem);
                policy
            };
            let origin = IngestionOrigin::fixture_instance("fixture-recovery-demo")
                .map_err(|_| GhostraceError::FixtureProvenance)?;
            for number in 1_u128..=2 {
                let timestamp = Utc
                    .timestamp_opt(1_735_700_100 + number as i64, 0)
                    .single()
                    .expect("fixed demo timestamp");
                let event = EventEnvelope::new(
                    &origin,
                    Uuid::from_u128(number + 100),
                    timestamp,
                    timestamp,
                    EventSource::Filesystem,
                    EventKind::Gap,
                    EventPayload::Gap(ghostrace::GapPayload {
                        source: EventSource::Filesystem,
                        reason_code: ReasonCode::try_from("recovery_demo").expect("reason"),
                        dropped_count: number as u64,
                        from_cursor: None,
                        to_cursor: None,
                        volume_digest: None,
                        root_ids: Vec::new(),
                        remediation: None,
                    }),
                    None,
                    policy.id.clone(),
                    policy.version,
                    Evidence::Direct,
                    None,
                )?;
                journal.ingest(&origin, &event, &policy)?;
            }
            let interval = RepairInterval::new(EventSource::Filesystem, 1, 1)?;
            let manifest = journal.repair_verified_copy(&destination, &[interval])?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
            journal.shutdown()?;
            Ok(())
        }
        Command::Archive { export, output, yes } => archive(export, output, yes),
        Command::VerifyArchive { archive, export } => verify_archive(archive, export),
        Command::Validate { export } => {
            let validated = validate_export(export)?;
            println!("validated {} event(s)", validated.event_count);
            Ok(())
        }
        Command::Schema => {
            println!("{EVENT_SCHEMA_JSON}");
            Ok(())
        }
        Command::ParquetProfile => {
            checked_in_profile()?;
            println!("{PARQUET_ARCHIVE_PROFILE_JSON}");
            Ok(())
        }
        Command::ShellSchema => {
            ghostrace::checked_in_shell_metadata()?;
            println!("{SHELL_METADATA_SCHEMA_JSON}");
            Ok(())
        }
        Command::Capture => capture(),
    }
}

fn fixture_policy() -> PolicyProfile {
    PolicyProfile::fixture_default()
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, GhostraceError> {
    value.parse::<DateTime<Utc>>().map_err(|_| {
        GhostraceError::RetentionPolicyInvalid("timestamp must be RFC3339 UTC".to_owned())
    })
}

fn parse_source(value: &str) -> Result<ghostrace::EventSource, GhostraceError> {
    match value {
        "filesystem" => Ok(ghostrace::EventSource::Filesystem),
        "frontmost_app" => Ok(ghostrace::EventSource::FrontmostApp),
        "shell" => Ok(ghostrace::EventSource::Shell),
        "git" => Ok(ghostrace::EventSource::Git),
        "browser" => Ok(ghostrace::EventSource::Browser),
        "lifecycle" => Ok(ghostrace::EventSource::Lifecycle),
        "fixture" => Ok(ghostrace::EventSource::Fixture),
        _ => Err(GhostraceError::RetentionPolicyInvalid(
            "source must be filesystem, frontmost_app, shell, git, browser, lifecycle, or fixture"
                .to_owned(),
        )),
    }
}

fn parse_repair_interval(value: &str) -> Result<RepairInterval, GhostraceError> {
    let mut parts = value.split(':');
    let source = parts.next().ok_or_else(|| {
        GhostraceError::RepairIntervalInvalid("interval requires source:start:end".to_owned())
    })?;
    let start = parts
        .next()
        .ok_or_else(|| {
            GhostraceError::RepairIntervalInvalid("interval start is missing".to_owned())
        })?
        .parse::<u64>()
        .map_err(|_| {
            GhostraceError::RepairIntervalInvalid("interval start is invalid".to_owned())
        })?;
    let end = parts
        .next()
        .ok_or_else(|| GhostraceError::RepairIntervalInvalid("interval end is missing".to_owned()))?
        .parse::<u64>()
        .map_err(|_| GhostraceError::RepairIntervalInvalid("interval end is invalid".to_owned()))?;
    if parts.next().is_some() {
        return Err(GhostraceError::RepairIntervalInvalid(
            "interval requires source:start:end".to_owned(),
        ));
    }
    RepairInterval::new(
        parse_source(source).map_err(|_| {
            GhostraceError::RepairIntervalInvalid("interval source is invalid".to_owned())
        })?,
        start,
        end,
    )
}

fn open_fixture_journal(path: PathBuf) -> Result<Journal, GhostraceError> {
    Journal::open_fixture(path, DeterministicKeyProvider::from_seed(FIXTURE_CLI_KEY_SEED))
}

fn open_export_input(
    fixture: Option<PathBuf>,
    journal: Option<PathBuf>,
) -> Result<(Journal, PolicyProfile, bool), GhostraceError> {
    match (fixture, journal) {
        (Some(fixture), None) => {
            let journal =
                Journal::in_memory(DeterministicKeyProvider::from_seed("fixture-export-v1"))?;
            let policy = fixture_policy();
            ingest_fixture(fixture, &journal, &policy)?;
            Ok((journal, policy, false))
        }
        (None, Some(journal_path)) => {
            let policy = fixture_policy();
            Ok((open_fixture_journal(journal_path)?, policy, true))
        }
        _ => unreachable!("clap enforces exactly one export input"),
    }
}

#[cfg(target_os = "macos")]
mod live {
    use std::{
        io::{BufRead, Write},
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    use ghostrace::{live::LiveHome, GhostraceError};

    use super::LiveCommand;

    static STOP: AtomicBool = AtomicBool::new(false);

    extern "C" fn request_stop(_: libc::c_int) {
        STOP.store(true, Ordering::SeqCst);
    }

    fn fail(error: impl std::fmt::Display) -> GhostraceError {
        GhostraceError::InvalidEvent(error.to_string())
    }

    fn home(dir: Option<PathBuf>) -> Result<PathBuf, GhostraceError> {
        match dir {
            Some(dir) => Ok(dir),
            None => LiveHome::default_dir().map_err(fail),
        }
    }

    /// Open the home and read its key, telling the user if macOS is waiting
    /// for keychain approval.
    fn open(dir: Option<PathBuf>) -> Result<LiveHome, GhostraceError> {
        let live = LiveHome::open(&home(dir)?).map_err(fail)?;
        let hint = keychain_hint();
        let unlocked = live.unlock();
        hint.store(true, Ordering::SeqCst);
        unlocked.map_err(fail)?;
        Ok(live)
    }

    /// If reading the key blocks, macOS is showing a keychain access dialog
    /// (after an upgrade, an unsigned binary has a new signature). Say so
    /// instead of appearing to hang.
    fn keychain_hint() -> std::sync::Arc<AtomicBool> {
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            if !flag.load(Ordering::SeqCst) {
                eprintln!(
                    "ghostrace: waiting for keychain access. If macOS asks, allow `ghostrace` to\n\
                     use its GHOSTRACE journal key (choose Always Allow after an upgrade)."
                );
            }
        });
        done
    }

    pub fn run(
        dir: Option<PathBuf>,
        command: Vec<std::ffi::OsString>,
    ) -> Result<(), GhostraceError> {
        let live = open(dir)?;
        let (program, args) = command.split_first().ok_or_else(|| fail("no command given"))?;
        let code = live.run(program, args).map_err(fail)?;
        std::process::exit(code);
    }

    pub fn dispatch(command: LiveCommand) -> Result<(), GhostraceError> {
        match command {
            LiveCommand::Forget { home: dir, yes } => {
                let dir = home(dir)?;
                let live = LiveHome::open(&dir).map_err(fail)?;
                if !yes {
                    print!("Delete the journal key and {}? The journal cannot be read afterwards. [y/N] ", dir.display());
                    std::io::stdout().flush().map_err(fail)?;
                    let mut answer = String::new();
                    std::io::stdin().lock().read_line(&mut answer).map_err(fail)?;
                    if !matches!(answer.trim(), "y" | "Y" | "yes") {
                        println!("Nothing was deleted.");
                        return Ok(());
                    }
                }
                live.forget().map_err(fail)?;
                println!("Deleted the journal key and the GHOSTRACE home.");
                Ok(())
            }
            LiveCommand::Init { home: dir } => {
                let dir = home(dir)?;
                LiveHome::init(&dir).map_err(fail)?;
                println!("GHOSTRACE home created at {}", dir.display());
                println!("Key custody: login keychain (explicit opt-in for unsigned builds).");
                println!(
                    "Nothing is recorded until you run `ghostrace run`, `ghostrace live watch`,"
                );
                println!("or `ghostrace live git-snapshot`.");
                Ok(())
            }
            LiveCommand::Status { home: dir, json } => {
                let status = open(dir)?.status().map_err(fail)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!("key custody:    {:?}", status.custody);
                    println!("events:         {}", status.events);
                    println!("gaps:           {}", status.gaps);
                    for (source, count) in &status.by_source {
                        println!("  {source:<12} {count}");
                    }
                    println!("watched roots:  {}", status.watched_roots);
                    if let Some(at) = status.last_event_at {
                        println!("last event:     {}", at.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                }
                Ok(())
            }
            LiveCommand::Timeline { home: dir, limit, json } => {
                let entries = open(dir)?.timeline(limit).map_err(fail)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&entries)?);
                    return Ok(());
                }
                for entry in entries {
                    println!(
                        "{}  {:<10} {:<9} {}\n          {}",
                        entry.observed_at.format("%H:%M:%S"),
                        entry.source,
                        format!("{:?}", entry.evidence).to_lowercase(),
                        entry.statement,
                        entry.event_id
                    );
                }
                Ok(())
            }
            LiveCommand::Explain { home: dir, event } => {
                println!("{}", open(dir)?.explain(event).map_err(fail)?);
                Ok(())
            }
            LiveCommand::Watch { folder, home: dir, seconds, yes } => {
                let mut live = open(dir)?;
                println!("{}", live.watch_preview(&folder).map_err(fail)?);
                if !yes {
                    print!("\nStart watching? [y/N] ");
                    std::io::stdout().flush().map_err(fail)?;
                    let mut answer = String::new();
                    std::io::stdin().lock().read_line(&mut answer).map_err(fail)?;
                    if !matches!(answer.trim(), "y" | "Y" | "yes") {
                        println!("Not started; nothing was recorded.");
                        return Ok(());
                    }
                }
                // SAFETY: the handler only stores to an atomic.
                unsafe {
                    libc::signal(
                        libc::SIGINT,
                        request_stop as extern "C" fn(libc::c_int) as libc::sighandler_t,
                    );
                }
                println!("Watching. Press Ctrl-C to stop.");
                let summary = live
                    .watch(&folder, seconds.map(Duration::from_secs), &|| {
                        STOP.load(Ordering::SeqCst)
                    })
                    .map_err(fail)?;
                println!(
                    "Stopped after {} s: {} change(s) recorded, {} outside scope, {} lost.",
                    summary.seconds,
                    summary.accepted_events,
                    summary.blocked_events,
                    summary.dropped_events
                );
                Ok(())
            }
            LiveCommand::GitSnapshot { path, home: dir } => {
                let summary = open(dir)?.git_snapshot(&path).map_err(fail)?;
                println!(
                    "Recorded Git snapshot: {} worktree, {} branch, {} changed path(s).",
                    summary.worktree_state, summary.branch_class, summary.changed
                );
                if let Some(movement) = summary.transition {
                    println!("Since the previous snapshot: {movement}.");
                }
                if let Some(gap) = summary.gap {
                    println!("History gap recorded: {gap} (earlier history cannot be re-proven).");
                }
                Ok(())
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod live {
    use std::path::PathBuf;

    use ghostrace::GhostraceError;

    use super::LiveCommand;

    pub fn run(_: Option<PathBuf>, _: Vec<std::ffi::OsString>) -> Result<(), GhostraceError> {
        Err(GhostraceError::InvalidEvent("the live journal is supported on macOS only".to_owned()))
    }

    pub fn dispatch(_: LiveCommand) -> Result<(), GhostraceError> {
        Err(GhostraceError::InvalidEvent("the live journal is supported on macOS only".to_owned()))
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
