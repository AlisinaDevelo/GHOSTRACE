use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ghostrace::{
    capture, explain, export_journal_with_confirmation, fixture::ingest_fixture, journal::Journal,
    policy::PolicyProfile, preview_export, validate_export, DeterministicKeyProvider,
    ExportRequest, GhostraceError, EVENT_SCHEMA_JSON,
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
