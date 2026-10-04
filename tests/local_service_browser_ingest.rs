//! The native host may relay only canonical browser metadata to LocalService.

#![cfg(unix)]

use std::{collections::BTreeSet, fs, os::unix::fs::PermissionsExt, thread, time::Duration};

use chrono::Utc;
use ghostrace::local_service::{BrowserNavigationRelay, BrowserRelayProof};
use ghostrace::{
    browser_navigation_from_request, browser_navigation_request, browser_relay_proof_mac,
    delivery_event_id, ingest_browser_navigation, BrowserEventClass, BrowserIngestService,
    BrowserNavigationAdmission, CanonicalNavigation, DeterministicKeyProvider, EventPayload,
    EventSource, Journal, LocalService, PairingRequest, PairingStore, PolicyProfile, ProfileClass,
    ServiceCapability, ServiceError, ServiceHandler, ServiceRequest, UrlShapePolicy,
    BROWSER_NAVIGATION_INGEST_METHOD, LOCAL_SERVICE_PROTOCOL_VERSION,
};
use tempfile::TempDir;
use uuid::Uuid;

const EXTENSION_ID: &str = "abcdefghijklmnopabcdefghijklmnop";
const EXTENSION_KEY_DIGEST: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PERMISSIONS_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CLIENT_NONCE: [u8; 32] = [7; 32];

fn private_parent() -> TempDir {
    let directory = tempfile::tempdir().expect("tempdir");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).expect("chmod");
    directory
}

fn policy() -> PolicyProfile {
    let mut policy = PolicyProfile::deny_by_default("browser-pairing-v1");
    policy.enable_source(EventSource::Browser);
    policy
}

fn admission(policy: UrlShapePolicy) -> BrowserNavigationAdmission {
    BrowserNavigationAdmission {
        event_id: Uuid::new_v4(),
        browser: "chrome".try_into().expect("browser name"),
        navigation: CanonicalNavigation::from_url(
            "https://user:secret@example.com/account?token=secret#secret",
            false,
            policy,
        )
        .expect("canonical navigation"),
        observed_at: Utc::now(),
        missing: 0,
    }
}

fn relay(admission: BrowserNavigationAdmission) -> BrowserNavigationRelay {
    let pairing_id = Uuid::new_v4();
    let client_nonce = [7; 32];
    let sequence = 1;
    let mut admission = admission;
    admission.event_id = delivery_event_id(pairing_id, &client_nonce, sequence);
    BrowserNavigationRelay::new(
        admission,
        BrowserRelayProof { pairing_id, client_nonce, sequence, mac: [9; 32] },
    )
}

fn pairing_request() -> PairingRequest {
    PairingRequest {
        browser_channel: "chrome".to_owned(),
        profile_class: ProfileClass::Default,
        extension_id: EXTENSION_ID.to_owned(),
        extension_key_digest: EXTENSION_KEY_DIGEST.to_owned(),
        permissions_digest: PERMISSIONS_DIGEST.to_owned(),
        event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
        retained_fields: vec!["origin".to_owned()],
        private_context_policy: "refuse_private_context".to_owned(),
    }
}

fn signed_relay(
    record: &ghostrace::PairingRecord,
    service_instance: Uuid,
    request_id: Uuid,
    mut admission: BrowserNavigationAdmission,
) -> BrowserNavigationRelay {
    let sequence = 1;
    admission.event_id = delivery_event_id(record.pairing_id, &CLIENT_NONCE, sequence);
    let mac = browser_relay_proof_mac(
        &record.secret_for_extension(),
        record.pairing_id,
        &CLIENT_NONCE,
        sequence,
        service_instance,
        request_id,
        admission.event_id,
        admission.observed_at,
        admission.browser.as_str(),
        &admission.navigation,
        admission.missing,
    )
    .expect("relay MAC");
    BrowserNavigationRelay::new(
        admission,
        BrowserRelayProof {
            pairing_id: record.pairing_id,
            client_nonce: CLIENT_NONCE,
            sequence,
            mac,
        },
    )
}

#[test]
fn browser_admission_round_trips_through_local_service_and_journal_writer() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("service"), [ServiceCapability::Ingest])
            .expect("service bind");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("browser-service-test"))
        .expect("journal");
    let pairing_directory = parent.path().join("pairings");
    let pairing_store = PairingStore::open(&pairing_directory).expect("pairing store");
    let approval = pairing_store.approve(pairing_request(), Utc::now()).expect("approval");
    let record = pairing_store
        .get(approval.view.pairing_id)
        .expect("pairing lookup")
        .expect("approved pairing");
    let instance = service.instance();
    let request_id = Uuid::new_v4();
    let submitted_admission = admission(UrlShapePolicy::OriginOnly);
    let submitted = signed_relay(&record, instance, request_id, submitted_admission);
    let expected_id = submitted.admission.event_id;
    let socket = service.socket_path().to_path_buf();
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        policy(),
        PairingStore::open(&pairing_directory).expect("service pairing store"),
    )
    .expect("handler");
    let client = thread::spawn(move || {
        ingest_browser_navigation(&socket, instance, request_id, 5_000, submitted)
    });
    service.serve_one(&handler).expect("serve browser admission");
    let acknowledgement = client.join().expect("client").expect("acknowledgement");
    assert_eq!(acknowledgement.event_id, expected_id);
    assert_eq!(acknowledgement.missing, 0);
    assert_eq!(acknowledgement.ingest_sequences.len(), 1);

    let events = journal.events().expect("journal events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.source, EventSource::Browser);
    let EventPayload::BrowserNavigation(payload) = &events[0].event.payload else {
        panic!("expected browser navigation")
    };
    assert_eq!(payload.url.as_str(), "https://example.com/");
    let encoded = serde_json::to_string(&events[0].event).expect("event JSON");
    assert!(!encoded.contains("secret"));
}

#[test]
fn service_request_rejects_path_retention_instead_of_silently_dropping_it() {
    let admission = admission(UrlShapePolicy::FirstPathSegment);
    assert!(admission.navigation.path_segment.is_some());
    assert_eq!(
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, relay(admission),),
        Err(ServiceError::BrowserIngestMalformed)
    );
}

#[test]
fn service_request_rejects_a_forged_noncanonical_origin_shape() {
    let mut admission = admission(UrlShapePolicy::OriginOnly);
    admission.navigation.host = Some("user:secret@example.com".to_owned());
    assert_eq!(
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, relay(admission)),
        Err(ServiceError::BrowserIngestMalformed)
    );
}

#[test]
fn service_request_preserves_the_withheld_private_network_origin_class() {
    let admission = BrowserNavigationAdmission {
        event_id: Uuid::new_v4(),
        browser: "chrome".try_into().expect("browser name"),
        navigation: CanonicalNavigation::from_url(
            "https://localhost:8443/private",
            false,
            UrlShapePolicy::OriginOnly,
        )
        .expect("private network navigation"),
        observed_at: Utc::now(),
        missing: 0,
    };
    assert!(
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, relay(admission)).is_ok()
    );
}

#[test]
fn service_parser_refuses_legacy_admission_without_a_relay_proof() {
    let admission = admission(UrlShapePolicy::OriginOnly);
    let request = ServiceRequest {
        protocol_version: LOCAL_SERVICE_PROTOCOL_VERSION,
        service_instance: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        deadline_ms: 5_000,
        capability: ServiceCapability::Ingest,
        method: BROWSER_NAVIGATION_INGEST_METHOD.to_owned(),
        params: serde_json::to_value(admission).expect("admission JSON"),
    };
    assert_eq!(
        browser_navigation_from_request(&request),
        Err(ServiceError::BrowserIngestMalformed)
    );
}

#[test]
fn service_request_refuses_a_proof_for_an_unrelated_event_id() {
    let mut relay = relay(admission(UrlShapePolicy::OriginOnly));
    relay.admission.event_id = Uuid::new_v4();
    assert_eq!(
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, relay),
        Err(ServiceError::BrowserIngestMalformed)
    );
}

#[test]
fn relay_proof_debug_does_not_expose_nonce_or_mac() {
    let proof = BrowserRelayProof {
        pairing_id: Uuid::new_v4(),
        client_nonce: [7; 32],
        sequence: 1,
        mac: [9; 32],
    };
    let rendered = format!("{proof:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("client_nonce: [7"));
    assert!(!rendered.contains("mac: [9"));
}

#[test]
fn service_request_debug_does_not_expose_relay_credentials() {
    let request = browser_navigation_request(
        Uuid::new_v4(),
        Uuid::new_v4(),
        5_000,
        relay(admission(UrlShapePolicy::OriginOnly)),
    )
    .expect("request");
    let rendered = format!("{request:?}");
    assert!(rendered.contains("params: \"<redacted>\""));
    assert!(!rendered.contains("[9, 9, 9"));
}

#[test]
fn service_ingest_is_capability_denied_without_an_explicit_grant() {
    let parent = private_parent();
    let mut service = LocalService::bind(&parent.path().join("service"), []).expect("service bind");
    let socket = service.socket_path().to_path_buf();
    let instance = service.instance();
    let request = browser_navigation_request(
        instance,
        Uuid::new_v4(),
        5_000,
        relay(admission(UrlShapePolicy::OriginOnly)),
    )
    .expect("request");
    let body = serde_json::to_vec(&request).expect("request JSON");
    let client = thread::spawn(move || {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;
        let mut stream = UnixStream::connect(socket).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
        stream.write_all(&(body.len() as u32).to_be_bytes()).expect("length");
        stream.write_all(&body).expect("body");
        let mut prefix = [0; 4];
        stream.read_exact(&mut prefix).expect("response prefix");
        let mut response = vec![0; u32::from_be_bytes(prefix) as usize];
        stream.read_exact(&mut response).expect("response");
        serde_json::from_slice::<ghostrace::ServiceResponse>(&response).expect("service response")
    });
    let handler = BrowserIngestService::new(
        Journal::in_memory(DeterministicKeyProvider::from_seed("denied-browser-service"))
            .expect("journal"),
        policy(),
    )
    .expect("handler");
    service.serve_one(&handler).expect("serve refusal");
    assert_eq!(
        client.join().expect("client"),
        ghostrace::ServiceResponse::Refused { error: ServiceError::CapabilityDenied }
    );
}

#[test]
fn service_refuses_unpaired_tampered_and_revoked_proofs_before_journal_ingest() {
    let parent = private_parent();
    let store = PairingStore::open(parent.path()).expect("store");
    let approval = store.approve(pairing_request(), Utc::now()).expect("approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("proof-negative")).expect("journal");
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        policy(),
        PairingStore::open(parent.path()).expect("service store"),
    )
    .expect("handler");
    let instance = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let original =
        signed_relay(&record, instance, request_id, admission(UrlShapePolicy::OriginOnly));
    let make_request =
        |relay| browser_navigation_request(instance, request_id, 5000, relay).expect("request");
    for variant in ["bad_mac", "changed_origin", "changed_timestamp", "changed_browser"] {
        let mut relay = original.clone();
        match variant {
            "bad_mac" => relay.proof.mac[0] ^= 1,
            "changed_origin" => {
                relay.admission.navigation = CanonicalNavigation::from_url(
                    "https://other.example/",
                    false,
                    UrlShapePolicy::OriginOnly,
                )
                .expect("origin")
            }
            "changed_timestamp" => relay.admission.observed_at += chrono::Duration::seconds(1),
            "changed_browser" => relay.admission.browser = "edge".try_into().expect("browser"),
            _ => unreachable!(),
        }
        assert_eq!(
            handler.handle(&make_request(relay)),
            Err(ServiceError::BrowserIngestRefused),
            "{variant}"
        );
        assert!(journal.events().expect("events").is_empty());
    }
    let unknown = ghostrace::PairingRecord::approve(pairing_request(), Utc::now())
        .expect("not persisted approval");
    let unknown_relay =
        signed_relay(&unknown, instance, request_id, admission(UrlShapePolicy::OriginOnly));
    assert_eq!(
        handler.handle(&make_request(unknown_relay)),
        Err(ServiceError::BrowserIngestRefused)
    );
    assert!(store.revoke(record.pairing_id).expect("revoke"));
    assert_eq!(handler.handle(&make_request(original)), Err(ServiceError::BrowserIngestRefused));
    assert!(journal.events().expect("events").is_empty());
}
