#![no_main]

use std::{collections::BTreeSet, sync::OnceLock};

use chrono::{TimeZone, Utc};
use ghostrace::{BrowserEventClass, ClientHello, PairingRecord, PairingRequest, ProfileClass};
use libfuzzer_sys::fuzz_target;

const EXTENSION_ID: &str = "abcdefghijklmnopabcdefghijklmnop";

fn approved_record() -> &'static PairingRecord {
    static RECORD: OnceLock<PairingRecord> = OnceLock::new();
    RECORD.get_or_init(|| {
        let request = PairingRequest {
            browser_channel: "chrome".to_owned(),
            profile_class: ProfileClass::Default,
            extension_id: EXTENSION_ID.to_owned(),
            extension_key_digest: "a".repeat(64),
            permissions_digest: "b".repeat(64),
            event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
            retained_fields: vec!["origin".to_owned()],
            private_context_policy: "refuse_private_context".to_owned(),
        };
        PairingRecord::approve(request, Utc.timestamp_opt(0, 0).single().expect("epoch"))
            .expect("test pairing")
    })
}

fuzz_target!(|data: &[u8]| {
    let mut record = approved_record().clone();
    let now = Utc.timestamp_opt(0, 0).single().expect("epoch");
    if data.first().is_some_and(|byte| byte & 1 == 1) {
        record.revoke();
    }
    let key_digest =
        if data.get(1).is_some_and(|byte| byte & 1 == 1) { "c".repeat(64) } else { "a".repeat(64) };
    let extension_id = if data.get(2).is_some_and(|byte| byte & 1 == 1) {
        "replaced-extension".to_owned()
    } else {
        EXTENSION_ID.to_owned()
    };
    let hello = ClientHello {
        pairing_id: record.pairing_id,
        extension_id,
        extension_key_digest: key_digest,
        permissions_digest: "b".repeat(64),
        client_nonce: [data.first().copied().unwrap_or_default(); 32],
    };
    let _ = record.admit(&hello, now);
});
