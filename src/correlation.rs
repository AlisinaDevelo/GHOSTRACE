//! Versioned, policy-bounded cross-source correlation rules.
//!
//! Correlation is an evidence transformation, not a causal oracle.  The
//! registry describes exactly which event fields a rule may inspect, the
//! maximum input/window bounds, exclusions, output evidence, and the fixture
//! classes that keep the rule honest.  Evaluation authorizes every event
//! against the caller's policy before the rule sees its bounded metadata.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    error::GhostraceError,
    model::{EventEnvelope, EventKind, EventSource, Evidence, SnapshotDigest},
    policy::PolicyProfile,
};

pub const CORRELATION_RULE_REGISTRY_VERSION: u32 = 1;
pub const CORRELATION_RULE_SCHEMA_VERSION: u32 = 1;
pub const CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION: u32 = 1;
pub const MAX_CORRELATION_WINDOW_SECONDS: i64 = 60;
pub const MAX_CORRELATION_INPUT_EVENTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationRuleId {
    CrossSourceTemporalAdjacency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationInputField {
    EventId,
    Source,
    EventKind,
    ObservedAt,
    EvidenceLevel,
    PolicyScope,
    CoverageMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationExclusion {
    SameSource,
    UnknownEvidence,
    UnknownCoverage,
    PolicyDenied,
    OutOfWindow,
    PrivateContext,
    UnsupportedEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationFixtureClass {
    Positive,
    Negative,
    Ambiguous,
    Adversarial,
    ClockSkew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationEvidenceOutput {
    InferredOnlyWhenInputsAreBounded,
    UnknownWhenCoverageIsUnbounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CorrelationRuleBounds {
    pub max_window_seconds: i64,
    pub max_input_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CorrelationRuleDescriptor {
    pub id: CorrelationRuleId,
    pub registry_version: u32,
    pub version: u32,
    pub input_fields: &'static [CorrelationInputField],
    pub bounds: CorrelationRuleBounds,
    pub exclusions: &'static [CorrelationExclusion],
    pub evidence_output: CorrelationEvidenceOutput,
    pub counterexample_fixtures: &'static [CorrelationFixtureClass],
}

const CROSS_SOURCE_INPUTS: &[CorrelationInputField] = &[
    CorrelationInputField::EventId,
    CorrelationInputField::Source,
    CorrelationInputField::EventKind,
    CorrelationInputField::ObservedAt,
    CorrelationInputField::EvidenceLevel,
    CorrelationInputField::PolicyScope,
    CorrelationInputField::CoverageMarker,
];

const CROSS_SOURCE_EXCLUSIONS: &[CorrelationExclusion] = &[
    CorrelationExclusion::SameSource,
    CorrelationExclusion::UnknownEvidence,
    CorrelationExclusion::UnknownCoverage,
    CorrelationExclusion::PolicyDenied,
    CorrelationExclusion::OutOfWindow,
    CorrelationExclusion::PrivateContext,
    CorrelationExclusion::UnsupportedEventKind,
];

const CROSS_SOURCE_FIXTURES: &[CorrelationFixtureClass] = &[
    CorrelationFixtureClass::Positive,
    CorrelationFixtureClass::Negative,
    CorrelationFixtureClass::Ambiguous,
    CorrelationFixtureClass::Adversarial,
    CorrelationFixtureClass::ClockSkew,
];

const CROSS_SOURCE_DESCRIPTOR: CorrelationRuleDescriptor = CorrelationRuleDescriptor {
    id: CorrelationRuleId::CrossSourceTemporalAdjacency,
    registry_version: CORRELATION_RULE_REGISTRY_VERSION,
    version: CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION,
    input_fields: CROSS_SOURCE_INPUTS,
    bounds: CorrelationRuleBounds {
        max_window_seconds: MAX_CORRELATION_WINDOW_SECONDS,
        max_input_events: MAX_CORRELATION_INPUT_EVENTS,
    },
    exclusions: CROSS_SOURCE_EXCLUSIONS,
    evidence_output: CorrelationEvidenceOutput::InferredOnlyWhenInputsAreBounded,
    counterexample_fixtures: CROSS_SOURCE_FIXTURES,
};

pub const fn rule_descriptors() -> &'static [CorrelationRuleDescriptor] {
    &[CROSS_SOURCE_DESCRIPTOR]
}

impl CorrelationRuleId {
    pub const fn descriptor(self) -> &'static CorrelationRuleDescriptor {
        match self {
            Self::CrossSourceTemporalAdjacency => &CROSS_SOURCE_DESCRIPTOR,
        }
    }
}

/// The policy and time scope that authorizes a correlation query.  The rule
/// receives this scope, not the policy's selected-root strings or payload
/// fields, and may only inspect events that the policy authorizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrelationQuery {
    pub policy_profile_id: String,
    pub policy_profile_version: u32,
    pub scope_digest: SnapshotDigest,
    pub sources: BTreeSet<EventSource>,
    pub observed_from: Option<DateTime<Utc>>,
    pub observed_until: Option<DateTime<Utc>>,
    pub max_events: usize,
}

impl CorrelationQuery {
    pub fn for_policy(policy: &PolicyProfile) -> Result<Self, GhostraceError> {
        let scope_digest = policy.to_document()?.scope_digest()?;
        let query = Self {
            policy_profile_id: policy.id.clone(),
            policy_profile_version: policy.version,
            scope_digest,
            sources: policy.enabled_sources.clone(),
            observed_from: None,
            observed_until: None,
            max_events: MAX_CORRELATION_INPUT_EVENTS,
        };
        query.validate(policy)?;
        Ok(query)
    }

    pub fn with_window(
        mut self,
        observed_from: Option<DateTime<Utc>>,
        observed_until: Option<DateTime<Utc>>,
    ) -> Result<Self, GhostraceError> {
        self.observed_from = observed_from;
        self.observed_until = observed_until;
        if self.observed_from.zip(self.observed_until).is_some_and(|(from, until)| {
            from > until
                || until.signed_duration_since(from).num_seconds() > MAX_CORRELATION_WINDOW_SECONDS
        }) {
            return Err(GhostraceError::QueryInvalid);
        }
        Ok(self)
    }

    pub fn with_sources<Sources>(mut self, sources: Sources) -> Result<Self, GhostraceError>
    where
        Sources: IntoIterator<Item = EventSource>,
    {
        self.sources = sources.into_iter().collect();
        if self.sources.is_empty() {
            return Err(GhostraceError::QueryInvalid);
        }
        Ok(self)
    }

    pub fn with_max_events(mut self, max_events: usize) -> Result<Self, GhostraceError> {
        self.max_events = max_events;
        if !(1..=MAX_CORRELATION_INPUT_EVENTS).contains(&max_events) {
            return Err(GhostraceError::QueryInvalid);
        }
        Ok(self)
    }

    fn validate(&self, policy: &PolicyProfile) -> Result<(), GhostraceError> {
        if self.policy_profile_id != policy.id
            || self.policy_profile_version != policy.version
            || self.scope_digest != policy.to_document()?.scope_digest()?
            || self.sources.is_empty()
            || !self.sources.is_subset(&policy.enabled_sources)
            || !(1..=MAX_CORRELATION_INPUT_EVENTS).contains(&self.max_events)
            || self.observed_from.zip(self.observed_until).is_some_and(|(from, until)| {
                from > until
                    || until.signed_duration_since(from).num_seconds()
                        > MAX_CORRELATION_WINDOW_SECONDS
            })
        {
            return Err(GhostraceError::QueryScopeMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationReason {
    BoundedCrossSourceAdjacency,
    NoEligibleInputs,
    RequiresDistinctSources,
    InputsExceedBound,
    PolicyScopeNotAuthorized,
    UnknownEvidence,
    UnknownCoverage,
    ClockSkew,
    EqualObservedTime,
    OutsideWindow,
    UnsupportedEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorrelationIdentity {
    pub registry_version: u32,
    pub rule_id: CorrelationRuleId,
    pub rule_version: u32,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorrelationResult {
    pub identity: CorrelationIdentity,
    pub evidence: Evidence,
    pub input_event_ids: Vec<Uuid>,
    pub gap_limited: bool,
    pub reason: CorrelationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleObservation {
    event_id: Uuid,
    source: EventSource,
    observed_at: DateTime<Utc>,
}

pub fn evaluate(
    rule: CorrelationRuleId,
    events: &[EventEnvelope],
    policy: &PolicyProfile,
    query: &CorrelationQuery,
) -> Result<CorrelationResult, GhostraceError> {
    let descriptor = rule.descriptor();
    query.validate(policy)?;
    if events.len() > query.max_events {
        return Ok(result(
            descriptor,
            query,
            &[],
            Evidence::Unknown,
            true,
            CorrelationReason::InputsExceedBound,
        ));
    }

    let mut visible = Vec::with_capacity(events.len());
    let mut scope_rejected = false;
    let mut coverage_unknown = false;
    let mut evidence_unknown = false;
    let mut clock_skew = false;
    let mut previous_observed_at = None;

    for event in events {
        if policy.authorize(event).is_err() {
            scope_rejected = true;
            continue;
        }
        if !query.sources.contains(&event.source)
            || query.observed_from.is_some_and(|from| event.observed_at < from)
            || query.observed_until.is_some_and(|until| event.observed_at > until)
        {
            continue;
        }
        if previous_observed_at.is_some_and(|previous| event.observed_at < previous) {
            clock_skew = true;
        }
        previous_observed_at = Some(event.observed_at);
        if is_coverage_marker(event.kind) {
            coverage_unknown = true;
        }
        if !matches!(event.evidence, Evidence::Direct | Evidence::Contextual) {
            evidence_unknown = true;
        }
        if !is_supported_observation(event.kind) {
            continue;
        }
        visible.push(VisibleObservation {
            event_id: event.event_id,
            source: event.source,
            observed_at: event.observed_at,
        });
    }

    if scope_rejected {
        return Ok(result(
            descriptor,
            query,
            &[],
            Evidence::Unknown,
            true,
            CorrelationReason::PolicyScopeNotAuthorized,
        ));
    }
    let mut input_ids = visible.iter().map(|observation| observation.event_id).collect::<Vec<_>>();
    input_ids.sort_unstable();
    if coverage_unknown {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::UnknownCoverage,
        ));
    }
    if evidence_unknown {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::UnknownEvidence,
        ));
    }
    if clock_skew {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::ClockSkew,
        ));
    }
    if visible.len() < 2 {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            if visible.is_empty() {
                CorrelationReason::NoEligibleInputs
            } else {
                CorrelationReason::RequiresDistinctSources
            },
        ));
    }

    visible.sort_by_key(|observation| (observation.observed_at, observation.event_id));
    let first = visible[0];
    let second = visible[1];
    input_ids = vec![first.event_id, second.event_id];
    if first.source == second.source {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::RequiresDistinctSources,
        ));
    }
    let delta = second.observed_at.signed_duration_since(first.observed_at).num_seconds();
    if delta == 0 {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::EqualObservedTime,
        ));
    }
    if delta < 0 || delta > descriptor.bounds.max_window_seconds {
        return Ok(result(
            descriptor,
            query,
            &input_ids,
            Evidence::Unknown,
            true,
            CorrelationReason::OutsideWindow,
        ));
    }
    Ok(result(
        descriptor,
        query,
        &input_ids,
        Evidence::Inferred,
        false,
        CorrelationReason::BoundedCrossSourceAdjacency,
    ))
}

fn is_supported_observation(kind: EventKind) -> bool {
    !is_coverage_marker(kind)
        && matches!(
            kind,
            EventKind::FilesystemChanged
                | EventKind::FrontmostAppChanged
                | EventKind::ShellStarted
                | EventKind::ShellFinished
                | EventKind::GitSnapshot
                | EventKind::BrowserNavigation
                | EventKind::BrowserBookmarkChanged
        )
}

fn is_coverage_marker(kind: EventKind) -> bool {
    matches!(kind, EventKind::Gap | EventKind::PolicyBlockedSummary | EventKind::SourceError)
}

fn result(
    descriptor: &CorrelationRuleDescriptor,
    query: &CorrelationQuery,
    input_event_ids: &[Uuid],
    evidence: Evidence,
    gap_limited: bool,
    reason: CorrelationReason,
) -> CorrelationResult {
    CorrelationResult {
        identity: CorrelationIdentity {
            registry_version: descriptor.registry_version,
            rule_id: descriptor.id,
            rule_version: descriptor.version,
            digest: identity_digest(
                descriptor.registry_version,
                descriptor.id,
                descriptor.version,
                query,
                input_event_ids,
            ),
        },
        evidence,
        input_event_ids: input_event_ids.to_vec(),
        gap_limited,
        reason,
    }
}

/// Deterministically identify an explanation from its ordered event IDs and
/// the rule version used to render it.  This is intentionally a digest rather
/// than a path, payload, or user-controlled label.
pub fn explanation_identity(
    events: &[EventEnvelope],
    policy_profile_id: &str,
    policy_profile_version: u32,
) -> String {
    let event_ids = events.iter().map(|event| event.event_id).collect::<Vec<_>>();
    identity_digest_for_parts(
        CORRELATION_RULE_REGISTRY_VERSION,
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        CROSS_SOURCE_TEMPORAL_ADJACENCY_VERSION,
        policy_profile_id,
        policy_profile_version,
        &event_ids,
    )
}

/// Test and migration helper for proving that a rule-version change changes
/// identity while retaining the same historical inputs.
pub fn explanation_identity_for_rule_version(
    events: &[EventEnvelope],
    policy_profile_id: &str,
    policy_profile_version: u32,
    rule_version: u32,
) -> String {
    let event_ids = events.iter().map(|event| event.event_id).collect::<Vec<_>>();
    identity_digest_for_parts(
        CORRELATION_RULE_REGISTRY_VERSION,
        CorrelationRuleId::CrossSourceTemporalAdjacency,
        rule_version,
        policy_profile_id,
        policy_profile_version,
        &event_ids,
    )
}

fn identity_digest(
    registry_version: u32,
    rule_id: CorrelationRuleId,
    rule_version: u32,
    query: &CorrelationQuery,
    input_event_ids: &[Uuid],
) -> String {
    let canonical = serde_json::to_vec(&(
        CORRELATION_RULE_SCHEMA_VERSION,
        registry_version,
        rule_id,
        rule_version,
        query,
        input_event_ids,
    ))
    .expect("correlation query identity serialization cannot fail");
    digest_bytes(&canonical)
}

fn identity_digest_for_parts(
    registry_version: u32,
    rule_id: CorrelationRuleId,
    rule_version: u32,
    policy_profile_id: &str,
    policy_profile_version: u32,
    input_event_ids: &[Uuid],
) -> String {
    let canonical = serde_json::to_vec(&(
        CORRELATION_RULE_SCHEMA_VERSION,
        registry_version,
        rule_id,
        rule_version,
        policy_profile_id,
        policy_profile_version,
        input_event_ids,
    ))
    .expect("correlation identity serialization cannot fail");
    digest_bytes(&canonical)
}

fn digest_bytes(canonical: &[u8]) -> String {
    let digest = Sha256::digest(canonical);
    let mut value = String::with_capacity(71);
    value.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing a digest cannot fail");
    }
    value
}
