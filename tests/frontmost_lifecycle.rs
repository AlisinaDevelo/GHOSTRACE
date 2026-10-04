//! Session lifecycle inference for live frontmost recording: sleep from wall
//! time outrunning uptime, lock and user switching from session state.

use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use ghostrace::{
    FrontmostApp, FrontmostBasis, FrontmostLifecycleMonitor, FrontmostNormalizer,
    FrontmostRawObservation, FrontmostRecord, FrontmostSessionSample, FrontmostSessionTracker,
    FrontmostSystemEvent::{self, *},
};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_767_254_400 + seconds, 0).single().expect("time")
}

fn sample(wall: i64, uptime_ms: u64, console: bool, locked: bool) -> FrontmostSessionSample {
    FrontmostSessionSample {
        wall: at(wall),
        uptime: Duration::from_millis(uptime_ms),
        on_console: Some(console),
        screen_locked: Some(locked),
        login_window_front: false,
    }
}

fn events(
    result: &(Vec<(FrontmostSystemEvent, DateTime<Utc>)>, bool),
) -> Vec<FrontmostSystemEvent> {
    result.0.iter().map(|(event, _)| *event).collect()
}

#[test]
fn an_active_session_produces_no_boundaries() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    for second in 0..5 {
        let result = monitor.sample(sample(second, second as u64 * 1000, true, false));
        assert_eq!(events(&result), []);
        assert!(result.1);
    }
}

#[test]
fn sleep_is_inferred_when_wall_time_outruns_uptime() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(0, 0, true, false));
    let result = monitor.sample(sample(600, 250, true, false));
    assert_eq!(result.0, [(WillSleep, at(0)), (DidWake, at(600))]);
    assert!(result.1);
    // Ordinary polling jitter is not sleep.
    let result = monitor.sample(sample(601, 1_500, true, false));
    assert_eq!(events(&result), []);
}

#[test]
fn a_backward_clock_change_is_not_sleep() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(100, 0, true, false));
    assert_eq!(events(&monitor.sample(sample(40, 250, true, false))), []);
}

#[test]
fn a_locked_screen_or_login_window_suspends_until_unlocked() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(0, 0, true, false));
    let locked = monitor.sample(sample(1, 1_000, true, true));
    assert_eq!(locked.0, [(ScreenLocked, at(1))]);
    assert!(!locked.1);
    assert_eq!(events(&monitor.sample(sample(2, 2_000, true, true))), []);
    let unlocked = monitor.sample(sample(3, 3_000, true, false));
    assert_eq!(unlocked.0, [(ScreenUnlocked, at(3))]);
    assert!(unlocked.1);

    let mut login = sample(4, 4_000, true, false);
    login.login_window_front = true;
    assert_eq!(events(&monitor.sample(login)), [ScreenLocked]);
}

#[test]
fn leaving_the_console_suspends_until_the_session_returns() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(0, 0, true, false));
    let away = monitor.sample(sample(1, 1_000, false, false));
    assert_eq!(events(&away), [SessionResignedActive]);
    assert!(!away.1);
    assert_eq!(events(&monitor.sample(sample(2, 2_000, true, false))), [SessionBecameActive]);
}

#[test]
fn sleeping_while_locked_resumes_only_at_unlock() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(0, 0, true, false));
    monitor.sample(sample(1, 1_000, true, true));
    let woke_locked = monitor.sample(sample(900, 1_250, true, true));
    assert_eq!(woke_locked.0, [(WillSleep, at(1)), (ScreenLocked, at(900))]);
    assert!(!woke_locked.1);
    assert_eq!(events(&monitor.sample(sample(901, 2_250, true, false))), [ScreenUnlocked]);
}

#[test]
fn waking_into_a_lock_screen_stays_suspended() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    monitor.sample(sample(0, 0, true, false));
    let result = monitor.sample(sample(3_600, 250, true, true));
    assert_eq!(result.0, [(WillSleep, at(0)), (ScreenLocked, at(3_600))]);
    assert!(!result.1);
}

#[test]
fn an_application_session_ends_when_sleep_began_not_at_wake() {
    let mut monitor = FrontmostLifecycleMonitor::new();
    let mut tracker = FrontmostSessionTracker::new(FrontmostNormalizer::new([1; 32]));
    tracker.observe_system(ObserverStarted, at(0));
    let raw: FrontmostRawObservation = serde_json::from_value(serde_json::json!({
        "transition": "activated",
        "observed_at": at(0),
        "bundle_identifier": "com.example.editor",
        "bundled": true,
        "activation_policy": "regular",
        "translocated": false,
        "signing": null,
        "process_id": 501,
        "process_started_micros": 1_767_254_400_000_000_i64,
    }))
    .expect("raw");
    monitor.sample(sample(0, 0, true, false));
    tracker.observe(&raw);
    monitor.sample(sample(60, 60_000, true, false));
    let mut records = Vec::new();
    for (event, when) in monitor.sample(sample(4_000, 60_250, true, false)).0 {
        records.extend(tracker.observe_system(event, when));
    }
    let closure = records
        .iter()
        .find_map(|record| match record {
            FrontmostRecord::App(observation) => Some(observation),
            FrontmostRecord::Coverage(_) => None,
        })
        .expect("the editor session is closed");
    assert_eq!(closure.basis, FrontmostBasis::InferredClosure);
    assert_eq!(closure.dwell_ms, Some(60_000), "dwell stops at the last awake sample");
    assert!(matches!(closure.app, FrontmostApp::Known { .. }));
    let resumed = records.iter().any(|record| {
        matches!(record, FrontmostRecord::Coverage(boundary)
            if boundary.event == DidWake && boundary.gap_started_at == Some(at(60)))
    });
    assert!(resumed, "{records:?}");
}
