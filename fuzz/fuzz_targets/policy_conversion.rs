#![no_main]

use ghostrace::{CanonicalNavigation, EventSource, PolicyProfile};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = data.get(..data.len().min(8 * 1024)).unwrap_or(data);
    let raw = String::from_utf8_lossy(input);
    let private_context = data.first().is_some_and(|byte| byte & 1 == 1);
    let _ = CanonicalNavigation::from_url(&raw, private_context, Default::default());

    let profile = PolicyProfile::deny_by_default("fuzz-policy-v1");
    let _ = profile.decide_record(EventSource::Browser, None, private_context);
});
