//! End to end: pairing, key agreement, MAC verification, protocol rules, and
//! URL minimization in the native host session.

use std::{collections::BTreeSet, time::Duration};

use chrono::Utc;
use ghostrace::{
    encode_hex, BrowserEventClass, ExtensionMessage, HostMessage, HostOutput, NativeHostSession,
    NativeSessionError, NavigationRefusal, NavigationTransition, PairedSession, PairingError,
    PairingRecord, PairingRequest, PathSegmentClass, ProfileClass, UrlShapePolicy,
};

const EXTENSION: &str = "abcdefghijklmnopabcdefghijklmnop";
const CLIENT_NONCE: [u8; 32] = [9; 32];

fn record() -> PairingRecord {
    PairingRecord::approve(
        PairingRequest {
            browser_channel: "chrome".to_owned(),
            profile_class: ProfileClass::Default,
            extension_id: EXTENSION.to_owned(),
            extension_key_digest: "a".repeat(64),
            permissions_digest: "b".repeat(64),
            event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
            retained_fields: vec!["origin".to_owned()],
            private_context_policy: "refuse_private_context".to_owned(),
        },
        Utc::now(),
    )
    .expect("approve")
}

fn hello(record: &PairingRecord, key_digest: &str) -> Vec<u8> {
    serde_json::to_vec(&ExtensionMessage::Hello {
        protocol_version: 1,
        seq: 1,
        pairing_id: record.pairing_id,
        extension_id: EXTENSION.to_owned(),
        extension_key_digest: key_digest.to_owned(),
        permissions_digest: "b".repeat(64),
        client_nonce: encode_hex(&CLIENT_NONCE),
    })
    .expect("hello")
}

/// The extension's side of the session, derived from the welcome.
struct Extension(PairedSession);

impl Extension {
    fn sign(&self, message: ExtensionMessage) -> Vec<u8> {
        let input = message.mac_input().expect("mac input");
        let mac = encode_hex(&self.0.mac(message.seq(), &input));
        let mut value = serde_json::to_value(&message).expect("json");
        value["mac"] = serde_json::Value::from(mac);
        serde_json::to_vec(&value).expect("bytes")
    }
}

fn nav_message(seq: u64, url: &str, private_context: bool) -> ExtensionMessage {
    ExtensionMessage::Navigation {
        seq,
        url: url.to_owned(),
        private_context,
        transition: NavigationTransition::Committed,
        mac: String::new(),
    }
}

fn open(record: &PairingRecord) -> (NativeHostSession, Extension) {
    open_with_policy(record, UrlShapePolicy::OriginOnly)
}

fn open_with_policy(
    record: &PairingRecord,
    policy: UrlShapePolicy,
) -> (NativeHostSession, Extension) {
    let mut host = NativeHostSession::new(policy);
    let reply = host
        .receive(&hello(record, &"a".repeat(64)), Duration::ZERO, Utc::now(), |_| {
            Some(record.clone())
        })
        .expect("hello admitted");
    let HostOutput::Reply(HostMessage::Welcome { protocol_version, host_nonce }) = reply else {
        panic!("expected welcome");
    };
    assert_eq!(protocol_version, 1);
    let mut nonce = [0u8; 32];
    for (index, byte) in nonce.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&host_nonce[index * 2..index * 2 + 2], 16).expect("hex");
    }
    let extension =
        Extension(PairedSession::derive(&record.secret_for_extension(), &nonce, &CLIENT_NONCE));
    (host, extension)
}

fn at(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

#[test]
fn an_authenticated_navigation_is_reduced_to_its_origin() {
    let record = record();
    let (mut host, extension) = open(&record);
    let output = host
        .receive(
            &extension.sign(nav_message(2, "https://user:pw@example.com/reset/token?q=1#f", false)),
            at(1),
            Utc::now(),
            |_| None,
        )
        .expect("navigation");
    let HostOutput::Navigation { navigation, transition, missing } = output else {
        panic!("expected navigation");
    };
    assert_eq!(navigation.origin(), "https://example.com");
    assert_eq!(transition, NavigationTransition::Committed);
    assert_eq!(missing, 0);

    let gap = host
        .receive(
            &extension.sign(nav_message(5, "https://example.org/", false)),
            at(2),
            Utc::now(),
            |_| None,
        )
        .expect("gap");
    assert!(matches!(gap, HostOutput::Navigation { missing: 2, .. }));

    let private = host
        .receive(
            &extension.sign(nav_message(6, "https://example.com/", true)),
            at(3),
            Utc::now(),
            |_| None,
        )
        .expect("private");
    assert_eq!(
        private,
        HostOutput::NavigationRefused { refusal: NavigationRefusal::PrivateContext, missing: 0 }
    );

    let goodbye = extension.sign(ExtensionMessage::Goodbye { seq: 7, mac: String::new() });
    assert_eq!(
        host.receive(&goodbye, at(4), Utc::now(), |_| None).expect("goodbye"),
        HostOutput::Closed
    );
}

#[test]
fn authenticated_path_policy_preserves_the_origin_digest_boundary() {
    let mut request = record().request;
    request.retained_fields.push("first_path_segment".to_owned());
    let record = PairingRecord::approve(request, Utc::now()).expect("approve path policy");
    let (mut host, extension) = open_with_policy(&record, UrlShapePolicy::FirstPathSegment);
    let mut segments = Vec::new();
    for (index, raw) in [
        "https://user:SENTINELPASS@example.com/Zt7xQ?SENTINELQUERY#SENTINELFRAG",
        "http://example.com/Zt7xQ",
    ]
    .iter()
    .enumerate()
    {
        let HostOutput::Navigation { navigation, missing, .. } = host
            .receive(
                &extension.sign(nav_message(index as u64 + 2, raw, false)),
                at(index as u64 + 1),
                Utc::now(),
                |_| None,
            )
            .expect("authenticated navigation")
        else {
            panic!("navigation was not admitted");
        };
        assert_eq!(missing, 0);
        assert!(!serde_json::to_string(&navigation).expect("JSON").contains("SENTINEL"));
        segments.push(navigation.path_segment.expect("path class"));
    }
    assert_eq!(
        segments[0],
        PathSegmentClass::Opaque {
            digest: "sha256:7101d302ea56404605c4f874aa18d234fa50a1bd13255fdad1253365b13411b1"
                .into(),
        }
    );
    assert_ne!(segments[0], segments[1]);
}

#[test]
fn tampered_forged_and_cross_session_messages_are_unauthenticated() {
    let record = record();
    let (mut host, extension) = open(&record);
    let signed = extension.sign(nav_message(2, "https://example.com/", false));
    let tampered =
        String::from_utf8(signed.clone()).expect("utf8").replace("example.com", "evil.example");
    assert_eq!(
        host.receive(tampered.as_bytes(), at(1), Utc::now(), |_| None).err(),
        Some(NativeSessionError::Unauthenticated)
    );
    let forged = serde_json::to_vec(&ExtensionMessage::Heartbeat { seq: 2, mac: "0".repeat(64) })
        .expect("json");
    assert_eq!(
        host.receive(&forged, at(1), Utc::now(), |_| None).err(),
        Some(NativeSessionError::Unauthenticated)
    );

    // A transcript signed in another session never verifies here.
    let (_, other) = open(&record);
    let replayed = other.sign(nav_message(2, "https://example.com/", false));
    assert_eq!(
        host.receive(&replayed, at(1), Utc::now(), |_| None).err(),
        Some(NativeSessionError::Unauthenticated)
    );

    // Authentication happens before sequence handling: the genuine message
    // is still accepted after the rejected ones.
    assert!(host.receive(&signed, at(2), Utc::now(), |_| None).is_ok());
    assert!(
        matches!(
            host.receive(&signed, at(3), Utc::now(), |_| None).err(),
            Some(NativeSessionError::Protocol(_))
        ),
        "an exact replay within the session is a sequence replay"
    );
}

#[test]
fn pairing_failures_refuse_the_session_with_fixed_codes() {
    let record = record();
    let mut host = NativeHostSession::new(UrlShapePolicy::OriginOnly);
    let unknown =
        host.receive(&hello(&record, &"a".repeat(64)), at(0), Utc::now(), |_| None).unwrap_err();
    assert_eq!(unknown, NativeSessionError::Pairing(PairingError::NotPaired));
    assert_eq!(unknown.code(), "not_paired");

    let mut host = NativeHostSession::new(UrlShapePolicy::OriginOnly);
    let replaced = host
        .receive(&hello(&record, &"c".repeat(64)), at(0), Utc::now(), |_| Some(record.clone()))
        .unwrap_err();
    assert_eq!(replaced.code(), "re_pairing_required");

    let mut revoked = record.clone();
    revoked.revoke();
    let mut host = NativeHostSession::new(UrlShapePolicy::OriginOnly);
    let error = host
        .receive(&hello(&record, &"a".repeat(64)), at(0), Utc::now(), |_| Some(revoked.clone()))
        .unwrap_err();
    assert_eq!(error.code(), "revoked");

    let mut host = NativeHostSession::new(UrlShapePolicy::OriginOnly);
    let early = serde_json::to_vec(&ExtensionMessage::Heartbeat { seq: 2, mac: "0".repeat(64) })
        .expect("json");
    assert!(host.receive(&early, at(0), Utc::now(), |_| Some(record.clone())).is_err());
}
