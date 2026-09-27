//! Native-messaging framing and protocol: every byte is hostile.

use std::time::Duration;

use ghostrace::{
    encode_frame, parse_message, ExtensionMessage, FrameDecoder, NativeMessagingError,
    ProtocolSession, SessionEvent, MAX_NATIVE_FRAME_BYTES, MAX_NATIVE_MESSAGES_PER_WINDOW,
    NATIVE_SESSION_IDLE_TIMEOUT,
};
use sha2::{Digest, Sha256};

const SENTINEL: &str = "SENTINEL-native-host-payload";

fn hello() -> Vec<u8> {
    br#"{"type":"hello","protocol_version":1,"seq":1}"#.to_vec()
}

fn nav(seq: u64) -> Vec<u8> {
    format!(
        r#"{{"type":"navigation","seq":{seq},"url":"https://example.com/","private_context":false,"transition":"committed"}}"#
    )
    .into_bytes()
}

fn at(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

#[test]
fn frames_round_trip_across_arbitrary_chunk_boundaries() {
    let mut stream = Vec::new();
    for body in [hello(), nav(2), nav(3)] {
        stream.extend(encode_frame(&body).expect("frame"));
    }
    for chunk in [1, 2, 3, 5, 7, 64, stream.len()] {
        let mut decoder = FrameDecoder::new();
        let mut frames = Vec::new();
        for piece in stream.chunks(chunk) {
            decoder.push(piece).expect("push");
            while let Some(frame) = decoder.next_frame().expect("frame") {
                frames.push(frame);
            }
        }
        decoder.finish().expect("clean end");
        assert_eq!(frames, vec![hello(), nav(2), nav(3)], "chunk {chunk}");
    }
}

#[test]
fn oversized_empty_truncated_and_invalid_frames_fail_closed() {
    let mut decoder = FrameDecoder::new();
    let oversized = ((MAX_NATIVE_FRAME_BYTES + 1) as u32).to_ne_bytes();
    assert_eq!(decoder.push(&oversized), Err(NativeMessagingError::FrameTooLarge));
    assert_eq!(decoder.push(&u32::MAX.to_ne_bytes()), Err(NativeMessagingError::FrameTooLarge));

    let mut empty = FrameDecoder::new();
    assert_eq!(empty.push(&0u32.to_ne_bytes()), Err(NativeMessagingError::EmptyFrame));

    let mut truncated = FrameDecoder::new();
    let frame = encode_frame(&hello()).expect("frame");
    truncated.push(&frame[..frame.len() - 3]).expect("partial");
    assert_eq!(truncated.next_frame(), Ok(None));
    assert_eq!(truncated.finish(), Err(NativeMessagingError::Truncated));

    assert_eq!(parse_message(&[0xff, 0xfe, b'{']), Err(NativeMessagingError::InvalidUtf8));
    assert_eq!(parse_message(b"{\"type\":"), Err(NativeMessagingError::MalformedJson));
    assert!(encode_frame(&vec![b'x'; MAX_NATIVE_FRAME_BYTES + 1]).is_err());
}

#[test]
fn unknown_types_fields_and_hostile_structure_are_refused() {
    for body in [
        r#"{"type":"cookies","seq":2}"#.to_string(),
        r#"{"type":"heartbeat","seq":2,"page_text":"x"}"#.to_string(),
        r#"{"type":"navigation","seq":2,"url":"https://e.com","private_context":false,"transition":"prerendered"}"#.to_string(),
    ] {
        assert_eq!(parse_message(body.as_bytes()), Err(NativeMessagingError::UnknownMessage), "{body}");
    }
    let deep = format!("{}{}", "[".repeat(64), "]".repeat(64));
    assert_eq!(parse_message(deep.as_bytes()), Err(NativeMessagingError::TooDeep));
    let wide = format!("[{}]", vec!["1"; 1_000].join(","));
    assert_eq!(parse_message(wide.as_bytes()), Err(NativeMessagingError::TooManyValues));
    // Brackets inside strings do not count as nesting.
    let quoted = format!(
        r#"{{"type":"navigation","seq":2,"url":"https://e.com/{}","private_context":false,"transition":"committed"}}"#,
        "[".repeat(40)
    );
    assert!(parse_message(quoted.as_bytes()).is_ok());
}

#[test]
fn the_session_enforces_hello_version_sequence_and_shutdown() {
    let mut session = ProtocolSession::new();
    assert_eq!(session.receive(&nav(1), at(0)), Err(NativeMessagingError::HelloRequired));
    assert!(matches!(
        session.receive(&hello(), at(1)),
        Ok(SessionEvent::Accepted(ExtensionMessage::Hello { .. }))
    ));
    assert!(matches!(session.receive(&nav(2), at(2)), Ok(SessionEvent::Accepted(_))));
    assert_eq!(
        session.receive(&nav(2), at(3)),
        Err(NativeMessagingError::Replay),
        "duplicate sequence"
    );
    assert_eq!(session.receive(&nav(1), at(3)), Err(NativeMessagingError::Replay), "old sequence");
    assert!(matches!(
        session.receive(&nav(5), at(4)),
        Ok(SessionEvent::AcceptedAfterGap { missing: 2, .. })
    ));
    assert_eq!(session.receive(br#"{"type":"goodbye","seq":6}"#, at(5)), Ok(SessionEvent::Closed));
    assert_eq!(session.receive(&nav(7), at(6)), Err(NativeMessagingError::TrailingData));

    let mut downgrade = ProtocolSession::new();
    assert_eq!(
        downgrade.receive(br#"{"type":"hello","protocol_version":0,"seq":1}"#, at(0)),
        Err(NativeMessagingError::UnsupportedVersion)
    );
    assert_eq!(downgrade.receive(&hello(), at(1)), Err(NativeMessagingError::TrailingData));

    let mut renegotiate = ProtocolSession::new();
    renegotiate.receive(&hello(), at(0)).expect("hello");
    assert_eq!(
        renegotiate.receive(br#"{"type":"hello","protocol_version":1,"seq":2}"#, at(1)),
        Err(NativeMessagingError::DuplicateHello)
    );
    assert_eq!(renegotiate.receive(&nav(3), at(2)), Err(NativeMessagingError::TrailingData));
}

#[test]
fn idle_sessions_time_out_and_floods_are_rate_limited() {
    let mut idle = ProtocolSession::new();
    idle.receive(&hello(), at(0)).expect("hello");
    let late = NATIVE_SESSION_IDLE_TIMEOUT + Duration::from_secs(1);
    assert_eq!(idle.receive(&nav(2), late), Err(NativeMessagingError::Timeout));

    let mut flood = ProtocolSession::new();
    flood.receive(&hello(), at(0)).expect("hello");
    let mut limited = false;
    for seq in 2..(MAX_NATIVE_MESSAGES_PER_WINDOW as u64 + 10) {
        match flood.receive(&nav(seq), at(1)) {
            Ok(_) => {}
            Err(NativeMessagingError::RateLimited) => {
                limited = true;
                break;
            }
            Err(error) => panic!("unexpected {error}"),
        }
    }
    assert!(limited);
    // The window slides: the session recovers after it passes.
    assert!(flood.receive(&nav(1_000), at(30)).is_ok());
}

#[test]
fn deterministic_fuzz_never_panics_or_echoes_input() {
    let valid = [hello(), nav(2), br#"{"type":"heartbeat","seq":3}"#.to_vec()];
    let mut state = 0u64;
    let mut next = || {
        state += 1;
        let digest = Sha256::digest(state.to_le_bytes());
        u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
    };
    for _ in 0..20_000 {
        let base = &valid[(next() % valid.len() as u64) as usize];
        let mut bytes = base.clone();
        bytes.extend_from_slice(SENTINEL.as_bytes());
        for _ in 0..(next() % 6) {
            let index = (next() % bytes.len() as u64) as usize;
            match next() % 4 {
                0 => bytes[index] = (next() % 256) as u8,
                1 => {
                    bytes.remove(index);
                }
                2 => bytes.insert(index, b"{[\"\\,:"[(next() % 6) as usize]),
                _ => bytes.truncate(index.max(1)),
            }
            if bytes.is_empty() {
                bytes.push(b'{');
            }
        }
        let mut session = ProtocolSession::new();
        let _ = session.receive(&hello(), at(0));
        if let Err(error) = session.receive(&bytes, at(1)) {
            assert!(!error.to_string().contains("SENTINEL"));
        }
        let mut decoder = FrameDecoder::new();
        let mut framed = (bytes.len() as u32).to_ne_bytes().to_vec();
        framed.extend_from_slice(&bytes);
        framed.truncate((next() as usize % (framed.len() + 1)).max(1));
        if decoder.push(&framed).is_ok() {
            let _ = decoder.next_frame();
            let _ = decoder.finish();
        }
    }
}
