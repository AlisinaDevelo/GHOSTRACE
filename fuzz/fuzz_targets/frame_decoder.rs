#![no_main]

use ghostrace::{
    encode_frame, FrameDecoder, NativeMessagingError, MAX_NATIVE_DECODER_BUFFER,
    MAX_NATIVE_FRAME_BYTES,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = FrameDecoder::new();
    let input_len =
        data.len().min(MAX_NATIVE_DECODER_BUFFER.saturating_add(MAX_NATIVE_FRAME_BYTES));
    let input = &data[..input_len];
    let chunk_size = data.first().map_or(1, |byte| (*byte as usize % 4096) + 1);
    let mut offset = 0usize;
    let mut accepted_bytes = 0usize;
    let mut decoded_frames = 0usize;
    let mut stopped = false;

    while offset < input.len() {
        let end = (offset + chunk_size).min(input.len());
        match decoder.push(&input[offset..end]) {
            Ok(()) => {
                accepted_bytes += end - offset;
                assert!(decoder.buffered_capacity() <= MAX_NATIVE_DECODER_BUFFER);
                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            assert!(!frame.is_empty());
                            assert!(frame.len() <= MAX_NATIVE_FRAME_BYTES);
                            decoded_frames += 1;
                            assert!(decoded_frames <= accepted_bytes / 5);
                        }
                        Ok(None) => break,
                        Err(_) => {
                            stopped = true;
                            break;
                        }
                    }
                }
            }
            Err(
                NativeMessagingError::BufferFull
                | NativeMessagingError::EmptyFrame
                | NativeMessagingError::FrameTooLarge,
            ) => {
                stopped = true;
            }
            Err(_) => {
                stopped = true;
            }
        }
        offset = end;
        if stopped {
            break;
        }
    }

    assert!(decoder.buffered_capacity() <= MAX_NATIVE_DECODER_BUFFER);
    if !stopped && decoder.finish().is_ok() {
        assert!(!decoder.has_buffered_data());
    }

    // Reach valid framing for every nonempty mutation as well as raw hostile
    // prefixes. The seed named `prefix` is escaped text, not a binary frame.
    let body = &data[..data.len().min(MAX_NATIVE_FRAME_BYTES)];
    if !body.is_empty() {
        let frame = encode_frame(body).expect("bounded nonempty body");
        let mut roundtrip = FrameDecoder::new();
        for chunk in frame.chunks(chunk_size) {
            roundtrip.push(chunk).expect("valid bounded prefix/body");
            assert!(roundtrip.buffered_capacity() <= MAX_NATIVE_DECODER_BUFFER);
        }
        assert_eq!(roundtrip.next_frame().expect("roundtrip frame").as_deref(), Some(body));
        assert_eq!(roundtrip.next_frame(), Ok(None));
        assert_eq!(roundtrip.finish(), Ok(()));
    }
});
