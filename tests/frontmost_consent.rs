//! The live consent text must state exactly the lifecycle coverage the recorder
//! implements (session state and uptime-based sleep detection) and its limits.

#![cfg(all(target_os = "macos", feature = "frontmost"))]

use ghostrace::live::APPS_CONSENT_PREVIEW;

#[test]
fn live_apps_consent_states_lifecycle_coverage_and_its_limits() {
    assert!(APPS_CONSENT_PREVIEW.contains("While the screen is\nlocked, another user has the console, or the Mac sleeps, no application is observed"));
    assert!(APPS_CONSENT_PREVIEW.contains("ends the application's dwell when it began"));
    assert!(APPS_CONSENT_PREVIEW.contains("accurate to that interval"));
    assert!(APPS_CONSENT_PREVIEW.contains("a clock change forward also looks like sleep"));
    assert!(APPS_CONSENT_PREVIEW.contains("No Accessibility or Screen Recording permission"));
    assert!(!APPS_CONSENT_PREVIEW.contains("not yet integrated"));
}
