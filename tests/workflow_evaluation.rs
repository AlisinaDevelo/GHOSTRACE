//! Developer-workflow evaluation of cross-source explanations.
//!
//! Each synthetic case is labelled supported, unsupported, conflicting, or
//! unknowable, and scored against the shipped cross-source adjacency rule.
//! The published report (`docs/evaluation/workflow-explanations-v1.json`) is
//! regenerated with `GHOSTRACE_WRITE_EVALUATION=1` and otherwise must match.

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use ghostrace::{
    evaluate_correlation, CorrelationQuery, CorrelationReason, CorrelationRuleId, EventEnvelope,
    Evidence, PolicyProfile,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CORPUS: &str = include_str!("../fixtures/workflow-evaluation-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    program: String,
    privacy: Privacy,
    epoch: DateTime<Utc>,
    window_seconds: i64,
    labels: Vec<String>,
    workflows: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    workflow: String,
    sources: String,
    events: Vec<EventSpec>,
    label: String,
    rationale: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventSpec {
    kind: String,
    at: i64,
    evidence: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct CaseResult {
    id: String,
    workflow: String,
    sources: String,
    label: String,
    prediction: String,
    reason: CorrelationReason,
    evidence: Evidence,
    correct: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Report {
    schema_version: u32,
    corpus: String,
    rule: String,
    cases: usize,
    precision: f64,
    coverage: f64,
    abstention_rate: f64,
    honest_abstention: f64,
    gap_visibility: f64,
    false_positives: Vec<String>,
    false_negatives: Vec<String>,
    results: Vec<CaseResult>,
}

fn envelope(index: usize, spec: &EventSpec, epoch: DateTime<Utc>) -> EventEnvelope {
    let (source, payload, instance) = match spec.kind.as_str() {
        "shell_started" => (
            "shell",
            json!({"type": "shell_started", "data": {"session_id": "session-eval", "shell_kind": "zsh"}}),
            "fixture-shell-eval",
        ),
        "shell_finished" => (
            "shell",
            json!({"type": "shell_finished", "data": {"session_id": "session-eval", "status": "succeeded", "exit_code": 0, "duration_ms": 1000}}),
            "fixture-shell-eval",
        ),
        "git_snapshot" => (
            "git",
            json!({"type": "git_snapshot", "data": {"repository_id": "repo-eval", "branch": "main", "head_oid": "0123456789abcdef0123456789abcdef01234567", "dirty": true, "changed_file_count": 1}}),
            "fixture-git-eval",
        ),
        "filesystem_changed" => (
            "filesystem",
            json!({"type": "filesystem_changed", "data": {"root_id": "workspace-demo", "path_class": "workspace_relative", "operation": "modified", "entry_kind": "file", "path_digest": format!("sha256:{}", "ab".repeat(32)), "size_bytes": 10}}),
            "fixture-fs-eval",
        ),
        "frontmost_app_changed" => (
            "frontmost_app",
            json!({"type": "frontmost_app_changed", "data": {"app_id": "com.example.editor", "change": "activated"}}),
            "fixture-frontmost-eval",
        ),
        "gap" => (
            "filesystem",
            json!({"type": "gap", "data": {"source": "filesystem", "reason_code": "fixture_history_rewrite", "dropped_count": 0}}),
            "fixture-fs-eval",
        ),
        kind => panic!("unsupported event kind {kind}"),
    };
    let at = epoch + Duration::seconds(spec.at);
    serde_json::from_value(json!({
        "schema_version": 1,
        "event_id": format!("00000000-0000-4000-8000-{index:012}"),
        "observed_at": at,
        "ingested_at": at,
        "source": source,
        "kind": spec.kind,
        "payload": payload,
        "collector_instance": instance,
        "provenance_version": "fixture-v1",
        "policy_profile_id": "fixture-default-v1",
        "policy_profile_version": 1,
        "evidence": spec.evidence,
        "parent_event_id": null,
    }))
    .unwrap_or_else(|error| panic!("{} envelope: {error}", spec.kind))
}

fn prediction(reason: CorrelationReason, evidence: Evidence) -> &'static str {
    match reason {
        CorrelationReason::BoundedCrossSourceAdjacency => {
            assert_eq!(evidence, Evidence::Inferred);
            "supported"
        }
        CorrelationReason::OutsideWindow
        | CorrelationReason::RequiresDistinctSources
        | CorrelationReason::NoEligibleInputs
        | CorrelationReason::UnsupportedEventKind => "unsupported",
        _ => "abstain",
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 1.0;
    }
    ((numerator as f64 / denominator as f64) * 10_000.0).round() / 10_000.0
}

fn evaluate() -> Report {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("workflow corpus");
    let policy = PolicyProfile::fixture_default();
    let query = CorrelationQuery::for_policy(&policy).expect("query");
    let mut results = Vec::new();
    let mut index = 1;
    for case in &corpus.cases {
        let events = case
            .events
            .iter()
            .map(|spec| {
                index += 1;
                envelope(index, spec, corpus.epoch)
            })
            .collect::<Vec<_>>();
        let result = evaluate_correlation(
            CorrelationRuleId::CrossSourceTemporalAdjacency,
            &events,
            &policy,
            &query,
        )
        .expect("evaluation");
        // Temporal adjacency is never upgraded into observed attribution.
        assert!(
            matches!(result.evidence, Evidence::Inferred | Evidence::Unknown),
            "{} produced {:?}",
            case.id,
            result.evidence
        );
        let predicted = prediction(result.reason, result.evidence);
        let correct = match case.label.as_str() {
            "supported" => predicted == "supported",
            "unsupported" => predicted == "unsupported",
            "conflicting" | "unknowable" => predicted == "abstain",
            label => panic!("unknown label {label}"),
        };
        results.push(CaseResult {
            id: case.id.clone(),
            workflow: case.workflow.clone(),
            sources: case.sources.clone(),
            label: case.label.clone(),
            prediction: predicted.to_owned(),
            reason: result.reason,
            evidence: result.evidence,
            correct,
        });
    }
    let count =
        |predicate: &dyn Fn(&CaseResult) -> bool| results.iter().filter(|r| predicate(r)).count();
    let predicted_supported = count(&|r| r.prediction == "supported");
    let true_supported = count(&|r| r.prediction == "supported" && r.label == "supported");
    let labelled_supported = count(&|r| r.label == "supported");
    let uncertain = count(&|r| r.label == "conflicting" || r.label == "unknowable");
    let honest = count(&|r| {
        (r.label == "conflicting" || r.label == "unknowable") && r.prediction == "abstain"
    });
    let gap_cases = corpus
        .cases
        .iter()
        .zip(&results)
        .filter(|(case, _)| case.events.iter().any(|event| event.kind == "gap"))
        .collect::<Vec<_>>();
    let gap_visible =
        gap_cases.iter().filter(|(_, result)| result.prediction != "supported").count();
    Report {
        schema_version: 1,
        corpus: corpus.program.clone(),
        rule: "cross_source_temporal_adjacency".to_owned(),
        cases: results.len(),
        precision: ratio(true_supported, predicted_supported),
        coverage: ratio(true_supported, labelled_supported),
        abstention_rate: ratio(count(&|r| r.prediction == "abstain"), results.len()),
        honest_abstention: ratio(honest, uncertain),
        gap_visibility: ratio(gap_visible, gap_cases.len()),
        false_positives: results
            .iter()
            .filter(|r| r.prediction == "supported" && r.label != "supported")
            .map(|r| r.id.clone())
            .collect(),
        false_negatives: results
            .iter()
            .filter(|r| r.label == "supported" && r.prediction != "supported")
            .map(|r| r.id.clone())
            .collect(),
        results,
    }
}

fn report_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/evaluation/workflow-explanations-v1.json")
}

#[test]
fn corpus_covers_the_required_workflows_and_labels() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("workflow corpus");
    assert_eq!(corpus.schema_version, 1);
    assert!(corpus.privacy.synthetic_only);
    assert!(!corpus.privacy.user_data_included);
    assert!(!corpus.privacy.network_required);
    assert_eq!(corpus.window_seconds, ghostrace::MAX_CORRELATION_WINDOW_SECONDS);
    for workflow in [
        "build",
        "test",
        "checkout",
        "rebase",
        "editor_save",
        "generated_files",
        "concurrent_unrelated",
    ] {
        assert!(corpus.cases.iter().any(|case| case.workflow == workflow), "missing {workflow}");
        assert!(corpus.workflows.iter().any(|known| known == workflow));
    }
    for label in &corpus.labels {
        assert!(corpus.cases.iter().any(|case| &case.label == label), "no {label} case");
    }
    assert!(corpus.cases.iter().all(|case| !case.rationale.is_empty()));
}

#[test]
fn published_report_matches_the_shipped_rule() {
    let report = evaluate();
    let rendered = serde_json::to_string_pretty(&report).expect("report JSON") + "\n";
    if std::env::var_os("GHOSTRACE_WRITE_EVALUATION").is_some() {
        std::fs::create_dir_all(report_path().parent().expect("parent")).expect("dir");
        std::fs::write(report_path(), &rendered).expect("write report");
    }
    let published: Value = serde_json::from_str(
        &std::fs::read_to_string(report_path()).expect("published report exists"),
    )
    .expect("published report JSON");
    assert_eq!(published, serde_json::to_value(&report).expect("report value"));
}

#[test]
fn every_counterexample_is_explained_by_a_named_limit() {
    let report = evaluate();
    for id in report.false_positives.iter().chain(&report.false_negatives) {
        let result = report.results.iter().find(|result| &result.id == id).expect("result");
        assert!(
            matches!(
                result.reason,
                CorrelationReason::BoundedCrossSourceAdjacency
                    | CorrelationReason::OutsideWindow
                    | CorrelationReason::EqualObservedTime
            ),
            "{id}: unexpected failure mode {:?}",
            result.reason
        );
    }
    assert!(report.gap_visibility >= 1.0, "a gap must never yield a supported claim");
}
