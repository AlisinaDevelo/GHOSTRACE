#![no_main]

use ghostrace::{parse_message, MAX_NATIVE_FRAME_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = data.get(..data.len().min(MAX_NATIVE_FRAME_BYTES)).unwrap_or(data);
    let _ = parse_message(input);
});
