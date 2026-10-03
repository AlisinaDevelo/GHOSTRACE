//! Frontmost attribution under lifecycle edge cases, through the real
//! normalizer, session tracker, and lifecycle monitor: rapid switching,
//! termination, unknown and excluded applications, lifecycle gaps, ordering,
//! and the absence of titles, documents, and process identity.

use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use ghostrace::{
    FrontmostApp, FrontmostBasis, FrontmostExclusions, FrontmostLifecycleMonitor,
    FrontmostNormalizer, FrontmostRawObservation, FrontmostRecord, FrontmostSessionSample,
    FrontmostSessionTracker, FrontmostSystemEvent, FrontmostTransition, FrontmostUnknownReason,
};
use serde_json::json;

fn at_ms(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(1_767_254_400_000 + ms).single().expect("time")
}

fn raw(transition: &str, ms: i64, bundle: Option<&str>, pid: i32) -> FrontmostRawObservation {
    serde_json::from_value(json!({
        "transition": transition,
        "observed_at": at_ms(ms),
        "bundle_identifier": bundle,
        "bundle_name": bundle.map(|id| id.rsplit('.').next().unwrap_or(id).to_owned()),
        "bundle_version": "1.0",
        "bundled": true,
        "activation_policy": "regular",
        "translocated": false,
        "signing": {"valid": true, "ad_hoc": false, "platform_binary": false, "team_identifier": "ABCDE12345"},
        "process_id": pid,
        "process_started_micros": 1_767_254_400_000_000_i64 + i64::from(pid),
    }))
    .expect("raw observation")
}

fn tracker() -> FrontmostSessionTracker {
    let mut tracker = FrontmostSessionTracker::with_exclusions(
        FrontmostNormalizer::new([9; 32]),
        FrontmostExclusions::new(["com.example.vault"]),
    );
    tracker.observe_system(FrontmostSystemEvent::ObserverStarted, at_ms(0));
    tracker
}

fn apps(records: &[FrontmostRecord]) -> Vec<&ghostrace::FrontmostObservation> {
    records
        .iter()
        .filter_map(|record| match record {
            FrontmostRecord::App(observation) => Some(observation),
            FrontmostRecord::Coverage(_) => None,
        })
        .collect()
}

#[test]
fn rapid_switching_is_ordered_bounded_and_dwell_never_exceeds_wall_time() {
    let mut tracker = tracker();
    let mut records = Vec::new();
    // 1,000 switches between three applications, 20-120 ms apart.
    let bundles = ["com.example.editor", "com.example.browser", "com.example.terminal"];
    let mut ms = 0_i64;
    for index in 0..1_000_usize {
        ms += 20 + (index as i64 * 37) % 100;
        let bundle = bundles[index % 3];
        records.extend(tracker.observe(&raw(
            "activated",
            ms,
            Some(bundle),
            100 + (index % 3) as i32,
        )));
    }
    records.extend(tracker.observe_system(FrontmostSystemEvent::ObserverStopped, at_ms(ms + 10)));
    let observations = apps(&records);

    // One activation per switch, and one closure per activation.
    let activations =
        observations.iter().filter(|o| o.transition == FrontmostTransition::Activated).count();
    let closures =
        observations.iter().filter(|o| o.basis == FrontmostBasis::InferredClosure).count();
    assert_eq!(activations, 1_000);
    assert_eq!(closures, 1_000);
    // Records come out in time order, and total dwell fits in the window.
    assert!(observations.windows(2).all(|pair| pair[0].observed_at <= pair[1].observed_at));
    let dwell: u64 = observations.iter().filter_map(|o| o.dwell_ms).sum();
    assert!(dwell <= (ms + 10) as u64, "dwell {dwell} exceeds the {ms} ms window");
    // Sessions under the transient bound are marked, longer ones are not.
    assert!(observations.iter().any(|o| o.transient));
    assert!(observations
        .iter()
        .filter(|o| o.dwell_ms.is_some_and(|d| d >= 500))
        .all(|o| !o.transient));
}

#[test]
fn replaying_the_same_inputs_gives_identical_records() {
    let run = || {
        let mut tracker = tracker();
        let mut out = Vec::new();
        for (index, ms) in [10, 15, 300, 305, 900].into_iter().enumerate() {
            let bundle = if index % 2 == 0 { "com.example.editor" } else { "com.example.browser" };
            out.extend(tracker.observe(&raw(
                "activated",
                ms,
                Some(bundle),
                200 + (index % 2) as i32,
            )));
        }
        serde_json::to_string(&out).expect("json")
    };
    assert_eq!(run(), run());
}

#[test]
fn a_repeated_activation_of_the_same_instance_is_one_session() {
    let mut tracker = tracker();
    let first = tracker.observe(&raw("activated", 100, Some("com.example.editor"), 300));
    let again = tracker.observe(&raw("activated", 200, Some("com.example.editor"), 300));
    assert_eq!(apps(&first).len(), 1);
    assert!(again.is_empty(), "a repeat must not split the session");
    // A relaunch is a new instance and a new session.
    let relaunched = tracker.observe(&raw("activated", 300, Some("com.example.editor"), 301));
    assert_eq!(apps(&relaunched).len(), 2);
}

#[test]
fn termination_ends_the_session_directly_and_a_stale_termination_carries_no_dwell() {
    let mut tracker = tracker();
    tracker.observe(&raw("activated", 0, Some("com.example.editor"), 400));
    let ended = tracker.observe(&raw("terminated", 1_500, Some("com.example.editor"), 400));
    let ended = apps(&ended);
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].transition, FrontmostTransition::Terminated);
    assert_eq!(ended[0].basis, FrontmostBasis::Direct);
    assert_eq!(ended[0].dwell_ms, Some(1_500));
    // Terminating an application that was not frontmost claims no time.
    let stale = tracker.observe(&raw("terminated", 1_600, Some("com.example.browser"), 401));
    assert_eq!(apps(&stale)[0].dwell_ms, None);
}

#[test]
fn unknown_and_excluded_applications_keep_no_identity() {
    let mut tracker = tracker();
    let excluded = tracker.observe(&raw("activated", 10, Some("com.example.vault"), 500));
    let no_process = tracker.observe(&raw("activated", 20, Some("com.example.editor"), 0));
    // No bundle ID and a signature that fails validation: nothing to identify.
    let mut anonymous = raw("activated", 30, None, 502);
    anonymous.signing.as_mut().expect("signing").valid = false;
    anonymous.bundle_name = None;
    let anonymous = tracker.observe(&anonymous);
    let reasons = [&excluded, &no_process, &anonymous]
        .iter()
        .map(|records| match &apps(records)[0].app {
            FrontmostApp::Unknown { reason } => *reason,
            known => panic!("identity kept: {known:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reasons,
        [
            FrontmostUnknownReason::Excluded,
            FrontmostUnknownReason::NoProcess,
            FrontmostUnknownReason::NoIdentity
        ]
    );
    let json = serde_json::to_string(&[excluded, no_process, anonymous]).expect("json");
    assert!(!json.contains("vault") && !json.contains("\"name\""), "{json}");
}

#[test]
fn lifecycle_gaps_close_sessions_at_the_boundary_and_reopen_only_on_resume() {
    let mut tracker = tracker();
    let mut monitor = FrontmostLifecycleMonitor::new();
    let sample = |wall: i64, uptime_ms: u64, locked: bool| FrontmostSessionSample {
        wall: at_ms(wall),
        uptime: Duration::from_millis(uptime_ms),
        on_console: Some(true),
        screen_locked: Some(locked),
        login_window_front: false,
    };
    let mut records = Vec::new();
    monitor.sample(sample(0, 0, false));
    records.extend(tracker.observe(&raw("activated", 0, Some("com.example.editor"), 600)));
    // Locked for a minute, then the Mac sleeps for an hour while locked.
    for (event, when) in monitor.sample(sample(5_000, 5_000, true)).0 {
        records.extend(tracker.observe_system(event, when));
    }
    for (event, when) in monitor.sample(sample(3_665_000, 5_250, true)).0 {
        records.extend(tracker.observe_system(event, when));
    }
    for (event, when) in monitor.sample(sample(3_666_000, 6_250, false)).0 {
        records.extend(tracker.observe_system(event, when));
    }
    let closure = apps(&records)
        .into_iter()
        .find(|o| o.basis == FrontmostBasis::InferredClosure)
        .expect("closed");
    assert_eq!(closure.dwell_ms, Some(5_000), "the editor's time ends at the lock");
    let resumed = records.iter().filter_map(|record| match record {
        FrontmostRecord::Coverage(boundary) if boundary.gap_started_at.is_some() => Some(boundary),
        _ => None,
    });
    let resumed = resumed.collect::<Vec<_>>();
    assert_eq!(resumed.len(), 1, "one gap from the lock to the unlock: {records:?}");
    assert_eq!(resumed[0].gap_started_at, Some(at_ms(5_000)));
    assert_eq!(resumed[0].event, FrontmostSystemEvent::ScreenUnlocked);
}

#[test]
fn records_carry_no_titles_documents_urls_or_process_identity() {
    let mut tracker = tracker();
    let mut records = tracker.observe(&raw("activated", 0, Some("com.example.editor"), 4242));
    records.extend(tracker.observe(&raw("activated", 700, Some("com.example.browser"), 4243)));
    let json = serde_json::to_string(&records).expect("json");
    for absent in ["4242", "4243", "title", "document", "url", "path", "accessibility", "window"] {
        assert!(!json.to_lowercase().contains(absent), "{absent} appears in {json}");
    }
    // Hostile fields are refused before normalization.
    for field in ["window_title", "document_path", "url", "ax_value"] {
        let mut input = json!({
            "transition": "activated", "observed_at": at_ms(0), "bundle_identifier": "com.example.editor",
            "bundled": true, "activation_policy": "regular", "translocated": false, "signing": null,
            "process_id": 1, "process_started_micros": 1
        });
        input[field] = json!("SENTINEL-SECRET");
        let parsed = FrontmostRawObservation::parse(&input.to_string());
        assert!(parsed.is_err(), "{field} was accepted");
    }
}

#[test]
fn an_unsigned_bundle_without_an_id_keeps_only_its_classes() {
    // By the 0098 rules an unsigned bundle is still a real launch instance;
    // it is kept as unsigned with no bundle ID, never with a guessed name.
    let mut tracker = tracker();
    let mut unsigned = raw("activated", 10, None, 700);
    unsigned.signing = None;
    unsigned.bundle_name = None;
    let records = tracker.observe(&unsigned);
    match &apps(&records)[0].app {
        FrontmostApp::Known { bundle_id: None, name: None, signing, .. } => {
            assert_eq!(*signing, ghostrace::FrontmostSigningIdentity::Unsigned)
        }
        other => panic!("{other:?}"),
    }
}
