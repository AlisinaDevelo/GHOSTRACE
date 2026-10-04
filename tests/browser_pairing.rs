//! Browser pairing, session keys, and replay protection.

use std::collections::BTreeSet;

use chrono::{Duration, TimeZone, Utc};
use ghostrace::{
    browser_relay_proof_mac, delivery_event_id, hmac_sha256_for_test,
    verify_browser_relay_proof_mac, BrowserEventClass, CanonicalNavigation, ClientHello,
    PairedSession, PairingError, PairingRecord, PairingRequest, ProfileClass, UrlShapePolicy,
    PAIRING_LIFETIME_DAYS,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn request() -> PairingRequest {
    PairingRequest {
        browser_channel: "chrome".to_owned(),
        profile_class: ProfileClass::Default,
        extension_id: "abcdefghijklmnopabcdefghijklmnop".to_owned(),
        extension_key_digest: "a".repeat(64),
        permissions_digest: "b".repeat(64),
        event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
        retained_fields: vec!["origin".to_owned(), "transition".to_owned()],
        private_context_policy: "refuse_private_context".to_owned(),
    }
}

fn hello(record: &PairingRecord, client_nonce: [u8; 32]) -> ClientHello {
    ClientHello {
        pairing_id: record.pairing_id,
        extension_id: record.request.extension_id.clone(),
        extension_key_digest: record.request.extension_key_digest.clone(),
        permissions_digest: record.request.permissions_digest.clone(),
        client_nonce,
    }
}

#[test]
fn hmac_matches_rfc_4231_vectors() {
    assert_eq!(
        hex(&hmac_sha256_for_test(&[0x0b; 20], b"Hi There")),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
    assert_eq!(
        hex(&hmac_sha256_for_test(b"Jefe", b"what do ya want for nothing?")),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
    // Test case 6: a key longer than the block size is hashed first.
    assert_eq!(
        hex(&hmac_sha256_for_test(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        )),
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
    );
}

#[test]
fn the_approval_shows_every_disclosed_fact_and_never_prints_the_secret() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().expect("time");
    let record = PairingRecord::approve(request(), now).expect("approve");
    assert_eq!(record.expires_at, now + Duration::days(PAIRING_LIFETIME_DAYS));
    let shown = serde_json::to_value(&record.request).expect("request JSON");
    for field in [
        "browser_channel",
        "profile_class",
        "extension_id",
        "extension_key_digest",
        "event_classes",
        "retained_fields",
        "private_context_policy",
    ] {
        assert!(shown.get(field).is_some(), "{field} is not shown");
    }
    let debug = format!("{record:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(&hex(&record.secret_for_extension())));
}

#[test]
fn both_sides_derive_the_same_key_and_macs_bind_sequence_and_body() {
    let now = Utc::now();
    let record = PairingRecord::approve(request(), now).expect("approve");
    let host = PairedSession::open(&record, &hello(&record, [7; 32]), now).expect("open");
    let extension =
        PairedSession::derive(&record.secret_for_extension(), &host.host_nonce, &[7; 32]);
    let mac = extension.mac(2, b"{\"type\":\"heartbeat\",\"seq\":2}");
    host.verify(2, b"{\"type\":\"heartbeat\",\"seq\":2}", &mac).expect("verifies");
    assert_eq!(
        host.verify(3, b"{\"type\":\"heartbeat\",\"seq\":2}", &mac),
        Err(PairingError::BadMac)
    );
    assert_eq!(
        host.verify(2, b"{\"type\":\"heartbeat\",\"seq\":9}", &mac),
        Err(PairingError::BadMac)
    );
    let mut flipped = mac;
    flipped[31] ^= 1;
    assert_eq!(
        host.verify(2, b"{\"type\":\"heartbeat\",\"seq\":2}", &flipped),
        Err(PairingError::BadMac)
    );
}

#[test]
fn a_transcript_from_an_earlier_session_never_verifies() {
    let now = Utc::now();
    let record = PairingRecord::approve(request(), now).expect("approve");
    let first = PairedSession::open(&record, &hello(&record, [1; 32]), now).expect("first");
    let recorded_mac = first.mac(2, b"body");
    // Same client nonce replayed, but the host picks a new nonce per session
    // (for example after a host restart).
    let second = PairedSession::open(&record, &hello(&record, [1; 32]), now).expect("second");
    assert_ne!(first.host_nonce, second.host_nonce);
    assert_eq!(second.verify(2, b"body", &recorded_mac), Err(PairingError::BadMac));
}

#[test]
fn copied_replaced_broadened_stale_and_revoked_pairings_are_refused() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().expect("time");
    let mut record = PairingRecord::approve(request(), now).expect("approve");
    let good = hello(&record, [3; 32]);
    record.admit(&good, now).expect("paired extension");

    let copied =
        ClientHello { extension_id: "ponmlkjihgfedcbaponmlkjihgfedcba".to_owned(), ..good.clone() };
    assert_eq!(record.admit(&copied, now), Err(PairingError::NotPaired));
    let unknown = ClientHello { pairing_id: uuid::Uuid::new_v4(), ..good.clone() };
    assert_eq!(record.admit(&unknown, now), Err(PairingError::NotPaired));

    let replaced = ClientHello { extension_key_digest: "c".repeat(64), ..good.clone() };
    assert_eq!(record.admit(&replaced, now), Err(PairingError::RePairingRequired));
    let broadened = ClientHello { permissions_digest: "d".repeat(64), ..good.clone() };
    assert_eq!(record.admit(&broadened, now), Err(PairingError::RePairingRequired));

    let stale = now + Duration::days(PAIRING_LIFETIME_DAYS);
    assert_eq!(record.admit(&good, stale), Err(PairingError::RePairingRequired));
    assert!(PairedSession::open(&record, &good, stale).is_err());

    record.revoke();
    assert_eq!(record.admit(&good, now), Err(PairingError::Revoked));
    assert!(PairedSession::open(&record, &good, now).is_err());
}

#[test]
fn each_approval_gets_an_independent_secret() {
    let now = Utc::now();
    let first = PairingRecord::approve(request(), now).expect("first");
    let second = PairingRecord::approve(request(), now).expect("second");
    assert_ne!(first.secret_for_extension(), second.secret_for_extension());
    assert_ne!(first.pairing_id, second.pairing_id);
}

#[test]
fn delivery_event_id_is_stable_only_for_the_pairing_nonce_and_sequence_tuple() {
    let now = Utc::now();
    let record = PairingRecord::approve(request(), now).expect("approve");
    let first = delivery_event_id(record.pairing_id, &[7; 32], 11);
    assert_eq!(first, delivery_event_id(record.pairing_id, &[7; 32], 11));
    assert_ne!(first, delivery_event_id(record.pairing_id, &[7; 32], 12));
    assert_ne!(first, delivery_event_id(uuid::Uuid::new_v4(), &[7; 32], 11));
    assert_ne!(first, delivery_event_id(record.pairing_id, &[8; 32], 11));
}

#[test]
fn browser_relay_proof_binds_service_request_and_canonical_admission() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().expect("time");
    let record = PairingRecord::approve(request(), now).expect("approve");
    let client_nonce = [7; 32];
    let service_instance = uuid::Uuid::new_v4();
    let request_id = uuid::Uuid::new_v4();
    let event_id = delivery_event_id(record.pairing_id, &client_nonce, 11);
    let navigation = CanonicalNavigation::from_url(
        "https://example.com/account?secret=redacted",
        false,
        UrlShapePolicy::OriginOnly,
    )
    .expect("canonical navigation");
    let mac = browser_relay_proof_mac(
        &record.secret_for_extension(),
        record.pairing_id,
        &client_nonce,
        11,
        service_instance,
        request_id,
        event_id,
        now,
        "chrome",
        &navigation,
        2,
    )
    .expect("relay MAC");
    verify_browser_relay_proof_mac(
        &record.secret_for_extension(),
        &mac,
        record.pairing_id,
        &client_nonce,
        11,
        service_instance,
        request_id,
        event_id,
        now,
        "chrome",
        &navigation,
        2,
    )
    .expect("relay MAC verifies");

    let mut changed_mac = mac;
    changed_mac[0] ^= 1;
    assert_eq!(
        verify_browser_relay_proof_mac(
            &record.secret_for_extension(),
            &changed_mac,
            record.pairing_id,
            &client_nonce,
            11,
            service_instance,
            request_id,
            event_id,
            now,
            "chrome",
            &navigation,
            2,
        ),
        Err(PairingError::BadMac)
    );
    assert_eq!(
        verify_browser_relay_proof_mac(
            &record.secret_for_extension(),
            &mac,
            record.pairing_id,
            &client_nonce,
            11,
            uuid::Uuid::new_v4(),
            request_id,
            event_id,
            now,
            "chrome",
            &navigation,
            2,
        ),
        Err(PairingError::BadMac)
    );
    assert_eq!(
        verify_browser_relay_proof_mac(
            &record.secret_for_extension(),
            &mac,
            record.pairing_id,
            &client_nonce,
            11,
            service_instance,
            request_id,
            event_id,
            now,
            "chrome",
            &navigation,
            3,
        ),
        Err(PairingError::BadMac)
    );
    let changed_navigation =
        CanonicalNavigation::from_url("https://other.example/", false, UrlShapePolicy::OriginOnly)
            .expect("changed canonical navigation");
    assert_eq!(
        verify_browser_relay_proof_mac(
            &record.secret_for_extension(),
            &mac,
            record.pairing_id,
            &client_nonce,
            11,
            service_instance,
            request_id,
            event_id,
            now,
            "chrome",
            &changed_navigation,
            2,
        ),
        Err(PairingError::BadMac)
    );
}
