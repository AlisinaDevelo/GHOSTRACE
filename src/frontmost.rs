//! Frontmost-application identity and session semantics.
//!
//! This is the normalization boundary used by the opt-in macOS NSWorkspace
//! adapter. The adapter reads only the bounded facts in
//! [`FrontmostRawObservation`] from `NSRunningApplication` and the code-signing
//! API; that type has no field for a window title, document name, URL,
//! accessibility data, menu state, or screen content, and strict
//! deserialization rejects any such field. Normalization keeps a lowercase
//! bundle identifier, the developer-set bundle name and short version from the
//! bundle's own `Info.plist` (never the localized or user-renamed display
//! name, and never the bundle path), a signing-identity class, an application kind and
//! location class, and a salted launch-instance digest in place of the process
//! ID and start time. Activation is contextual evidence only: it never proves
//! that the application caused a filesystem change.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::model::{ApplicationId, SnapshotDigest};

/// Version of the frontmost observation contract.
pub const FRONTMOST_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized raw observation accepted from an adapter.
pub const MAX_FRONTMOST_RAW_BYTES: usize = 4 * 1024;
/// Activations shorter than this are marked transient.
pub const FRONTMOST_TRANSIENT_DWELL_MS: u64 = 500;

/// Checked-in JSON Schema for normalized observations.
pub const FRONTMOST_SCHEMA_JSON: &str = include_str!("../schemas/frontmost-observation-v1.json");
/// Checked-in outcome corpus.
pub const FRONTMOST_IDENTITY_CORPUS_JSON: &str =
    include_str!("../fixtures/frontmost-identity-v1.json");

const LAUNCH_INSTANCE_DOMAIN: &[u8] = b"ghostrace-frontmost-launch-instance-v1\0";
const TEAM_ID_LEN: usize = 10;
/// Longest bundle name kept; a longer one is dropped rather than truncated.
pub const MAX_FRONTMOST_APP_NAME_CHARS: usize = 64;
/// Longest bundle short version kept.
pub const MAX_FRONTMOST_APP_VERSION_CHARS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum FrontmostError {
    #[error("frontmost observation is malformed")]
    Malformed,
    #[error("frontmost observation exceeds its byte bound")]
    TooLarge,
}

/// The NSWorkspace notification that produced an observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostTransition {
    Activated,
    Deactivated,
    Terminated,
}

/// `NSApplicationActivationPolicy` as reported by the running application.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostActivationPolicy {
    Regular,
    Accessory,
    Prohibited,
    Unknown,
}

/// Code-signing facts from `SecCodeCopySigningInformation`, without the
/// certificate chain, entitlements, or designated requirement text.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontmostSigningInput {
    /// Whether the dynamic code validity check passed.
    pub valid: bool,
    pub ad_hoc: bool,
    pub platform_binary: bool,
    pub team_identifier: Option<String>,
}

/// Everything an adapter may pass across the boundary. Raw values are
/// consumed by normalization and never serialized.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontmostRawObservation {
    pub transition: FrontmostTransition,
    pub observed_at: DateTime<Utc>,
    pub bundle_identifier: Option<String>,
    /// `CFBundleName` from the bundle's unlocalized Info.plist.
    #[serde(default)]
    pub bundle_name: Option<String>,
    /// `CFBundleShortVersionString` from the bundle's unlocalized Info.plist.
    #[serde(default)]
    pub bundle_version: Option<String>,
    /// Whether the executable lives inside an application bundle.
    pub bundled: bool,
    pub activation_policy: FrontmostActivationPolicy,
    /// Whether Gatekeeper App Translocation is running the bundle from a
    /// randomized read-only mount.
    pub translocated: bool,
    pub signing: Option<FrontmostSigningInput>,
    pub process_id: i32,
    /// Process start time in microseconds since the Unix epoch.
    pub process_started_micros: i64,
}

impl FrontmostRawObservation {
    /// Parse a bounded adapter record. Unknown fields, including titles,
    /// URLs, documents, and accessibility values, are rejected.
    pub fn parse(input: &str) -> Result<Self, FrontmostError> {
        if input.len() > MAX_FRONTMOST_RAW_BYTES {
            return Err(FrontmostError::TooLarge);
        }
        serde_json::from_str(input).map_err(|_| FrontmostError::Malformed)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum FrontmostSigningIdentity {
    /// Signed with a Developer ID or App Store certificate for this team.
    Developer {
        team_id: String,
    },
    /// An Apple platform binary.
    Platform,
    AdHoc,
    Unsigned,
    /// The signature was present but did not validate, or was unavailable.
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostAppKind {
    Regular,
    /// An accessory or background helper that can still become frontmost.
    Helper,
    /// An unbundled executable, such as a command-line tool with a window.
    CommandLine,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostAppLocation {
    Installed,
    Translocated,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostUnknownReason {
    /// No bundle identifier, no valid signature, and no usable process.
    NoIdentity,
    /// A private application or a user exclusion; identity is withheld
    /// before persistence.
    Excluded,
    /// The process identity was invalid, so no launch instance exists.
    NoProcess,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "identity", rename_all = "snake_case", deny_unknown_fields)]
pub enum FrontmostApp {
    Known {
        bundle_id: Option<ApplicationId>,
        /// Developer-set bundle name, when present and within bounds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Bundle short version, when present and within bounds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        signing: FrontmostSigningIdentity,
        kind: FrontmostAppKind,
        location: FrontmostAppLocation,
        launch_instance: SnapshotDigest,
    },
    Unknown {
        reason: FrontmostUnknownReason,
    },
}

impl FrontmostApp {
    fn launch_instance(&self) -> Option<&SnapshotDigest> {
        match self {
            Self::Known { launch_instance, .. } => Some(launch_instance),
            Self::Unknown { .. } => None,
        }
    }
}

/// A normalized, retainable observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontmostObservation {
    pub schema_version: u32,
    pub transition: FrontmostTransition,
    pub observed_at: DateTime<Utc>,
    pub app: FrontmostApp,
    /// Time the session was frontmost, set on the event that ends it.
    pub dwell_ms: Option<u64>,
    /// The ended session was shorter than [`FRONTMOST_TRANSIENT_DWELL_MS`].
    pub transient: bool,
    /// Whether an NSWorkspace notification reported this transition or the
    /// tracker closed the session at a later boundary.
    pub basis: FrontmostBasis,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostBasis {
    /// Reported by an activation, deactivation, or termination notification.
    Direct,
    /// Closed by the tracker at the next activation, suspension, or observer
    /// boundary because the notification that should have ended it was not
    /// seen. The dwell ends at that boundary, never later.
    InferredClosure,
}

/// Session and power notifications that bound frontmost coverage.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostSystemEvent {
    ObserverStarted,
    ObserverStopped,
    WillSleep,
    DidWake,
    ScreenLocked,
    ScreenUnlocked,
    /// Fast user switching moved this login session to the background.
    SessionResignedActive,
    SessionBecameActive,
}

impl FrontmostSystemEvent {
    fn suspends(self) -> bool {
        matches!(
            self,
            Self::ObserverStopped
                | Self::WillSleep
                | Self::ScreenLocked
                | Self::SessionResignedActive
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontmostCoverageState {
    /// Observation started cleanly.
    Started,
    /// Coverage stopped at a known boundary; activity after it is unobserved.
    Suspended,
    /// Coverage returned after a suspension.
    Resumed,
    /// The observer restarted without a clean stop; the interval since the
    /// last observation is a gap and no session continues across it.
    Interrupted,
}

/// A coverage boundary. `gap_started_at` is when coverage was lost, set on
/// the boundary that ends a gap.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontmostCoverageBoundary {
    pub schema_version: u32,
    pub event: FrontmostSystemEvent,
    pub state: FrontmostCoverageState,
    pub observed_at: DateTime<Utc>,
    pub gap_started_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum FrontmostRecord {
    App(FrontmostObservation),
    Coverage(FrontmostCoverageBoundary),
}

/// Bundle identifiers whose identity is withheld before persistence:
/// private-context applications and user exclusions.
#[derive(Clone, Debug, Default)]
pub struct FrontmostExclusions {
    bundle_ids: BTreeSet<String>,
}

impl FrontmostExclusions {
    pub fn new<I, S>(bundle_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            bundle_ids: bundle_ids
                .into_iter()
                .map(|bundle_id| bundle_id.as_ref().to_ascii_lowercase())
                .collect(),
        }
    }

    fn excludes(&self, raw: &FrontmostRawObservation) -> bool {
        raw.bundle_identifier
            .as_deref()
            .is_some_and(|bundle_id| self.bundle_ids.contains(&bundle_id.to_ascii_lowercase()))
    }
}

/// Normalizes raw observations with a per-journal salt so launch instances
/// cannot be linked across journals or mapped back to a process ID.
pub struct FrontmostNormalizer {
    salt: [u8; 32],
}

impl FrontmostNormalizer {
    pub fn new(salt: [u8; 32]) -> Self {
        Self { salt }
    }

    pub fn normalize(&self, raw: &FrontmostRawObservation) -> FrontmostApp {
        let bundle_id = raw
            .bundle_identifier
            .as_deref()
            .map(str::to_ascii_lowercase)
            .and_then(|value| ApplicationId::try_from(value).ok());
        let signing = signing_identity(raw.signing.as_ref());
        if raw.process_id <= 0 || raw.process_started_micros <= 0 {
            return FrontmostApp::Unknown { reason: FrontmostUnknownReason::NoProcess };
        }
        // A bundle with neither a usable identifier nor a verifiable signature
        // has no identity worth retaining. An unbundled executable is still a
        // real launch instance and is kept as a command-line app.
        if raw.bundled && bundle_id.is_none() && signing == FrontmostSigningIdentity::Unknown {
            return FrontmostApp::Unknown { reason: FrontmostUnknownReason::NoIdentity };
        }
        let kind = if !raw.bundled {
            FrontmostAppKind::CommandLine
        } else {
            match raw.activation_policy {
                FrontmostActivationPolicy::Regular => FrontmostAppKind::Regular,
                FrontmostActivationPolicy::Accessory | FrontmostActivationPolicy::Prohibited => {
                    FrontmostAppKind::Helper
                }
                FrontmostActivationPolicy::Unknown => FrontmostAppKind::Unknown,
            }
        };
        let location = if raw.translocated {
            FrontmostAppLocation::Translocated
        } else {
            FrontmostAppLocation::Installed
        };
        // Name and version describe a bundle, so an unbundled executable has
        // neither even if an adapter supplied them.
        let (name, version) = if raw.bundled {
            (
                raw.bundle_name.as_deref().and_then(plain_app_name),
                raw.bundle_version.as_deref().and_then(plain_app_version),
            )
        } else {
            (None, None)
        };
        FrontmostApp::Known {
            bundle_id,
            name,
            version,
            signing,
            kind,
            location,
            launch_instance: self.launch_instance(raw.process_id, raw.process_started_micros),
        }
    }

    fn launch_instance(&self, process_id: i32, started_micros: i64) -> SnapshotDigest {
        let mut hasher = Sha256::new();
        hasher.update(LAUNCH_INSTANCE_DOMAIN);
        hasher.update(self.salt);
        hasher.update(process_id.to_le_bytes());
        hasher.update(started_micros.to_le_bytes());
        let hex = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        SnapshotDigest::try_from(format!("sha256:{hex}")).expect("sha256 digest is valid")
    }
}

/// Keep a bundle name only if it is short, printable text. Anything else is
/// dropped whole: a truncated name could still carry what made it unusual.
pub(crate) fn plain_app_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let valid = !name.is_empty()
        && name.chars().count() <= MAX_FRONTMOST_APP_NAME_CHARS
        && !name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\'))
        && !name.chars().any(is_invisible_format);
    valid.then(|| name.to_owned())
}

/// Keep a version only if it looks like one: ASCII letters, digits, and
/// `. - _ + ( )` or spaces.
pub(crate) fn plain_app_version(raw: &str) -> Option<String> {
    let version = raw.trim();
    let valid = !version.is_empty()
        && version.len() <= MAX_FRONTMOST_APP_VERSION_CHARS
        && version.bytes().any(|byte| byte.is_ascii_digit())
        && version.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-_+() ".contains(&byte));
    valid.then(|| version.to_owned())
}

/// Bidirectional overrides and zero-width characters can make a name display
/// as something else.
fn is_invisible_format(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

fn signing_identity(input: Option<&FrontmostSigningInput>) -> FrontmostSigningIdentity {
    let Some(input) = input else {
        return FrontmostSigningIdentity::Unsigned;
    };
    if !input.valid {
        return FrontmostSigningIdentity::Unknown;
    }
    if input.platform_binary {
        return FrontmostSigningIdentity::Platform;
    }
    if input.ad_hoc {
        return FrontmostSigningIdentity::AdHoc;
    }
    match input.team_identifier.as_deref() {
        Some(team)
            if team.len() == TEAM_ID_LEN
                && team.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()) =>
        {
            FrontmostSigningIdentity::Developer { team_id: team.to_owned() }
        }
        _ => FrontmostSigningIdentity::Unknown,
    }
}

/// Tracks which launch instance is frontmost and turns notifications into
/// session records that never claim coverage the observer did not have.
///
/// - A repeated activation of the frontmost instance is suppressed.
/// - The record that ends a session carries its dwell time and is marked
///   transient when shorter than [`FRONTMOST_TRANSIENT_DWELL_MS`].
/// - An activation while another session is open closes that session by
///   inference at the activation time, so a missed deactivation cannot
///   extend it.
/// - Sleep, lock, fast user switching, and observer stops close the open
///   session at the boundary and suspend coverage until the matching resume.
/// - An observer start without a clean stop is an interruption: the open
///   session is dropped without a dwell and the interval since the last
///   observation is reported as a gap.
/// - Excluded and private applications keep no identity or launch instance.
/// - A deactivation for an instance that is not frontmost carries no dwell,
///   and a backwards clock is clamped to a zero dwell.
pub struct FrontmostSessionTracker {
    normalizer: FrontmostNormalizer,
    exclusions: FrontmostExclusions,
    active: Option<ActiveSession>,
    suspended_at: Option<DateTime<Utc>>,
    observing: bool,
    last_seen: Option<DateTime<Utc>>,
}

struct ActiveSession {
    app: FrontmostApp,
    instance: SnapshotDigest,
    started: DateTime<Utc>,
}

impl FrontmostSessionTracker {
    pub fn new(normalizer: FrontmostNormalizer) -> Self {
        Self::with_exclusions(normalizer, FrontmostExclusions::default())
    }

    pub fn with_exclusions(
        normalizer: FrontmostNormalizer,
        exclusions: FrontmostExclusions,
    ) -> Self {
        Self {
            normalizer,
            exclusions,
            active: None,
            suspended_at: None,
            observing: false,
            last_seen: None,
        }
    }

    pub fn observe(&mut self, raw: &FrontmostRawObservation) -> Vec<FrontmostRecord> {
        self.last_seen = Some(raw.observed_at);
        let app = if self.exclusions.excludes(raw) {
            FrontmostApp::Unknown { reason: FrontmostUnknownReason::Excluded }
        } else {
            self.normalizer.normalize(raw)
        };
        let instance = app.launch_instance().cloned();
        let mut records = Vec::new();
        match raw.transition {
            FrontmostTransition::Activated => {
                if instance.is_some()
                    && self.active.as_ref().map(|session| &session.instance) == instance.as_ref()
                {
                    return records;
                }
                if let Some(previous) = self.active.take() {
                    records.push(closure(previous, raw.observed_at));
                }
                self.active = instance.map(|instance| ActiveSession {
                    app: app.clone(),
                    instance,
                    started: raw.observed_at,
                });
                records.push(FrontmostRecord::App(observation(
                    raw.transition,
                    raw.observed_at,
                    app,
                    None,
                    FrontmostBasis::Direct,
                )));
            }
            FrontmostTransition::Deactivated | FrontmostTransition::Terminated => {
                let dwell = match (&self.active, &instance) {
                    (Some(session), Some(instance)) if &session.instance == instance => {
                        let dwell = dwell_ms(session.started, raw.observed_at);
                        self.active = None;
                        Some(dwell)
                    }
                    _ => None,
                };
                records.push(FrontmostRecord::App(observation(
                    raw.transition,
                    raw.observed_at,
                    app,
                    dwell,
                    FrontmostBasis::Direct,
                )));
            }
        }
        records
    }

    pub fn observe_system(
        &mut self,
        event: FrontmostSystemEvent,
        observed_at: DateTime<Utc>,
    ) -> Vec<FrontmostRecord> {
        let mut records = Vec::new();
        let boundary = |state, gap_started_at| {
            FrontmostRecord::Coverage(FrontmostCoverageBoundary {
                schema_version: FRONTMOST_SCHEMA_VERSION,
                event,
                state,
                observed_at,
                gap_started_at,
            })
        };
        if event == FrontmostSystemEvent::ObserverStarted {
            if self.observing || self.active.is_some() {
                // No clean stop was seen: drop the open session without a
                // dwell rather than stretch it across the outage.
                self.active = None;
                records.push(boundary(
                    FrontmostCoverageState::Interrupted,
                    self.last_seen.or(self.suspended_at),
                ));
            } else if let Some(suspended_at) = self.suspended_at {
                records.push(boundary(FrontmostCoverageState::Resumed, Some(suspended_at)));
            } else {
                records.push(boundary(FrontmostCoverageState::Started, None));
            }
            self.observing = true;
            self.suspended_at = None;
        } else if event.suspends() {
            if let Some(previous) = self.active.take() {
                records.push(closure(previous, observed_at));
            }
            if self.suspended_at.is_none() {
                self.suspended_at = Some(observed_at);
            }
            if event == FrontmostSystemEvent::ObserverStopped {
                self.observing = false;
            }
            records.push(boundary(FrontmostCoverageState::Suspended, None));
        } else {
            records.push(boundary(FrontmostCoverageState::Resumed, self.suspended_at.take()));
        }
        self.last_seen = Some(observed_at);
        records
    }
}

fn closure(session: ActiveSession, at: DateTime<Utc>) -> FrontmostRecord {
    FrontmostRecord::App(observation(
        FrontmostTransition::Deactivated,
        at,
        session.app,
        Some(dwell_ms(session.started, at)),
        FrontmostBasis::InferredClosure,
    ))
}

fn dwell_ms(started: DateTime<Utc>, ended: DateTime<Utc>) -> u64 {
    u64::try_from(ended.signed_duration_since(started).num_milliseconds()).unwrap_or(0)
}

fn observation(
    transition: FrontmostTransition,
    observed_at: DateTime<Utc>,
    app: FrontmostApp,
    dwell_ms: Option<u64>,
    basis: FrontmostBasis,
) -> FrontmostObservation {
    FrontmostObservation {
        schema_version: FRONTMOST_SCHEMA_VERSION,
        transition,
        observed_at,
        app,
        dwell_ms,
        transient: dwell_ms.is_some_and(|dwell| dwell < FRONTMOST_TRANSIENT_DWELL_MS),
        basis,
    }
}
