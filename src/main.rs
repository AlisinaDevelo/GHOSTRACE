use std::{collections::BTreeSet, io::Write, path::PathBuf};

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
#[cfg(unix)]
use ghostrace::{
    read_native_service_endpoint, run_native_host_stdio, validate_caller_origin, BrowserEventClass,
    LocalServiceClient, PairingRequest, PairingStore, ProfileClass, UrlShapePolicy,
    NATIVE_HOST_STORE_DIR,
};
#[cfg(all(unix, target_os = "macos"))]
use ghostrace::{NativeHostHealth, NativeHostInstaller, NATIVE_HOST_CHANNELS};
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
    /// Inspect offline aggregate health without reading a key or retained payloads.
    Health {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
    },
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
    /// Manage explicit browser pairing or run the stdio native-messaging host.
    #[cfg(unix)]
    NativeHost {
        #[command(subcommand)]
        command: NativeHostCommand,
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
    /// Print the strict v1 metadata schema for the explicit shell wrapper.
    ShellSchema,
    /// Ambient capture remains disabled; explicit macOS live commands are separate.
    Capture,
}

#[cfg(unix)]
#[derive(Debug, Subcommand)]
enum NativeHostCommand {
    /// Show the approval request and persist a new pairing after confirmation.
    Pair {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long, default_value = "chrome")]
        browser: String,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long)]
        extension_id: String,
        #[arg(long)]
        extension_key_digest: String,
        #[arg(long)]
        permissions_digest: String,
        /// Skip the interactive approval prompt only when the user has already
        /// confirmed the printed request out of band.
        #[arg(long)]
        yes: bool,
    },
    /// List secret-free persisted pairing approvals.
    List {
        #[arg(long)]
        home: Option<PathBuf>,
    },
    /// Revoke one pairing and retain a tombstone against replay.
    Revoke {
        #[arg(long)]
        home: Option<PathBuf>,
        pairing_id: Uuid,
    },
    /// Run the browser-launched stdio host against an already-running LocalService.
    Run {
        #[arg(long)]
        home: Option<PathBuf>,
        /// User-owned LocalService Unix socket; no TCP fallback exists.
        #[arg(long)]
        service_socket: PathBuf,
        /// Current LocalService instance UUID, published out of band.
        #[arg(long)]
        service_instance: Uuid,
        /// Exact Chromium caller origin supplied for this host process.
        #[arg(long)]
        caller_origin: String,
    },
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
    /// Show what `ghostrace run` records and, once you agree, allow it.
    ConsentShell {
        #[arg(long)]
        home: Option<PathBuf>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Withdraw consent to `ghostrace run`; later runs refuse before starting.
    RevokeShell {
        #[arg(long)]
        home: Option<PathBuf>,
    },
    /// Export the journal as validated JSONL after showing what it discloses,
    /// and optionally a Parquet archive (builds with `--features parquet`).
    Export {
        #[arg(long)]
        output: PathBuf,
        /// Also write a Parquet cold archive from the export.
        #[arg(long)]
        parquet: Option<PathBuf>,
        #[arg(long)]
        home: Option<PathBuf>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Write the timeline as one offline HTML file. It is an unencrypted copy.
    Report {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        home: Option<PathBuf>,
        /// Confirm that the report is an unencrypted copy.
        #[arg(long)]
        yes: bool,
    },
    /// Record which application is in front, after confirming what is kept.
    /// Needs a build with `--features frontmost`.
    Apps {
        #[arg(long)]
        home: Option<PathBuf>,
        /// Stop after this many seconds (default: until Ctrl-C).
        #[arg(long)]
        seconds: Option<u64>,
        /// Never identify this bundle ID (repeatable).
        #[arg(long = "exclude")]
        exclude: Vec<String>,
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

#[cfg(unix)]
fn native_failure(error: impl std::fmt::Display) -> GhostraceError {
    GhostraceError::InvalidEvent(format!("native host operation failed: {error}"))
}

#[cfg(unix)]
fn native_home(dir: Option<PathBuf>) -> Result<PathBuf, GhostraceError> {
    let dir = match dir {
        Some(dir) => dir,
        None => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| native_failure("GHOSTRACE home is not configured"))?;
            PathBuf::from(home).join("Library/Application Support/GHOSTRACE")
        }
    };
    let metadata = std::fs::symlink_metadata(&dir).map_err(native_failure)?;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(native_failure("GHOSTRACE home is not private to this user"));
    }
    Ok(dir)
}

#[cfg(unix)]
fn native_store(home: Option<PathBuf>) -> Result<PairingStore, GhostraceError> {
    let home = native_home(home)?;
    PairingStore::open(home.join(NATIVE_HOST_STORE_DIR)).map_err(native_failure)
}

#[cfg(unix)]
fn native_profile_class(value: &str) -> Result<ProfileClass, GhostraceError> {
    match value {
        "default" => Ok(ProfileClass::Default),
        "named" => Ok(ProfileClass::Named),
        _ => Err(native_failure("profile must be default or named")),
    }
}

#[cfg(unix)]
fn native_confirmation() -> Result<bool, GhostraceError> {
    print!("Approve this browser pairing? [y/N] ");
    std::io::stdout().flush().map_err(native_failure)?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).map_err(native_failure)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

#[cfg(unix)]
fn dispatch_native_host(command: NativeHostCommand) -> Result<(), GhostraceError> {
    match command {
        NativeHostCommand::Pair {
            home,
            browser,
            profile,
            extension_id,
            extension_key_digest,
            permissions_digest,
            yes,
        } => {
            let store = native_store(home)?;
            let request = PairingRequest {
                browser_channel: browser,
                profile_class: native_profile_class(&profile)?,
                extension_id,
                extension_key_digest,
                permissions_digest,
                event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
                retained_fields: vec!["origin".to_owned()],
                private_context_policy: "refuse_private_context".to_owned(),
            };
            println!("{}", serde_json::to_string_pretty(&request)?);
            if !yes && !native_confirmation()? {
                println!("Pairing not approved; no secret was generated.");
                return Ok(());
            }
            let approval = store.approve(request, Utc::now()).map_err(native_failure)?;
            println!("{}", serde_json::to_string_pretty(&approval)?);
            Ok(())
        }
        NativeHostCommand::List { home } => {
            let store = native_store(home)?;
            println!("{}", serde_json::to_string_pretty(&store.list().map_err(native_failure)?)?);
            Ok(())
        }
        NativeHostCommand::Revoke { home, pairing_id } => {
            let store = native_store(home)?;
            let changed = store.revoke(pairing_id).map_err(native_failure)?;
            println!("{}", serde_json::json!({ "pairing_id": pairing_id, "revoked": changed }));
            Ok(())
        }
        NativeHostCommand::Run { home, service_socket, service_instance, caller_origin } => {
            native_host_run(home, service_socket, service_instance, caller_origin)
        }
    }
}

#[cfg(all(unix, target_os = "macos"))]
fn native_host_run(
    home: Option<PathBuf>,
    service_socket: PathBuf,
    service_instance: Uuid,
    caller_origin: String,
) -> Result<(), GhostraceError> {
    // Chrome supplies the caller origin as argv[1]. Validate it before opening
    // or consuming stdin so an invalid launch context cannot reach framing or
    // pairing code.
    validate_caller_origin(&caller_origin).map_err(native_failure)?;
    let home = native_home(home)?;
    let store = PairingStore::open(home.join(NATIVE_HOST_STORE_DIR)).map_err(native_failure)?;
    let service = LocalServiceClient::new(service_socket, service_instance);
    let result = run_native_host_stdio(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        store,
        service,
        UrlShapePolicy::OriginOnly,
        &caller_origin,
    );
    let summary = result.map_err(native_failure)?;
    eprintln!(
        "native host stopped after {} frame(s), {} recorded event(s)",
        summary.frames, summary.recorded_events
    );
    Ok(())
}

#[cfg(all(unix, target_os = "macos"))]
fn native_host_entry(caller_origin: String) -> Result<(), GhostraceError> {
    // This is the manifest-launched shape: Chrome invokes the absolute binary
    // with exactly one argument (the caller origin), never a subcommand or
    // service flags. The LocalService owner publishes the current endpoint
    // receipt beside pairing state before installing/using the manifest.
    let extension_id = validate_caller_origin(&caller_origin).map_err(native_failure)?;
    let home = native_home(None)?;
    verify_native_host_manifest(&home, &extension_id)?;
    let endpoint = read_native_service_endpoint(&home).map_err(native_failure)?;
    native_host_run(Some(home), endpoint.socket_path, endpoint.service_instance, caller_origin)
}

#[cfg(all(unix, target_os = "macos"))]
fn verify_native_host_manifest(
    home: &std::path::Path,
    extension_id: &str,
) -> Result<(), GhostraceError> {
    let support_root =
        home.parent().ok_or_else(|| native_failure("native host support root is unavailable"))?;
    let host_binary = std::env::current_exe().map_err(native_failure)?;
    let installer = NativeHostInstaller::new(support_root, host_binary, extension_id)
        .map_err(native_failure)?;
    let mut installed = false;
    for (channel, _) in NATIVE_HOST_CHANNELS {
        match installer.verify(channel).map_err(native_failure)? {
            NativeHostHealth::Intact => installed = true,
            NativeHostHealth::NotInstalled => {}
            NativeHostHealth::Missing | NativeHostHealth::Drifted => {
                return Err(native_failure("native host manifest verification failed"));
            }
        }
    }
    if !installed {
        return Err(native_failure("native host manifest is not installed"));
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn native_host_entry(caller_origin: String) -> Result<(), GhostraceError> {
    let _ = caller_origin;
    Err(native_failure("the native host requires macOS live support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn native_host_run(
    _: Option<PathBuf>,
    _: PathBuf,
    _: Uuid,
    _: String,
) -> Result<(), GhostraceError> {
    Err(native_failure("the native host requires macOS live support"))
}

fn run(cli: Cli) -> Result<(), GhostraceError> {
    match cli.command {
        Command::Health { journal, json } => {
            let report = ghostrace::health::HealthReport::inspect(&journal);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.human());
            }
            if !report.readable() {
                // The bounded report is the entire refusal, including on stderr.
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Run { home, command } => live::run(home, command),
        Command::Live { command } => live::dispatch(command),
        #[cfg(unix)]
        Command::NativeHost { command } => dispatch_native_host(command),
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

    use ghostrace::{
        live::{LiveHome, SHELL_CONSENT_PREVIEW},
        report::REPORT_PLAINTEXT_NOTICE,
        GhostraceError,
    };

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

    fn confirmed(question: &str) -> Result<bool, GhostraceError> {
        print!("{question} [y/N] ");
        std::io::stdout().flush().map_err(fail)?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer).map_err(fail)?;
        Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
    }

    #[cfg(feature = "frontmost")]
    fn apps(
        dir: Option<PathBuf>,
        seconds: Option<u64>,
        exclude: Vec<String>,
        yes: bool,
    ) -> Result<(), GhostraceError> {
        use ghostrace::live::{APPS_CONSENT_PREVIEW, DEFAULT_APP_EXCLUSIONS};
        let mut live = open(dir)?;
        println!("{APPS_CONSENT_PREVIEW}");
        println!(
            "Excluded: {}",
            DEFAULT_APP_EXCLUSIONS
                .iter()
                .copied()
                .chain(exclude.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        );
        if !yes && !confirmed("\nStart recording?")? {
            println!("Not started; nothing was recorded.");
            return Ok(());
        }
        // SAFETY: the handler only stores to an atomic.
        unsafe {
            libc::signal(
                libc::SIGINT,
                request_stop as extern "C" fn(libc::c_int) as libc::sighandler_t,
            );
        }
        println!("Recording. Press Ctrl-C to stop.");
        let summary = live
            .apps(seconds.map(Duration::from_secs), &exclude, &|| STOP.load(Ordering::SeqCst))
            .map_err(fail)?;
        println!(
            "Stopped after {} s: {} activation(s) recorded, {} withheld, {} gap(s).",
            summary.seconds, summary.activations, summary.withheld, summary.gaps
        );
        Ok(())
    }

    #[cfg(not(feature = "frontmost"))]
    fn apps(
        _: Option<PathBuf>,
        _: Option<u64>,
        _: Vec<String>,
        _: bool,
    ) -> Result<(), GhostraceError> {
        Err(fail("this build has no frontmost adapter; rebuild with --features frontmost"))
    }

    pub fn dispatch(command: LiveCommand) -> Result<(), GhostraceError> {
        match command {
            LiveCommand::ConsentShell { home: dir, yes } => {
                let mut live = LiveHome::open(&home(dir)?).map_err(fail)?;
                println!("{SHELL_CONSENT_PREVIEW}\n");
                if !yes && !confirmed("Allow `ghostrace run`?")? {
                    println!("Not allowed; `ghostrace run` will refuse.");
                    return Ok(());
                }
                live.grant_shell_consent().map_err(fail)?;
                println!("Allowed. Revoke with `ghostrace live revoke-shell`.");
                Ok(())
            }
            LiveCommand::Export { output, parquet, home: dir, yes } => {
                let live = open(dir)?;
                if parquet.is_some() && !cfg!(feature = "parquet") {
                    return Err(fail(
                        "this build has no Parquet writer; rebuild with --features parquet",
                    ));
                }
                let mut asked = |preview: &ghostrace::ExportPreview| {
                    let summary = serde_json::to_value(preview).unwrap_or_default();
                    println!("{}", preview.warning());
                    println!(
                        "Events: {}  Sources: {}  Gaps: {}",
                        summary["event_count"],
                        summary["sources"],
                        summary["gaps"].as_array().map_or(0, Vec::len)
                    );
                    println!(
                        "Plan {}\nSnapshot {}",
                        preview.plan_digest(),
                        preview.snapshot_digest()
                    );
                    yes || confirmed("Write this plaintext export?").unwrap_or(false)
                };
                let Some(result) = live.export(&output, &mut asked).map_err(fail)? else {
                    println!("No export was written.");
                    return Ok(());
                };
                println!(
                    "Exported {} event(s) to {}.",
                    result.manifest.coverage.event_count,
                    output.display()
                );
                #[cfg(feature = "parquet")]
                if let Some(archive) = parquet {
                    let receipt = live.archive(&output, &archive).map_err(fail)?;
                    println!(
                        "Archived {} row(s) to {} (verified against the export).",
                        receipt.row_count,
                        archive.display()
                    );
                }
                Ok(())
            }
            LiveCommand::Report { output, home: dir, yes } => {
                let live = open(dir)?;
                println!("{REPORT_PLAINTEXT_NOTICE}");
                if !yes && !confirmed("Write the report?")? {
                    println!("No report was written.");
                    return Ok(());
                }
                let events = live.write_report(&output).map_err(fail)?;
                println!("Wrote {} ({events} event(s)).", output.display());
                Ok(())
            }
            LiveCommand::Apps { home: dir, seconds, exclude, yes } => {
                apps(dir, seconds, exclude, yes)
            }
            LiveCommand::RevokeShell { home: dir } => {
                let mut live = LiveHome::open(&home(dir)?).map_err(fail)?;
                if live.revoke_shell_consent().map_err(fail)? {
                    println!("Consent withdrawn; `ghostrace run` now refuses before starting.");
                } else {
                    println!("There was no consent to withdraw.");
                }
                Ok(())
            }
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
                    "Nothing is recorded until you ask. `ghostrace run -- <command>` requires\n\
                     `ghostrace live consent-shell`; `ghostrace live watch <folder>` has its own\n\
                     consent preview, and `ghostrace live git-snapshot` is explicitly invoked and\n\
                     policy-gated without requiring shell consent. Never recorded: file contents or readable\n\
                     file names, command arguments, environment, terminal input or output,\n\
                     branch or remote names, or which app made a change."
                );
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
                    let consent = if status.shell_consent { "granted" } else { "not granted" };
                    println!("run consent:    {consent}");
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
                let announced = std::cell::Cell::new(false);
                let summary = live
                    .watch(&folder, seconds.map(Duration::from_secs), &|| {
                        // The first stop check runs after collector.start()
                        // succeeds, so this line is an actual readiness signal.
                        if !announced.replace(true) {
                            println!("Watching. Press Ctrl-C to stop.");
                        }
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
    #[cfg(unix)]
    {
        let mut arguments = std::env::args_os();
        let _binary = arguments.next();
        let first = arguments.next();
        let has_extra = arguments.next().is_some();
        if let Some(first) = first.and_then(|value| value.into_string().ok()) {
            if first.starts_with("chrome-extension://") {
                if has_extra {
                    // A manifest launch has exactly one caller-origin
                    // argument. Reject malformed argv before Clap can echo an
                    // untrusted origin/argument and before touching HOME or
                    // consuming stdin.
                    eprintln!("error: native host operation failed: invalid native host arguments");
                    std::process::exit(1);
                }
                if let Err(error) = native_host_entry(first) {
                    eprintln!("error: {error}");
                    std::process::exit(1);
                }
                return;
            }
        }
    }
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
