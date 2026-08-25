use std::{fs, path::PathBuf};

use chrono::{DateTime, Utc};
use ghostrace::{
    evaluate_correlation, explanation_identity, explanation_identity_for_rule_version,
    read_fixture, rule_descriptors, CorrelationFixtureClass, CorrelationReason, CorrelationRuleId,
    Evidence, PolicyProfile, CORRELATION_RULE_REGISTRY_VERSION,
    CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RuleFixtureManifest {
    schema_version: u32,
    registry_version: u32,
    rules: Vec<RuleFixtureRule>,
    privacy: RuleFixturePrivacy,
}

#[derive(Debug, Deserialize)]
struct RuleFixtureRule {
    id: String,
    version: u32,
    fixtures: Vec<RuleFixtureCase>,
}

#[derive(Debug, Deserialize)]
struct RuleFixtureCase {
    id: String,
    class: CorrelationFixtureClass,
}

#[derive(Debug, Deserialize)]
struct RuleFixturePrivacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
}

fn fixture() -> Vec<ghostrace::EventEnvelope> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    read_fixture(path).expect("causal fixture")
}

fn timestamp(value: &str) -> DateTime<Utc> {
    value.parse().expect("timestamp")
}

fn query(policy: &PolicyProfile) -> ghostrace::CorrelationQuery {
    ghostrace::CorrelationQuery::for_policy(policy).expect("correlation query")
}

#[test]
fn registry_is_inspectable_and_fixture_manifest_covers_every_counterexample_class() {
    let descriptor = CorrelationRuleId::CrossSourceTemporalAdjacency.descriptor();
    assert_eq!(descriptor.registry_version, CORRELATION_RULE_REGISTRY_VERSION);
    assert_eq!(descriptor.version, CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION);
    assert!(descriptor.bounds.max_window_seconds > 0);
    assert!(descriptor.bounds.max_input_events > 0);
    assert!(descriptor.input_fields.contains(&ghostrace::CorrelationInputField::PolicyScope));
    assert!(descriptor.input_fields.contains(&ghostrace::CorrelationInputField::CoverageMarker));
    assert!(descriptor.exclusions.contains(&ghostrace::CorrelationExclusion::UnknownCoverage));
    assert_eq!(rule_descriptors().len(), 1);

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/correlation-rules-v1.json");
    let manifest: RuleFixtureManifest =
        serde_json::from_str(&fs::read_to_string(path).expect("rule fixture")).expect("JSON");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.registry_version, CORRELATION_RULE_REGISTRY_VERSION);
    assert!(manifest.privacy.synthetic_only);
    assert!(!manifest.privacy.user_data_included);
    assert!(!manifest.privacy.network_required);
    assert_eq!(manifest.rules.len(), 1);
    let rule = &manifest.rules[0];
    assert_eq!(rule.id, "cross_source_temporal_adjacency");
    assert_eq!(rule.version, CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION);
    let classes = rule.fixtures.iter().map(|fixture| fixture.class).collect::<Vec<_>>();
    assert_eq!(rule.fixtures.len(), 5);
    assert!(rule.fixtures.iter().any(|fixture| fixture.id == "positive_cross_source_observations"));
    assert!(classes.contains(&CorrelationFixtureClass::Positive));
    assert!(classes.contains(&CorrelationFixtureClass::Negative));
    assert!(classes.contains(&CorrelationFixtureClass::Ambiguous));
    assert!(classes.contains(&CorrelationFixtureClass::Adversarial));
    assert!(classes.contains(&CorrelationFixtureClass::ClockSkew));
}

#[test]
fn positive_and_negative_results_are_bounded_and_never_upgrade_unknown_scope() {
    let events = fixture();
    let policy = PolicyProfile::fixture_default();
    let positive = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[3].clone()],
        &policy,
        &query(&policy),
    )
    .expect("positive evaluation");
    assert_eq!(positive.evidence, Evidence::Inferred);
    assert_eq!(positive.reason, CorrelationReason::BoundedCrossSourceAdjacency);
    assert_eq!(positive.input_event_ids, vec![events[1].event_id, events[3].event_id]);
    assert!(!positive.gap_limited);

    let same_source = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[3].clone()],
        &policy,
        &query(&policy),
    )
    .expect("negative evaluation");
    assert_eq!(same_source.evidence, Evidence::Unknown);
    assert_eq!(same_source.reason, CorrelationReason::RequiresDistinctSources);
    assert!(same_source.gap_limited);

    let mut adversarial = events[3].clone();
    adversarial.policy_profile_version = 99;
    let scoped = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), adversarial],
        &policy,
        &query(&policy),
    )
    .expect("adversarial evaluation");
    assert_eq!(scoped.evidence, Evidence::Unknown);
    assert_eq!(scoped.reason, CorrelationReason::PolicyScopeNotAuthorized);
    assert!(scoped.input_event_ids.is_empty());
}

#[test]
fn unknown_intervals_evidence_and_clock_skew_abstain() {
    let events = fixture();
    let policy = PolicyProfile::fixture_default();
    let mut unknown = events[3].clone();
    unknown.evidence = Evidence::Unknown;
    let evidence = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), unknown],
        &policy,
        &query(&policy),
    )
    .expect("unknown evidence evaluation");
    assert_eq!(evidence.evidence, Evidence::Unknown);
    assert_eq!(evidence.reason, CorrelationReason::UnknownEvidence);

    let gap = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[5].clone(), events[3].clone()],
        &policy,
        &query(&policy),
    )
    .expect("gap evaluation");
    assert_eq!(gap.evidence, Evidence::Unknown);
    assert_eq!(gap.reason, CorrelationReason::UnknownCoverage);
    assert!(gap.gap_limited);

    let mut skewed_first = events[1].clone();
    skewed_first.observed_at = timestamp("2026-01-01T00:00:10Z");
    let mut skewed_second = events[3].clone();
    skewed_second.observed_at = timestamp("2026-01-01T00:00:01Z");
    let skew = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[skewed_first, skewed_second],
        &policy,
        &query(&policy),
    )
    .expect("clock-skew evaluation");
    assert_eq!(skew.evidence, Evidence::Unknown);
    assert_eq!(skew.reason, CorrelationReason::ClockSkew);
}

#[test]
fn query_scope_bounds_and_rule_version_identity_are_reproducible() {
    let events = fixture();
    let policy = PolicyProfile::fixture_default();
    let bounded = query(&policy)
        .with_window(
            Some(timestamp("2026-01-01T00:00:00Z")),
            Some(timestamp("2026-01-01T00:00:03Z")),
        )
        .expect("window");
    let result = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[3].clone()],
        &policy,
        &bounded,
    )
    .expect("bounded evaluation");
    assert_eq!(result.evidence, Evidence::Inferred);

    let narrow =
        query(&policy).with_sources([ghostrace::EventSource::Shell]).expect("source filter");
    let narrow_result = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[3].clone()],
        &policy,
        &narrow,
    )
    .expect("narrow evaluation");
    assert_eq!(narrow_result.evidence, Evidence::Unknown);
    assert_eq!(narrow_result.reason, CorrelationReason::RequiresDistinctSources);

    let identity = explanation_identity(&events[..4], &policy.id, policy.version);
    assert_eq!(identity, explanation_identity(&events[..4], &policy.id, policy.version));
    assert_ne!(
        identity,
        explanation_identity_for_rule_version(
            &events[..4],
            &policy.id,
            policy.version,
            CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION + 1,
        )
    );

    let oversized = query(&policy).with_max_events(1).expect("small bound");
    let bounded_result = evaluate_correlation(
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        &[events[1].clone(), events[3].clone()],
        &policy,
        &oversized,
    )
    .expect("oversized evaluation");
    assert_eq!(bounded_result.evidence, Evidence::Unknown);
    assert_eq!(bounded_result.reason, CorrelationReason::InputsExceedBound);

    let malformed = query(&policy).with_window(
        Some(timestamp("2026-01-01T00:00:04Z")),
        Some(timestamp("2026-01-01T00:00:03Z")),
    );
    assert!(malformed.is_err());
    let oversized_window = query(&policy).with_window(
        Some(timestamp("2026-01-01T00:00:00Z")),
        Some(timestamp("2026-01-01T00:01:01Z")),
    );
    assert!(oversized_window.is_err());
}
