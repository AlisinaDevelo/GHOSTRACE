use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ghostrace::{
    capture, explain, export_fixture, fixture::ingest_fixture, journal::Journal,
    policy::PolicyProfile, DeterministicKeyProvider, GhostraceError, EVENT_SCHEMA_JSON,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "ghostrace", version, about = "Fixture-only local event journal")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Ingest a checked-in JSONL fixture in memory and explain one event.
    Demo {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        event: Uuid,
    },
    /// Ingest a fixture and stream a versioned JSONL export.
    Export {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Print the checked-in event envelope JSON Schema.
    Schema,
    /// Live capture is intentionally unavailable in this vertical slice.
    Capture,
}

fn run(cli: Cli) -> Result<(), GhostraceError> {
    match cli.command {
        Command::Demo { fixture, event } => {
            let journal =
                Journal::in_memory(DeterministicKeyProvider::from_seed("fixture-demo-v1"))?;
            let policy = PolicyProfile::fixture_default();
            ingest_fixture(fixture, &journal, &policy)?;
            let explanation = explain(&journal, event)?;
            println!("{}", explanation.to_pretty_json()?);
            Ok(())
        }
        Command::Export { fixture, output, force } => {
            let manifest = export_fixture(fixture, &output, force)?;
            println!("exported {} event(s)", manifest.coverage.event_count);
            Ok(())
        }
        Command::Schema => {
            println!("{EVENT_SCHEMA_JSON}");
            Ok(())
        }
        Command::Capture => capture(),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
