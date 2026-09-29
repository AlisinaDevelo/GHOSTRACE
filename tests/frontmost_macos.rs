//! The NSWorkspace adapter against the real window server. The adapter only
//! sees focus changes on the main thread, so this file has no test harness:
//! `main` runs each check on the main thread. The focus-switch check moves
//! focus to Finder and back, so it runs only with GHOSTRACE_FRONTMOST_TEST=1.

#[cfg(all(target_os = "macos", feature = "frontmost"))]
fn main() {
    macos::the_frontmost_application_normalizes_without_paths_or_process_ids();
    println!("ok: the frontmost application normalizes without paths or process IDs");
    macos::a_focus_switch_is_observed_with_bundle_name_version_and_signing();
}

#[cfg(not(all(target_os = "macos", feature = "frontmost")))]
fn main() {}

#[cfg(all(target_os = "macos", feature = "frontmost"))]
mod macos {
    use std::{process::Command, time::Duration};

    use ghostrace::{
        frontmost_macos::{current_frontmost, is_main_thread, FrontmostProbe},
        FrontmostApp, FrontmostNormalizer, FrontmostSigningIdentity,
    };

    pub fn the_frontmost_application_normalizes_without_paths_or_process_ids() {
        let Some(raw) = current_frontmost() else {
            eprintln!("no frontmost application in this session (headless runner)");
            return;
        };
        assert!(raw.process_id > 0);
        let app = FrontmostNormalizer::new([3; 32]).normalize(&raw);
        let json = serde_json::to_string(&app).expect("json");
        assert!(!json.contains(&format!("\"{}\"", raw.process_id)), "{json}");
        assert!(
            !json.contains("/Applications")
                && !json.contains("/System")
                && !json.contains("/Users")
        );
        if raw.process_started_micros > 0 {
            assert!(matches!(app, FrontmostApp::Known { .. }), "{json}");
        }
    }

    fn enabled() -> bool {
        std::env::var_os("GHOSTRACE_FRONTMOST_TEST").is_some_and(|value| value == "1")
    }

    fn wait_for(
        probe: &mut FrontmostProbe,
        bundle: &str,
    ) -> Option<ghostrace::FrontmostRawObservation> {
        for _ in 0..40 {
            if let Some(raw) = probe.poll(Duration::from_millis(250)) {
                if raw.bundle_identifier.as_deref() == Some(bundle) {
                    return Some(raw);
                }
            }
        }
        None
    }

    pub fn a_focus_switch_is_observed_with_bundle_name_version_and_signing() {
        if !enabled() {
            eprintln!("set GHOSTRACE_FRONTMOST_TEST=1 to switch focus to Finder and back");
            return;
        }
        assert!(is_main_thread());
        let mut probe = FrontmostProbe::new();
        let before = probe.poll(Duration::from_millis(250)).expect("a frontmost application");
        let previous = before.bundle_identifier.clone().expect("the current app has a bundle ID");
        assert_ne!(previous, "com.apple.finder", "start the test from another application");

        assert!(Command::new("/usr/bin/open")
            .args(["-a", "Finder"])
            .status()
            .expect("open")
            .success());
        let finder = wait_for(&mut probe, "com.apple.finder");
        // Give focus back whatever happened.
        let _ = Command::new("/usr/bin/open").args(["-b", &previous]).status();
        let returned = wait_for(&mut probe, &previous);

        let finder = finder.expect("Finder activation was observed");
        assert_eq!(finder.bundle_name.as_deref(), Some("Finder"));
        assert!(finder
            .bundle_version
            .as_deref()
            .is_some_and(|v| v.chars().any(|c| c.is_ascii_digit())));
        match FrontmostNormalizer::new([3; 32]).normalize(&finder) {
            FrontmostApp::Known { name, signing, .. } => {
                assert_eq!(name.as_deref(), Some("Finder"));
                assert_eq!(signing, FrontmostSigningIdentity::Platform);
            }
            other => panic!("Finder normalized to {other:?}"),
        }
        assert!(returned.is_some(), "focus returned to {previous}");
        println!("ok: a focus switch to Finder and back was observed");
    }
}
