//! Native-messaging framing and protocol: every byte is hostile.

use std::time::Duration;

use ghostrace::{
    encode_frame, parse_message, ExtensionMessage, FrameDecoder, NativeMessagingError,
    ProtocolSession, SessionEvent, MAX_NATIVE_DECODER_BUFFER, MAX_NATIVE_FRAME_BYTES,
    MAX_NATIVE_MESSAGES_PER_WINDOW, MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES,
    MAX_NATIVE_MESSAGE_STRING_BYTES, NATIVE_SESSION_IDLE_TIMEOUT,
};
use sha2::{Digest, Sha256};

const SENTINEL: &str = "SENTINEL-native-host-payload";

const PAIRING: &str = r#""pairing_id":"00000000-0000-4000-8000-000000000001","extension_id":"abcdefghijklmnopabcdefghijklmnop","extension_key_digest":"aa","permissions_digest":"bb","client_nonce":"cc""#;

fn hello_with(version: u32, seq: u64) -> Vec<u8> {
    format!(r#"{{"type":"hello","protocol_version":{version},"seq":{seq},{PAIRING}}}"#).into_bytes()
}

fn hello() -> Vec<u8> {
    hello_with(1, 1)
}

fn nav(seq: u64) -> Vec<u8> {
    format!(
        r#"{{"type":"navigation","seq":{seq},"url":"https://example.com/","private_context":false,"transition":"committed","mac":"00"}}"#
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

    let mut split_oversized = FrameDecoder::new();
    split_oversized.push(&[0xff]).expect("partial prefix");
    assert_eq!(split_oversized.push(&[0xff, 0xff, 0xff]), Err(NativeMessagingError::FrameTooLarge));

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
        r#"{{"type":"navigation","seq":2,"url":"https://e.com/{}","private_context":false,"transition":"committed","mac":"00"}}"#,
        "[".repeat(40)
    );
    assert!(parse_message(quoted.as_bytes()).is_ok());
}

#[test]
fn parser_accepts_only_the_four_defined_v1_message_types() {
    let messages = [
        hello(),
        nav(2),
        br#"{"type":"heartbeat","seq":3,"mac":"00"}"#.to_vec(),
        br#"{"type":"goodbye","seq":4,"mac":"00"}"#.to_vec(),
    ];
    for body in messages {
        parse_message(&body).expect("defined v1 message");
    }
}

#[test]
fn fields_and_strings_are_bounded_before_typed_deserialization() {
    let oversized_url = format!(
        r#"{{"type":"navigation","seq":2,"url":"https://e.com/{}","private_context":false,"transition":"committed","mac":"00"}}"#,
        "x".repeat(MAX_NATIVE_MESSAGE_STRING_BYTES)
    );
    assert_eq!(parse_message(oversized_url.as_bytes()), Err(NativeMessagingError::StringTooLong));

    let over_budget = format!(
        r#"{{"type":"hello","protocol_version":1,"seq":1,"pairing_id":"00000000-0000-4000-8000-000000000001","extension_id":"{}","extension_key_digest":"{}","permissions_digest":"{}","client_nonce":"cc"}}"#,
        "a".repeat(MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES / 3),
        "b".repeat(MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES / 3),
        "c".repeat(MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES / 3),
    );
    assert_eq!(
        parse_message(over_budget.as_bytes()),
        Err(NativeMessagingError::StringBudgetExceeded)
    );

    let too_wide = br#"{"type":"navigation","seq":2,"url":"https://e.com/","private_context":false,"transition":"committed","mac":"00","x1":null,"x2":null,"x3":null}"#;
    assert_eq!(parse_message(too_wide), Err(NativeMessagingError::TooManyFields));

    let duplicate = br#"{"type":"heartbeat","seq":2,"mac":"00","mac":"01"}"#;
    assert_eq!(parse_message(duplicate), Err(NativeMessagingError::DuplicateField));

    let escaped_key = br#"{"ty\u0070e":"heartbeat","seq":2,"mac":"00"}"#;
    assert_eq!(parse_message(escaped_key), Err(NativeMessagingError::MalformedJson));
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
    assert_eq!(
        session.receive(br#"{"type":"goodbye","seq":6,"mac":"00"}"#, at(5)),
        Ok(SessionEvent::Closed)
    );
    assert_eq!(session.receive(&nav(7), at(6)), Err(NativeMessagingError::TrailingData));

    let mut downgrade = ProtocolSession::new();
    assert_eq!(
        downgrade.receive(&hello_with(0, 1), at(0)),
        Err(NativeMessagingError::UnsupportedVersion)
    );
    assert_eq!(downgrade.receive(&hello(), at(1)), Err(NativeMessagingError::TrailingData));

    let mut renegotiate = ProtocolSession::new();
    renegotiate.receive(&hello(), at(0)).expect("hello");
    assert_eq!(
        renegotiate.receive(&hello_with(1, 2), at(1)),
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
fn rejected_frames_consume_the_ingress_rate_budget() {
    let mut session = ProtocolSession::new_at(Duration::ZERO);
    let malformed = br#"{"type":"not-a-message"}"#;
    for _ in 0..MAX_NATIVE_MESSAGES_PER_WINDOW {
        assert_eq!(session.receive(malformed, at(0)), Err(NativeMessagingError::UnknownMessage));
    }
    assert_eq!(session.receive(&hello(), at(0)), Err(NativeMessagingError::RateLimited));
}

#[test]
fn preadmitted_frames_are_not_charged_twice() {
    let mut session = ProtocolSession::new_at(Duration::ZERO);
    session.admit_attempt(at(0)).expect("hello admission");
    session.receive_after_admission(&hello(), at(0)).expect("hello");
    for seq in 2..=MAX_NATIVE_MESSAGES_PER_WINDOW as u64 {
        session.admit_attempt(at(0)).expect("message admission");
        session.receive_after_admission(&nav(seq), at(0)).expect("message");
    }
    assert_eq!(session.receive(&nav(999), at(0)), Err(NativeMessagingError::RateLimited));
}

#[test]
fn an_initial_hello_must_arrive_before_the_idle_deadline() {
    let mut session = ProtocolSession::new_at(Duration::ZERO);
    assert_eq!(
        session.check_deadline(NATIVE_SESSION_IDLE_TIMEOUT),
        Err(NativeMessagingError::Timeout)
    );
}

#[test]
fn deterministic_fuzz_never_panics_or_echoes_input() {
    let valid = [hello(), nav(2), br#"{"type":"heartbeat","seq":3,"mac":"00"}"#.to_vec()];
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

#[test]
fn the_decoder_buffer_is_bounded_regardless_of_chunk_size() {
    let mut decoder = FrameDecoder::new();
    assert_eq!(
        decoder.push(&vec![0u8; MAX_NATIVE_DECODER_BUFFER + 1]),
        Err(NativeMessagingError::BufferFull),
        "an oversized first chunk must not be buffered"
    );
    // Many small valid frames in one chunk within the bound are fine.
    let mut stream = Vec::new();
    for seq in 2..200u64 {
        stream.extend(encode_frame(&nav(seq)).expect("frame"));
    }
    assert!(stream.len() <= MAX_NATIVE_DECODER_BUFFER);
    decoder.push(&stream).expect("valid batch");
    let mut count = 0;
    while decoder.next_frame().expect("frame").is_some() {
        count += 1;
    }
    assert_eq!(count, 198);
}
