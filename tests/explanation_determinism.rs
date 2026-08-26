use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::PathBuf,
};

use chrono::{DateTime, Duration, Utc};
use ghostrace::{
    evaluate_correlation, explain, read_fixture, render_claim, AppChange, ApplicationId,
    BookmarkChange, BookmarkId, BrowserBookmarkChangedPayload, BrowserName, ClaimLocale,
    ClaimTemplateId, CorrelationQuery, CorrelationReason, CorrelationRuleId,
    DeterministicKeyProvider, EventEnvelope, EventKind, EventPayload, EventSource, Evidence,
    FolderId, FrontmostAppChangedPayload, GapBehavior, IngestionOrigin, Journal, PolicyProfile,
    QueryRequest, ReasonCode, SanitizedUrl, SessionId, ShellFinishedPayload, ShellStatus,
    CLAIM_GRAMMAR_VERSION,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct CounterexampleFixture {
    schema_version: u32,
    grammar_version: u32,
    property_dimensions: Vec<String>,
    golden_matrix: GoldenMatrix,
    conflict_outcomes: Vec<ConflictOutcome>,
    mutations: Vec<MutationCase>,
    privacy: PrivacyContract,
}

#[derive(Debug, Deserialize)]
struct GoldenMatrix {
    claim_templates: Vec<ClaimTemplateId>,
    evidence_levels: Vec<Evidence>,
    gap_states: Vec<GapState>,
    expected_gap_behavior: ExpectedGapBehavior,
}

#[derive(Debug, Deserialize)]
struct GapState {
    id: String,
    present: bool,
}

#[derive(Debug, Deserialize)]
struct ExpectedGapBehavior {
    ordinary: GapBehavior,
    status: GapBehavior,
}

#[derive(Debug, Deserialize)]
struct ConflictOutcome {
    id: String,
    template: ClaimTemplateId,
    evidence: Evidence,
    claim_state: String,
}

#[derive(Debug, Deserialize)]
struct MutationCase {
    id: String,
    baseline_evidence: Option<Evidence>,
    mutated_evidence: Option<Evidence>,
    mutated_reason: Option<CorrelationReason>,
    baseline_statement_count: Option<usize>,
    mutated_statement_count: Option<usize>,
    removed_template: Option<ClaimTemplateId>,
    mutated_state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrivacyContract {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
}

fn fixture_contract() -> CounterexampleFixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/explanation-counterexamples-v1.json");
    serde_json::from_str(&fs::read_to_string(path).expect("counterexample fixture"))
        .expect("counterexample JSON")
}

fn timestamp(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).expect("valid timestamp")
}

fn causal_id(value: u16) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{value:012x}")).expect("causal UUID")
}

fn wrapper<T>(value: &str) -> T
where
    T: TryFrom<String>,
    T::Error: std::fmt::Debug,
{
    T::try_from(value.to_owned()).expect("valid semantic wrapper")
}

fn matrix_event(
    id: u128,
    observed_at: DateTime<Utc>,
    source: EventSource,
    kind: EventKind,
    payload: EventPayload,
    evidence: Evidence,
) -> EventEnvelope {
    let origin = IngestionOrigin::fixture_instance("fixture-matrix").expect("fixture origin");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        observed_at,
        observed_at,
        source,
        kind,
        payload,
        None,
        "fixture-default-v1",
        1,
        evidence,
        None,
    )
    .expect("valid matrix event")
}

fn matrix_events() -> Vec<EventEnvelope> {
    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    let mut events = read_fixture(&fixture_path).expect("causal fixture");
    let observed_at = timestamp(1_735_689_600);
    events.extend([
        matrix_event(
            101,
            observed_at + Duration::seconds(8),
            EventSource::FrontmostApp,
            EventKind::FrontmostAppChanged,
            EventPayload::FrontmostAppChanged(FrontmostAppChangedPayload {
                app_id: wrapper::<ApplicationId>("com.example.editor"),
                change: AppChange::Activated,
                previous_app_id: None,
            }),
            Evidence::Direct,
        ),
        matrix_event(
            102,
            observed_at + Duration::seconds(9),
            EventSource::Shell,
            EventKind::ShellFinished,
            EventPayload::ShellFinished(ShellFinishedPayload {
                session_id: wrapper::<SessionId>("session-matrix"),
                status: ShellStatus::Succeeded,
                exit_code: Some(0),
                duration_ms: 42,
            }),
            Evidence::Direct,
        ),
        matrix_event(
            103,
            observed_at + Duration::seconds(10),
            EventSource::Browser,
            EventKind::BrowserBookmarkChanged,
            EventPayload::BrowserBookmarkChanged(BrowserBookmarkChangedPayload {
                browser: wrapper::<BrowserName>("fixture-browser"),
                bookmark_id: wrapper::<BookmarkId>("bookmark-matrix"),
                change: BookmarkChange::Created,
                url: SanitizedUrl::parse("https://example.test/docs/bookmark").expect("URL"),
                folder_id: Some(wrapper::<FolderId>("folder-matrix")),
                private_context: false,
            }),
            Evidence::Contextual,
        ),
        matrix_event(
            104,
            observed_at + Duration::seconds(11),
            EventSource::Git,
            EventKind::SourceError,
            EventPayload::SourceError(ghostrace::SourceErrorPayload {
                source: EventSource::Git,
                reason_code: wrapper::<ReasonCode>("fixture_source_error"),
                retryable: true,
            }),
            Evidence::Unknown,
        ),
    ]);
    for event in &mut events {
        // The property corpus is intentionally independent. Parent-chain
        // determinism is exercised separately against causal-chain.jsonl.
        event.parent_event_id = None;
    }
    events
}

fn template_rank(template: ClaimTemplateId) -> usize {
    match template {
        ClaimTemplateId::FilesystemObservation => 0,
        ClaimTemplateId::FrontmostObservation => 1,
        ClaimTemplateId::ShellStartedObservation => 2,
        ClaimTemplateId::ShellFinishedObservation => 3,
        ClaimTemplateId::GitSnapshotObservation => 4,
        ClaimTemplateId::BrowserNavigationObservation => 5,
        ClaimTemplateId::BrowserBookmarkObservation => 6,
        ClaimTemplateId::CollectorStartedObservation => 7,
        ClaimTemplateId::CollectorStoppedObservation => 8,
        ClaimTemplateId::CoverageGap => 9,
        ClaimTemplateId::PolicyDenied => 10,
        ClaimTemplateId::SourceError => 11,
    }
}

fn sorted_matrix_events() -> Vec<EventEnvelope> {
    let mut events = matrix_events();
    events.sort_by_key(|event| template_rank(ClaimTemplateId::for_kind(event.kind)));
    events
}

fn permutations(values: &mut [usize], output: &mut Vec<Vec<usize>>, start: usize) {
    if start == values.len() {
        output.push(values.to_vec());
        return;
    }
    for index in start..values.len() {
        values.swap(start, index);
        permutations(values, output, start + 1);
        values.swap(start, index);
    }
}

fn claim_projection(
    events: impl IntoIterator<Item = EventEnvelope>,
    gap_present: bool,
) -> BTreeMap<Uuid, Vec<u8>> {
    events
        .into_iter()
        .map(|event| {
            let id = event.event_id;
            let claim = render_claim(&event, ClaimLocale::En, gap_present).expect("claim");
            (id, serde_json::to_vec(&claim).expect("claim JSON"))
        })
        .collect()
}

fn journal_for_events(seed: &str, events: &[EventEnvelope]) -> Journal {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed(seed)).expect("journal");
    let origin = IngestionOrigin::fixture();
    journal
        .ingest_batch(&origin, events, &PolicyProfile::fixture_default())
        .expect("independent fixture ingest");
    journal
}

fn query_projection(journal: &Journal, request: QueryRequest) -> BTreeMap<Uuid, Vec<u8>> {
    let mut token = None;
    let mut events = Vec::new();
    loop {
        let page = journal.query_page(&request, token.as_deref()).expect("query page");
        events.extend(page.events.into_iter().map(|stored| stored.event));
        token = page.next_page_token;
        if token.is_none() {
            break;
        }
    }
    claim_projection(events, false)
}

fn causal_chain() -> Vec<EventEnvelope> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    read_fixture(&path).expect("causal fixture")
}

fn explanation_for(events: &[EventEnvelope], seed: &str) -> ghostrace::Explanation {
    let journal = journal_for_events(seed, events);
    explain(&journal, causal_id(8)).expect("explanation")
}

#[test]
fn golden_matrix_covers_every_template_evidence_gap_and_conflict_outcome() {
    let contract = fixture_contract();
    assert_eq!(contract.schema_version, 1);
    assert_eq!(contract.grammar_version, CLAIM_GRAMMAR_VERSION);
    assert!(contract.privacy.synthetic_only);
    assert!(!contract.privacy.user_data_included);
    assert!(!contract.privacy.network_required);
    assert_eq!(
        contract.property_dimensions,
        vec![
            "ingestion_permutation",
            "equal_observed_timestamps",
            "irrelevant_events",
            "query_page_boundaries"
        ]
    );
    assert_eq!(contract.golden_matrix.claim_templates.len(), 12);
    assert_eq!(
        contract.golden_matrix.evidence_levels,
        vec![Evidence::Direct, Evidence::Contextual, Evidence::Inferred, Evidence::Unknown]
    );
    assert_eq!(contract.golden_matrix.gap_states.len(), 2);
    assert_eq!(contract.golden_matrix.gap_states[0].id, "no_recorded_gap");
    assert_eq!(contract.golden_matrix.gap_states[1].id, "recorded_gap");
    assert!(!contract.golden_matrix.gap_states[0].present);
    assert!(contract.golden_matrix.gap_states[1].present);

    let events = sorted_matrix_events();
    let actual_templates =
        events.iter().map(|event| ClaimTemplateId::for_kind(event.kind)).collect::<HashSet<_>>();
    let expected_templates =
        contract.golden_matrix.claim_templates.iter().copied().collect::<HashSet<_>>();
    assert_eq!(actual_templates, expected_templates);

    for event in &events {
        let template = ClaimTemplateId::for_kind(event.kind);
        for evidence in contract.golden_matrix.evidence_levels.iter().copied() {
            for gap in &contract.golden_matrix.gap_states {
                let mut variant = event.clone();
                variant.evidence = evidence;
                let claim =
                    render_claim(&variant, ClaimLocale::En, gap.present).expect("golden claim");
                let repeated =
                    render_claim(&variant, ClaimLocale::En, gap.present).expect("repeat claim");
                assert_eq!(
                    serde_json::to_vec(&claim).expect("claim bytes"),
                    serde_json::to_vec(&repeated).expect("repeat bytes")
                );
                assert_eq!(claim.template, template);
                assert_eq!(claim.evidence, evidence);
                assert_eq!(claim.gap_behavior, template.descriptor().gap_behavior);
                let expected_gap_behavior = if matches!(
                    template,
                    ClaimTemplateId::CoverageGap
                        | ClaimTemplateId::PolicyDenied
                        | ClaimTemplateId::SourceError
                ) {
                    contract.golden_matrix.expected_gap_behavior.status
                } else {
                    contract.golden_matrix.expected_gap_behavior.ordinary
                };
                assert_eq!(claim.gap_behavior, expected_gap_behavior);
                assert_eq!(
                    claim.gap_limited,
                    gap.present && expected_gap_behavior == GapBehavior::LimitInterpretation
                );
                assert!(claim.citations.contains(&variant.event_id));
                assert!(!claim.text.to_ascii_lowercase().contains("caused"));
                assert!(!claim.text.to_ascii_lowercase().contains("intent"));
            }
        }
    }

    for outcome in &contract.conflict_outcomes {
        let event = events
            .iter()
            .find(|event| ClaimTemplateId::for_kind(event.kind) == outcome.template)
            .expect("conflict template event");
        let mut variant = event.clone();
        variant.evidence = outcome.evidence;
        let claim = render_claim(&variant, ClaimLocale::En, true).expect("conflict claim");
        assert_eq!(claim.evidence, outcome.evidence, "conflict {}", outcome.id);
        match outcome.claim_state.as_str() {
            "explicit_unknown" => {
                assert_eq!(claim.gap_behavior, GapBehavior::ExplicitStatus);
                assert!(!claim.gap_limited);
            }
            "downgraded_unknown" => {
                assert_eq!(claim.evidence, Evidence::Unknown);
                assert!(claim.gap_limited);
            }
            other => panic!("unrecognized conflict state {other}"),
        }
    }
}

#[test]
fn property_permutations_equal_times_irrelevant_events_and_page_boundaries_preserve_claims() {
    let events = sorted_matrix_events();
    let policy = PolicyProfile::fixture_default();
    let request = QueryRequest::for_policy(&policy).expect("query request");
    let baseline = claim_projection(
        events.clone().into_iter().filter(|event| event.kind != EventKind::PolicyBlockedSummary),
        false,
    );

    let mut indexes = [0usize, 1, 2, 3];
    let mut orders = Vec::new();
    permutations(&mut indexes, &mut orders, 0);
    assert_eq!(orders.len(), 24);
    for (variant_index, order) in orders.iter().enumerate() {
        let mut permuted = Vec::with_capacity(events.len());
        permuted.extend(order.iter().map(|index| events[*index].clone()));
        permuted.extend(events.iter().skip(4).cloned());
        let journal = journal_for_events(&format!("permutation-{variant_index}"), &permuted);
        let projected = query_projection(&journal, request.clone());
        assert_eq!(projected, baseline, "ingestion permutation changed supported claims");
    }

    let equal_time_events = events
        .iter()
        .cloned()
        .map(|mut event| {
            event.observed_at = timestamp(1_735_689_600);
            event.ingested_at = timestamp(1_735_689_600);
            event
        })
        .collect::<Vec<_>>();
    let equal_baseline = claim_projection(
        equal_time_events
            .clone()
            .into_iter()
            .filter(|event| event.kind != EventKind::PolicyBlockedSummary),
        false,
    );
    for (variant_index, order) in orders.iter().take(8).enumerate() {
        let mut permuted = Vec::with_capacity(equal_time_events.len());
        permuted.extend(order.iter().map(|index| equal_time_events[*index].clone()));
        permuted.extend(equal_time_events.iter().skip(4).cloned());
        let journal = journal_for_events(&format!("equal-time-{variant_index}"), &permuted);
        let projected = query_projection(&journal, request.clone());
        assert_eq!(projected, equal_baseline, "equal timestamps changed supported claims");
    }

    for page_size in [1, 2, 3, 5, 256] {
        let mut paged_request = request.clone();
        paged_request.page_size = page_size;
        let journal = journal_for_events(&format!("page-size-{page_size}"), &events);
        assert_eq!(query_projection(&journal, paged_request), baseline, "page size changed claims");
    }

    let filesystem_only = events
        .iter()
        .filter(|event| event.kind == EventKind::FilesystemChanged)
        .cloned()
        .collect::<Vec<_>>();
    let mut filtered_request = request.clone();
    filtered_request.kind = Some(EventKind::FilesystemChanged);
    filtered_request.page_size = 1;
    let without_irrelevant = journal_for_events("irrelevant-baseline", &filesystem_only);
    let with_irrelevant = journal_for_events("irrelevant-expanded", &events);
    assert_eq!(
        query_projection(&without_irrelevant, filtered_request.clone()),
        query_projection(&with_irrelevant, filtered_request),
        "irrelevant events changed filtered claims"
    );
}

#[test]
fn explanation_bytes_and_identity_are_stable_with_irrelevant_events() {
    let original = causal_chain();
    let baseline = explanation_for(&original, "explanation-baseline");
    let baseline_bytes = baseline.to_pretty_json().expect("pretty explanation").into_bytes();
    let mut expanded = original.clone();
    let mut irrelevant = sorted_matrix_events()
        .into_iter()
        .find(|event| event.kind == EventKind::FrontmostAppChanged)
        .expect("irrelevant event");
    irrelevant.event_id = causal_id(900);
    expanded.push(irrelevant);
    let expanded_explanation = explanation_for(&expanded, "explanation-expanded");
    assert_eq!(
        baseline_bytes,
        expanded_explanation.to_pretty_json().expect("repeat explanation").into_bytes()
    );
    assert_eq!(baseline.explanation_identity, expanded_explanation.explanation_identity);
    assert_eq!(baseline.chain_event_ids, expanded_explanation.chain_event_ids);
    assert_eq!(baseline.statements, expanded_explanation.statements);
}

#[test]
fn mutation_cases_remove_required_observations_and_downgrade_or_remove_claims() {
    let contract = fixture_contract();
    let events = causal_chain();
    let policy = PolicyProfile::fixture_default();
    let query = CorrelationQuery::for_policy(&policy).expect("correlation query");
    let baseline = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[3].clone()],
        &policy,
        &query,
    )
    .expect("baseline correlation");
    let removed_observation = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone()],
        &policy,
        &query,
    )
    .expect("mutated correlation");
    let correlation_mutation = contract
        .mutations
        .iter()
        .find(|mutation| mutation.id == "remove_cross_source_observation")
        .expect("correlation mutation");
    assert_eq!(baseline.evidence, correlation_mutation.baseline_evidence.expect("baseline"));
    assert_eq!(
        removed_observation.evidence,
        correlation_mutation.mutated_evidence.expect("mutated")
    );
    assert_eq!(removed_observation.reason, correlation_mutation.mutated_reason.expect("reason"));

    let original = explanation_for(&events, "mutation-original");
    let removed_id = causal_id(4);
    let mut mutated_events =
        events.into_iter().filter(|event| event.event_id != removed_id).collect::<Vec<_>>();
    for event in &mut mutated_events {
        if event.parent_event_id == Some(removed_id) {
            event.parent_event_id = Some(causal_id(3));
        }
    }
    let mutated = explanation_for(&mutated_events, "mutation-removed-parent");
    let parent_mutation = contract
        .mutations
        .iter()
        .find(|mutation| mutation.id == "remove_parent_observation")
        .expect("parent mutation");
    assert_eq!(
        original.statements.len(),
        parent_mutation.baseline_statement_count.expect("baseline count")
    );
    assert_eq!(
        mutated.statements.len(),
        parent_mutation.mutated_statement_count.expect("mutated count")
    );
    assert!(!mutated.chain_event_ids.contains(&removed_id));
    assert!(!mutated.statements.iter().any(|statement| statement.template
        == parent_mutation.removed_template.expect("removed template")));
    assert_eq!(parent_mutation.mutated_state.as_deref(), Some("claim_removed"));
}
