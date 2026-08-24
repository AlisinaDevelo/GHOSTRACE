//! Deny-by-default policy decisions.  This module is deliberately independent
//! of any operating-system collector so a future collector cannot bypass it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{error::GhostraceError, model::*};

/// Current on-disk capture-policy document schema.
pub const POLICY_DOCUMENT_SCHEMA_VERSION: u32 = 1;

/// A strict, versioned policy document. Its serialized representation is the
/// durable policy record; [`PolicyProfile`] is the runtime authorization view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyDocument {
    pub schema_version: u32,
    pub id: String,
    pub version: u32,
    pub enabled_sources: BTreeSet<EventSource>,
    pub selected_roots: BTreeSet<String>,
    pub allow_private_context: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedPolicyDocument {
    schema_version: u32,
    id: String,
    version: u32,
    enabled_sources: Vec<EventSource>,
    selected_roots: Vec<String>,
    allow_private_context: bool,
}

impl<'de> Deserialize<'de> for PolicyDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UncheckedPolicyDocument::deserialize(deserializer)?;
        Self::try_from(raw).map_err(D::Error::custom)
    }
}

impl TryFrom<UncheckedPolicyDocument> for PolicyDocument {
    type Error = GhostraceError;

    fn try_from(raw: UncheckedPolicyDocument) -> Result<Self, Self::Error> {
        if raw.schema_version != POLICY_DOCUMENT_SCHEMA_VERSION {
            return Err(GhostraceError::UnsupportedPolicySchema(raw.schema_version));
        }
        let enabled_sources = unique_sources(raw.enabled_sources)?;
        let selected_roots = unique_roots(raw.selected_roots)?;
        let document = Self {
            schema_version: raw.schema_version,
            id: raw.id,
            version: raw.version,
            enabled_sources,
            selected_roots,
            allow_private_context: raw.allow_private_context,
        };
        document.validate()?;
        Ok(document)
    }
}

impl PolicyDocument {
    pub fn new<Sources, Roots, Root>(
        id: impl Into<String>,
        version: u32,
        enabled_sources: Sources,
        selected_roots: Roots,
        allow_private_context: bool,
    ) -> Result<Self, GhostraceError>
    where
        Sources: IntoIterator<Item = EventSource>,
        Roots: IntoIterator<Item = Root>,
        Root: Into<String>,
    {
        let document = Self {
            schema_version: POLICY_DOCUMENT_SCHEMA_VERSION,
            id: id.into(),
            version,
            enabled_sources: unique_sources(enabled_sources.into_iter().collect())?,
            selected_roots: unique_roots(selected_roots.into_iter().map(Into::into).collect())?,
            allow_private_context,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn from_profile(profile: &PolicyProfile) -> Result<Self, GhostraceError> {
        Self::new(
            profile.id.clone(),
            profile.version,
            profile.enabled_sources.iter().copied(),
            profile.selected_roots.iter().cloned(),
            profile.allow_private_context,
        )
    }

    pub fn from_json(input: &str) -> Result<Self, GhostraceError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_json(&self) -> Result<String, GhostraceError> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    /// Return a stable digest of the policy choices without exposing the
    /// selected roots in a consent receipt. Identity and version are carried
    /// separately by the receipt, so the digest represents scope semantics.
    pub fn scope_digest(&self) -> Result<SnapshotDigest, GhostraceError> {
        self.validate()?;
        let canonical = serde_json::to_vec(&(
            &self.enabled_sources,
            &self.selected_roots,
            self.allow_private_context,
        ))?;
        let digest = Sha256::digest(canonical);
        let mut encoded = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        SnapshotDigest::try_from(format!("sha256:{encoded}"))
    }

    pub fn to_profile(&self) -> Result<PolicyProfile, GhostraceError> {
        self.validate()?;
        Ok(PolicyProfile {
            id: self.id.clone(),
            version: self.version,
            enabled_sources: self.enabled_sources.clone(),
            selected_roots: self.selected_roots.clone(),
            allow_private_context: self.allow_private_context,
        })
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != POLICY_DOCUMENT_SCHEMA_VERSION {
            return Err(GhostraceError::UnsupportedPolicySchema(self.schema_version));
        }
        if self.version == 0 || validate_identifier("policy_profile_id", &self.id).is_err() {
            return Err(GhostraceError::PolicyMigration(
                "policy identity or version is invalid".to_owned(),
            ));
        }
        if self.selected_roots.len() > 256
            || self
                .selected_roots
                .iter()
                .any(|root| validate_identifier("selected_root", root).is_err())
        {
            return Err(GhostraceError::PolicyMigration("selected root set is invalid".to_owned()));
        }
        Ok(())
    }

    pub fn migration_from(&self, previous: &Self) -> Result<PolicyMigration, GhostraceError> {
        previous.validate()?;
        self.validate()?;
        if self.id != previous.id {
            return Err(GhostraceError::PolicyMigration(
                "policy identity cannot change during migration".to_owned(),
            ));
        }
        if self.version == previous.version {
            return Err(GhostraceError::PolicyMigration(
                "duplicate policy identity and version".to_owned(),
            ));
        }
        if self.version < previous.version {
            return Err(GhostraceError::PolicyMigration(
                "policy versions must increase monotonically".to_owned(),
            ));
        }
        let mut changed = Vec::new();
        if self.enabled_sources != previous.enabled_sources {
            changed.push(PolicyChange::EnabledSources);
        }
        if self.selected_roots != previous.selected_roots {
            changed.push(PolicyChange::SelectedRoots);
        }
        if self.allow_private_context != previous.allow_private_context {
            changed.push(PolicyChange::PrivateContext);
        }
        if changed.is_empty() {
            Ok(PolicyMigration::PreservedChoices {
                id: self.id.clone(),
                from_version: previous.version,
                to_version: self.version,
            })
        } else {
            Ok(PolicyMigration::RequiresReconfirmation {
                id: self.id.clone(),
                from_version: previous.version,
                to_version: self.version,
                changed,
            })
        }
    }
}

fn unique_sources(values: Vec<EventSource>) -> Result<BTreeSet<EventSource>, GhostraceError> {
    let mut result = BTreeSet::new();
    for value in values {
        if !result.insert(value) {
            return Err(GhostraceError::PolicyMigration(
                "policy document contains duplicate sources".to_owned(),
            ));
        }
    }
    Ok(result)
}

fn unique_roots(values: Vec<String>) -> Result<BTreeSet<String>, GhostraceError> {
    let mut result = BTreeSet::new();
    for value in values {
        if !result.insert(value) {
            return Err(GhostraceError::PolicyMigration(
                "policy document contains duplicate roots".to_owned(),
            ));
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChange {
    EnabledSources,
    SelectedRoots,
    PrivateContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyMigration {
    PreservedChoices {
        id: String,
        from_version: u32,
        to_version: u32,
    },
    RequiresReconfirmation {
        id: String,
        from_version: u32,
        to_version: u32,
        changed: Vec<PolicyChange>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyMigrationOutcome {
    Installed { id: String, version: u32 },
    PreservedChoices { id: String, from_version: u32, to_version: u32 },
    Reconfirmed { id: String, from_version: u32, to_version: u32, changed: Vec<PolicyChange> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyHistory {
    documents: BTreeMap<String, PolicyDocument>,
}

impl PolicyHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self, id: &str) -> Option<&PolicyDocument> {
        self.documents.get(id)
    }

    pub fn apply(
        &mut self,
        candidate: PolicyDocument,
        reconfirmed: bool,
    ) -> Result<PolicyMigrationOutcome, GhostraceError> {
        candidate.validate()?;
        let outcome = match self.documents.get(&candidate.id) {
            None => PolicyMigrationOutcome::Installed {
                id: candidate.id.clone(),
                version: candidate.version,
            },
            Some(previous) => match candidate.migration_from(previous)? {
                PolicyMigration::PreservedChoices { id, from_version, to_version } => {
                    PolicyMigrationOutcome::PreservedChoices { id, from_version, to_version }
                }
                PolicyMigration::RequiresReconfirmation {
                    id,
                    from_version,
                    to_version,
                    changed,
                } if reconfirmed => {
                    PolicyMigrationOutcome::Reconfirmed { id, from_version, to_version, changed }
                }
                PolicyMigration::RequiresReconfirmation { .. } => {
                    return Err(GhostraceError::PolicyMigration(
                        "policy semantics changed; explicit reconfirmation is required".to_owned(),
                    ));
                }
            },
        };
        self.documents.insert(candidate.id.clone(), candidate);
        Ok(outcome)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    PolicyAllowed,
    SourceNotEnabled,
    RootNotSelected,
    PrivateContext,
    EmptyProfileId,
    PolicyProfileMismatch,
    InvalidProfile,
    FixtureOnly,
    MalformedInput,
    UnsupportedScope,
    InternalFailure,
    RedactionRequired,
    SummaryOnly,
    Refused,
}

impl PolicyReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::PolicyAllowed => "policy_allowed",
            Self::SourceNotEnabled => "source_not_enabled",
            Self::RootNotSelected => "root_not_selected",
            Self::PrivateContext => "private_context",
            Self::EmptyProfileId => "empty_profile_id",
            Self::PolicyProfileMismatch => "policy_profile_mismatch",
            Self::InvalidProfile => "invalid_policy_profile",
            Self::FixtureOnly => "fixture_only",
            Self::MalformedInput => "malformed_input",
            Self::UnsupportedScope => "unsupported_scope",
            Self::InternalFailure => "internal_failure",
            Self::RedactionRequired => "redaction_required",
            Self::SummaryOnly => "summary_only",
            Self::Refused => "refused",
        }
    }

    pub fn diagnostic(self) -> PolicyDiagnostic {
        match self {
            Self::PolicyAllowed => PolicyDiagnostic::Accepted,
            Self::MalformedInput | Self::InvalidProfile | Self::EmptyProfileId => {
                PolicyDiagnostic::MalformedInput
            }
            Self::UnsupportedScope => PolicyDiagnostic::UnsupportedScope,
            Self::InternalFailure => PolicyDiagnostic::InternalFailure,
            Self::SourceNotEnabled
            | Self::RootNotSelected
            | Self::PrivateContext
            | Self::PolicyProfileMismatch
            | Self::FixtureOnly
            | Self::RedactionRequired
            | Self::SummaryOnly
            | Self::Refused => PolicyDiagnostic::PolicyDenied,
        }
    }
}

/// The finite public diagnostic classes exposed by the policy gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDiagnostic {
    Accepted,
    PolicyDenied,
    MalformedInput,
    UnsupportedScope,
    InternalFailure,
}

impl PolicyDiagnostic {
    pub fn code(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::PolicyDenied => "policy_denied",
            Self::MalformedInput => "malformed_input",
            Self::UnsupportedScope => "unsupported_scope",
            Self::InternalFailure => "internal_failure",
        }
    }
}

/// The bounded action recorded for a policy decision. Redaction and summary
/// outcomes carry no rejected value; they are only an explicit disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyOutcome {
    Allow,
    Deny,
    Redact,
    Summarize,
    Refuse,
}

impl PolicyOutcome {
    pub fn code(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Redact => "redact",
            Self::Summarize => "summarize",
            Self::Refuse => "refuse",
        }
    }
}

/// A privacy-bounded, serializable decision record. It intentionally reports
/// only whether a root was present, never the root or rejected observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyDecisionRecord {
    pub outcome: PolicyOutcome,
    pub reason: PolicyReason,
    pub source: EventSource,
    pub policy_id: Option<PolicyProfileId>,
    pub policy_version: u32,
    pub root_present: bool,
    pub private_context: bool,
}

impl PolicyDecisionRecord {
    pub fn reason_code(&self) -> &'static str {
        self.reason.code()
    }

    pub fn diagnostic(&self) -> PolicyDiagnostic {
        self.reason.diagnostic()
    }

    pub fn redact(mut self) -> Self {
        self.outcome = PolicyOutcome::Redact;
        self.reason = PolicyReason::RedactionRequired;
        self
    }

    pub fn summarize(mut self) -> Self {
        self.outcome = PolicyOutcome::Summarize;
        self.reason = PolicyReason::SummaryOnly;
        self
    }

    pub fn refuse(mut self, reason: PolicyReason) -> Self {
        self.outcome = PolicyOutcome::Refuse;
        self.reason = reason;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allowed { source: EventSource, root_id: Option<String> },
    Denied { source: EventSource, root_id: Option<String>, reason: PolicyReason },
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Allowed { .. } => None,
            Self::Denied { reason, .. } => Some(reason.code()),
        }
    }
}

/// A versioned consent profile.  An empty profile denies every source and root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyProfile {
    pub id: String,
    pub version: u32,
    pub enabled_sources: BTreeSet<EventSource>,
    pub selected_roots: BTreeSet<String>,
    pub allow_private_context: bool,
}

impl PolicyProfile {
    pub fn from_document(document: &PolicyDocument) -> Result<Self, GhostraceError> {
        document.to_profile()
    }

    pub fn to_document(&self) -> Result<PolicyDocument, GhostraceError> {
        PolicyDocument::from_profile(self)
    }

    pub fn new(id: impl Into<String>) -> Self {
        Self::deny_by_default(id)
    }

    pub fn deny_by_default(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 1,
            enabled_sources: BTreeSet::new(),
            selected_roots: BTreeSet::new(),
            allow_private_context: false,
        }
    }

    /// The fixture profile is explicit and only accepts the selected root in the
    /// checked-in causal-chain fixture. It does not authorize an arbitrary path
    /// or any live collector.
    pub fn fixture_default() -> Self {
        let mut profile = Self::deny_by_default("fixture-default-v1");
        profile.enabled_sources = [
            EventSource::Filesystem,
            EventSource::FrontmostApp,
            EventSource::Shell,
            EventSource::Git,
            EventSource::Browser,
            EventSource::Lifecycle,
        ]
        .into_iter()
        .collect();
        profile.selected_roots.insert("workspace-demo".to_owned());
        profile
    }

    pub fn enable_source(&mut self, source: EventSource) {
        self.enabled_sources.insert(source);
    }

    pub fn select_root(&mut self, root_id: impl Into<String>) {
        self.selected_roots.insert(root_id.into());
    }

    pub fn allow_private_context(&mut self) {
        self.allow_private_context = true;
    }

    pub fn decide(
        &self,
        source: EventSource,
        root_id: Option<&str>,
        private_context: bool,
    ) -> PolicyDecision {
        // A rejected observation may be an attacker-controlled path. Only
        // report a root identifier that satisfies the same bounded identifier
        // contract as selected policy roots; otherwise report presence only.
        let safe_root = root_id.and_then(|value| {
            validate_identifier("selected_root", value).ok().map(|_| value.to_owned())
        });
        if !self.enabled_sources.contains(&source) {
            return PolicyDecision::Denied {
                source,
                root_id: safe_root,
                reason: PolicyReason::SourceNotEnabled,
            };
        }
        if let Some(root_id) = root_id {
            if !self.selected_roots.contains(root_id) {
                return PolicyDecision::Denied {
                    source,
                    root_id: safe_root,
                    reason: PolicyReason::RootNotSelected,
                };
            }
        }
        if private_context && !self.allow_private_context {
            return PolicyDecision::Denied {
                source,
                root_id: safe_root,
                reason: PolicyReason::PrivateContext,
            };
        }
        PolicyDecision::Allowed { source, root_id: safe_root }
    }

    pub fn decide_record(
        &self,
        source: EventSource,
        root_id: Option<&str>,
        private_context: bool,
    ) -> PolicyDecisionRecord {
        let policy_id = PolicyProfileId::try_from(self.id.clone()).ok();
        let profile_valid = self.version != 0
            && policy_id.is_some()
            && self.selected_roots.len() <= 256
            && self
                .selected_roots
                .iter()
                .all(|root| validate_identifier("selected_root", root).is_ok());
        if !profile_valid {
            return PolicyDecisionRecord {
                outcome: PolicyOutcome::Refuse,
                reason: PolicyReason::InvalidProfile,
                source,
                policy_id,
                policy_version: self.version,
                root_present: root_id.is_some(),
                private_context,
            };
        }
        let decision = self.decide(source, root_id, private_context);
        let (outcome, reason) = match decision {
            PolicyDecision::Allowed { .. } => (PolicyOutcome::Allow, PolicyReason::PolicyAllowed),
            PolicyDecision::Denied { reason, .. } => (PolicyOutcome::Deny, reason),
        };
        PolicyDecisionRecord {
            outcome,
            reason,
            source,
            policy_id,
            policy_version: self.version,
            root_present: root_id.is_some(),
            private_context,
        }
    }

    pub fn authorize(&self, event: &EventEnvelope) -> Result<(), GhostraceError> {
        if self.id.trim().is_empty() {
            return Err(GhostraceError::PolicyDenied {
                reason: PolicyReason::EmptyProfileId.code().to_owned(),
            });
        }
        if self.version == 0
            || validate_identifier("policy_profile_id", &self.id).is_err()
            || self.selected_roots.len() > 256
            || self
                .selected_roots
                .iter()
                .any(|root| validate_identifier("selected_root", root).is_err())
        {
            return Err(GhostraceError::PolicyDenied {
                reason: PolicyReason::InvalidProfile.code().to_owned(),
            });
        }
        if event.policy_profile_id.as_str() != self.id
            || event.policy_profile_version != self.version
        {
            return Err(GhostraceError::PolicyDenied {
                reason: PolicyReason::PolicyProfileMismatch.code().to_owned(),
            });
        }
        let decision =
            self.decide(event.source, event.payload.root_id(), event.payload.private_context());
        if decision.is_allowed() {
            Ok(())
        } else {
            Err(GhostraceError::PolicyDenied {
                reason: decision
                    .reason_code()
                    .unwrap_or(PolicyReason::SourceNotEnabled.code())
                    .to_owned(),
            })
        }
    }

    pub fn is_source_enabled(&self, source: EventSource) -> bool {
        self.enabled_sources.contains(&source)
    }
}
