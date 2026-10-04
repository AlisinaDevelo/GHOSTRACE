#![no_main]

use std::{collections::BTreeSet, sync::OnceLock};

use chrono::{TimeZone, Utc};
use ghostrace::{
    BrowserEventClass, ClientHello, PairedSession, PairingError, PairingRecord, PairingRequest,
    ProfileClass,
};
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
    let flags = data.first().copied().unwrap_or_default();
    let revoked = flags & 1 != 0;
    let replaced_key = flags & 2 != 0;
    let replaced_extension = flags & 4 != 0;
    let replaced_permissions = flags & 8 != 0;
    let copied_pairing = flags & 16 != 0;
    let expired = flags & 32 != 0;
    let now = if expired { record.expires_at } else { record.approved_at };
    if revoked {
        record.revoke();
    }
    let key_digest = if replaced_key { "c".repeat(64) } else { "a".repeat(64) };
    let extension_id =
        if replaced_extension { "replaced-extension".to_owned() } else { EXTENSION_ID.to_owned() };
    let hello = ClientHello {
        pairing_id: if copied_pairing { uuid::Uuid::nil() } else { record.pairing_id },
        extension_id,
        extension_key_digest: key_digest,
        permissions_digest: if replaced_permissions { "d".repeat(64) } else { "b".repeat(64) },
        client_nonce: [data.get(1).copied().unwrap_or_default(); 32],
    };
    // Check the security decision, not merely whether admission panics.
    let expected = if copied_pairing || replaced_extension {
        Err(PairingError::NotPaired)
    } else if revoked {
        Err(PairingError::Revoked)
    } else if expired || replaced_key || replaced_permissions {
        Err(PairingError::RePairingRequired)
    } else {
        Ok(())
    };
    assert_eq!(record.admit(&hello, now), expected);

    // Replay from another session, sequence, or body must not authenticate.
    let host_nonce = [data.get(2).copied().unwrap_or_default(); 32];
    let session = PairedSession::derive(&[0x42; 32], &host_nonce, &hello.client_nonce);
    let body = &data[..data.len().min(4096)];
    let mac = session.mac(2, body);
    assert_eq!(session.verify(2, body, &mac), Ok(()));
    assert_eq!(session.verify(3, body, &mac), Err(PairingError::BadMac));
    let mut changed_body = body.to_vec();
    changed_body.push(0);
    assert_eq!(session.verify(2, &changed_body, &mac), Err(PairingError::BadMac));
    let mut next_host_nonce = host_nonce;
    next_host_nonce[0] ^= 1;
    let next_session = PairedSession::derive(&[0x42; 32], &next_host_nonce, &hello.client_nonce);
    assert_eq!(next_session.verify(2, body, &mac), Err(PairingError::BadMac));
});
