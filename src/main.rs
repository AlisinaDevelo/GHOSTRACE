use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use ghostrace::{
    capture, explain, export_journal_with_confirmation, fixture::ingest_fixture, journal::Journal,
    policy::PolicyProfile, preview_export, validate_export, DeterministicKeyProvider,
    ExportRequest, GhostraceError, RetentionConfirmation, RetentionPolicy, RootId, SnapshotDigest,
    EVENT_SCHEMA_JSON,
};
use uuid::Uuid;

const FIXTURE_CLI_KEY_SEED: &str = "fixture-cli-v1";

#[derive(Debug, Parser)]
#[command(name = "ghostrace", version, about = "Fixture-only local event journal")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
    /// Validate a JSONL export before consuming its records.
    Validate {
        #[arg(long)]
        export: PathBuf,
    },
    /// Print the checked-in event envelope JSON Schema.
    Schema,
    /// Live capture is intentionally unavailable in this vertical slice.
    Capture,
}

fn run(cli: Cli) -> Result<(), GhostraceError> {
    match cli.command {
        Command::Init { journal } => {
            let journal = open_fixture_journal(journal)?;
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
        Command::Validate { export } => {
            let validated = validate_export(export)?;
            println!("validated {} event(s)", validated.event_count);
            Ok(())
        }
        Command::Schema => {
            println!("{EVENT_SCHEMA_JSON}");
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

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
