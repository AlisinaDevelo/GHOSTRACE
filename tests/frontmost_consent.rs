//! The live consent text must distinguish integrated observations from pending
//! lifecycle support. Synthetic tracker coverage is not a live sleep/wake claim.

#![cfg(all(target_os = "macos", feature = "frontmost"))]

use ghostrace::live::APPS_CONSENT_PREVIEW;

#[test]
fn live_apps_consent_discloses_unintegrated_lifecycle_boundaries() {
    assert!(APPS_CONSENT_PREVIEW.contains("window pauses application observations"));
    assert!(APPS_CONSENT_PREVIEW
        .contains("Sleep/wake and complete lock-lifecycle detection\nare not yet integrated"));
    assert!(APPS_CONSENT_PREVIEW.contains("dwell can span an unobserved boundary"));
    assert!(APPS_CONSENT_PREVIEW.contains("No Accessibility or Screen Recording permission"));
    assert!(
        !APPS_CONSENT_PREVIEW.contains("nothing is recorded and the interval is marked as a gap")
    );
}
