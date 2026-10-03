#![no_main]

use ghostrace::{FrameDecoder, MAX_NATIVE_DECODER_BUFFER};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = FrameDecoder::new();
    let input = data.get(..data.len().min(MAX_NATIVE_DECODER_BUFFER + 1)).unwrap_or(data);
    if decoder.push(input).is_ok() {
        while decoder.next_frame().ok().flatten().is_some() {}
        let _ = decoder.finish();
    }
});
