//! Versioned temporal evidence and deterministic display ordering.
//!
//! Source timestamps are observations, not causal proof.  The journal's
//! durable ingest sequence is a separate monotonic fact.  This module keeps
//! those facts distinct, supplies one versioned total-order contract for the
//! database and export paths, and reports the cases where a display order
//! necessarily falls back to ingest sequence or an event-ID tie-breaker.

use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Version of the `(source_observed_at, ingest_seq, event_id)` display order.
pub const ORDERING_CONTRACT_VERSION: u32 = 1;
/// Version of the adapter-boundary timing fixture shape.
pub const TEMPORAL_OBSERVATION_SCHEMA_VERSION: u32 = 1;
/// A lag above this bound is reported as delayed delivery context.
pub const TEMPORAL_DELAY_THRESHOLD_SECONDS: i64 = 60;

/// Which retained fact supports a particular ordering decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalEvidenceBasis {
    SourceObservation,
    IngestSequence,
    EventId,
}

/// Timing facts at an adapter boundary.  `source_observed_at` is optional
/// because a source may not provide a wall-clock value.  `ingested_at` is the
/// local wall-clock receipt, `monotonic_sequence` is optional process-local
/// elapsed-time evidence, and `ingest_seq` is the durable journal sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalObservation {
    pub event_id: Uuid,
    pub source_observed_at: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    pub monotonic_sequence: Option<u64>,
    pub ingest_seq: u64,
}

impl TemporalObservation {
    /// Returns the stable key used by the database and export order.
    pub fn stable_order_key(&self) -> StableOrderKey {
        StableOrderKey {
            contract_version: ORDERING_CONTRACT_VERSION,
            source_observed_at: self.source_observed_at,
            ingest_seq: self.ingest_seq,
            event_id: self.event_id,
        }
    }
}

/// The public, serialized ordering key.  Missing source time is represented
/// explicitly; its comparison falls back to ingest sequence after events with
/// a known source observation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableOrderKey {
    pub contract_version: u32,
    pub source_observed_at: Option<DateTime<Utc>>,
    pub ingest_seq: u64,
    pub event_id: Uuid,
}

impl Ord for StableOrderKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.contract_version
            .cmp(&other.contract_version)
            .then_with(|| match (self.source_observed_at, other.source_observed_at) {
                (Some(left), Some(right)) => left.cmp(&right),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then_with(|| self.ingest_seq.cmp(&other.ingest_seq))
            .then_with(|| self.event_id.cmp(&other.event_id))
    }
}

impl PartialOrd for StableOrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A per-event explanation of which fact supports display order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalOrderDecision {
    pub event_id: Uuid,
    pub basis: TemporalEvidenceBasis,
    pub ambiguous: bool,
    pub reason: Option<String>,
}

/// Deterministic analysis of observations supplied in durable ingest order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalAnalysis {
    pub decisions: Vec<TemporalOrderDecision>,
    pub warnings: Vec<String>,
}

/// Analyze clock skew, delayed delivery, equal timestamps, and missing source
/// time without upgrading any of them into causal evidence.
pub fn analyze_temporal_observations(observations: &[TemporalObservation]) -> TemporalAnalysis {
    let mut decisions = Vec::with_capacity(observations.len());
    let mut warnings = Vec::new();
    let mut previous: Option<&TemporalObservation> = None;
    for observation in observations {
        let mut basis = if observation.source_observed_at.is_none() {
            TemporalEvidenceBasis::IngestSequence
        } else {
            TemporalEvidenceBasis::SourceObservation
        };
        let mut ambiguous = false;
        let mut reason = None;

        if observation.source_observed_at.is_none() {
            let warning = format!(
                "temporal ambiguity: source observation time is missing for event {}; display order uses ingest sequence",
                observation.event_id
            );
            warnings.push(warning.clone());
            ambiguous = true;
            reason = Some("source observation time is missing".to_owned());
        }

        if let Some(previous) = previous {
            match (previous.source_observed_at, observation.source_observed_at) {
                (Some(previous_time), Some(current_time)) if current_time < previous_time => {
                    warnings.push(format!(
                        "temporal ambiguity: source clock rollback between events {} and {}; ingest sequence remains the durable order",
                        previous.event_id, observation.event_id
                    ));
                    ambiguous = true;
                    reason = Some("source clock rollback".to_owned());
                }
                (Some(previous_time), Some(current_time)) if current_time == previous_time => {
                    basis = if previous.ingest_seq == observation.ingest_seq {
                        TemporalEvidenceBasis::EventId
                    } else {
                        TemporalEvidenceBasis::IngestSequence
                    };
                    ambiguous = true;
                    let warning = format!(
                        "temporal ambiguity: equal source observation timestamps for events {} and {}; display order uses ingest sequence then event ID",
                        previous.event_id, observation.event_id
                    );
                    warnings.push(warning.clone());
                    reason = Some("equal source observation timestamps".to_owned());
                }
                _ => {}
            }
            if let (Some(previous_sequence), Some(current_sequence)) =
                (previous.monotonic_sequence, observation.monotonic_sequence)
            {
                if current_sequence < previous_sequence {
                    warnings.push(format!(
                        "temporal ambiguity: monotonic sequence regressed between events {} and {}",
                        previous.event_id, observation.event_id
                    ));
                    ambiguous = true;
                    reason = Some("monotonic sequence regression".to_owned());
                }
            }
        }

        if let Some(source_time) = observation.source_observed_at {
            let lag = observation.ingested_at.signed_duration_since(source_time).num_seconds();
            if lag > TEMPORAL_DELAY_THRESHOLD_SECONDS {
                warnings.push(format!(
                    "temporal ambiguity: ingest lag of {lag}s for event {}; source order may reflect a delayed batch or sleep",
                    observation.event_id
                ));
                ambiguous = true;
                reason = Some("delayed ingest".to_owned());
            }
        }

        decisions.push(TemporalOrderDecision {
            event_id: observation.event_id,
            basis,
            ambiguous,
            reason,
        });
        previous = Some(observation);
    }
    TemporalAnalysis { decisions, warnings }
}

/// Compare event metadata using the same key used by query and export.
pub fn compare_event_order(
    left_observed_at: DateTime<Utc>,
    left_ingest_seq: u64,
    left_event_id: Uuid,
    right_observed_at: DateTime<Utc>,
    right_ingest_seq: u64,
    right_event_id: Uuid,
) -> Ordering {
    StableOrderKey {
        contract_version: ORDERING_CONTRACT_VERSION,
        source_observed_at: Some(left_observed_at),
        ingest_seq: left_ingest_seq,
        event_id: left_event_id,
    }
    .cmp(&StableOrderKey {
        contract_version: ORDERING_CONTRACT_VERSION,
        source_observed_at: Some(right_observed_at),
        ingest_seq: right_ingest_seq,
        event_id: right_event_id,
    })
}
