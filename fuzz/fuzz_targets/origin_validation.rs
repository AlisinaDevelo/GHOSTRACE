#![no_main]

use ghostrace::{validate_chromium_caller_origin, validate_safari_identity, MAX_SAFARI_ID_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let suffix = data.get(..data.len().min(MAX_SAFARI_ID_BYTES)).unwrap_or(data);

    let mut chromium = b"chrome-extension://".to_vec();
    chromium.extend_from_slice(suffix);
    chromium.push(b'/');
    if let Ok(origin) = String::from_utf8(chromium) {
        let expected = suffix.len() == 32 && suffix.iter().all(|byte| (b'a'..=b'p').contains(byte));
        assert_eq!(validate_chromium_caller_origin(&origin).is_ok(), expected);
    }

    let mut bundle = b"com.example.".to_vec();
    bundle.extend_from_slice(suffix);
    if let Ok(bundle) = String::from_utf8(bundle) {
        let result = validate_safari_identity(&bundle, "default");
        if result.is_ok() {
            assert!(bundle.len() <= MAX_SAFARI_ID_BYTES);
            assert!(bundle.is_ascii());
            assert!(bundle.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label.as_bytes()[0].is_ascii_alphanumeric()
                    && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                    && label.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            }));
        }
        assert!(validate_safari_identity(&bundle, "private/context").is_err());
        assert!(validate_safari_identity(&bundle, "").is_err());
    }
});
