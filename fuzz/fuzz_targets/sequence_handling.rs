#![no_main]

use std::time::Duration;

use ghostrace::ProtocolSession;
use libfuzzer_sys::fuzz_target;

const HELLO: &[u8] = br#"{"type":"hello","protocol_version":1,"seq":1,"pairing_id":"00000000-0000-4000-8000-000000000001","extension_id":"abcdefghijklmnopabcdefghijklmnop","extension_key_digest":"aa","permissions_digest":"bb","client_nonce":"cc"}"#;

fuzz_target!(|data: &[u8]| {
    let mut session = ProtocolSession::new_at(Duration::ZERO);
    let _ = session.receive(HELLO, Duration::ZERO);
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
        let message = format!(r#"{{"type":"heartbeat","seq":{seq},"mac":"00"}}"#);
        let now = if byte & 0x80 != 0 {
            Duration::from_secs(121)
        } else {
            Duration::from_millis(index as u64)
        };
        let _ = session.receive(message.as_bytes(), now);
        if session.check_deadline(now).is_err() {
            break;
        }
    }
});
