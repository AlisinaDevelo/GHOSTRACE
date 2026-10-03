#![no_main]

use ghostrace::{parse_message, MAX_NATIVE_FRAME_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = data.get(..data.len().min(MAX_NATIVE_FRAME_BYTES)).unwrap_or(data);
    match parse_message(input) {
        Ok(message) => {
            let canonical = serde_json::to_vec(&message).expect("typed message serialization");
            assert!(canonical.len() <= MAX_NATIVE_FRAME_BYTES);
            assert_eq!(parse_message(&canonical), Ok(message));
        }
        Err(error) => {
            // Refusal diagnostics are fixed codes, never attacker bytes.
            assert!(error.to_string().len() <= 128);
        }
    }
});
