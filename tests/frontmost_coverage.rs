//! Frontmost coverage across startup, login, fast user switching, lock, sleep,
//! wake, Mission Control, termination, and observer restart.

use chrono::{DateTime, Duration, Utc};
use ghostrace::{
    FrontmostApp, FrontmostBasis, FrontmostCoverageState, FrontmostExclusions, FrontmostNormalizer,
    FrontmostRawObservation, FrontmostRecord, FrontmostSessionTracker, FrontmostSystemEvent,
    FrontmostTransition, FrontmostUnknownReason,
};
use serde::Deserialize;
use serde_json::{json, Value};

const CORPUS: &str = include_str!("../fixtures/frontmost-coverage-v1.json");
const OBSERVATION_SCHEMA: &str = include_str!("../schemas/frontmost-observation-v1.json");
const COVERAGE_SCHEMA: &str = include_str!("../schemas/frontmost-coverage-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    program: String,
    privacy: Privacy,
    epoch: DateTime<Utc>,
    exclusions: Vec<String>,
    transition_table: Vec<TableRow>,
    sequences: Vec<Sequence>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
    retains_excluded_identity: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableRow {
    scenario: String,
    notification: String,
    classification: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sequence {
    id: String,
    steps: Vec<(Step, Vec<Expected>)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    #[serde(default)]
    app: Option<AppStep>,
    #[serde(default)]
    system: Option<FrontmostSystemEvent>,
    #[serde(default)]
    at: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppStep {
    transition: FrontmostTransition,
    bundle: String,
    pid: i32,
    at: f64,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Expected {
    record: String,
    #[serde(default)]
    transition: Option<FrontmostTransition>,
    #[serde(default)]
    basis: Option<FrontmostBasis>,
    #[serde(default)]
    dwell_ms: Option<u64>,
    #[serde(default)]
    excluded: Option<bool>,
    #[serde(default)]
    state: Option<FrontmostCoverageState>,
    #[serde(default)]
    gap: Option<bool>,
}

fn corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("coverage corpus")
}

fn at(epoch: DateTime<Utc>, seconds: f64) -> DateTime<Utc> {
    epoch + Duration::milliseconds((seconds * 1000.0).round() as i64)
}

fn raw(epoch: DateTime<Utc>, step: &AppStep) -> FrontmostRawObservation {
    FrontmostRawObservation::parse(
        &json!({
            "transition": step.transition,
            "observed_at": at(epoch, step.at),
            "bundle_identifier": step.bundle,
            "bundled": true,
            "activation_policy": "regular",
            "translocated": false,
            "signing": {"valid": true, "ad_hoc": false, "platform_binary": false, "team_identifier": "ABCDE12345"},
            "process_id": step.pid,
            "process_started_micros": 1_767_254_400_000_000_i64 + i64::from(step.pid),
        })
        .to_string(),
    )
    .expect("raw observation")
}

fn tracker(corpus: &Corpus) -> FrontmostSessionTracker {
    FrontmostSessionTracker::with_exclusions(
        FrontmostNormalizer::new([7; 32]),
        FrontmostExclusions::new(&corpus.exclusions),
    )
}

fn apply(
    tracker: &mut FrontmostSessionTracker,
    epoch: DateTime<Utc>,
    step: &Step,
) -> Vec<FrontmostRecord> {
    match (&step.app, step.system) {
        (Some(app), None) => tracker.observe(&raw(epoch, app)),
        (None, Some(event)) => {
            tracker.observe_system(event, at(epoch, step.at.expect("system time")))
        }
        _ => panic!("a step is either an app or a system event"),
    }
}

fn summarize(record: &FrontmostRecord) -> Expected {
    match record {
        FrontmostRecord::App(observation) => Expected {
            record: "app".to_owned(),
            transition: Some(observation.transition),
            basis: Some(observation.basis),
            dwell_ms: observation.dwell_ms,
            excluded: matches!(
                observation.app,
                FrontmostApp::Unknown { reason: FrontmostUnknownReason::Excluded }
            )
            .then_some(true),
            state: None,
            gap: None,
        },
        FrontmostRecord::Coverage(boundary) => Expected {
            record: "coverage".to_owned(),
            transition: None,
            basis: None,
            dwell_ms: None,
            excluded: None,
            state: Some(boundary.state),
            gap: Some(boundary.gap_started_at.is_some()),
        },
    }
}

fn validator(schema: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(schema).expect("schema");
    jsonschema::options().should_validate_formats(true).build(&schema).expect("valid schema")
}

#[test]
fn corpus_classifies_every_required_transition() {
    let corpus = corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.program, "ghostrace-frontmost-coverage-v1");
    assert!(corpus.privacy.synthetic_only);
    assert!(!corpus.privacy.user_data_included);
    assert!(!corpus.privacy.network_required);
    assert!(!corpus.privacy.retains_excluded_identity);
    for scenario in [
        "startup",
        "login",
        "fast_user_switching",
        "lock",
        "sleep",
        "wake",
        "mission_control",
        "app_termination",
        "observer_restart",
    ] {
        assert!(
            corpus.transition_table.iter().any(|row| row.scenario == scenario),
            "missing {scenario}"
        );
    }
    for row in &corpus.transition_table {
        assert!(!row.notification.is_empty());
        assert!(
            ["direct", "inferred_closure", "gap"].contains(&row.classification.as_str()),
            "{}",
            row.scenario
        );
    }
}

#[test]
fn every_sequence_produces_exactly_the_expected_records() {
    let corpus = corpus();
    let observation_schema = validator(OBSERVATION_SCHEMA);
    let coverage_schema = validator(COVERAGE_SCHEMA);
    for sequence in &corpus.sequences {
        let mut tracker = tracker(&corpus);
        for (index, (step, expected)) in sequence.steps.iter().enumerate() {
            let records = apply(&mut tracker, corpus.epoch, step);
            let actual = records.iter().map(summarize).collect::<Vec<_>>();
            assert_eq!(&actual, expected, "{} step {index}", sequence.id);
            for record in &records {
                let (json, schema) = match record {
                    FrontmostRecord::App(observation) => {
                        (serde_json::to_value(observation).expect("JSON"), &observation_schema)
                    }
                    FrontmostRecord::Coverage(boundary) => {
                        (serde_json::to_value(boundary).expect("JSON"), &coverage_schema)
                    }
                };
                assert!(schema.is_valid(&json), "{} step {index}: {json}", sequence.id);
            }
        }
    }
}

#[test]
fn no_session_dwell_spans_a_suspension_or_interruption() {
    let corpus = corpus();
    for sequence in &corpus.sequences {
        let mut tracker = tracker(&corpus);
        let mut boundaries = Vec::new();
        let mut sessions = Vec::new();
        for (step, _) in &sequence.steps {
            for record in apply(&mut tracker, corpus.epoch, step) {
                match record {
                    FrontmostRecord::Coverage(boundary)
                        if matches!(
                            boundary.state,
                            FrontmostCoverageState::Suspended | FrontmostCoverageState::Interrupted
                        ) =>
                    {
                        boundaries.push(boundary.observed_at)
                    }
                    FrontmostRecord::App(observation) => {
                        if let Some(dwell) = observation.dwell_ms {
                            let end = observation.observed_at;
                            sessions.push((end - Duration::milliseconds(dwell as i64), end));
                        }
                    }
                    FrontmostRecord::Coverage(_) => {}
                }
            }
        }
        for (start, end) in sessions {
            for boundary in &boundaries {
                assert!(
                    !(start < *boundary && *boundary < end),
                    "{}: a session spans a coverage boundary",
                    sequence.id
                );
            }
        }
    }
}

#[test]
fn excluded_applications_keep_no_identity_in_any_record() {
    let corpus = corpus();
    let sequence =
        corpus.sequences.iter().find(|s| s.id == "private_application_is_withheld").expect("seq");
    let mut tracker = tracker(&corpus);
    let mut serialized = String::new();
    for (step, _) in &sequence.steps {
        for record in apply(&mut tracker, corpus.epoch, step) {
            serialized.push_str(&serde_json::to_string(&record).expect("JSON"));
        }
    }
    assert!(!serialized.contains("private-notes"));
    assert!(serialized.contains("\"excluded\""));
}
