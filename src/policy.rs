//! Deny-by-default policy decisions.  This module is deliberately independent
//! of any operating-system collector so a future collector cannot bypass it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{error::GhostraceError, model::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    SourceNotEnabled,
    RootNotSelected,
    PrivateContext,
    EmptyProfileId,
    PolicyProfileMismatch,
    InvalidProfile,
    FixtureOnly,
}

impl PolicyReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::SourceNotEnabled => "source_not_enabled",
            Self::RootNotSelected => "root_not_selected",
            Self::PrivateContext => "private_context",
            Self::EmptyProfileId => "empty_profile_id",
            Self::PolicyProfileMismatch => "policy_profile_mismatch",
            Self::InvalidProfile => "invalid_policy_profile",
            Self::FixtureOnly => "fixture_only",
        }
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
        let root = root_id.map(str::to_owned);
        if !self.enabled_sources.contains(&source) {
            return PolicyDecision::Denied {
                source,
                root_id: root,
                reason: PolicyReason::SourceNotEnabled,
            };
        }
        if let Some(root_id) = root_id {
            if !self.selected_roots.contains(root_id) {
                return PolicyDecision::Denied {
                    source,
                    root_id: root,
                    reason: PolicyReason::RootNotSelected,
                };
            }
        }
        if private_context && !self.allow_private_context {
            return PolicyDecision::Denied {
                source,
                root_id: root,
                reason: PolicyReason::PrivateContext,
            };
        }
        PolicyDecision::Allowed { source, root_id: root }
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
        if event.policy_profile_id != self.id || event.policy_profile_version != self.version {
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
