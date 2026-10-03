#![no_main]

use ghostrace::{validate_chromium_caller_origin, validate_safari_identity, MAX_SAFARI_ID_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let suffix = data.get(..data.len().min(MAX_SAFARI_ID_BYTES)).unwrap_or(data);

    let mut chromium = b"chrome-extension://".to_vec();
    chromium.extend_from_slice(suffix);
    chromium.push(b'/');
    if let Ok(origin) = String::from_utf8(chromium) {
        let _ = validate_chromium_caller_origin(&origin);
    }

    let mut bundle = b"com.example.".to_vec();
    bundle.extend_from_slice(suffix);
    if let Ok(bundle) = String::from_utf8(bundle) {
        let _ = validate_safari_identity(&bundle, "default");
    }
});
