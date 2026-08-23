//! Deterministic parent-chain explanation with explicit evidence labels.

use std::collections::HashSet;

use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::GhostraceError,
    journal::Journal,
    model::{EventEnvelope, EventKind, Evidence},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExplanationStatement {
    pub event_id: Uuid,
    pub parent_event_id: Option<Uuid>,
    pub evidence: Evidence,
    pub citations: Vec<Uuid>,
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
    let statements = reverse_chain
        .iter()
        .map(|event| ExplanationStatement {
            event_id: event.event_id,
            parent_event_id: event.parent_event_id,
            evidence: event.evidence,
            citations: vec![event.event_id],
            statement: event.payload.summary(),
        })
        .collect();
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
