//! Versioned, bounded exclusion matching for the pre-persistence policy gate.
//!
//! Matching is deliberately not regex based. Patterns are compiled into a tiny
//! `*`/`?` glob language and evaluated with a greedy linear-time matcher. Inputs
//! are bounded before matching, so an attacker-shaped pattern cannot create an
//! unbounded backtracking path or leak the value into a decision record.

use std::collections::BTreeMap;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    error::GhostraceError,
    model::{ApplicationId, EntryKind, RootId},
};

pub const EXCLUSION_POLICY_SCHEMA_VERSION: u32 = 1;
pub const MAX_EXCLUSION_RULES: usize = 128;
pub const MAX_EXCLUSION_PATTERN_BYTES: usize = 128;
pub const MAX_EXCLUSION_SUBJECT_BYTES: usize = 1024;
pub const MAX_EXCLUSION_POLICY_VERSIONS: usize = 64;

/// The outcome of an exclusion rule. Safety outcomes are ordered explicitly;
/// a later allow can never weaken an earlier deny or redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionAction {
    Allow,
    Summarize,
    Redact,
    Deny,
}

impl ExclusionAction {
    fn safety_rank(self) -> u8 {
        match self {
            Self::Allow => 1,
            Self::Summarize => 2,
            Self::Redact => 3,
            Self::Deny => 4,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Summarize => "summarize",
            Self::Redact => "redact",
            Self::Deny => "deny",
        }
    }
}

/// A stable rule class. The class order is only a tie-break after safety: a
/// deny always beats a redact, regardless of where either rule appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionKind {
    Vcs,
    TemporaryFile,
    FileKind,
    Application,
    Root,
    Subtree,
    User,
}

impl ExclusionKind {
    fn precedence(self) -> u8 {
        match self {
            Self::Vcs => 1,
            Self::TemporaryFile => 2,
            Self::FileKind => 3,
            Self::Application => 4,
            Self::Root => 5,
            Self::Subtree => 6,
            Self::User => 7,
        }
    }
}

/// A serializable rule. Pattern-bearing rules use the bounded glob language:
/// `*` matches any sequence (including `/`), `?` matches one character, and a
/// backslash escapes `*`, `?`, or `\`. Root rules remain exact identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExclusionRule {
    Root { root: String, action: ExclusionAction },
    Subtree { pattern: String, action: ExclusionAction },
    FileKind { file_kind: EntryKind, action: ExclusionAction },
    Application { pattern: String, action: ExclusionAction },
    TemporaryFile { action: ExclusionAction },
    Vcs { action: ExclusionAction },
    User { pattern: String, action: ExclusionAction },
}

impl ExclusionRule {
    pub fn root(root: impl Into<String>, action: ExclusionAction) -> Result<Self, GhostraceError> {
        let rule = Self::Root { root: root.into(), action };
        rule.validate()?;
        Ok(rule)
    }

    pub fn subtree(
        pattern: impl Into<String>,
        action: ExclusionAction,
    ) -> Result<Self, GhostraceError> {
        let rule = Self::Subtree { pattern: pattern.into(), action };
        rule.validate()?;
        Ok(rule)
    }

    pub fn file_kind(kind: EntryKind, action: ExclusionAction) -> Self {
        Self::FileKind { file_kind: kind, action }
    }

    pub fn application(
        pattern: impl Into<String>,
        action: ExclusionAction,
    ) -> Result<Self, GhostraceError> {
        let rule = Self::Application { pattern: pattern.into(), action };
        rule.validate()?;
        Ok(rule)
    }

    pub fn temporary_file(action: ExclusionAction) -> Self {
        Self::TemporaryFile { action }
    }

    pub fn vcs(action: ExclusionAction) -> Self {
        Self::Vcs { action }
    }

    pub fn user(
        pattern: impl Into<String>,
        action: ExclusionAction,
    ) -> Result<Self, GhostraceError> {
        let rule = Self::User { pattern: pattern.into(), action };
        rule.validate()?;
        Ok(rule)
    }

    fn kind(&self) -> ExclusionKind {
        match self {
            Self::Root { .. } => ExclusionKind::Root,
            Self::Subtree { .. } => ExclusionKind::Subtree,
            Self::FileKind { .. } => ExclusionKind::FileKind,
            Self::Application { .. } => ExclusionKind::Application,
            Self::TemporaryFile { .. } => ExclusionKind::TemporaryFile,
            Self::Vcs { .. } => ExclusionKind::Vcs,
            Self::User { .. } => ExclusionKind::User,
        }
    }

    fn action(&self) -> ExclusionAction {
        match self {
            Self::Root { action, .. }
            | Self::Subtree { action, .. }
            | Self::FileKind { action, .. }
            | Self::Application { action, .. }
            | Self::TemporaryFile { action }
            | Self::Vcs { action }
            | Self::User { action, .. } => *action,
        }
    }

    fn specificity(&self) -> usize {
        match self {
            Self::Root { root, .. } => root.len(),
            Self::Subtree { pattern, .. }
            | Self::Application { pattern, .. }
            | Self::User { pattern, .. } => {
                pattern.chars().filter(|c| !matches!(c, '*' | '?')).count()
            }
            Self::FileKind { .. } | Self::TemporaryFile { .. } | Self::Vcs { .. } => 0,
        }
    }

    fn pattern(&self) -> Option<&str> {
        match self {
            Self::Root { root, .. } => Some(root),
            Self::Subtree { pattern, .. }
            | Self::Application { pattern, .. }
            | Self::User { pattern, .. } => Some(pattern),
            Self::FileKind { .. } | Self::TemporaryFile { .. } | Self::Vcs { .. } => None,
        }
    }

    fn validate(&self) -> Result<(), GhostraceError> {
        match self {
            Self::Root { root, .. } => {
                RootId::try_from(root.clone()).map(|_| ()).map_err(|_| invalid_exclusion())
            }
            Self::Subtree { pattern, .. }
            | Self::Application { pattern, .. }
            | Self::User { pattern, .. } => compile_pattern(pattern).map(|_| ()),
            Self::FileKind { .. } | Self::TemporaryFile { .. } | Self::Vcs { .. } => Ok(()),
        }
    }
}

/// The ephemeral observation attributes evaluated before an event is built.
/// It owns no path and is never embedded in an [`ExclusionDecision`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExclusionSubject<'a> {
    root_id: &'a str,
    relative_path: Option<&'a str>,
    file_kind: Option<EntryKind>,
    application_id: Option<&'a str>,
    temporary_file: bool,
    vcs: bool,
}

impl<'a> ExclusionSubject<'a> {
    pub fn new(root_id: &'a str) -> Self {
        Self {
            root_id,
            relative_path: None,
            file_kind: None,
            application_id: None,
            temporary_file: false,
            vcs: false,
        }
    }

    pub fn with_relative_path(mut self, path: &'a str) -> Self {
        self.relative_path = Some(path);
        self
    }

    pub fn with_file_kind(mut self, kind: EntryKind) -> Self {
        self.file_kind = Some(kind);
        self
    }

    pub fn with_application(mut self, application_id: &'a str) -> Self {
        self.application_id = Some(application_id);
        self
    }

    pub fn temporary_file(mut self) -> Self {
        self.temporary_file = true;
        self
    }

    pub fn vcs(mut self) -> Self {
        self.vcs = true;
        self
    }

    fn normalized(self) -> Result<NormalizedSubject, SubjectError> {
        let root = RootId::try_from(self.root_id).map_err(|_| SubjectError::Invalid)?;
        let path = self.relative_path.map(normalize_path).transpose()?;
        let application = self
            .application_id
            .map(|value| ApplicationId::try_from(value).map_err(|_| SubjectError::Invalid))
            .transpose()?;
        let user_key = match path.as_deref() {
            Some(path) => format!("{}/{}", root.as_str(), path),
            None => root.as_str().to_owned(),
        };
        if user_key.len() > MAX_EXCLUSION_SUBJECT_BYTES {
            return Err(SubjectError::TooLarge);
        }
        Ok(NormalizedSubject {
            root: root.as_str().to_owned(),
            path,
            application: application.map(|value| value.as_str().to_ascii_lowercase()),
            user_key: user_key.to_ascii_lowercase(),
            file_kind: self.file_kind,
            temporary_file: self.temporary_file,
            vcs: self.vcs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSubject {
    root: String,
    path: Option<String>,
    application: Option<String>,
    user_key: String,
    file_kind: Option<EntryKind>,
    temporary_file: bool,
    vcs: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubjectError {
    Invalid,
    TooLarge,
}

/// A privacy-bounded result. No matched pattern, path, or application value is
/// retained in this record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    NoMatch,
    InvalidSubject,
    Vcs,
    TemporaryFile,
    FileKind,
    Application,
    Root,
    Subtree,
    User,
}

impl ExclusionReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::NoMatch => "no_exclusion_match",
            Self::InvalidSubject => "invalid_exclusion_subject",
            Self::Vcs => "vcs_exclusion",
            Self::TemporaryFile => "temporary_file_exclusion",
            Self::FileKind => "file_kind_exclusion",
            Self::Application => "application_exclusion",
            Self::Root => "root_exclusion",
            Self::Subtree => "subtree_exclusion",
            Self::User => "user_exclusion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExclusionDecision {
    pub policy_version: u32,
    pub action: ExclusionAction,
    pub matched_kind: Option<ExclusionKind>,
    pub reason: ExclusionReason,
}

impl ExclusionDecision {
    pub fn is_allowed(self) -> bool {
        self.action == ExclusionAction::Allow
    }

    pub fn reason_code(self) -> &'static str {
        self.reason.code()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExclusionPolicy {
    pub schema_version: u32,
    pub version: u32,
    pub rules: Vec<ExclusionRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedExclusionPolicy {
    schema_version: u32,
    version: u32,
    rules: Vec<ExclusionRule>,
}

impl<'de> Deserialize<'de> for ExclusionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UncheckedExclusionPolicy::deserialize(deserializer)?;
        let policy =
            Self { schema_version: raw.schema_version, version: raw.version, rules: raw.rules };
        policy.validate().map_err(D::Error::custom)?;
        Ok(policy)
    }
}

impl ExclusionPolicy {
    pub fn new(version: u32) -> Result<Self, GhostraceError> {
        let policy =
            Self { schema_version: EXCLUSION_POLICY_SCHEMA_VERSION, version, rules: Vec::new() };
        policy.validate()?;
        Ok(policy)
    }

    pub fn with_rules(
        version: u32,
        rules: impl IntoIterator<Item = ExclusionRule>,
    ) -> Result<Self, GhostraceError> {
        let policy = Self {
            schema_version: EXCLUSION_POLICY_SCHEMA_VERSION,
            version,
            rules: rules.into_iter().collect(),
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn add_rule(&mut self, rule: ExclusionRule) -> Result<(), GhostraceError> {
        if self.rules.len() >= MAX_EXCLUSION_RULES {
            return Err(invalid_exclusion());
        }
        rule.validate()?;
        self.rules.push(rule);
        Ok(())
    }

    pub fn from_json(input: &str) -> Result<Self, GhostraceError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_json(&self) -> Result<String, GhostraceError> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    pub fn scope_digest(&self) -> Result<crate::model::SnapshotDigest, GhostraceError> {
        self.validate()?;
        // Rule order is not semantics. Sorting canonical rule JSON keeps the
        // digest stable when a caller merely reorders its input list.
        let mut canonical_rules =
            self.rules.iter().map(serde_json::to_string).collect::<Result<Vec<_>, _>>()?;
        canonical_rules.sort();
        let canonical = serde_json::to_vec(&(self.schema_version, self.version, canonical_rules))?;
        let digest = Sha256::digest(canonical);
        let encoded = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        crate::model::SnapshotDigest::try_from(format!("sha256:{encoded}"))
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != EXCLUSION_POLICY_SCHEMA_VERSION
            || self.version == 0
            || self.rules.len() > MAX_EXCLUSION_RULES
        {
            return Err(invalid_exclusion());
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        Ok(())
    }

    /// Evaluate one future observation. The input order is never a precedence
    /// input: safety action, rule class, specificity, and canonical pattern are.
    pub fn evaluate(&self, subject: ExclusionSubject<'_>) -> ExclusionDecision {
        if self.validate().is_err() {
            return self.invalid_decision();
        }
        let normalized = match subject.normalized() {
            Ok(value) => value,
            Err(_) => return self.invalid_decision(),
        };
        let mut best: Option<Candidate<'_>> = None;
        for rule in &self.rules {
            let Some(candidate) = Candidate::from_rule(rule, &normalized) else {
                continue;
            };
            if best.as_ref().is_none_or(|current| candidate > *current) {
                best = Some(candidate);
            }
        }
        best.map_or(
            ExclusionDecision {
                policy_version: self.version,
                action: ExclusionAction::Allow,
                matched_kind: None,
                reason: ExclusionReason::NoMatch,
            },
            |candidate| ExclusionDecision {
                policy_version: self.version,
                action: candidate.action,
                matched_kind: Some(candidate.kind),
                reason: match candidate.kind {
                    ExclusionKind::Vcs => ExclusionReason::Vcs,
                    ExclusionKind::TemporaryFile => ExclusionReason::TemporaryFile,
                    ExclusionKind::FileKind => ExclusionReason::FileKind,
                    ExclusionKind::Application => ExclusionReason::Application,
                    ExclusionKind::Root => ExclusionReason::Root,
                    ExclusionKind::Subtree => ExclusionReason::Subtree,
                    ExclusionKind::User => ExclusionReason::User,
                },
            },
        )
    }

    fn invalid_decision(&self) -> ExclusionDecision {
        ExclusionDecision {
            policy_version: self.version,
            action: ExclusionAction::Deny,
            matched_kind: None,
            reason: ExclusionReason::InvalidSubject,
        }
    }
}

/// Retains validated policy versions so a new policy is applied only to future
/// observations. Existing evidence keeps its recorded version and can be
/// re-evaluated against that exact historical policy when explicitly requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusionPolicyHistory {
    versions: BTreeMap<u32, ExclusionPolicy>,
    current_version: u32,
}

impl ExclusionPolicyHistory {
    pub fn new(initial: ExclusionPolicy) -> Result<Self, GhostraceError> {
        initial.validate()?;
        let current_version = initial.version;
        Ok(Self { versions: BTreeMap::from([(current_version, initial)]), current_version })
    }

    pub fn current(&self) -> &ExclusionPolicy {
        self.versions.get(&self.current_version).expect("history has current policy")
    }

    pub fn install(&mut self, next: ExclusionPolicy) -> Result<(), GhostraceError> {
        next.validate()?;
        if next.version <= self.current_version {
            return Err(GhostraceError::PolicyMigration(
                "exclusion policy versions must increase monotonically".to_owned(),
            ));
        }
        if self.versions.len() >= MAX_EXCLUSION_POLICY_VERSIONS {
            return Err(GhostraceError::PolicyMigration(
                "exclusion policy history is bounded".to_owned(),
            ));
        }
        self.current_version = next.version;
        self.versions.insert(next.version, next);
        Ok(())
    }

    pub fn evaluate_future(&self, subject: ExclusionSubject<'_>) -> ExclusionDecision {
        self.current().evaluate(subject)
    }

    pub fn evaluate_recorded(
        &self,
        policy_version: u32,
        subject: ExclusionSubject<'_>,
    ) -> Result<ExclusionDecision, GhostraceError> {
        self.versions.get(&policy_version).map(|policy| policy.evaluate(subject)).ok_or_else(|| {
            GhostraceError::PolicyMigration("recorded exclusion version is unavailable".to_owned())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate<'a> {
    action: ExclusionAction,
    kind: ExclusionKind,
    specificity: usize,
    pattern: Option<&'a str>,
}

impl PartialOrd for Candidate<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.action
            .safety_rank()
            .cmp(&other.action.safety_rank())
            .then_with(|| self.kind.precedence().cmp(&other.kind.precedence()))
            .then_with(|| self.specificity.cmp(&other.specificity))
            .then_with(|| self.pattern.cmp(&other.pattern))
    }
}

impl<'a> Candidate<'a> {
    fn from_rule(rule: &'a ExclusionRule, subject: &NormalizedSubject) -> Option<Self> {
        let matches = match rule {
            ExclusionRule::Root { root, .. } => root == &subject.root,
            ExclusionRule::Subtree { pattern, .. } => {
                subject.path.as_deref().is_some_and(|path| subtree_match(pattern, path))
            }
            ExclusionRule::FileKind { file_kind, .. } => subject.file_kind == Some(*file_kind),
            ExclusionRule::Application { pattern, .. } => subject
                .application
                .as_deref()
                .is_some_and(|application| glob_match(pattern, application)),
            ExclusionRule::TemporaryFile { .. } => subject.temporary_file,
            ExclusionRule::Vcs { .. } => subject.vcs,
            ExclusionRule::User { pattern, .. } => glob_match(pattern, &subject.user_key),
        };
        matches.then_some(Self {
            action: rule.action(),
            kind: rule.kind(),
            specificity: rule.specificity(),
            pattern: rule.pattern(),
        })
    }
}

fn invalid_exclusion() -> GhostraceError {
    GhostraceError::PolicyMigration("exclusion rule is invalid".to_owned())
}

fn normalize_path(path: &str) -> Result<String, SubjectError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(SubjectError::Invalid);
    }
    let mut components = Vec::new();
    for component in path.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." || component.chars().any(char::is_control) {
            return Err(SubjectError::Invalid);
        }
        components.push(component.to_lowercase());
    }
    let normalized = components.join("/");
    if normalized.is_empty() || normalized.len() > MAX_EXCLUSION_SUBJECT_BYTES {
        return Err(SubjectError::TooLarge);
    }
    Ok(normalized)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobToken {
    Literal(char),
    AnyOne,
    AnyMany,
}

fn compile_pattern(pattern: &str) -> Result<Vec<GlobToken>, GhostraceError> {
    if pattern.is_empty() || pattern.len() > MAX_EXCLUSION_PATTERN_BYTES {
        return Err(invalid_exclusion());
    }
    let mut tokens = Vec::new();
    let mut escaped = false;
    for character in pattern.chars() {
        if escaped {
            if character.is_control() || !matches!(character, '*' | '?' | '\\' | '/') {
                return Err(invalid_exclusion());
            }
            tokens.push(GlobToken::Literal(character.to_lowercase().next().unwrap_or(character)));
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '*' => tokens.push(GlobToken::AnyMany),
            '?' => tokens.push(GlobToken::AnyOne),
            c if c.is_control() => return Err(invalid_exclusion()),
            c => tokens.push(GlobToken::Literal(c.to_lowercase().next().unwrap_or(c))),
        }
    }
    if escaped || tokens.is_empty() {
        return Err(invalid_exclusion());
    }
    if pattern.split('/').any(|component| component == "..") || pattern.starts_with('/') {
        return Err(invalid_exclusion());
    }
    Ok(tokens)
}

fn glob_match(pattern: &str, value: &str) -> bool {
    let Ok(tokens) = compile_pattern(pattern) else {
        return false;
    };
    let value = value.to_lowercase().chars().collect::<Vec<_>>();
    let mut token_index = 0;
    let mut value_index = 0;
    let mut star_index = None;
    let mut star_value_index = 0;
    while value_index < value.len() {
        match tokens.get(token_index) {
            Some(GlobToken::Literal(expected)) if *expected == value[value_index] => {
                token_index += 1;
                value_index += 1;
            }
            Some(GlobToken::AnyOne) => {
                token_index += 1;
                value_index += 1;
            }
            Some(GlobToken::AnyMany) => {
                star_index = Some(token_index);
                star_value_index = value_index;
                token_index += 1;
            }
            _ if star_index.is_some() => {
                token_index = star_index.expect("star index") + 1;
                star_value_index += 1;
                value_index = star_value_index;
            }
            _ => return false,
        }
    }
    while matches!(tokens.get(token_index), Some(GlobToken::AnyMany)) {
        token_index += 1;
    }
    token_index == tokens.len()
}

fn subtree_match(pattern: &str, path: &str) -> bool {
    if glob_match(pattern, path) {
        return true;
    }
    let has_wildcard = pattern.chars().any(|c| matches!(c, '*' | '?'));
    if has_wildcard {
        return false;
    }
    let pattern = pattern.to_lowercase();
    path == pattern || path.starts_with(&format!("{pattern}/"))
}
