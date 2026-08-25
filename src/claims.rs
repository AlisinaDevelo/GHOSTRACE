//! Versioned, bounded claim templates for evidence-linked explanations.
//!
//! Claims are intentionally narrower than event payload summaries. A template
//! can describe what an event establishes, but it cannot manufacture intent,
//! completeness, process attribution, causality, or a rename pairing.

use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::GhostraceError,
    model::{EventEnvelope, EventKind, EventPayload, Evidence, FileOperation},
};

pub const CLAIM_GRAMMAR_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimTemplateId {
    FilesystemObservation,
    FrontmostObservation,
    ShellStartedObservation,
    ShellFinishedObservation,
    GitSnapshotObservation,
    BrowserNavigationObservation,
    BrowserBookmarkObservation,
    CollectorStartedObservation,
    CollectorStoppedObservation,
    CoverageGap,
    PolicyDenied,
    SourceError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredFact {
    EventId,
    Source,
    EventKind,
    ObservedAt,
    EvidenceLevel,
    NormalizedPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProhibitedImplication {
    Intent,
    Completeness,
    ProcessAttribution,
    RenameIdentity,
    UnsupportedCausality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRequirement {
    PreserveEventLabel,
    DirectSourceRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapBehavior {
    ExplicitStatus,
    LimitInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimLocale {
    En,
    EnGb,
}

impl ClaimLocale {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::EnGb => "en-GB",
        }
    }

    fn filesystem_label(self) -> &'static str {
        match self {
            Self::En => "filesystem",
            Self::EnGb => "file system",
        }
    }

    fn gap_limit_suffix(self) -> &'static str {
        match self {
            Self::En | Self::EnGb => " Coverage is limited by a recorded gap.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ClaimTemplateDescriptor {
    pub id: ClaimTemplateId,
    pub required_facts: &'static [RequiredFact],
    pub prohibited_implications: &'static [ProhibitedImplication],
    pub evidence_requirement: EvidenceRequirement,
    pub gap_behavior: GapBehavior,
}

const OBSERVATION_FACTS: &[RequiredFact] = &[
    RequiredFact::EventId,
    RequiredFact::Source,
    RequiredFact::EventKind,
    RequiredFact::ObservedAt,
    RequiredFact::EvidenceLevel,
    RequiredFact::NormalizedPayload,
];
const OBSERVATION_PROHIBITIONS: &[ProhibitedImplication] = &[
    ProhibitedImplication::Intent,
    ProhibitedImplication::Completeness,
    ProhibitedImplication::ProcessAttribution,
    ProhibitedImplication::UnsupportedCausality,
];
const FILESYSTEM_PROHIBITIONS: &[ProhibitedImplication] = &[
    ProhibitedImplication::Intent,
    ProhibitedImplication::Completeness,
    ProhibitedImplication::ProcessAttribution,
    ProhibitedImplication::RenameIdentity,
    ProhibitedImplication::UnsupportedCausality,
];
const STATUS_PROHIBITIONS: &[ProhibitedImplication] = &[
    ProhibitedImplication::Intent,
    ProhibitedImplication::Completeness,
    ProhibitedImplication::ProcessAttribution,
    ProhibitedImplication::UnsupportedCausality,
];

const FILESYSTEM_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::FilesystemObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: FILESYSTEM_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const OBSERVATION_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::FrontmostObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const SHELL_STARTED_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::ShellStartedObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const SHELL_FINISHED_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::ShellFinishedObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const GIT_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::GitSnapshotObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const BROWSER_NAVIGATION_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::BrowserNavigationObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const BROWSER_BOOKMARK_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::BrowserBookmarkObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: OBSERVATION_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const COLLECTOR_STARTED_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::CollectorStartedObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: STATUS_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const COLLECTOR_STOPPED_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::CollectorStoppedObservation,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: STATUS_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::LimitInterpretation,
};
const GAP_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::CoverageGap,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: STATUS_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::ExplicitStatus,
};
const POLICY_DENIED_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::PolicyDenied,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: STATUS_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::ExplicitStatus,
};
const SOURCE_ERROR_TEMPLATE: ClaimTemplateDescriptor = ClaimTemplateDescriptor {
    id: ClaimTemplateId::SourceError,
    required_facts: OBSERVATION_FACTS,
    prohibited_implications: STATUS_PROHIBITIONS,
    evidence_requirement: EvidenceRequirement::PreserveEventLabel,
    gap_behavior: GapBehavior::ExplicitStatus,
};

impl ClaimTemplateId {
    pub const fn descriptor(self) -> &'static ClaimTemplateDescriptor {
        match self {
            Self::FilesystemObservation => &FILESYSTEM_TEMPLATE,
            Self::FrontmostObservation => &OBSERVATION_TEMPLATE,
            Self::ShellStartedObservation => &SHELL_STARTED_TEMPLATE,
            Self::ShellFinishedObservation => &SHELL_FINISHED_TEMPLATE,
            Self::GitSnapshotObservation => &GIT_TEMPLATE,
            Self::BrowserNavigationObservation => &BROWSER_NAVIGATION_TEMPLATE,
            Self::BrowserBookmarkObservation => &BROWSER_BOOKMARK_TEMPLATE,
            Self::CollectorStartedObservation => &COLLECTOR_STARTED_TEMPLATE,
            Self::CollectorStoppedObservation => &COLLECTOR_STOPPED_TEMPLATE,
            Self::CoverageGap => &GAP_TEMPLATE,
            Self::PolicyDenied => &POLICY_DENIED_TEMPLATE,
            Self::SourceError => &SOURCE_ERROR_TEMPLATE,
        }
    }

    pub const fn for_kind(kind: EventKind) -> Self {
        match kind {
            EventKind::FilesystemChanged => Self::FilesystemObservation,
            EventKind::FrontmostAppChanged => Self::FrontmostObservation,
            EventKind::ShellStarted => Self::ShellStartedObservation,
            EventKind::ShellFinished => Self::ShellFinishedObservation,
            EventKind::GitSnapshot => Self::GitSnapshotObservation,
            EventKind::BrowserNavigation => Self::BrowserNavigationObservation,
            EventKind::BrowserBookmarkChanged => Self::BrowserBookmarkObservation,
            EventKind::CollectorStarted => Self::CollectorStartedObservation,
            EventKind::CollectorStopped => Self::CollectorStoppedObservation,
            EventKind::Gap => Self::CoverageGap,
            EventKind::PolicyBlockedSummary => Self::PolicyDenied,
            EventKind::SourceError => Self::SourceError,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderedClaim {
    pub grammar_version: u32,
    pub template: ClaimTemplateId,
    pub locale: ClaimLocale,
    pub evidence: Evidence,
    pub citations: Vec<Uuid>,
    pub gap_behavior: GapBehavior,
    pub gap_limited: bool,
    pub text: String,
}

pub fn render_claim(
    event: &EventEnvelope,
    locale: ClaimLocale,
    gap_present: bool,
) -> Result<RenderedClaim, GhostraceError> {
    let template = ClaimTemplateId::for_kind(event.kind);
    let descriptor = template.descriptor();
    if matches!(descriptor.evidence_requirement, EvidenceRequirement::DirectSourceRequired)
        && event.evidence != Evidence::Direct
    {
        return Err(GhostraceError::InvalidEvent(
            "claim requires direct source evidence".to_owned(),
        ));
    }
    let mut text = format!(
        "{} event {}: {}",
        evidence_label(event.evidence),
        event.event_id,
        render_body(event, locale)
    );
    let gap_limited = gap_present && descriptor.gap_behavior == GapBehavior::LimitInterpretation;
    if gap_limited {
        text.push_str(locale.gap_limit_suffix());
    }
    validate_claim_text(&text, descriptor.prohibited_implications)?;
    Ok(RenderedClaim {
        grammar_version: CLAIM_GRAMMAR_VERSION,
        template,
        locale,
        evidence: event.evidence,
        citations: vec![event.event_id],
        gap_behavior: descriptor.gap_behavior,
        gap_limited,
        text,
    })
}

fn evidence_label(evidence: Evidence) -> &'static str {
    match evidence {
        Evidence::Direct => "Direct observation:",
        Evidence::Contextual => "Contextual observation:",
        Evidence::Inferred => "Inferred relationship:",
        Evidence::Unknown => "Unknown observation:",
    }
}

fn render_body(event: &EventEnvelope, locale: ClaimLocale) -> String {
    match &event.payload {
        EventPayload::FilesystemChanged(payload) => match payload.operation {
            FileOperation::Renamed => format!(
                "{} rename event ({:?}) in root {}. Old-to-new identity is not established.",
                locale.filesystem_label(),
                payload.entry_kind,
                payload.root_id
            ),
            operation => format!(
                "{} {} ({:?}, {:?}) in root {}.",
                locale.filesystem_label(),
                debug_lower(operation),
                payload.entry_kind,
                payload.path_class,
                payload.root_id
            ),
        },
        EventPayload::FrontmostAppChanged(payload) => {
            format!("frontmost application {} ({:?}).", debug_lower(payload.change), payload.app_id)
        }
        EventPayload::ShellStarted(payload) => format!(
            "shell session started ({}, session {}).",
            payload.shell_kind, payload.session_id
        ),
        EventPayload::ShellFinished(payload) => format!(
            "shell session finished ({:?}, {} ms, session {}).",
            payload.status, payload.duration_ms, payload.session_id
        ),
        EventPayload::GitSnapshot(payload) => format!(
            "Git snapshot for repository {} at {} ({} changed files; dirty={}).",
            payload.repository_id, payload.head_oid, payload.changed_file_count, payload.dirty
        ),
        EventPayload::BrowserNavigation(payload) => {
            format!("browser navigation in {} to {}.", payload.browser, payload.url)
        }
        EventPayload::BrowserBookmarkChanged(payload) => format!(
            "browser bookmark {} {} in {}.",
            payload.bookmark_id,
            debug_lower(payload.change),
            payload.browser
        ),
        EventPayload::CollectorStarted(payload) => {
            format!("{} collector started ({}).", payload.collector, payload.instance_label)
        }
        EventPayload::CollectorStopped(payload) => {
            format!("{} collector stopped ({}).", payload.collector, payload.instance_label)
        }
        EventPayload::Gap(payload) => format!(
            "{} coverage gap: {} ({} events were not observed).",
            payload.source, payload.reason_code, payload.dropped_count
        ),
        EventPayload::PolicyBlockedSummary(payload) => format!(
            "{} policy denial: {} event(s) ({}) were blocked.",
            payload.source, payload.count, payload.reason_code
        ),
        EventPayload::SourceError(payload) => format!(
            "{} source error: {} (retryable={}). Coverage is unknown.",
            payload.source, payload.reason_code, payload.retryable
        ),
    }
}

fn debug_lower<T: Debug>(value: T) -> String {
    format!("{value:?}").to_lowercase()
}

fn validate_claim_text(
    text: &str,
    prohibitions: &[ProhibitedImplication],
) -> Result<(), GhostraceError> {
    let normalized = text.to_ascii_lowercase();
    for prohibition in prohibitions {
        let forbidden = match prohibition {
            ProhibitedImplication::Intent => ["intent", "intended"].as_slice(),
            ProhibitedImplication::Completeness => {
                ["complete history", "complete causal", "all events"].as_slice()
            }
            ProhibitedImplication::ProcessAttribution => {
                ["process attribution", "attributed to a process"].as_slice()
            }
            ProhibitedImplication::RenameIdentity => {
                ["renamed from", "renamed to", "old path", "new path"].as_slice()
            }
            ProhibitedImplication::UnsupportedCausality => {
                ["caused", "because", "therefore"].as_slice()
            }
        };
        if forbidden.iter().any(|term| normalized.contains(term)) {
            return Err(GhostraceError::InvalidEvent(
                "claim template emitted a prohibited implication".to_owned(),
            ));
        }
    }
    Ok(())
}
