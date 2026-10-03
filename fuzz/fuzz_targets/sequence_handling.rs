#![no_main]

use std::time::Duration;

use ghostrace::{
    ExtensionMessage, NativeMessagingError, ProtocolSession, SessionEvent,
    MAX_NATIVE_MESSAGES_PER_WINDOW,
};
use libfuzzer_sys::fuzz_target;

const HELLO: &[u8] = br#"{"type":"hello","protocol_version":1,"seq":1,"pairing_id":"00000000-0000-4000-8000-000000000001","extension_id":"abcdefghijklmnopabcdefghijklmnop","extension_key_digest":"aa","permissions_digest":"bb","client_nonce":"cc"}"#;

fuzz_target!(|data: &[u8]| {
    let mut session = ProtocolSession::new_at(Duration::ZERO);
    assert!(matches!(
        session.receive(HELLO, Duration::ZERO),
        Ok(SessionEvent::Accepted(ExtensionMessage::Hello { .. }))
    ));
    let mut last_seq = 1_u64;
    for (index, byte) in data.iter().take(256).enumerate() {
        // Generate syntactically valid messages so this target reaches the
        // session's replay, gap, rate, and timeout rules instead of spending
        // every iteration in JSON syntax rejection.
        let seq = match byte & 0x03 {
            0 => index as u64 + 2,
            1 => index as u64 + 3,
            2 => index as u64 + 1,
            _ => 1,
        };
        let goodbye = byte & 0x10 != 0;
        let message_type = if goodbye { "goodbye" } else { "heartbeat" };
        let message = format!(r#"{{"type":"{message_type}","seq":{seq},"mac":"00"}}"#);
        let now = if byte & 0x80 != 0 {
            Duration::from_secs(121)
        } else {
            Duration::from_millis(index as u64)
        };
        let result = session.receive(message.as_bytes(), now);
        if byte & 0x80 != 0 {
            assert_eq!(result, Err(NativeMessagingError::Timeout));
            assert_eq!(session.check_deadline(now), Err(NativeMessagingError::TrailingData));
            break;
        }
        // Hello also consumes one attempt. All non-timeout times are within
        // a single 10-second window, even for rejected/replayed messages.
        if index + 1 >= MAX_NATIVE_MESSAGES_PER_WINDOW {
            assert_eq!(result, Err(NativeMessagingError::RateLimited));
            continue;
        }
        if seq <= last_seq {
            assert_eq!(result, Err(NativeMessagingError::Replay));
            continue;
        }
        if goodbye {
            assert_eq!(result, Ok(SessionEvent::Closed));
            assert_eq!(session.check_deadline(now), Err(NativeMessagingError::TrailingData));
            break;
        }
        let expected_message = ExtensionMessage::Heartbeat { seq, mac: "00".to_owned() };
        let missing = seq - last_seq - 1;
        let expected = if missing == 0 {
            SessionEvent::Accepted(expected_message)
        } else {
            SessionEvent::AcceptedAfterGap { message: expected_message, missing }
        };
        assert_eq!(result, Ok(expected));
        last_seq = seq;
    }
});
