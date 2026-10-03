#![no_main]

use ghostrace::{
    CanonicalNavigation, EventSource, NavigationHostClass, NavigationRefusal, PolicyOutcome,
    PolicyProfile,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = data.get(..data.len().min(8 * 1024)).unwrap_or(data);
    let raw = String::from_utf8_lossy(input);
    let private_context = data.first().is_some_and(|byte| byte & 1 == 1);
    let result = CanonicalNavigation::from_url(&raw, private_context, Default::default());
    if private_context {
        assert_eq!(result, Err(NavigationRefusal::PrivateContext));
    }
    if let Ok(navigation) = result {
        assert!(matches!(navigation.scheme.as_str(), "http" | "https"));
        assert!(navigation.path_segment.is_none());
        if navigation.host_class == NavigationHostClass::PrivateNetwork {
            assert!(navigation.host.is_none());
        } else {
            let host = navigation.host.expect("public navigation host");
            assert!(!host.is_empty());
            assert!(!host.contains(['@', '/', '?', '#']));
        }
    }

    // Arbitrary path/query bytes cannot reintroduce sensitive fields into an
    // origin-only observation of a fixed, synthetic public site.
    let synthetic = format!(
        "https://fuzz-user:fuzz-password@example.com/fuzz-private-path/{raw}?fuzz-private-query#fuzz-private-fragment"
    );
    if let Ok(navigation) = CanonicalNavigation::from_url(&synthetic, false, Default::default()) {
        assert_eq!(navigation.origin(), "https://example.com");
        assert!(navigation.path_segment.is_none());
        let encoded = serde_json::to_string(&navigation).expect("navigation serialization");
        for prohibited in [
            "fuzz-user",
            "fuzz-password",
            "fuzz-private-path",
            "fuzz-private-query",
            "fuzz-private-fragment",
        ] {
            assert!(!encoded.contains(prohibited));
        }
    }

    let profile = PolicyProfile::deny_by_default("fuzz-policy-v1");
    let decision = profile.decide_record(EventSource::Browser, None, private_context);
    assert_eq!(decision.outcome, PolicyOutcome::Deny);
    assert!(serde_json::to_string(&decision).expect("decision serialization").len() <= 512);
});
