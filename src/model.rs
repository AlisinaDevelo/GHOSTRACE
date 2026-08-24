//! Versioned, typed event data.  Payload structs intentionally contain only
//! privacy-minimized fields; there is no catch-all map or raw collector blob.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use url::Url;
use uuid::Uuid;

use crate::error::GhostraceError;

/// The first public event schema.  Readers must reject versions they do not understand.
pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const PROVENANCE_VERSION: &str = "fixture-v1";
pub const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;
pub const MAX_METADATA_FIELD_BYTES: usize = 4 * 1024;
pub const MAX_BROWSER_URL_BYTES: usize = 8 * 1024;
/// Maximum UTF-8 byte length for an opaque semantic identifier.
pub const MAX_IDENTIFIER_BYTES: usize = 128;
/// Maximum UTF-8 byte length for an application bundle identifier.
pub const MAX_APP_IDENTIFIER_BYTES: usize = 255;
/// Maximum UTF-8 byte length for a Git branch name.
pub const MAX_BRANCH_BYTES: usize = 255;
/// Maximum UTF-8 byte length for a source cursor token.
pub const MAX_CURSOR_BYTES: usize = 256;
/// Canonical tagged SHA-256 digest length (`sha256:` plus 64 lowercase hex bytes).
pub const SHA256_DIGEST_BYTES: usize = 71;

pub type EventId = Uuid;
pub type Source = EventSource;
pub type Confidence = Evidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Filesystem,
    FrontmostApp,
    Shell,
    Git,
    Browser,
    Lifecycle,
    Fixture,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Filesystem => "filesystem",
            Self::FrontmostApp => "frontmost_app",
            Self::Shell => "shell",
            Self::Git => "git",
            Self::Browser => "browser",
            Self::Lifecycle => "lifecycle",
            Self::Fixture => "fixture",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    FilesystemChanged,
    FrontmostAppChanged,
    ShellStarted,
    ShellFinished,
    GitSnapshot,
    BrowserNavigation,
    BrowserBookmarkChanged,
    CollectorStarted,
    CollectorStopped,
    Gap,
    PolicyBlockedSummary,
    SourceError,
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FilesystemChanged => "filesystem_changed",
            Self::FrontmostAppChanged => "frontmost_app_changed",
            Self::ShellStarted => "shell_started",
            Self::ShellFinished => "shell_finished",
            Self::GitSnapshot => "git_snapshot",
            Self::BrowserNavigation => "browser_navigation",
            Self::BrowserBookmarkChanged => "browser_bookmark_changed",
            Self::CollectorStarted => "collector_started",
            Self::CollectorStopped => "collector_stopped",
            Self::Gap => "gap",
            Self::PolicyBlockedSummary => "policy_blocked_summary",
            Self::SourceError => "source_error",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Direct,
    Contextual,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathClass {
    WorkspaceRelative,
    HomeRelative,
    AbsoluteRedacted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppChange {
    Launched,
    Activated,
    Deactivated,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellStatus {
    Succeeded,
    Failed,
    Signaled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookmarkChange {
    Created,
    Updated,
    Deleted,
}

/// A URL safe to store in a browser event.  Parsing removes userinfo,
/// query, and fragment components; no raw URL is retained in the type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SanitizedUrl(String);

impl SanitizedUrl {
    pub fn sanitize(raw: &str) -> Result<Self, GhostraceError> {
        Self::parse(raw)
    }

    pub fn parse(raw: &str) -> Result<Self, GhostraceError> {
        if raw.len() > MAX_BROWSER_URL_BYTES {
            return Err(GhostraceError::InvalidUrl(format!(
                "URL exceeds the {MAX_BROWSER_URL_BYTES}-byte limit"
            )));
        }
        let mut url = Url::parse(raw).map_err(|e| GhostraceError::InvalidUrl(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(GhostraceError::InvalidUrl(
                "only http and https URLs are accepted".to_owned(),
            ));
        }
        if url.host_str().is_none() {
            return Err(GhostraceError::InvalidUrl("URL must have a host".to_owned()));
        }
        url.set_username("")
            .map_err(|_| GhostraceError::InvalidUrl("cannot remove URL username".to_owned()))?;
        url.set_password(None)
            .map_err(|_| GhostraceError::InvalidUrl("cannot remove URL password".to_owned()))?;
        url.set_query(None);
        url.set_fragment(None);
        Ok(Self(url.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub type BrowserUrl = SanitizedUrl;

impl fmt::Display for SanitizedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for SanitizedUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SanitizedUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemChangedPayload {
    pub root_id: String,
    pub path_class: PathClass,
    pub operation: FileOperation,
    pub entry_kind: EntryKind,
    #[serde(default)]
    pub path_digest: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontmostAppChangedPayload {
    pub app_id: String,
    pub change: AppChange,
    #[serde(default)]
    pub previous_app_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellStartedPayload {
    pub session_id: String,
    pub shell_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellFinishedPayload {
    pub session_id: String,
    pub status: ShellStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSnapshotPayload {
    pub repository_id: String,
    pub branch: String,
    pub head_oid: String,
    pub dirty: bool,
    pub changed_file_count: u64,
    #[serde(default)]
    pub snapshot_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNavigationPayload {
    pub browser: String,
    pub url: SanitizedUrl,
    #[serde(default)]
    pub private_context: bool,
}

impl BrowserNavigationPayload {
    pub fn new(
        browser: impl Into<String>,
        raw_url: &str,
        private_context: bool,
    ) -> Result<Self, GhostraceError> {
        if private_context {
            return Err(GhostraceError::PrivateContext);
        }
        Ok(Self { browser: browser.into(), url: SanitizedUrl::parse(raw_url)?, private_context })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserBookmarkChangedPayload {
    pub browser: String,
    pub bookmark_id: String,
    pub change: BookmarkChange,
    pub url: SanitizedUrl,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub private_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectorLifecyclePayload {
    pub collector: EventSource,
    pub instance_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapPayload {
    pub source: EventSource,
    pub reason_code: String,
    pub dropped_count: u64,
    #[serde(default)]
    pub from_cursor: Option<String>,
    #[serde(default)]
    pub to_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBlockedSummaryPayload {
    pub source: EventSource,
    pub reason_code: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceErrorPayload {
    pub source: EventSource,
    pub reason_code: String,
    pub retryable: bool,
}

/// Typed payload union.  The `type` discriminator makes an accidental raw
/// collector blob impossible to deserialize into the event model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum EventPayload {
    FilesystemChanged(FilesystemChangedPayload),
    FrontmostAppChanged(FrontmostAppChangedPayload),
    ShellStarted(ShellStartedPayload),
    ShellFinished(ShellFinishedPayload),
    GitSnapshot(GitSnapshotPayload),
    BrowserNavigation(BrowserNavigationPayload),
    BrowserBookmarkChanged(BrowserBookmarkChangedPayload),
    CollectorStarted(CollectorLifecyclePayload),
    CollectorStopped(CollectorLifecyclePayload),
    Gap(GapPayload),
    PolicyBlockedSummary(PolicyBlockedSummaryPayload),
    SourceError(SourceErrorPayload),
}

impl EventPayload {
    pub fn kind(&self) -> EventKind {
        match self {
            Self::FilesystemChanged(_) => EventKind::FilesystemChanged,
            Self::FrontmostAppChanged(_) => EventKind::FrontmostAppChanged,
            Self::ShellStarted(_) => EventKind::ShellStarted,
            Self::ShellFinished(_) => EventKind::ShellFinished,
            Self::GitSnapshot(_) => EventKind::GitSnapshot,
            Self::BrowserNavigation(_) => EventKind::BrowserNavigation,
            Self::BrowserBookmarkChanged(_) => EventKind::BrowserBookmarkChanged,
            Self::CollectorStarted(_) => EventKind::CollectorStarted,
            Self::CollectorStopped(_) => EventKind::CollectorStopped,
            Self::Gap(_) => EventKind::Gap,
            Self::PolicyBlockedSummary(_) => EventKind::PolicyBlockedSummary,
            Self::SourceError(_) => EventKind::SourceError,
        }
    }

    pub fn root_id(&self) -> Option<&str> {
        match self {
            Self::FilesystemChanged(payload) => Some(&payload.root_id),
            _ => None,
        }
    }

    pub fn private_context(&self) -> bool {
        match self {
            Self::BrowserNavigation(payload) => payload.private_context,
            Self::BrowserBookmarkChanged(payload) => payload.private_context,
            _ => false,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::FilesystemChanged(payload) => format!(
                "filesystem {} ({:?}, {:?}) in root {}",
                format_args!("{:?}", payload.operation).to_string().to_lowercase(),
                payload.entry_kind,
                payload.path_class,
                payload.root_id
            ),
            Self::FrontmostAppChanged(payload) => {
                format!("frontmost app {} ({:?})", payload.app_id, payload.change)
            }
            Self::ShellStarted(payload) => {
                format!("shell session {} started ({})", payload.session_id, payload.shell_kind)
            }
            Self::ShellFinished(payload) => format!(
                "shell session {} finished ({:?}, {} ms)",
                payload.session_id, payload.status, payload.duration_ms
            ),
            Self::GitSnapshot(payload) => format!(
                "git snapshot {} at {} ({} changed files, dirty={})",
                payload.repository_id, payload.head_oid, payload.changed_file_count, payload.dirty
            ),
            Self::BrowserNavigation(payload) => {
                format!("browser navigation in {} to {}", payload.browser, payload.url)
            }
            Self::BrowserBookmarkChanged(payload) => format!(
                "browser bookmark {} {:?} in {}",
                payload.bookmark_id, payload.change, payload.browser
            ),
            Self::CollectorStarted(payload) => {
                format!("collector {} started ({})", payload.collector, payload.instance_label)
            }
            Self::CollectorStopped(payload) => {
                format!("collector {} stopped ({})", payload.collector, payload.instance_label)
            }
            Self::Gap(payload) => format!(
                "coverage gap for {}: {} ({} events)",
                payload.source, payload.reason_code, payload.dropped_count
            ),
            Self::PolicyBlockedSummary(payload) => format!(
                "policy blocked {} {} event(s) ({})",
                payload.source, payload.count, payload.reason_code
            ),
            Self::SourceError(payload) => format!(
                "source error for {}: {} (retryable={})",
                payload.source, payload.reason_code, payload.retryable
            ),
        }
    }

    fn validate(&self) -> Result<(), GhostraceError> {
        match self {
            Self::FilesystemChanged(payload) => {
                validate_identifier("root_id", &payload.root_id)?;
                validate_optional_digest("path_digest", payload.path_digest.as_deref())?;
            }
            Self::FrontmostAppChanged(payload) => {
                validate_app_identifier("app_id", &payload.app_id)?;
                validate_optional_app_identifier(
                    "previous_app_id",
                    payload.previous_app_id.as_deref(),
                )?;
            }
            Self::ShellStarted(payload) => {
                validate_identifier("session_id", &payload.session_id)?;
                validate_identifier("shell_kind", &payload.shell_kind)?;
            }
            Self::ShellFinished(payload) => {
                validate_identifier("session_id", &payload.session_id)?;
            }
            Self::GitSnapshot(payload) => {
                validate_identifier("repository_id", &payload.repository_id)?;
                validate_branch("branch", &payload.branch)?;
                validate_git_object_id(&payload.head_oid)?;
                validate_optional_digest("snapshot_digest", payload.snapshot_digest.as_deref())?;
            }
            Self::BrowserNavigation(payload) => {
                validate_identifier("browser", &payload.browser)?;
            }
            Self::BrowserBookmarkChanged(payload) => {
                validate_identifier("browser", &payload.browser)?;
                validate_identifier("bookmark_id", &payload.bookmark_id)?;
                validate_optional_identifier("folder_id", payload.folder_id.as_deref())?;
            }
            Self::CollectorStarted(payload) | Self::CollectorStopped(payload) => {
                validate_identifier("instance_label", &payload.instance_label)?;
            }
            Self::Gap(payload) => {
                validate_reason_code(&payload.reason_code)?;
                validate_optional_cursor("from_cursor", payload.from_cursor.as_deref())?;
                validate_optional_cursor("to_cursor", payload.to_cursor.as_deref())?;
            }
            Self::PolicyBlockedSummary(payload) => {
                validate_reason_code(&payload.reason_code)?;
            }
            Self::SourceError(payload) => {
                validate_reason_code(&payload.reason_code)?;
            }
        }
        Ok(())
    }
}

fn invalid_identifier(name: &str, contract: &str) -> GhostraceError {
    // Never include the rejected value: identifiers can contain customer names,
    // paths, or credential material and errors are routinely surfaced in logs.
    GhostraceError::InvalidEvent(format!("{name} is not a canonical {contract}"))
}

pub(crate) fn validate_identifier(name: &str, value: &str) -> Result<(), GhostraceError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value.is_ascii()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        || !value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !value.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        || value.contains("..")
    {
        return Err(invalid_identifier(name, "opaque identifier"));
    }
    Ok(())
}

fn validate_optional_identifier(name: &str, value: Option<&str>) -> Result<(), GhostraceError> {
    if let Some(value) = value {
        validate_identifier(name, value)?;
    }
    Ok(())
}

fn validate_app_identifier(name: &str, value: &str) -> Result<(), GhostraceError> {
    if value.is_empty() || value.len() > MAX_APP_IDENTIFIER_BYTES || !value.is_ascii() {
        return Err(invalid_identifier(name, "application identifier"));
    }
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || !label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            || !label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(invalid_identifier(name, "application identifier"));
        }
    }
    Ok(())
}

fn validate_optional_app_identifier(name: &str, value: Option<&str>) -> Result<(), GhostraceError> {
    if let Some(value) = value {
        validate_app_identifier(name, value)?;
    }
    Ok(())
}

fn validate_branch(name: &str, value: &str) -> Result<(), GhostraceError> {
    let invalid = value.is_empty()
        || value.len() > MAX_BRANCH_BYTES
        || !value.is_ascii()
        || !value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !value.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-'))
        || value.contains("..")
        || value.contains("//")
        || value.contains("@{")
        || value.split('/').any(|component| component.is_empty() || component == ".")
        || value.ends_with(".lock");
    if invalid {
        return Err(invalid_identifier(name, "Git branch name"));
    }
    Ok(())
}

fn validate_git_object_id(value: &str) -> Result<(), GhostraceError> {
    if !matches!(value.len(), 40 | 64)
        || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_identifier("head_oid", "Git object ID"));
    }
    Ok(())
}

fn validate_digest(name: &str, value: &str) -> Result<(), GhostraceError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(invalid_identifier(name, "SHA-256 digest"));
    };
    if value.len() != SHA256_DIGEST_BYTES
        || hex.len() != 64
        || !hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_identifier(name, "SHA-256 digest"));
    }
    Ok(())
}

fn validate_optional_digest(name: &str, value: Option<&str>) -> Result<(), GhostraceError> {
    if let Some(value) = value {
        validate_digest(name, value)?;
    }
    Ok(())
}

fn validate_cursor(name: &str, value: &str) -> Result<(), GhostraceError> {
    if value.is_empty()
        || value.len() > MAX_CURSOR_BYTES
        || !value.is_ascii()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        || !value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !value.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        || value.contains("..")
    {
        return Err(invalid_identifier(name, "source cursor"));
    }
    Ok(())
}

fn validate_optional_cursor(name: &str, value: Option<&str>) -> Result<(), GhostraceError> {
    if let Some(value) = value {
        validate_cursor(name, value)?;
    }
    Ok(())
}

fn validate_reason_code(value: &str) -> Result<(), GhostraceError> {
    let mut bytes = value.bytes();
    if value.len() > 128
        || !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(GhostraceError::InvalidEvent(
            "reason_code must be lower snake case and at most 128 bytes".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(try_from = "UncheckedEventEnvelope")]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
    pub source: EventSource,
    pub kind: EventKind,
    pub payload: EventPayload,
    pub collector_instance: String,
    pub source_cursor: Option<String>,
    pub provenance_version: String,
    pub policy_profile_id: String,
    pub policy_profile_version: u32,
    pub evidence: Evidence,
    pub parent_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedEventEnvelope {
    schema_version: u32,
    event_id: Uuid,
    observed_at: DateTime<Utc>,
    ingested_at: DateTime<Utc>,
    source: EventSource,
    kind: EventKind,
    payload: EventPayload,
    collector_instance: String,
    #[serde(default)]
    source_cursor: Option<String>,
    provenance_version: String,
    policy_profile_id: String,
    policy_profile_version: u32,
    evidence: Evidence,
    #[serde(default)]
    parent_event_id: Option<Uuid>,
}

impl TryFrom<UncheckedEventEnvelope> for EventEnvelope {
    type Error = String;

    fn try_from(raw: UncheckedEventEnvelope) -> Result<Self, Self::Error> {
        let event = Self {
            schema_version: raw.schema_version,
            event_id: raw.event_id,
            observed_at: raw.observed_at,
            ingested_at: raw.ingested_at,
            source: raw.source,
            kind: raw.kind,
            payload: raw.payload,
            collector_instance: raw.collector_instance,
            source_cursor: raw.source_cursor,
            provenance_version: raw.provenance_version,
            policy_profile_id: raw.policy_profile_id,
            policy_profile_version: raw.policy_profile_version,
            evidence: raw.evidence,
            parent_event_id: raw.parent_event_id,
        };
        event.validate().map_err(|error| error.to_string())?;
        Ok(event)
    }
}

impl<'de> Deserialize<'de> for EventEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UncheckedEventEnvelope::deserialize(deserializer)?;
        raw.try_into().map_err(D::Error::custom)
    }
}

impl EventEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: Uuid,
        observed_at: DateTime<Utc>,
        ingested_at: DateTime<Utc>,
        source: EventSource,
        kind: EventKind,
        payload: EventPayload,
        collector_instance: impl Into<String>,
        source_cursor: Option<String>,
        provenance_version: impl Into<String>,
        policy_profile_id: impl Into<String>,
        policy_profile_version: u32,
        evidence: Evidence,
        parent_event_id: Option<Uuid>,
    ) -> Result<Self, GhostraceError> {
        let event = Self {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id,
            observed_at,
            ingested_at,
            source,
            kind,
            payload,
            collector_instance: collector_instance.into(),
            source_cursor,
            provenance_version: provenance_version.into(),
            policy_profile_id: policy_profile_id.into(),
            policy_profile_version,
            evidence,
            parent_event_id,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != EVENT_SCHEMA_VERSION {
            return Err(GhostraceError::UnsupportedSchema(self.schema_version));
        }
        if self.event_id.is_nil() {
            return Err(GhostraceError::InvalidEvent("event_id must not be nil".to_owned()));
        }
        if self.ingested_at < self.observed_at {
            return Err(GhostraceError::InvalidEvent(
                "ingested_at must not precede observed_at".to_owned(),
            ));
        }
        validate_identifier("collector_instance", &self.collector_instance)?;
        validate_identifier("provenance_version", &self.provenance_version)?;
        validate_identifier("policy_profile_id", &self.policy_profile_id)?;
        if self.policy_profile_version == 0 {
            return Err(GhostraceError::InvalidEvent(
                "policy_profile_version must be greater than zero".to_owned(),
            ));
        }
        validate_optional_cursor("source_cursor", self.source_cursor.as_deref())?;
        if self.parent_event_id == Some(self.event_id) {
            return Err(GhostraceError::InvalidEvent(
                "parent_event_id must differ from event_id".to_owned(),
            ));
        }
        if self.parent_event_id.is_some_and(|event_id| event_id.is_nil()) {
            return Err(GhostraceError::InvalidEvent("parent_event_id must not be nil".to_owned()));
        }
        if self.payload.kind() != self.kind {
            return Err(GhostraceError::InvalidEvent(format!(
                "kind {} does not match payload {}",
                self.kind,
                self.payload.kind()
            )));
        }
        if serde_json::to_vec(&self.payload)?.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(GhostraceError::InvalidEvent(format!(
                "payload exceeds the {MAX_EVENT_PAYLOAD_BYTES}-byte limit"
            )));
        }
        self.payload.validate()?;
        if self.payload.private_context() {
            return Err(GhostraceError::PrivateContext);
        }
        let source_matches = match self.kind {
            EventKind::FilesystemChanged => self.source == EventSource::Filesystem,
            EventKind::FrontmostAppChanged => self.source == EventSource::FrontmostApp,
            EventKind::ShellStarted | EventKind::ShellFinished => self.source == EventSource::Shell,
            EventKind::GitSnapshot => self.source == EventSource::Git,
            EventKind::BrowserNavigation | EventKind::BrowserBookmarkChanged => {
                self.source == EventSource::Browser
            }
            EventKind::CollectorStarted | EventKind::CollectorStopped => {
                self.source == EventSource::Lifecycle
            }
            EventKind::Gap | EventKind::PolicyBlockedSummary | EventKind::SourceError => true,
        };
        if !source_matches {
            return Err(GhostraceError::InvalidEvent(format!(
                "source {} does not support kind {}",
                self.source, self.kind
            )));
        }
        let status_source = match &self.payload {
            EventPayload::Gap(payload) => Some(payload.source),
            EventPayload::PolicyBlockedSummary(payload) => Some(payload.source),
            EventPayload::SourceError(payload) => Some(payload.source),
            _ => None,
        };
        if status_source.is_some_and(|source| source != self.source) {
            return Err(GhostraceError::InvalidEvent(
                "status payload source must match the event source".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn to_json_line(&self) -> Result<String, GhostraceError> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }
}
