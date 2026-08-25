//! Explicit, replayable consent state transitions.
//!
//! Consent receipts deliberately carry only policy identity, version, a scope
//! digest, and bounded metadata. They never carry selected roots or other
//! source observations. The state machine is deny-by-default and only a
//! recorded `grant` transition can make collection eligible again.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::GhostraceError,
    model::{OpaqueIdentifier, PolicyProfileId, ReasonCode, RootId, SnapshotDigest},
    policy::PolicyDocument,
};

/// Maximum number of retained-field or coverage-limit identifiers shown in a
/// consent preview. The bound keeps a user-visible confirmation finite.
pub const MAX_CONSENT_PREVIEW_ITEMS: usize = 64;

/// The effective consent state after the last accepted receipt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentState {
    #[default]
    Inactive,
    Active,
    Suspended,
    Revoked,
    DeletionRequested,
}

impl ConsentState {
    pub fn allows_capture(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether this state is a bounded terminal outcome for an observation
    /// session. Terminal states never reopen without a new explicit grant.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Revoked | Self::DeletionRequested)
    }
}

/// A user-visible state transition. Every field is bounded and privacy-safe:
/// scope is represented by a digest rather than by roots or paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentTransitionKind {
    Grant,
    ScopeChanged,
    Suspended,
    Revoked,
    DeletionRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsentReceipt {
    pub sequence: u64,
    pub transition: ConsentTransitionKind,
    pub state: ConsentState,
    pub policy_id: PolicyProfileId,
    pub policy_version: u32,
    pub scope_digest: SnapshotDigest,
    pub occurred_at: DateTime<Utc>,
    pub actor: OpaqueIdentifier,
    pub reason: ReasonCode,
}

/// The user-visible scope and limitation summary shown before a live root can
/// be enabled. Root identities are canonical opaque IDs, never raw filesystem
/// paths. This preview is not a receipt and is intentionally not persisted by
/// the consent state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsentPreview {
    policy_id: PolicyProfileId,
    policy_version: u32,
    canonical_roots: Vec<RootId>,
    excluded_roots: Vec<RootId>,
    retained_fields: Vec<OpaqueIdentifier>,
    coverage_limits: Vec<OpaqueIdentifier>,
    scope_digest: SnapshotDigest,
}

/// An explicit acknowledgement of a rendered [`ConsentPreview`]. The private
/// payload prevents callers from constructing a confirmation without first
/// validating and displaying a preview.
#[derive(Debug, PartialEq, Eq)]
pub struct ConsentConfirmation {
    preview: ConsentPreview,
}

impl ConsentPreview {
    /// Build a deterministic, bounded preview from a versioned policy. The
    /// selected and excluded roots are rendered as canonical opaque IDs; the
    /// retained fields and known coverage limits are required to be explicit.
    pub fn from_policy<Fields, Field, Limits, Limit>(
        document: &PolicyDocument,
        retained_fields: Fields,
        coverage_limits: Limits,
    ) -> Result<Self, GhostraceError>
    where
        Fields: IntoIterator<Item = Field>,
        Field: Into<String>,
        Limits: IntoIterator<Item = Limit>,
        Limit: Into<String>,
    {
        document.validate()?;
        let canonical_roots = document
            .selected_roots
            .iter()
            .cloned()
            .map(RootId::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        if canonical_roots.is_empty() {
            return Err(GhostraceError::ConsentTransition(
                "an explicit root scope is required before capture".to_owned(),
            ));
        }
        let excluded_roots = document
            .excluded_roots
            .iter()
            .cloned()
            .map(RootId::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let retained_fields = bounded_preview_identifiers(retained_fields, "retained fields")?;
        let coverage_limits = bounded_preview_identifiers(coverage_limits, "coverage limits")?;
        Ok(Self {
            policy_id: PolicyProfileId::try_from(document.id.clone())?,
            policy_version: document.version,
            canonical_roots,
            excluded_roots,
            retained_fields,
            coverage_limits,
            scope_digest: document.scope_digest()?,
        })
    }

    /// Confirm this exact rendered preview. The confirmation is consumed by
    /// [`ConsentStateMachine::grant_preview`] and cannot be silently reused.
    pub fn confirm(self) -> ConsentConfirmation {
        ConsentConfirmation { preview: self }
    }

    pub fn policy_id(&self) -> &PolicyProfileId {
        &self.policy_id
    }

    pub fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub fn canonical_roots(&self) -> &[RootId] {
        &self.canonical_roots
    }

    pub fn excluded_roots(&self) -> &[RootId] {
        &self.excluded_roots
    }

    pub fn retained_fields(&self) -> &[OpaqueIdentifier] {
        &self.retained_fields
    }

    pub fn coverage_limits(&self) -> &[OpaqueIdentifier] {
        &self.coverage_limits
    }

    pub fn scope_digest(&self) -> &SnapshotDigest {
        &self.scope_digest
    }
}

fn bounded_preview_identifiers<I, S>(
    values: I,
    label: &str,
) -> Result<Vec<OpaqueIdentifier>, GhostraceError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut identifiers = BTreeSet::new();
    for value in values {
        let identifier = OpaqueIdentifier::try_from(value.into())?;
        if !identifiers.insert(identifier) {
            return Err(GhostraceError::ConsentTransition(format!(
                "{label} must not contain duplicates"
            )));
        }
    }
    if identifiers.is_empty() {
        return Err(GhostraceError::ConsentTransition(format!(
            "{label} must be declared before capture"
        )));
    }
    if identifiers.len() > MAX_CONSENT_PREVIEW_ITEMS {
        return Err(GhostraceError::ConsentTransition(format!(
            "{label} exceed the {MAX_CONSENT_PREVIEW_ITEMS}-item bound"
        )));
    }
    Ok(identifiers.into_iter().collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PolicyContext {
    id: PolicyProfileId,
    version: u32,
    scope_digest: SnapshotDigest,
}

/// An in-memory consent ledger that can be rebuilt from receipts after a
/// restart. Invalid ordering or transitions fail closed and do not produce a
/// partially applied state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsentStateMachine {
    state: ConsentState,
    context: Option<PolicyContext>,
    receipts: Vec<ConsentReceipt>,
}

impl ConsentStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> ConsentState {
        self.state
    }

    pub fn is_capture_allowed(&self) -> bool {
        self.state.allows_capture()
    }

    pub fn receipts(&self) -> &[ConsentReceipt] {
        &self.receipts
    }

    /// Install an explicit grant. A grant is the only transition that can
    /// return a suspended, revoked, or deletion-requested machine to `Active`.
    pub fn grant(
        &mut self,
        document: &PolicyDocument,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        let context = context_for(document)?;
        self.grant_context(context, occurred_at, actor, reason)
    }

    /// Install a grant only after the caller has explicitly confirmed the
    /// user-visible root scope preview. The receipt retains the policy context
    /// and digest, never the preview's root or field names.
    pub fn grant_preview(
        &mut self,
        confirmation: ConsentConfirmation,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        let context = PolicyContext {
            id: confirmation.preview.policy_id,
            version: confirmation.preview.policy_version,
            scope_digest: confirmation.preview.scope_digest,
        };
        self.grant_context(context, occurred_at, actor, reason)
    }

    fn grant_context(
        &mut self,
        context: PolicyContext,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        let actor = bounded_actor(actor)?;
        let reason = bounded_reason(reason)?;
        let receipt = self.receipt(
            ConsentTransitionKind::Grant,
            ConsentState::Active,
            context,
            occurred_at,
            actor,
            reason,
        );
        self.apply_receipt(receipt.clone())?;
        Ok(receipt)
    }

    /// Change the scope while active. Policy identity is immutable and policy
    /// versions must increase; the explicit call itself is the reconfirmation.
    pub fn change_scope(
        &mut self,
        document: &PolicyDocument,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        let context = context_for(document)?;
        let actor = bounded_actor(actor)?;
        let reason = bounded_reason(reason)?;
        let receipt = self.receipt(
            ConsentTransitionKind::ScopeChanged,
            ConsentState::Active,
            context,
            occurred_at,
            actor,
            reason,
        );
        self.apply_receipt(receipt.clone())?;
        Ok(receipt)
    }

    pub fn suspend(
        &mut self,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        self.apply_current_transition(
            ConsentTransitionKind::Suspended,
            ConsentState::Suspended,
            occurred_at,
            actor,
            reason,
        )
    }

    /// Revocation is applied synchronously to the state machine. Callers must
    /// check `is_capture_allowed` before retaining any subsequent observation;
    /// cleanup can run later without reopening this gate.
    pub fn revoke(
        &mut self,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        self.apply_current_transition(
            ConsentTransitionKind::Revoked,
            ConsentState::Revoked,
            occurred_at,
            actor,
            reason,
        )
    }

    pub fn request_deletion(
        &mut self,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        self.apply_current_transition(
            ConsentTransitionKind::DeletionRequested,
            ConsentState::DeletionRequested,
            occurred_at,
            actor,
            reason,
        )
    }

    /// Rebuild state from a complete, ordered receipt stream. Sequence gaps,
    /// state mismatches, and attempts to reactivate through a non-grant
    /// transition are rejected before the returned machine is observable.
    pub fn replay(receipts: &[ConsentReceipt]) -> Result<Self, GhostraceError> {
        let mut machine = Self::new();
        for receipt in receipts {
            machine.apply_receipt(receipt.clone())?;
        }
        Ok(machine)
    }

    fn receipt(
        &self,
        transition: ConsentTransitionKind,
        state: ConsentState,
        context: PolicyContext,
        occurred_at: DateTime<Utc>,
        actor: OpaqueIdentifier,
        reason: ReasonCode,
    ) -> ConsentReceipt {
        ConsentReceipt {
            sequence: self.receipts.len() as u64 + 1,
            transition,
            state,
            policy_id: context.id,
            policy_version: context.version,
            scope_digest: context.scope_digest,
            occurred_at,
            actor,
            reason,
        }
    }

    fn apply_current_transition(
        &mut self,
        transition: ConsentTransitionKind,
        state: ConsentState,
        occurred_at: DateTime<Utc>,
        actor: &str,
        reason: &str,
    ) -> Result<ConsentReceipt, GhostraceError> {
        let context = self.context.clone().ok_or_else(|| {
            GhostraceError::ConsentTransition("a policy grant is required first".to_owned())
        })?;
        let actor = bounded_actor(actor)?;
        let reason = bounded_reason(reason)?;
        let receipt = self.receipt(transition, state, context, occurred_at, actor, reason);
        self.apply_receipt(receipt.clone())?;
        Ok(receipt)
    }

    fn apply_receipt(&mut self, receipt: ConsentReceipt) -> Result<(), GhostraceError> {
        let expected_sequence = self.receipts.len().checked_add(1).ok_or_else(|| {
            GhostraceError::ConsentTransition("receipt sequence exhausted".to_owned())
        })? as u64;
        if receipt.sequence != expected_sequence || receipt.policy_version == 0 {
            return Err(GhostraceError::ConsentTransition(
                "receipt sequence or policy version is invalid".to_owned(),
            ));
        }
        if self.receipts.last().is_some_and(|previous| receipt.occurred_at < previous.occurred_at) {
            return Err(GhostraceError::ConsentTransition(
                "receipt time moved backwards".to_owned(),
            ));
        }

        let receipt_context = PolicyContext {
            id: receipt.policy_id.clone(),
            version: receipt.policy_version,
            scope_digest: receipt.scope_digest.clone(),
        };
        let next_state = match receipt.transition {
            ConsentTransitionKind::Grant => {
                if receipt.state != ConsentState::Active
                    || self.state == ConsentState::Active
                    || self
                        .context
                        .as_ref()
                        .is_some_and(|previous| previous.id != receipt_context.id)
                    || self
                        .context
                        .as_ref()
                        .is_some_and(|previous| receipt_context.version < previous.version)
                {
                    return Err(GhostraceError::ConsentTransition(
                        "grant is not valid for the current consent history".to_owned(),
                    ));
                }
                ConsentState::Active
            }
            ConsentTransitionKind::ScopeChanged => {
                let Some(previous) = self.context.as_ref() else {
                    return Err(GhostraceError::ConsentTransition(
                        "scope change requires an existing grant".to_owned(),
                    ));
                };
                if self.state != ConsentState::Active
                    || receipt.state != ConsentState::Active
                    || receipt_context.id != previous.id
                    || receipt_context.version <= previous.version
                    || receipt_context.scope_digest == previous.scope_digest
                {
                    return Err(GhostraceError::ConsentTransition(
                        "scope change must be an active, forward policy change".to_owned(),
                    ));
                }
                ConsentState::Active
            }
            ConsentTransitionKind::Suspended => {
                self.require_current_context(&receipt_context)?;
                if self.state != ConsentState::Active || receipt.state != ConsentState::Suspended {
                    return Err(GhostraceError::ConsentTransition(
                        "only active consent can be suspended".to_owned(),
                    ));
                }
                ConsentState::Suspended
            }
            ConsentTransitionKind::Revoked => {
                self.require_current_context(&receipt_context)?;
                if !matches!(self.state, ConsentState::Active | ConsentState::Suspended)
                    || receipt.state != ConsentState::Revoked
                {
                    return Err(GhostraceError::ConsentTransition(
                        "only active or suspended consent can be revoked".to_owned(),
                    ));
                }
                ConsentState::Revoked
            }
            ConsentTransitionKind::DeletionRequested => {
                self.require_current_context(&receipt_context)?;
                if matches!(self.state, ConsentState::Inactive | ConsentState::DeletionRequested)
                    || receipt.state != ConsentState::DeletionRequested
                {
                    return Err(GhostraceError::ConsentTransition(
                        "deletion intent requires an existing non-deleted grant".to_owned(),
                    ));
                }
                ConsentState::DeletionRequested
            }
        };

        self.state = next_state;
        self.context = Some(receipt_context);
        self.receipts.push(receipt);
        Ok(())
    }

    fn require_current_context(&self, candidate: &PolicyContext) -> Result<(), GhostraceError> {
        if self.context.as_ref() != Some(candidate) {
            return Err(GhostraceError::ConsentTransition(
                "receipt policy context does not match the active grant".to_owned(),
            ));
        }
        Ok(())
    }
}

fn context_for(document: &PolicyDocument) -> Result<PolicyContext, GhostraceError> {
    document.validate()?;
    Ok(PolicyContext {
        id: PolicyProfileId::try_from(document.id.clone())?,
        version: document.version,
        scope_digest: document.scope_digest()?,
    })
}

fn bounded_actor(value: &str) -> Result<OpaqueIdentifier, GhostraceError> {
    OpaqueIdentifier::try_from(value)
}

fn bounded_reason(value: &str) -> Result<ReasonCode, GhostraceError> {
    ReasonCode::try_from(value)
}
