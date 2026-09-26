//! Frontmost-application identity and session semantics.
//!
//! This is the normalization boundary for a future NSWorkspace activation
//! adapter. The adapter reads only the bounded facts in
//! [`FrontmostRawObservation`] from `NSRunningApplication` and the code-signing
//! API; that type has no field for a window title, document name, URL,
//! accessibility data, menu state, or screen content, and strict
//! deserialization rejects any such field. Normalization keeps a lowercase
//! bundle identifier, a signing-identity class, an application kind and
//! location class, and a salted launch-instance digest in place of the process
//! ID and start time. Activation is contextual evidence only: it never proves
//! that the application caused a filesystem change.

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
    /// The process identity was invalid, so no launch instance exists.
    NoProcess,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "identity", rename_all = "snake_case", deny_unknown_fields)]
pub enum FrontmostApp {
    Known {
        bundle_id: Option<ApplicationId>,
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
        FrontmostApp::Known {
            bundle_id,
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

/// Tracks which launch instance is frontmost and turns raw notifications
/// into session-level observations.
///
/// - A repeated activation of the already-frontmost instance is suppressed.
/// - The event that ends a session carries its dwell time and is marked
///   transient when shorter than [`FRONTMOST_TRANSIENT_DWELL_MS`].
/// - A deactivation or termination for an instance that is not frontmost
///   carries no dwell time rather than an invented one.
/// - Observations earlier than the current session start are clamped to a
///   zero dwell instead of producing a negative duration.
pub struct FrontmostSessionTracker {
    normalizer: FrontmostNormalizer,
    active: Option<(SnapshotDigest, DateTime<Utc>)>,
}

impl FrontmostSessionTracker {
    pub fn new(normalizer: FrontmostNormalizer) -> Self {
        Self { normalizer, active: None }
    }

    pub fn observe(&mut self, raw: &FrontmostRawObservation) -> Option<FrontmostObservation> {
        let app = self.normalizer.normalize(raw);
        let instance = app.launch_instance().cloned();
        match raw.transition {
            FrontmostTransition::Activated => {
                if instance.is_some()
                    && self.active.as_ref().map(|(active, _)| active) == instance.as_ref()
                {
                    return None;
                }
                self.active = instance.map(|instance| (instance, raw.observed_at));
                Some(observation(raw, app, None))
            }
            FrontmostTransition::Deactivated | FrontmostTransition::Terminated => {
                let dwell = match (&self.active, &instance) {
                    (Some((active, started)), Some(instance)) if active == instance => {
                        let millis =
                            raw.observed_at.signed_duration_since(*started).num_milliseconds();
                        self.active = None;
                        Some(u64::try_from(millis).unwrap_or(0))
                    }
                    _ => None,
                };
                Some(observation(raw, app, dwell))
            }
        }
    }
}

fn observation(
    raw: &FrontmostRawObservation,
    app: FrontmostApp,
    dwell_ms: Option<u64>,
) -> FrontmostObservation {
    FrontmostObservation {
        schema_version: FRONTMOST_SCHEMA_VERSION,
        transition: raw.transition,
        observed_at: raw.observed_at,
        app,
        dwell_ms,
        transient: dwell_ms.is_some_and(|dwell| dwell < FRONTMOST_TRANSIENT_DWELL_MS),
    }
}
