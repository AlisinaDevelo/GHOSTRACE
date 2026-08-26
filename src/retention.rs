//! Deterministic, read-only retention planning.
//!
//! A retention plan is an inspectable scope decision, not a deletion command.
//! It is evaluated inside one SQLite read transaction, binds the committed
//! ingest upper bound and an authenticated candidate-set digest, and carries
//! enough bounded metadata for a later destructive command to refuse scope
//! expansion. Exports, database backups, and legal holds are deliberately
//! outside this contract.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    crypto::KeyProvider,
    error::GhostraceError,
    export_schema::hex_digest,
    journal::for_each_retention_row,
    model::{EventEnvelope, EventPayload, EventSource, RootId, SnapshotDigest},
};

/// Retention policy and plan wire-contract version.
pub const RETENTION_PLAN_SCHEMA_VERSION: u32 = 1;
/// Version of the logical retention-deletion receipt contract.
pub const RETENTION_DELETION_SCHEMA_VERSION: u32 = 1;
/// The documented default is a 90-day window anchored at the caller's
/// supplied `as_of` timestamp. The anchor is explicit so tests and receipts
/// remain deterministic.
pub const DEFAULT_RETENTION_DAYS: i64 = 90;
/// Gap summaries are intentionally bounded even when a journal contains a
/// very large number of coverage markers.
pub const MAX_RETENTION_GAP_SUMMARIES: usize = 1024;
/// Generation zero identifies a legacy payload envelope that pre-dates the
/// self-describing ciphertext metadata. It is never a current key generation.
pub const LEGACY_KEY_GENERATION: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetentionStorageMetadata {
    pub(crate) ingest_seq: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) key_generation: u32,
}

const NON_GOAL_EXPORTS: &str = "exports are separate plaintext artifacts and are not journal rows";
const NON_GOAL_BACKUPS: &str =
    "database backups and SQLite sidecars are separate artifacts and are not selected";
const NON_GOAL_LEGAL_HOLDS: &str =
    "legal holds are not implemented and are never inferred from an export or backup";

fn preserve_gaps_by_default() -> bool {
    true
}

/// Selection policy for the journal event table.
///
/// `source` and `root_id` constrain the scope. `before` selects observations
/// older than a UTC cutoff. The two `retain_at_most_*` fields select the oldest
/// rows necessary to leave at most the requested newest event count/bytes in
/// the scope. Multiple selectors are unioned; gap preservation is applied
/// before all selectors. The precedence is therefore scope, gap protection,
/// time, event-count, then byte-limit for the reported primary reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    pub schema_version: u32,
    #[serde(default)]
    pub before: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source: Option<EventSource>,
    #[serde(default)]
    pub root_id: Option<RootId>,
    #[serde(default)]
    pub retain_at_most_events: Option<u64>,
    #[serde(default)]
    pub retain_at_most_bytes: Option<u64>,
    #[serde(default = "preserve_gaps_by_default")]
    pub preserve_gaps: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            schema_version: RETENTION_PLAN_SCHEMA_VERSION,
            before: None,
            source: None,
            root_id: None,
            retain_at_most_events: None,
            retain_at_most_bytes: None,
            preserve_gaps: true,
        }
    }
}

impl RetentionPolicy {
    /// Build the documented default policy: delete observations older than
    /// ninety days relative to `as_of`, while retaining coverage gaps.
    pub fn default_at(as_of: DateTime<Utc>) -> Self {
        Self { before: Some(as_of - Duration::days(DEFAULT_RETENTION_DAYS)), ..Self::default() }
    }

    /// Build a policy with one explicit time cutoff and the safe gap default.
    pub fn before(cutoff: DateTime<Utc>) -> Self {
        Self { before: Some(cutoff), ..Self::default() }
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != RETENTION_PLAN_SCHEMA_VERSION {
            return Err(GhostraceError::RetentionPolicyInvalid(
                "retention plan schema version is unsupported".to_owned(),
            ));
        }
        if self.before.is_none()
            && self.retain_at_most_events.is_none()
            && self.retain_at_most_bytes.is_none()
        {
            return Err(GhostraceError::RetentionPolicyInvalid(
                "at least one time, event-count, or byte selector is required".to_owned(),
            ));
        }
        if self.root_id.is_some()
            && self.source.is_some_and(|source| source != EventSource::Filesystem)
        {
            return Err(GhostraceError::RetentionPolicyInvalid(
                "root scope is only valid for filesystem events".to_owned(),
            ));
        }
        Ok(())
    }

    fn matches_scope(&self, event: &EventEnvelope) -> bool {
        self.source.is_none_or(|source| source == event.source)
            && self.root_id.as_ref().is_none_or(|root| {
                event.payload.root_id().is_some_and(|event_root| event_root == root.as_str())
            })
    }
}

/// The primary reason assigned to an affected event. Reasons are disjoint and
/// follow the documented time, event-count, byte-limit precedence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionSelectionReason {
    Time,
    EventCount,
    ByteLimit,
}

/// A bounded description of a gap in the selected scope. `selected` is false
/// when the default gap-preservation rule protects it from deletion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionGapSummary {
    pub event_id: Uuid,
    pub source: EventSource,
    pub observed_at: DateTime<Utc>,
    pub reason_code: String,
    pub dropped_count: u64,
    pub selected: bool,
}

/// A confirmation capability for the future destructive retention command.
/// It contains no payloads or paths and cannot name rows outside the original
/// snapshot boundary and candidate-set digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionConfirmation {
    pub schema_version: u32,
    pub plan_digest: SnapshotDigest,
    pub candidate_set_digest: SnapshotDigest,
    pub snapshot_boundary: u64,
    pub confirmed: bool,
}

impl RetentionConfirmation {
    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != RETENTION_PLAN_SCHEMA_VERSION {
            return Err(GhostraceError::RetentionPolicyInvalid(
                "retention confirmation schema version is unsupported".to_owned(),
            ));
        }
        if !self.confirmed {
            return Err(GhostraceError::RetentionConfirmationMismatch);
        }
        Ok(())
    }
}

/// Read-only output of a retention dry-run. The candidate IDs are represented
/// by a digest rather than retained in an unbounded vector; a future deletion
/// operation must use this digest and the snapshot boundary to prove that its
/// scope did not expand.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPlan {
    pub schema_version: u32,
    pub policy: RetentionPolicy,
    pub plan_digest: SnapshotDigest,
    pub candidate_set_digest: SnapshotDigest,
    pub snapshot_boundary: u64,
    pub snapshot_event_count: u64,
    pub scoped_event_count: u64,
    pub scoped_bytes: u64,
    pub eligible_event_count: u64,
    pub eligible_bytes: u64,
    pub affected_event_count: u64,
    pub affected_observed_from: Option<DateTime<Utc>>,
    pub affected_observed_until: Option<DateTime<Utc>>,
    pub affected_ingested_from: Option<DateTime<Utc>>,
    pub affected_ingested_until: Option<DateTime<Utc>>,
    pub affected_sources: BTreeMap<EventSource, u64>,
    pub affected_key_generations: BTreeMap<u32, u64>,
    pub scoped_gap_count: u64,
    pub affected_gap_count: u64,
    pub protected_gap_count: u64,
    pub gaps: Vec<RetentionGapSummary>,
    pub gaps_truncated: bool,
    /// This is a conservative lower bound consisting of encrypted payload
    /// bytes. SQLite page reuse, WAL truncation, and external copies are not
    /// represented as guaranteed filesystem reclamation.
    pub estimated_reclaimed_bytes: u64,
    pub selection_reasons: BTreeMap<RetentionSelectionReason, u64>,
    pub non_goals: Vec<String>,
}

impl RetentionPlan {
    /// Bind an explicit confirmation to this immutable dry-run result.
    pub fn confirm(&self) -> RetentionConfirmation {
        RetentionConfirmation {
            schema_version: self.schema_version,
            plan_digest: self.plan_digest.clone(),
            candidate_set_digest: self.candidate_set_digest.clone(),
            snapshot_boundary: self.snapshot_boundary,
            confirmed: true,
        }
    }

    /// Check that a confirmation names this exact dry-run. A future deletion
    /// command must perform this check before executing any transaction.
    pub fn matches_confirmation(&self, confirmation: &RetentionConfirmation) -> bool {
        confirmation.validate().is_ok()
            && confirmation.schema_version == self.schema_version
            && confirmation.plan_digest == self.plan_digest
            && confirmation.candidate_set_digest == self.candidate_set_digest
            && confirmation.snapshot_boundary == self.snapshot_boundary
    }

    pub fn digest(&self) -> Result<SnapshotDigest, GhostraceError> {
        digest_material(self)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        self.policy.validate()?;
        if self.schema_version != RETENTION_PLAN_SCHEMA_VERSION
            || self.plan_digest != self.digest()?
        {
            return Err(GhostraceError::RetentionPolicyInvalid(
                "retention plan digest or schema is invalid".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn legal_hold_is_not_supported(&self) -> bool {
        self.non_goals.iter().any(|value| value == NON_GOAL_LEGAL_HOLDS)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct CandidateIdentity {
    event_id: Uuid,
    ingest_seq: u64,
    observed_at: DateTime<Utc>,
    ingested_at: DateTime<Utc>,
    source: EventSource,
    payload_bytes: u64,
    key_generation: u32,
}

/// One event selected by a validated retention plan. The payload is never
/// retained in this helper; deletion only needs its durable identity and
/// ingest order for foreign-key-safe removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetentionCandidate {
    pub(crate) event_id: Uuid,
    pub(crate) ingest_seq: u64,
}

#[derive(Clone, Debug, Default)]
struct ScopeStats {
    scoped_event_count: u64,
    scoped_bytes: u64,
    eligible_event_count: u64,
    eligible_bytes: u64,
    scoped_gap_count: u64,
}

#[derive(Clone, Debug)]
struct Selection {
    selected: bool,
    reason: Option<RetentionSelectionReason>,
    eligible: bool,
    is_gap: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DigestMaterial<'a> {
    schema_version: u32,
    policy: &'a RetentionPolicy,
    snapshot_boundary: u64,
    snapshot_event_count: u64,
    scoped_event_count: u64,
    scoped_bytes: u64,
    eligible_event_count: u64,
    eligible_bytes: u64,
    affected_event_count: u64,
    affected_observed_from: Option<DateTime<Utc>>,
    affected_observed_until: Option<DateTime<Utc>>,
    affected_ingested_from: Option<DateTime<Utc>>,
    affected_ingested_until: Option<DateTime<Utc>>,
    affected_sources: &'a BTreeMap<EventSource, u64>,
    affected_key_generations: &'a BTreeMap<u32, u64>,
    scoped_gap_count: u64,
    affected_gap_count: u64,
    protected_gap_count: u64,
    gaps: &'a [RetentionGapSummary],
    gaps_truncated: bool,
    estimated_reclaimed_bytes: u64,
    selection_reasons: &'a BTreeMap<RetentionSelectionReason, u64>,
    non_goals: &'a [String],
    candidate_set_digest: &'a SnapshotDigest,
}

fn digest_material(plan: &RetentionPlan) -> Result<SnapshotDigest, GhostraceError> {
    let material = DigestMaterial {
        schema_version: plan.schema_version,
        policy: &plan.policy,
        snapshot_boundary: plan.snapshot_boundary,
        snapshot_event_count: plan.snapshot_event_count,
        scoped_event_count: plan.scoped_event_count,
        scoped_bytes: plan.scoped_bytes,
        eligible_event_count: plan.eligible_event_count,
        eligible_bytes: plan.eligible_bytes,
        affected_event_count: plan.affected_event_count,
        affected_observed_from: plan.affected_observed_from,
        affected_observed_until: plan.affected_observed_until,
        affected_ingested_from: plan.affected_ingested_from,
        affected_ingested_until: plan.affected_ingested_until,
        affected_sources: &plan.affected_sources,
        affected_key_generations: &plan.affected_key_generations,
        scoped_gap_count: plan.scoped_gap_count,
        affected_gap_count: plan.affected_gap_count,
        protected_gap_count: plan.protected_gap_count,
        gaps: &plan.gaps,
        gaps_truncated: plan.gaps_truncated,
        estimated_reclaimed_bytes: plan.estimated_reclaimed_bytes,
        selection_reasons: &plan.selection_reasons,
        non_goals: &plan.non_goals,
        candidate_set_digest: &plan.candidate_set_digest,
    };
    let encoded = serde_json::to_vec(&material)?;
    SnapshotDigest::try_from(format!("sha256:{}", hex_digest(Sha256::digest(encoded).as_slice())))
        .map_err(|_| GhostraceError::RetentionPolicyInvalid("plan digest is invalid".to_owned()))
}

fn candidate_set_digest(hasher: Sha256) -> Result<SnapshotDigest, GhostraceError> {
    SnapshotDigest::try_from(format!("sha256:{}", hex_digest(hasher.finalize().as_slice())))
        .map_err(|_| {
            GhostraceError::RetentionPolicyInvalid("candidate digest is invalid".to_owned())
        })
}

fn update_candidate_digest(
    hasher: &mut Sha256,
    event: &EventEnvelope,
    metadata: RetentionStorageMetadata,
) -> Result<(), GhostraceError> {
    let identity = CandidateIdentity {
        event_id: event.event_id,
        ingest_seq: metadata.ingest_seq,
        observed_at: event.observed_at,
        ingested_at: event.ingested_at,
        source: event.source,
        payload_bytes: metadata.payload_bytes,
        key_generation: metadata.key_generation,
    };
    hasher.update(serde_json::to_vec(&identity)?);
    hasher.update([b'\n']);
    Ok(())
}

fn gap_summary(event: &EventEnvelope, selected: bool) -> Option<RetentionGapSummary> {
    let EventPayload::Gap(payload) = &event.payload else {
        return None;
    };
    Some(RetentionGapSummary {
        event_id: event.event_id,
        source: payload.source,
        observed_at: event.observed_at,
        reason_code: payload.reason_code.as_str().to_owned(),
        dropped_count: payload.dropped_count,
        selected,
    })
}

fn selection_for(
    policy: &RetentionPolicy,
    event: &EventEnvelope,
    stats: &ScopeStats,
    eligible_index: u64,
    remaining_eligible_bytes: u64,
) -> Selection {
    let is_gap = matches!(event.payload, EventPayload::Gap(_));
    let eligible = !(policy.preserve_gaps && is_gap);
    if !eligible {
        return Selection { selected: false, reason: None, eligible, is_gap };
    }
    let time = policy.before.is_some_and(|cutoff| event.observed_at < cutoff);
    let event_count = policy
        .retain_at_most_events
        .map(|keep| eligible_index < stats.eligible_event_count.saturating_sub(keep))
        .unwrap_or(false);
    let byte_limit =
        policy.retain_at_most_bytes.is_some_and(|limit| remaining_eligible_bytes > limit);
    let reason = if time {
        Some(RetentionSelectionReason::Time)
    } else if event_count {
        Some(RetentionSelectionReason::EventCount)
    } else if byte_limit {
        Some(RetentionSelectionReason::ByteLimit)
    } else {
        None
    };
    Selection { selected: reason.is_some(), reason, eligible, is_gap }
}

/// Evaluate a policy inside the caller's read transaction. This function is
/// `pub(crate)` so the Journal can keep its SQLite and key-provider handles
/// private while tests and the CLI use the stable public `Journal` method.
pub(crate) fn plan_from_connection(
    connection: &rusqlite::Connection,
    provider: &dyn KeyProvider,
    policy: &RetentionPolicy,
) -> Result<RetentionPlan, GhostraceError> {
    policy.validate()?;
    let snapshot_boundary: u64 = connection
        .query_row("SELECT COALESCE(MAX(ingest_seq), 0) FROM events", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(GhostraceError::from)
        .and_then(|value| {
            u64::try_from(value).map_err(|_| {
                GhostraceError::RetentionPolicyInvalid("snapshot boundary is invalid".to_owned())
            })
        })?;
    let snapshot_event_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM events WHERE ingest_seq <= ?1",
            [i64::try_from(snapshot_boundary).map_err(|_| {
                GhostraceError::RetentionPolicyInvalid("snapshot boundary is too large".to_owned())
            })?],
            |row| row.get::<_, i64>(0),
        )
        .map_err(GhostraceError::from)
        .and_then(|value| {
            u64::try_from(value).map_err(|_| {
                GhostraceError::RetentionPolicyInvalid("snapshot event count is invalid".to_owned())
            })
        })?;

    let mut stats = ScopeStats::default();
    for_each_retention_row(connection, provider, snapshot_boundary, |stored, metadata| {
        let event = &stored.event;
        if !policy.matches_scope(event) {
            return Ok(());
        }
        stats.scoped_event_count = stats.scoped_event_count.saturating_add(1);
        stats.scoped_bytes = stats.scoped_bytes.saturating_add(metadata.payload_bytes);
        let eligible = !(policy.preserve_gaps && matches!(event.payload, EventPayload::Gap(_)));
        if matches!(event.payload, EventPayload::Gap(_)) {
            stats.scoped_gap_count = stats.scoped_gap_count.saturating_add(1);
        }
        if eligible {
            stats.eligible_event_count = stats.eligible_event_count.saturating_add(1);
            stats.eligible_bytes = stats.eligible_bytes.saturating_add(metadata.payload_bytes);
        }
        Ok(())
    })?;

    let mut affected_event_count = 0_u64;
    let mut affected_observed_from: Option<DateTime<Utc>> = None;
    let mut affected_observed_until: Option<DateTime<Utc>> = None;
    let mut affected_ingested_from: Option<DateTime<Utc>> = None;
    let mut affected_ingested_until: Option<DateTime<Utc>> = None;
    let mut affected_sources = BTreeMap::new();
    let mut affected_key_generations = BTreeMap::new();
    let mut affected_gap_count = 0_u64;
    let mut protected_gap_count = 0_u64;
    let mut gaps = Vec::new();
    let mut gaps_truncated = false;
    let mut estimated_reclaimed_bytes = 0_u64;
    let mut selection_reasons = BTreeMap::new();
    let mut candidate_hasher = Sha256::new();
    let mut eligible_index = 0_u64;
    let mut remaining_eligible_bytes = stats.eligible_bytes;

    for_each_retention_row(connection, provider, snapshot_boundary, |stored, metadata| {
        let event = &stored.event;
        if !policy.matches_scope(event) {
            return Ok(());
        }
        let selection =
            selection_for(policy, event, &stats, eligible_index, remaining_eligible_bytes);
        if selection.eligible {
            eligible_index = eligible_index.saturating_add(1);
            remaining_eligible_bytes =
                remaining_eligible_bytes.saturating_sub(metadata.payload_bytes);
        }
        if selection.is_gap {
            if selection.selected {
                affected_gap_count = affected_gap_count.saturating_add(1);
            } else if policy.preserve_gaps {
                protected_gap_count = protected_gap_count.saturating_add(1);
            }
            if gaps.len() < MAX_RETENTION_GAP_SUMMARIES {
                if let Some(summary) = gap_summary(event, selection.selected) {
                    gaps.push(summary);
                }
            } else {
                gaps_truncated = true;
            }
        }
        if !selection.selected {
            return Ok(());
        }
        let reason = selection.reason.expect("selected retention event has a reason");
        *selection_reasons.entry(reason).or_insert(0) += 1;
        affected_event_count = affected_event_count.saturating_add(1);
        affected_observed_from = Some(
            affected_observed_from.map_or(event.observed_at, |value| value.min(event.observed_at)),
        );
        affected_observed_until = Some(
            affected_observed_until.map_or(event.observed_at, |value| value.max(event.observed_at)),
        );
        affected_ingested_from = Some(
            affected_ingested_from.map_or(event.ingested_at, |value| value.min(event.ingested_at)),
        );
        affected_ingested_until = Some(
            affected_ingested_until.map_or(event.ingested_at, |value| value.max(event.ingested_at)),
        );
        *affected_sources.entry(event.source).or_insert(0) += 1;
        *affected_key_generations.entry(metadata.key_generation).or_insert(0) += 1;
        estimated_reclaimed_bytes =
            estimated_reclaimed_bytes.saturating_add(metadata.payload_bytes);
        update_candidate_digest(&mut candidate_hasher, event, metadata)?;
        Ok(())
    })?;

    let candidate_set_digest = candidate_set_digest(candidate_hasher)?;
    let non_goals = vec![
        NON_GOAL_EXPORTS.to_owned(),
        NON_GOAL_BACKUPS.to_owned(),
        NON_GOAL_LEGAL_HOLDS.to_owned(),
    ];
    let mut plan = RetentionPlan {
        schema_version: RETENTION_PLAN_SCHEMA_VERSION,
        policy: policy.clone(),
        plan_digest: SnapshotDigest::try_from(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        )
        .map_err(|_| {
            GhostraceError::RetentionPolicyInvalid("plan digest placeholder is invalid".to_owned())
        })?,
        candidate_set_digest,
        snapshot_boundary,
        snapshot_event_count,
        scoped_event_count: stats.scoped_event_count,
        scoped_bytes: stats.scoped_bytes,
        eligible_event_count: stats.eligible_event_count,
        eligible_bytes: stats.eligible_bytes,
        affected_event_count,
        affected_observed_from,
        affected_observed_until,
        affected_ingested_from,
        affected_ingested_until,
        affected_sources,
        affected_key_generations,
        scoped_gap_count: stats.scoped_gap_count,
        affected_gap_count,
        protected_gap_count,
        gaps,
        gaps_truncated,
        estimated_reclaimed_bytes,
        selection_reasons,
        non_goals,
    };
    plan.plan_digest = plan.digest()?;
    Ok(plan)
}

/// Re-evaluate the candidate set for a previously validated plan inside a
/// write transaction. This keeps the deletion path bound to the same policy,
/// snapshot boundary, and candidate digest used by the dry-run receipt.
pub(crate) fn candidate_events_from_plan(
    connection: &rusqlite::Connection,
    provider: &dyn KeyProvider,
    plan: &RetentionPlan,
) -> Result<Vec<RetentionCandidate>, GhostraceError> {
    let stats = ScopeStats {
        scoped_event_count: plan.scoped_event_count,
        scoped_bytes: plan.scoped_bytes,
        eligible_event_count: plan.eligible_event_count,
        eligible_bytes: plan.eligible_bytes,
        scoped_gap_count: plan.scoped_gap_count,
    };
    let mut candidates = Vec::new();
    let mut candidate_hasher = Sha256::new();
    let mut eligible_index = 0_u64;
    let mut remaining_eligible_bytes = stats.eligible_bytes;
    for_each_retention_row(connection, provider, plan.snapshot_boundary, |stored, metadata| {
        let event = &stored.event;
        if !plan.policy.matches_scope(event) {
            return Ok(());
        }
        let selection =
            selection_for(&plan.policy, event, &stats, eligible_index, remaining_eligible_bytes);
        if selection.eligible {
            eligible_index = eligible_index.saturating_add(1);
            remaining_eligible_bytes =
                remaining_eligible_bytes.saturating_sub(metadata.payload_bytes);
        }
        if !selection.selected {
            return Ok(());
        }
        update_candidate_digest(&mut candidate_hasher, event, metadata)?;
        candidates
            .push(RetentionCandidate { event_id: event.event_id, ingest_seq: metadata.ingest_seq });
        Ok(())
    })?;
    let digest = candidate_set_digest(candidate_hasher)?;
    if digest != plan.candidate_set_digest
        || u64::try_from(candidates.len()).map_err(|_| {
            GhostraceError::RetentionPolicyInvalid("candidate count is too large".to_owned())
        })? != plan.affected_event_count
    {
        return Err(GhostraceError::RetentionConfirmationMismatch);
    }
    Ok(candidates)
}
