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
                validate_metadata("root_id", &payload.root_id)?;
                validate_optional_metadata("path_digest", payload.path_digest.as_deref())?;
            }
            Self::FrontmostAppChanged(payload) => {
                validate_metadata("app_id", &payload.app_id)?;
                validate_optional_metadata("previous_app_id", payload.previous_app_id.as_deref())?;
            }
            Self::ShellStarted(payload) => {
                validate_metadata("session_id", &payload.session_id)?;
                validate_metadata("shell_kind", &payload.shell_kind)?;
            }
            Self::ShellFinished(payload) => {
                validate_metadata("session_id", &payload.session_id)?;
            }
            Self::GitSnapshot(payload) => {
                validate_metadata("repository_id", &payload.repository_id)?;
                validate_metadata("branch", &payload.branch)?;
                if !matches!(payload.head_oid.len(), 40 | 64)
                    || !payload
                        .head_oid
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(GhostraceError::InvalidEvent(
                        "head_oid must be a lowercase 40- or 64-character hexadecimal object ID"
                            .to_owned(),
                    ));
                }
                validate_optional_metadata("snapshot_digest", payload.snapshot_digest.as_deref())?;
            }
            Self::BrowserNavigation(payload) => {
                validate_metadata("browser", &payload.browser)?;
            }
            Self::BrowserBookmarkChanged(payload) => {
                validate_metadata("browser", &payload.browser)?;
                validate_metadata("bookmark_id", &payload.bookmark_id)?;
                validate_optional_metadata("folder_id", payload.folder_id.as_deref())?;
            }
            Self::CollectorStarted(payload) | Self::CollectorStopped(payload) => {
                validate_metadata("instance_label", &payload.instance_label)?;
            }
            Self::Gap(payload) => {
                validate_reason_code(&payload.reason_code)?;
                validate_optional_metadata("from_cursor", payload.from_cursor.as_deref())?;
                validate_optional_metadata("to_cursor", payload.to_cursor.as_deref())?;
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

fn validate_metadata(name: &str, value: &str) -> Result<(), GhostraceError> {
    if value.trim().is_empty()
        || value.len() > MAX_METADATA_FIELD_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(GhostraceError::InvalidEvent(format!(
            "{name} must be non-empty, bounded metadata without control characters"
        )));
    }
    Ok(())
}

fn validate_optional_metadata(name: &str, value: Option<&str>) -> Result<(), GhostraceError> {
    if let Some(value) = value {
        validate_metadata(name, value)?;
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
        if self.collector_instance.trim().is_empty() {
            return Err(GhostraceError::InvalidEvent(
                "collector_instance must not be empty".to_owned(),
            ));
        }
        if self.collector_instance.len() > MAX_METADATA_FIELD_BYTES
            || self.collector_instance.chars().any(char::is_control)
        {
            return Err(GhostraceError::InvalidEvent(
                "collector_instance exceeds the metadata limit".to_owned(),
            ));
        }
        if self.provenance_version.trim().is_empty() {
            return Err(GhostraceError::InvalidEvent(
                "provenance_version must not be empty".to_owned(),
            ));
        }
        if self.provenance_version.len() > MAX_METADATA_FIELD_BYTES
            || self.provenance_version.chars().any(char::is_control)
        {
            return Err(GhostraceError::InvalidEvent(
                "provenance_version exceeds the metadata limit".to_owned(),
            ));
        }
        if self.policy_profile_id.trim().is_empty() {
            return Err(GhostraceError::InvalidEvent(
                "policy_profile_id must not be empty".to_owned(),
            ));
        }
        if self.policy_profile_id.len() > MAX_METADATA_FIELD_BYTES
            || self.policy_profile_id.chars().any(char::is_control)
        {
            return Err(GhostraceError::InvalidEvent(
                "policy_profile_id exceeds the metadata limit".to_owned(),
            ));
        }
        if self.policy_profile_version == 0 {
            return Err(GhostraceError::InvalidEvent(
                "policy_profile_version must be greater than zero".to_owned(),
            ));
        }
        if self.source_cursor.as_deref().is_some_and(str::is_empty) {
            return Err(GhostraceError::InvalidEvent(
                "source_cursor must be omitted or non-empty".to_owned(),
            ));
        }
        if self.source_cursor.as_deref().is_some_and(|cursor| {
            cursor.len() > MAX_METADATA_FIELD_BYTES || cursor.chars().any(char::is_control)
        }) {
            return Err(GhostraceError::InvalidEvent(
                "source_cursor exceeds the metadata limit".to_owned(),
            ));
        }
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
