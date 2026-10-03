//! The native host may relay only canonical browser metadata to LocalService.

#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, thread, time::Duration};

use chrono::Utc;
use ghostrace::{
    browser_navigation_request, ingest_browser_navigation, BrowserIngestService,
    BrowserNavigationAdmission, CanonicalNavigation, DeterministicKeyProvider, EventPayload,
    EventSource, Journal, LocalService, PolicyProfile, ServiceCapability, ServiceError,
    UrlShapePolicy,
};
use tempfile::TempDir;
use uuid::Uuid;

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

#[test]
fn browser_admission_round_trips_through_local_service_and_journal_writer() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("service"), [ServiceCapability::Ingest])
            .expect("service bind");
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("browser-service-test"))
        .expect("journal");
    let handler = BrowserIngestService::new(journal.clone(), policy()).expect("handler");
    let expected_id = admission(UrlShapePolicy::OriginOnly).event_id;
    let submitted = BrowserNavigationAdmission {
        event_id: expected_id,
        ..admission(UrlShapePolicy::OriginOnly)
    };
    let socket = service.socket_path().to_path_buf();
    let instance = service.instance();
    let client = thread::spawn(move || {
        ingest_browser_navigation(&socket, instance, Uuid::new_v4(), 5_000, submitted)
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
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, admission),
        Err(ServiceError::BrowserIngestMalformed)
    );
}

#[test]
fn service_request_rejects_a_forged_noncanonical_origin_shape() {
    let mut admission = admission(UrlShapePolicy::OriginOnly);
    admission.navigation.host = Some("user:secret@example.com".to_owned());
    assert_eq!(
        browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, admission),
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
    assert!(browser_navigation_request(Uuid::new_v4(), Uuid::new_v4(), 5_000, admission).is_ok());
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
        admission(UrlShapePolicy::OriginOnly),
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
