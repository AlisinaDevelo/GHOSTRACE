//! Deterministic parent-chain explanation with explicit evidence labels.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use uuid::Uuid;

use crate::{
    claims::{render_claim, ClaimLocale, ClaimTemplateId, GapBehavior, CLAIM_GRAMMAR_VERSION},
    error::GhostraceError,
    journal::Journal,
    model::{EventEnvelope, EventKind, Evidence},
    ordering::{analyze_temporal_observations, TemporalObservation},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExplanationStatement {
    pub event_id: Uuid,
    pub parent_event_id: Option<Uuid>,
    pub evidence: Evidence,
    pub citations: Vec<Uuid>,
    pub grammar_version: u32,
    pub template: ClaimTemplateId,
    pub locale: ClaimLocale,
    pub gap_behavior: GapBehavior,
    pub gap_limited: bool,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoverageSummary {
    pub chain_event_count: usize,
    pub gap_event_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Explanation {
    pub target_event_id: Uuid,
    pub chain_event_ids: Vec<Uuid>,
    pub statements: Vec<ExplanationStatement>,
    pub coverage: CoverageSummary,
}

impl Explanation {
    pub fn to_pretty_json(&self) -> Result<String, GhostraceError> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

pub fn explain(journal: &Journal, target: Uuid) -> Result<Explanation, GhostraceError> {
    let mut reverse_chain = Vec::<EventEnvelope>::new();
    let mut seen = HashSet::new();
    let mut current = journal.event(target)?;
    loop {
        if !seen.insert(current.event_id) {
            return Err(GhostraceError::InvalidEvent("parent chain contains a cycle".to_owned()));
        }
        let parent = current.parent_event_id;
        reverse_chain.push(current);
        match parent {
            Some(parent_id) => current = journal.event(parent_id)?,
            None => break,
        }
    }
    reverse_chain.reverse();
    let chain_event_ids = reverse_chain.iter().map(|event| event.event_id).collect::<Vec<_>>();
    let gap_events =
        reverse_chain.iter().filter(|event| event.kind == EventKind::Gap).collect::<Vec<_>>();
    let mut warnings = gap_events
        .iter()
        .map(|event| format!("coverage gap is present; evidence cites event {}", event.event_id))
        .collect::<Vec<_>>();
    warnings.extend(reverse_chain.iter().filter(|event| event.kind == EventKind::SourceError).map(
        |event| format!("source error limits coverage; evidence cites event {}", event.event_id),
    ));
    let ingest_sequences = journal
        .events()?
        .into_iter()
        .map(|stored| (stored.event.event_id, stored.ingest_seq))
        .collect::<HashMap<_, _>>();
    let mut temporal_observations = reverse_chain
        .iter()
        .map(|event| TemporalObservation {
            event_id: event.event_id,
            source_observed_at: Some(event.observed_at),
            ingested_at: event.ingested_at,
            monotonic_sequence: None,
            ingest_seq: *ingest_sequences.get(&event.event_id).unwrap_or(&0),
        })
        .collect::<Vec<_>>();
    temporal_observations.sort_by_key(|observation| observation.ingest_seq);
    warnings.extend(analyze_temporal_observations(&temporal_observations).warnings);
    let statements = reverse_chain
        .iter()
        .map(|event| {
            let claim = render_claim(event, ClaimLocale::En, !gap_events.is_empty())?;
            debug_assert_eq!(claim.grammar_version, CLAIM_GRAMMAR_VERSION);
            Ok(ExplanationStatement {
                event_id: event.event_id,
                parent_event_id: event.parent_event_id,
                evidence: claim.evidence,
                citations: claim.citations,
                grammar_version: claim.grammar_version,
                template: claim.template,
                locale: claim.locale,
                gap_behavior: claim.gap_behavior,
                gap_limited: claim.gap_limited,
                statement: claim.text,
            })
        })
        .collect::<Result<Vec<_>, GhostraceError>>()?;
    Ok(Explanation {
        target_event_id: target,
        chain_event_ids,
        statements,
        coverage: CoverageSummary {
            chain_event_count: reverse_chain.len(),
            gap_event_count: gap_events.len(),
            warnings,
        },
    })
}
