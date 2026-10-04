#![no_main]

use ghostrace::{
    encode_chromium_stream, encode_safari_envelope, normalize_chromium_stream,
    normalize_safari_envelope, MAX_NATIVE_FRAME_BYTES,
};
use libfuzzer_sys::fuzz_target;

const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
const BUNDLE: &str = "com.example.ghostrace";

fuzz_target!(|data: &[u8]| {
    let body = data.get(..data.len().min(MAX_NATIVE_FRAME_BYTES)).unwrap_or(data);
    if body.is_empty() || std::str::from_utf8(body).is_err() {
        return;
    }
    let Ok(chromium_stream) = encode_chromium_stream(&[body]) else {
        return;
    };
    let Ok(chromium) = normalize_chromium_stream(&chromium_stream, ORIGIN) else {
        return;
    };
    let Ok(safari_envelope) = encode_safari_envelope(BUNDLE, "default", body) else {
        return;
    };
    let Ok(safari) = normalize_safari_envelope(&safari_envelope) else {
        return;
    };
    assert_eq!(chromium.len(), 1);
    assert_eq!(chromium[0].body, safari.body);
});
