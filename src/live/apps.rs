//! `ghostrace live apps`: record which application is in front, for as long
//! as the user lets it run.
//!
//! Samples come from the NSWorkspace adapter, which must run on the main
//! thread. Each change of launch instance is an activation; the tracker closes
//! the previous session at that moment by inference. While the screen is
//! locked macOS reports the login window as frontmost, so that is recorded as
//! a coverage boundary, never as time spent in an application.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use rand_core::RngCore;
use serde::Serialize;
use uuid::Uuid;

use super::{op, LiveError, LiveHome};
use crate::{
    frontmost::{
        FrontmostApp, FrontmostBasis, FrontmostCoverageState, FrontmostExclusions,
        FrontmostNormalizer, FrontmostRecord, FrontmostSessionTracker, FrontmostSystemEvent,
        FrontmostTransition,
    },
    frontmost_macos::{is_main_thread, FrontmostProbe},
    model::{
        AppChange, ApplicationId, CollectorLifecyclePayload, EventEnvelope, EventKind,
        EventPayload, EventSource, Evidence, FrontmostAppChangedPayload, GapPayload,
        IngestionOrigin, InstanceLabel, ReasonCode, RootId,
    },
    policy::{PolicyDocument, PolicyProfile},
    writer::{Writer, WriterConfig, WriterOutcome},
};

const APPS_ROOT: &str = "frontmost";
const APPS_POLICY: &str = "apps";
const LOGIN_WINDOW: &str = "com.apple.loginwindow";
const POLL: Duration = Duration::from_millis(250);

/// Applications never identified, in addition to the user's own list.
pub const DEFAULT_APP_EXCLUSIONS: &[&str] = &[
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.apple.keychainaccess",
    "com.apple.passwords",
    "com.bitwarden.desktop",
];

/// What `ghostrace live apps` records, shown before it starts.
pub const APPS_CONSENT_PREVIEW: &str = "GHOSTRACE will record which application is in front \
until you stop it:\n\
  - its bundle ID, developer-set name and version, and signing class\n\
  - when it came to the front and how long it stayed\n\
Never recorded: window titles, documents, web addresses, what you type or see, or\n\
anything from password managers and the applications you exclude. While the screen\n\
is locked or the Mac sleeps, nothing is recorded and the interval is marked as a gap.\n\
No Accessibility or Screen Recording permission is used.";

/// What an apps session recorded.
#[derive(Clone, Debug, Serialize)]
pub struct AppsSummary {
    pub seconds: u64,
    pub activations: u64,
    pub withheld: u64,
    pub gaps: u64,
}

struct Recorder {
    writer: Writer,
    origin: IngestionOrigin,
    policy: PolicyProfile,
    previous: Option<ApplicationId>,
    summary: AppsSummary,
}

impl Recorder {
    fn write(
        &mut self,
        at: DateTime<Utc>,
        source: EventSource,
        kind: EventKind,
        payload: EventPayload,
        evidence: Evidence,
    ) -> Result<(), LiveError> {
        let event = EventEnvelope::new(
            &self.origin,
            Uuid::new_v4(),
            at,
            Utc::now(),
            source,
            kind,
            payload,
            None,
            self.policy.id.clone(),
            self.policy.version,
            evidence,
            None,
        )
        .map_err(op("record application"))?;
        match self
            .writer
            .submit(self.origin.clone(), vec![event], self.policy.clone(), Vec::new())
            .map_err(op("write"))?
        {
            WriterOutcome::Committed(_) => Ok(()),
            WriterOutcome::Gap(_) => {
                Err(LiveError::Operation("write", "journal refused the event".to_owned()))
            }
        }
    }

    fn lifecycle(&mut self, kind: EventKind) -> Result<(), LiveError> {
        let payload = CollectorLifecyclePayload {
            collector: EventSource::FrontmostApp,
            instance_label: InstanceLabel::try_from("ghostrace-apps").map_err(op("label"))?,
        };
        let payload = match kind {
            EventKind::CollectorStarted => EventPayload::CollectorStarted(payload),
            _ => EventPayload::CollectorStopped(payload),
        };
        self.write(Utc::now(), EventSource::Lifecycle, kind, payload, Evidence::Direct)
    }

    fn records(&mut self, records: Vec<FrontmostRecord>) -> Result<(), LiveError> {
        for record in records {
            match record {
                FrontmostRecord::App(observation) => {
                    let FrontmostApp::Known { bundle_id: Some(app_id), name, version, .. } =
                        observation.app
                    else {
                        if observation.transition == FrontmostTransition::Activated {
                            self.summary.withheld += 1;
                        }
                        continue;
                    };
                    let change = match observation.transition {
                        FrontmostTransition::Activated => AppChange::Activated,
                        FrontmostTransition::Deactivated => AppChange::Deactivated,
                        FrontmostTransition::Terminated => AppChange::Terminated,
                    };
                    let previous_app_id = if change == AppChange::Activated {
                        self.summary.activations += 1;
                        self.previous.replace(app_id.clone())
                    } else {
                        None
                    };
                    let evidence = match observation.basis {
                        FrontmostBasis::Direct => Evidence::Direct,
                        FrontmostBasis::InferredClosure => Evidence::Inferred,
                    };
                    let payload = EventPayload::FrontmostAppChanged(FrontmostAppChangedPayload {
                        app_id,
                        change,
                        previous_app_id,
                        app_name: name,
                        app_version: version,
                        dwell_ms: observation.dwell_ms,
                    });
                    self.write(
                        observation.observed_at,
                        EventSource::FrontmostApp,
                        EventKind::FrontmostAppChanged,
                        payload,
                        evidence,
                    )?;
                }
                FrontmostRecord::Coverage(boundary) => {
                    let reason = match (boundary.state, boundary.event) {
                        (FrontmostCoverageState::Interrupted, _) => {
                            "frontmost_observer_interrupted"
                        }
                        (FrontmostCoverageState::Resumed, FrontmostSystemEvent::ScreenUnlocked) => {
                            "frontmost_screen_locked"
                        }
                        (FrontmostCoverageState::Resumed, FrontmostSystemEvent::DidWake) => {
                            "frontmost_asleep"
                        }
                        (FrontmostCoverageState::Resumed, _) => "frontmost_not_observed",
                        _ => continue,
                    };
                    if boundary.gap_started_at.is_none() {
                        continue;
                    }
                    self.summary.gaps += 1;
                    // A new session starts after the gap; it has no known predecessor.
                    self.previous = None;
                    let payload = EventPayload::Gap(GapPayload {
                        source: EventSource::FrontmostApp,
                        reason_code: ReasonCode::try_from(reason).map_err(op("record gap"))?,
                        dropped_count: 0,
                        from_cursor: None,
                        to_cursor: None,
                        volume_digest: None,
                        root_ids: vec![RootId::try_from(APPS_ROOT).map_err(op("record gap"))?],
                        remediation: None,
                    });
                    self.write(
                        boundary.observed_at,
                        EventSource::FrontmostApp,
                        EventKind::Gap,
                        payload,
                        Evidence::Unknown,
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl LiveHome {
    /// The per-journal salt for launch-instance digests, created on first use.
    fn frontmost_salt(&mut self) -> Result<[u8; 32], LiveError> {
        if let Some(hex) = &self.config.frontmost_salt {
            let mut salt = [0u8; 32];
            if hex.len() == 64 {
                for (index, byte) in salt.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                        .map_err(|_| LiveError::Config)?;
                }
                return Ok(salt);
            }
            return Err(LiveError::Config);
        }
        let mut salt = [0u8; 32];
        rand_core::OsRng.try_fill_bytes(&mut salt).map_err(op("generate salt"))?;
        self.config.frontmost_salt = Some(salt.iter().map(|byte| format!("{byte:02x}")).collect());
        self.save()?;
        Ok(salt)
    }

    /// Record frontmost-application changes until `deadline` or `stop`.
    /// Must be called on the main thread.
    pub fn apps(
        &mut self,
        deadline: Option<Duration>,
        excluded: &[String],
        stop: &dyn Fn() -> bool,
    ) -> Result<AppsSummary, LiveError> {
        if !is_main_thread() {
            return Err(LiveError::Operation(
                "apps",
                "frontmost changes are only visible on the main thread".to_owned(),
            ));
        }
        let salt = self.frontmost_salt()?;
        let document = PolicyDocument::new(
            APPS_POLICY,
            1,
            [EventSource::FrontmostApp, EventSource::Lifecycle],
            [APPS_ROOT],
            false,
        )
        .map_err(op("build policy"))?;
        let policy = PolicyProfile::from_document(&document).map_err(op("build policy"))?;
        let journal = self.journal()?;
        let mut recorder = Recorder {
            writer: Writer::new(journal.clone(), WriterConfig::default())
                .map_err(op("prepare writer"))?,
            origin: IngestionOrigin::live("live-apps").map_err(op("prepare origin"))?,
            policy,
            previous: None,
            summary: AppsSummary { seconds: 0, activations: 0, withheld: 0, gaps: 0 },
        };
        let exclusions = FrontmostExclusions::new(
            DEFAULT_APP_EXCLUSIONS.iter().copied().chain(excluded.iter().map(String::as_str)),
        );
        let mut tracker =
            FrontmostSessionTracker::with_exclusions(FrontmostNormalizer::new(salt), exclusions);
        let mut probe = FrontmostProbe::new();
        let started = Instant::now();
        let mut locked = false;
        recorder.lifecycle(EventKind::CollectorStarted)?;
        recorder
            .records(tracker.observe_system(FrontmostSystemEvent::ObserverStarted, Utc::now()))?;
        while !stop() && deadline.is_none_or(|limit| started.elapsed() < limit) {
            let Some(raw) = probe.poll(POLL) else { continue };
            if raw.bundle_identifier.as_deref() == Some(LOGIN_WINDOW) {
                if !locked {
                    locked = true;
                    recorder.records(
                        tracker.observe_system(FrontmostSystemEvent::ScreenLocked, raw.observed_at),
                    )?;
                }
                continue;
            }
            if locked {
                locked = false;
                recorder.records(
                    tracker.observe_system(FrontmostSystemEvent::ScreenUnlocked, raw.observed_at),
                )?;
            }
            recorder.records(tracker.observe(&raw))?;
        }
        recorder
            .records(tracker.observe_system(FrontmostSystemEvent::ObserverStopped, Utc::now()))?;
        recorder.lifecycle(EventKind::CollectorStopped)?;
        let mut summary = recorder.summary.clone();
        drop(recorder);
        journal.shutdown().map_err(op("close journal"))?;
        summary.seconds = started.elapsed().as_secs();
        Ok(summary)
    }
}
