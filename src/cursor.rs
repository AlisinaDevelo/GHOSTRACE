//! Typed cursor identity and transition rules.
//!
//! A cursor is evidence-bound state, not merely a string copied from a
//! collector.  The contract keeps source identity separate from the token,
//! makes ordering explicit, and requires a named reset/wrap operation whenever
//! an opaque or regressed stream cannot be compared safely.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    error::GhostraceError,
    model::{CollectorInstanceId, EventSource, SnapshotDigest, SourceCursor},
    volume::VolumeIdentity,
};

pub const CURSOR_CONTRACT_VERSION: u32 = 1;
pub const REPLAY_BOUNDARY_CONTRACT_VERSION: u32 = 1;

/// The FSEvents stream scope is part of cursor identity.  Per-host and
/// per-device IDs are not interchangeable, and fixture cursors must never be
/// mistaken for a live stream cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStreamMode {
    Fixture,
    PerHost,
    PerDevice,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CursorIdentity {
    pub source: EventSource,
    pub collector_instance: CollectorInstanceId,
    pub stream_mode: CursorStreamMode,
    #[serde(default)]
    pub volume: Option<VolumeIdentity>,
}

/// Settings that define which source history a cursor can safely replay.
/// These are stored as bounded, path-free evidence alongside the cursor; a
/// numeric event ID without this context is not a valid recovery boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayConfiguration {
    pub contract_version: u32,
    pub root_scope_digest: SnapshotDigest,
    pub exclusions_digest: SnapshotDigest,
    pub since_when: u64,
    pub latency_millis: u64,
    pub file_events: bool,
}

impl ReplayConfiguration {
    pub fn new(
        root_scope_digest: SnapshotDigest,
        exclusions_digest: SnapshotDigest,
        since_when: u64,
        latency: Duration,
        file_events: bool,
    ) -> Result<Self, GhostraceError> {
        let latency_millis = latency.as_millis();
        if latency_millis > 3_600_000 || latency_millis > u128::from(u64::MAX) {
            return Err(GhostraceError::InvalidEvent(
                "replay latency exceeds the one-hour boundary".to_owned(),
            ));
        }
        Ok(Self {
            contract_version: REPLAY_BOUNDARY_CONTRACT_VERSION,
            root_scope_digest,
            exclusions_digest,
            since_when,
            latency_millis: latency_millis as u64,
            file_events,
        })
    }

    pub fn digest(&self) -> Result<SnapshotDigest, GhostraceError> {
        if self.contract_version != REPLAY_BOUNDARY_CONTRACT_VERSION {
            return Err(GhostraceError::InvalidEvent(
                "replay configuration contract version is unsupported".to_owned(),
            ));
        }
        let encoded = serde_json::to_vec(self)?;
        let digest = Sha256::digest(encoded);
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        SnapshotDigest::try_from(format!("sha256:{hex}"))
    }
}

/// The complete durable replay boundary.  A changed root, exclusion set,
/// latency, since-when, or file-event setting is a different boundary and is
/// refused until the caller explicitly establishes a new epoch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayBoundary {
    pub contract_version: u32,
    pub identity: CursorIdentity,
    pub configuration: ReplayConfiguration,
}

impl ReplayBoundary {
    pub fn new(
        identity: CursorIdentity,
        configuration: ReplayConfiguration,
    ) -> Result<Self, GhostraceError> {
        if identity.volume.is_none() || matches!(identity.stream_mode, CursorStreamMode::Fixture) {
            return Err(GhostraceError::InvalidEvent(
                "durable replay boundaries require a live volume identity".to_owned(),
            ));
        }
        if configuration.contract_version != REPLAY_BOUNDARY_CONTRACT_VERSION {
            return Err(GhostraceError::InvalidEvent(
                "replay configuration contract version is unsupported".to_owned(),
            ));
        }
        Ok(Self { contract_version: REPLAY_BOUNDARY_CONTRACT_VERSION, identity, configuration })
    }

    pub fn digest(&self) -> Result<SnapshotDigest, GhostraceError> {
        if self.contract_version != REPLAY_BOUNDARY_CONTRACT_VERSION {
            return Err(GhostraceError::InvalidEvent(
                "replay boundary contract version is unsupported".to_owned(),
            ));
        }
        let encoded = serde_json::to_vec(self)?;
        let digest = Sha256::digest(encoded);
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        SnapshotDigest::try_from(format!("sha256:{hex}"))
    }
}

impl CursorIdentity {
    pub fn new(
        source: EventSource,
        collector_instance: impl Into<String>,
    ) -> Result<Self, GhostraceError> {
        Ok(Self {
            source,
            collector_instance: CollectorInstanceId::try_from(collector_instance.into())?,
            stream_mode: CursorStreamMode::Fixture,
            volume: None,
        })
    }

    /// Construct a live cursor identity bound to one stream mode and volume.
    /// A matching collector instance or path string alone is not sufficient to
    /// resume this cursor.
    pub fn for_volume(
        source: EventSource,
        collector_instance: impl Into<String>,
        stream_mode: CursorStreamMode,
        volume: VolumeIdentity,
    ) -> Result<Self, GhostraceError> {
        if matches!(stream_mode, CursorStreamMode::Fixture) {
            return Err(GhostraceError::InvalidEvent(
                "fixture stream mode cannot bind a live volume".to_owned(),
            ));
        }
        Ok(Self {
            source,
            collector_instance: CollectorInstanceId::try_from(collector_instance.into())?,
            stream_mode,
            volume: Some(volume),
        })
    }

    pub fn collector_instance(&self) -> &str {
        self.collector_instance.as_str()
    }

    pub fn stream_mode(&self) -> CursorStreamMode {
        self.stream_mode
    }

    pub fn volume(&self) -> Option<&VolumeIdentity> {
        self.volume.as_ref()
    }

    /// Return whether a stored cursor can be resumed by a candidate source.
    /// Every identity component must match, including volume and stream mode.
    pub fn can_resume_from(&self, candidate: &Self) -> bool {
        self.source == candidate.source
            && self.collector_instance == candidate.collector_instance
            && self.stream_mode == candidate.stream_mode
            && self.volume == candidate.volume
            && self.volume.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorKind {
    Opaque,
    Sequence,
    Reset,
    Wrap,
}

impl CursorKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Sequence => "sequence",
            Self::Reset => "reset",
            Self::Wrap => "wrap",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, GhostraceError> {
        match value {
            "opaque" => Ok(Self::Opaque),
            "sequence" => Ok(Self::Sequence),
            "reset" => Ok(Self::Reset),
            "wrap" => Ok(Self::Wrap),
            _ => Err(GhostraceError::MigrationLedger("cursor state kind is invalid".to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CursorToken {
    raw: SourceCursor,
    kind: CursorKind,
    epoch: Option<u64>,
    position: Option<u128>,
}

impl CursorToken {
    pub fn new(cursor: SourceCursor) -> Self {
        let raw = cursor.as_str();
        if let Some((kind, epoch, position)) = parse_ordered(raw) {
            return Self { raw: cursor, kind, epoch: Some(epoch), position: Some(position) };
        }
        // `cursor-<decimal>` was the fixture cursor before the typed contract
        // landed.  It is safe to treat that legacy shape as epoch zero.
        if let Some(position) = raw.strip_prefix("cursor-").and_then(parse_u128) {
            return Self {
                raw: cursor,
                kind: CursorKind::Sequence,
                epoch: Some(0),
                position: Some(position),
            };
        }
        Self { raw: cursor, kind: CursorKind::Opaque, epoch: None, position: None }
    }

    pub fn raw(&self) -> &SourceCursor {
        &self.raw
    }

    pub fn kind(&self) -> CursorKind {
        self.kind
    }

    pub fn epoch(&self) -> Option<u64> {
        self.epoch
    }

    pub fn position(&self) -> Option<u128> {
        self.position
    }

    pub fn is_ordered(&self) -> bool {
        self.position.is_some()
    }

    pub fn compare(&self, candidate: &Self) -> CursorOrder {
        if self.raw == candidate.raw {
            return CursorOrder::Equal;
        }
        match ((self.epoch, self.position), (candidate.epoch, candidate.position)) {
            (
                (Some(current_epoch), Some(current_position)),
                (Some(candidate_epoch), Some(candidate_position)),
            ) => {
                if (candidate_epoch, candidate_position) > (current_epoch, current_position) {
                    CursorOrder::Advance
                } else {
                    CursorOrder::Regression
                }
            }
            _ => CursorOrder::Unknown,
        }
    }

    pub fn transition(&self, candidate: &Self) -> CursorTransition {
        if self.raw == candidate.raw {
            return CursorTransition::Duplicate;
        }
        match candidate.kind {
            CursorKind::Reset => CursorTransition::Reset,
            CursorKind::Wrap => CursorTransition::Wrap,
            CursorKind::Opaque | CursorKind::Sequence => match self.compare(candidate) {
                CursorOrder::Advance => CursorTransition::Advance,
                CursorOrder::Regression => CursorTransition::Regression,
                CursorOrder::Unknown => CursorTransition::Unknown,
                CursorOrder::Equal => CursorTransition::Duplicate,
            },
        }
    }

    pub fn reset(&self, candidate: Self) -> Result<Self, GhostraceError> {
        if candidate.kind != CursorKind::Reset {
            return Err(GhostraceError::CursorControlInvalid);
        }
        Ok(candidate)
    }

    pub fn wrap(&self, candidate: Self) -> Result<Self, GhostraceError> {
        if candidate.kind != CursorKind::Wrap {
            return Err(GhostraceError::CursorControlInvalid);
        }
        Ok(candidate)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorOrder {
    Equal,
    Advance,
    Regression,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorTransition {
    Initial,
    Duplicate,
    Advance,
    Reset,
    Wrap,
    Regression,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStatus {
    Active,
    Reset,
    Wrapped,
    Invalidated,
}

impl CursorStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Reset => "reset",
            Self::Wrapped => "wrapped",
            Self::Invalidated => "invalidated",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, GhostraceError> {
        match value {
            "active" => Ok(Self::Active),
            "reset" => Ok(Self::Reset),
            "wrapped" => Ok(Self::Wrapped),
            "invalidated" => Ok(Self::Invalidated),
            _ => Err(GhostraceError::MigrationLedger("cursor status is invalid".to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CursorState {
    pub identity: CursorIdentity,
    pub token: CursorToken,
    pub status: CursorStatus,
    pub epoch: u64,
    pub policy_profile_id: Option<String>,
    pub policy_profile_version: Option<u32>,
    pub last_event_id: Option<String>,
    pub boundary: Option<ReplayBoundary>,
}

impl CursorState {
    pub fn invalidated(&self) -> bool {
        self.status == CursorStatus::Invalidated
    }

    pub fn can_accept_first_event(&self) -> bool {
        matches!(self.status, CursorStatus::Reset | CursorStatus::Wrapped)
            && self.last_event_id.is_none()
    }
}

fn parse_ordered(value: &str) -> Option<(CursorKind, u64, u128)> {
    let mut fields = value.split('-');
    let kind = match fields.next()? {
        "seq" => CursorKind::Sequence,
        "reset" => CursorKind::Reset,
        "wrap" => CursorKind::Wrap,
        _ => return None,
    };
    let epoch = fields.next()?.parse().ok()?;
    let position = fields.next()?.parse().ok()?;
    if fields.next().is_some() {
        return None;
    }
    Some((kind, epoch, position))
}

fn parse_u128(value: &str) -> Option<u128> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}
