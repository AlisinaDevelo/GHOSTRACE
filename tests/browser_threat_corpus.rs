//! Browser integration threat corpus: every case names the layer that
//! validates it and its outcome; enforced URL cases run against the shipped
//! sanitizer; every accepted risk in ADR 0005 is tied to a test, permission,
//! user control, and rollback path.

use std::{collections::BTreeSet, path::Path};

use ghostrace::SanitizedUrl;
use serde::Deserialize;

const CORPUS: &str = include_str!("../fixtures/browser-threat-corpus-v1.json");
const ADR: &str = include_str!("../docs/adr/0005-browser-transport-and-permissions.md");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    program: String,
    privacy: Privacy,
    layers: Vec<String>,
    outcomes: Vec<String>,
    categories: Vec<String>,
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
    category: String,
    layer: String,
    enforced_by: String,
    status: String,
    input: Input,
    outcome: String,
    #[serde(default)]
    canonical: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    open_finding: Option<String>,
    #[serde(default)]
    accepted_risk: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("browser threat corpus")
}

#[test]
fn corpus_covers_every_required_threat_category_and_names_a_layer_and_outcome() {
    let corpus = corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.program, "ghostrace-browser-threat-corpus-v1");
    assert!(corpus.privacy.synthetic_only);
    assert!(!corpus.privacy.user_data_included);
    assert!(!corpus.privacy.network_required);
    let used = corpus.cases.iter().map(|case| case.category.as_str()).collect::<BTreeSet<_>>();
    for category in &corpus.categories {
        assert!(used.contains(category.as_str()), "no case for {category}");
    }
    let mut ids = BTreeSet::new();
    for case in &corpus.cases {
        assert!(ids.insert(case.id.as_str()), "duplicate {}", case.id);
        assert!(corpus.layers.contains(&case.layer), "{}: unknown layer", case.id);
        assert!(corpus.outcomes.contains(&case.outcome), "{}: unknown outcome", case.id);
        assert!(corpus.categories.contains(&case.category), "{}: unknown category", case.id);
        assert!(case.input.url.is_some() || case.input.description.is_some(), "{}", case.id);
        assert!(case.note.as_ref().is_none_or(|note| !note.is_empty()));
    }
}

#[test]
fn enforced_url_cases_match_the_shipped_sanitizer_exactly() {
    for case in corpus().cases.iter().filter(|case| case.status == "enforced") {
        assert_eq!(case.enforced_by, "SanitizedUrl::parse", "{}", case.id);
        let raw = case.input.url.as_deref().expect("enforced cases carry a URL");
        let result = SanitizedUrl::parse(raw);
        match case.outcome.as_str() {
            "reject" => assert!(result.is_err(), "{} was accepted", case.id),
            "accept_canonicalized" => {
                let url = result.unwrap_or_else(|_| panic!("{} was rejected", case.id));
                assert_eq!(Some(url.as_str()), case.canonical.as_deref(), "{}", case.id);
                assert!(!url.as_str().contains('?') && !url.as_str().contains('#'), "{}", case.id);
                assert!(!url.as_str().contains('@') || url.as_str().contains("/@"), "{}", case.id);
            }
            outcome => panic!("{}: enforced URL case with outcome {outcome}", case.id),
        }
    }
}

#[test]
fn specified_cases_name_an_existing_ledger_task_and_open_findings_are_visible() {
    let tasks = Path::new(env!("CARGO_MANIFEST_DIR")).join(".forge/tasks");
    for case in corpus().cases.iter().filter(|case| case.status == "specified") {
        let task = case.enforced_by.strip_prefix("task ").expect("specified cases name a task");
        let exists = std::fs::read_dir(&tasks)
            .expect("ledger")
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with(&format!("{task}-")));
        assert!(exists, "{}: task {task} does not exist", case.id);
        assert_ne!(
            case.layer, "sanitized_url",
            "{}: specified cases target unbuilt layers",
            case.id
        );
    }
    let findings = corpus().cases.iter().filter(|case| case.open_finding.is_some()).count();
    assert!(findings >= 3, "open canonicalization findings must stay visible until task 0106");
}

#[test]
fn every_accepted_risk_links_a_test_permission_user_control_and_rollback() {
    let corpus = corpus();
    let ids = corpus.cases.iter().map(|case| case.id.as_str()).collect::<BTreeSet<_>>();
    let section = ADR.split("## Accepted risks").nth(1).expect("accepted risks section");
    let rows = section
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.starts_with("| R-"))
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    let mut risks = BTreeSet::new();
    for row in rows {
        let cells = row.trim_matches('|').split('|').map(str::trim).collect::<Vec<_>>();
        assert_eq!(cells.len(), 6, "{row}");
        assert!(cells.iter().all(|cell| !cell.is_empty()), "{row}");
        assert!(ids.contains(cells[2]), "{}: test {} is not a corpus case", cells[0], cells[2]);
        risks.insert(cells[0]);
    }
    for case in &corpus.cases {
        if let Some(risk) = &case.accepted_risk {
            assert!(risks.contains(risk.as_str()), "{}: {risk} missing from ADR 0005", case.id);
        }
    }
}
