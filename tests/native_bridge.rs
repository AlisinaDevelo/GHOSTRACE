use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, io::AsRawFd, net::UnixStream},
    sync::{mpsc::sync_channel, Arc, Barrier, Mutex},
    time::Duration,
};

#[cfg(target_os = "macos")]
use std::{os::fd::RawFd, path::PathBuf, time::Instant};

use chrono::{DateTime, Utc};
#[cfg(target_os = "macos")]
use ghostrace::{
    browser_navigation_request, browser_relay_proof_mac, delivery_event_id,
    ingest_browser_navigation, BrowserIngestService, BrowserNavigationRelay, BrowserRelayProof,
    CanonicalNavigation, DeterministicKeyProvider, EventKind, EventPayload, EventSource, Journal,
    LocalService, NativeHostInstaller, PolicyProfile, ServiceCapability, ServiceHandler,
    MAX_NATIVE_FRAME_BYTES, NATIVE_HOST_STORE_DIR,
};
use ghostrace::{
    encode_frame, encode_hex, read_native_service_endpoint, run_native_host_stdio,
    validate_pairing_request, write_native_service_endpoint, BridgeOutput, BrowserEventClass,
    BrowserNavigationAck, BrowserNavigationAdmission, ExtensionMessage, FrameDecoder, HostMessage,
    NativeBridge, NativeBridgeError, NativeBridgeSink, NativeHostSession, NativeServiceEndpoint,
    PairedSession, PairingRecord, PairingRequest, PairingStore, ProfileClass, UrlShapePolicy,
};
#[cfg(target_os = "macos")]
use std::process::{Child, Command, ExitStatus, Stdio};
use tempfile::TempDir;

const EXTENSION: &str = "abcdefghijklmnopabcdefghijklmnop";
const CALLER_ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
const KEY_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PERMISSIONS_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CLIENT_NONCE: [u8; 32] = [7; 32];

fn request() -> PairingRequest {
    PairingRequest {
        browser_channel: "chrome".to_owned(),
        profile_class: ProfileClass::Default,
        extension_id: EXTENSION.to_owned(),
        extension_key_digest: KEY_DIGEST.to_owned(),
        permissions_digest: PERMISSIONS_DIGEST.to_owned(),
        event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
        retained_fields: vec!["origin".to_owned()],
        private_context_policy: "refuse_private_context".to_owned(),
    }
}

fn hello_for(record: &PairingRecord, extension_id: &str, key_digest: &str) -> Vec<u8> {
    serde_json::to_vec(&ExtensionMessage::Hello {
        protocol_version: 1,
        seq: 1,
        pairing_id: record.pairing_id,
        extension_id: extension_id.to_owned(),
        extension_key_digest: key_digest.to_owned(),
        permissions_digest: PERMISSIONS_DIGEST.to_owned(),
        client_nonce: encode_hex(&CLIENT_NONCE),
    })
    .expect("hello serializes")
}

fn hello(record: &PairingRecord, key_digest: &str) -> Vec<u8> {
    hello_for(record, EXTENSION, key_digest)
}

fn navigation(seq: u64, url: &str) -> ExtensionMessage {
    ExtensionMessage::Navigation {
        seq,
        url: url.to_owned(),
        private_context: false,
        transition: ghostrace::NavigationTransition::Committed,
        mac: String::new(),
    }
}

fn signed(session: &PairedSession, message: ExtensionMessage) -> Vec<u8> {
    let mac = encode_hex(&session.mac(message.seq(), &message.mac_input().expect("MAC input")));
    let mut value = serde_json::to_value(message).expect("message serializes");
    value["mac"] = serde_json::Value::String(mac);
    serde_json::to_vec(&value).expect("signed message serializes")
}

/// Cap successful reads independently of the kernel's stream coalescing.
struct ChunkedInput {
    reader: UnixStream,
    max_read: usize,
}

impl Read for ChunkedInput {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let length = bytes.len().min(self.max_read);
        self.reader.read(&mut bytes[..length])
    }
}

impl AsRawFd for ChunkedInput {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.reader.as_raw_fd()
    }
}

fn input_stream(input: Vec<u8>) -> UnixStream {
    let (mut writer, reader) = UnixStream::pair().expect("stream pair");
    writer.write_all(&input).expect("write input");
    writer.shutdown(std::net::Shutdown::Write).expect("close input");
    reader
}

#[derive(Clone, Default)]
struct RecordingSink {
    admissions: Arc<Mutex<Vec<BrowserNavigationAdmission>>>,
}

impl NativeBridgeSink for RecordingSink {
    fn service_instance(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(1)
    }

    fn ingest_browser_navigation(
        &self,
        _request_id: uuid::Uuid,
        relay: ghostrace::BrowserNavigationRelay,
    ) -> Result<BrowserNavigationAck, NativeBridgeError> {
        let admission = relay.admission;
        let response = BrowserNavigationAck {
            event_id: admission.event_id,
            ingest_sequences: vec![1, 2],
            missing: admission.missing,
        };
        self.admissions.lock().expect("sink lock").push(admission);
        Ok(response)
    }
}

#[derive(Clone)]
struct BlockingSink {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    admissions: Arc<Mutex<Vec<BrowserNavigationAdmission>>>,
}

impl NativeBridgeSink for BlockingSink {
    fn service_instance(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(1)
    }

    fn ingest_browser_navigation(
        &self,
        _request_id: uuid::Uuid,
        relay: ghostrace::BrowserNavigationRelay,
    ) -> Result<BrowserNavigationAck, NativeBridgeError> {
        let admission = relay.admission;
        self.entered.wait();
        self.release.wait();
        let response = BrowserNavigationAck {
            event_id: admission.event_id,
            ingest_sequences: vec![1],
            missing: admission.missing,
        };
        self.admissions.lock().expect("sink lock").push(admission);
        Ok(response)
    }
}

#[derive(Clone, Copy)]
struct UncertainSink;

impl NativeBridgeSink for UncertainSink {
    fn service_instance(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(1)
    }

    fn ingest_browser_navigation(
        &self,
        _request_id: uuid::Uuid,
        _relay: ghostrace::BrowserNavigationRelay,
    ) -> Result<BrowserNavigationAck, NativeBridgeError> {
        Err(NativeBridgeError::JournalUncertain)
    }
}

fn open_bridge<S: NativeBridgeSink>(
    store: PairingStore,
    sink: S,
    record: &PairingRecord,
    wall_clock: DateTime<Utc>,
) -> (NativeBridge<S>, PairedSession) {
    let mut bridge = NativeBridge::new(store, sink, UrlShapePolicy::OriginOnly, CALLER_ORIGIN)
        .expect("bridge opens");
    let welcome = bridge
        .receive(&hello(record, KEY_DIGEST), Duration::ZERO, wall_clock)
        .expect("hello admitted");
    let BridgeOutput::Reply(HostMessage::Welcome { host_nonce, .. }) = welcome else {
        panic!("expected welcome")
    };
    let mut host_nonce_bytes = [0_u8; 32];
    for (index, byte) in host_nonce_bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&host_nonce[index * 2..index * 2 + 2], 16)
            .expect("host nonce is hex");
    }
    let session =
        PairedSession::derive(&record.secret_for_extension(), &host_nonce_bytes, &CLIENT_NONCE);
    (bridge, session)
}

#[test]
fn pairing_state_is_encrypted_and_survives_restart_without_leaking_secret() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let stored_secret = store
        .get(approval.view.pairing_id)
        .expect("lookup")
        .expect("record")
        .secret_for_extension();
    assert_eq!(approval.extension_secret, encode_hex(&stored_secret));
    let debug = format!("{approval:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(&approval.extension_secret));

    let bytes = fs::read(directory.path().join("pairings.enc")).expect("encrypted state");
    assert!(!String::from_utf8_lossy(&bytes).contains(EXTENSION));
    assert_eq!(
        fs::metadata(directory.path().join("pairing.key")).expect("key").permissions().mode()
            & 0o077,
        0
    );

    drop(store);
    let reopened = PairingStore::open(directory.path()).expect("store restarts");
    let record = reopened
        .get(approval.view.pairing_id)
        .expect("lookup after restart")
        .expect("record after restart");
    assert_eq!(record.secret_for_extension(), stored_secret);
    assert_eq!(reopened.list().expect("list").len(), 1);
}

#[test]
fn revocation_persists_and_tampered_store_is_refused() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    assert!(store.revoke(approval.view.pairing_id).expect("revoke"));
    drop(store);

    let reopened = PairingStore::open(directory.path()).expect("store restarts");
    assert!(reopened.get(approval.view.pairing_id).expect("lookup").expect("record").revoked);

    let state_path = directory.path().join("pairings.enc");
    let mut bytes = fs::read(&state_path).expect("state");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x80;
    fs::write(state_path, bytes).expect("tamper synthetic state");
    assert!(matches!(PairingStore::open(directory.path()), Err(NativeBridgeError::Storage)));
}

#[test]
fn pairing_approval_refuses_unrepresentable_retained_fields() {
    let mut pairing = request();
    pairing.retained_fields.push("first_path_segment".to_owned());
    assert_eq!(validate_pairing_request(&pairing), Err(NativeBridgeError::PairingRequestInvalid));
}

#[test]
fn service_endpoint_receipt_is_bounded_private_and_restartable() {
    let directory = TempDir::new().expect("temp directory");
    let socket = directory.path().join("service").join("ghostrace.sock");
    let instance = uuid::Uuid::new_v4();
    write_native_service_endpoint(directory.path(), socket.clone(), instance)
        .expect("write endpoint");
    let endpoint = read_native_service_endpoint(directory.path()).expect("read endpoint");
    assert_eq!(endpoint, NativeServiceEndpoint::new(socket, instance).expect("endpoint"));
    assert_eq!(
        fs::metadata(directory.path().join("native-host/service.endpoint"))
            .expect("endpoint metadata")
            .permissions()
            .mode()
            & 0o077,
        0
    );
}

#[test]
fn bridge_rejects_a_path_retention_policy_until_the_approval_schema_can_retain_it() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    assert_eq!(
        NativeBridge::new(
            store,
            RecordingSink::default(),
            UrlShapePolicy::FirstPathSegment,
            CALLER_ORIGIN,
        )
        .err(),
        Some(NativeBridgeError::PairingRequestInvalid)
    );
}

#[test]
fn revoking_an_active_pairing_stops_the_next_authenticated_frame() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, now);
    let revoker = PairingStore::open(directory.path()).expect("revoker opens");
    assert!(revoker.revoke(record.pairing_id).expect("revoke"));
    let result = bridge.receive(
        &signed(&extension, navigation(2, "https://example.com/")),
        Duration::from_secs(1),
        now,
    );
    assert_eq!(
        result,
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Pairing(
            ghostrace::PairingError::Revoked,
        )))
    );
    assert!(admissions.lock().expect("sink lock").is_empty());
}

#[test]
fn revocation_waits_for_an_inflight_admitted_sink_before_returning() {
    let directory = TempDir::new().expect("temp directory");
    let setup = PairingStore::open(directory.path()).expect("store opens");
    let approval = setup.approve(request(), Utc::now()).expect("pairing approval");
    let record = setup.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let admissions = Arc::new(Mutex::new(Vec::new()));
    let sink = BlockingSink {
        entered: entered.clone(),
        release: release.clone(),
        admissions: admissions.clone(),
    };
    // Open the revoker before the bridge acquires its shared admission lease:
    // opening a store itself validates the lock under a bounded exclusive
    // operation, while the actual revoke below is the barrier under test.
    let revoker = PairingStore::open(directory.path()).expect("revoker store");
    let (mut bridge, extension) = open_bridge(
        PairingStore::open(directory.path()).expect("bridge store"),
        sink,
        &record,
        now,
    );
    let navigation_body = signed(&extension, navigation(2, "https://example.com/"));
    let bridge_thread =
        std::thread::spawn(move || bridge.receive(&navigation_body, Duration::from_secs(1), now));

    // The sink has an authenticated, currently admitted frame and deliberately
    // holds the call open. A successful revoke receipt must not precede it.
    entered.wait();
    let (revoke_sender, revoke_receiver) = sync_channel(1);
    let pairing_id = record.pairing_id;
    let revoke_thread = std::thread::spawn(move || {
        let result = revoker.revoke(pairing_id);
        revoke_sender.send(result).expect("send revoke result");
    });
    assert!(matches!(
        revoke_receiver.recv_timeout(Duration::from_millis(20)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));

    release.wait();
    assert!(matches!(
        bridge_thread.join().expect("bridge join"),
        Ok(BridgeOutput::Recorded { .. })
    ));
    revoke_thread.join().expect("revoke join");
    assert_eq!(revoke_receiver.recv().expect("revoke result"), Ok(true));
    assert!(
        PairingStore::open(directory.path())
            .expect("reopen")
            .get(pairing_id)
            .expect("lookup tombstone")
            .expect("tombstone")
            .revoked
    );
    assert_eq!(admissions.lock().expect("sink lock").len(), 1);
}

#[test]
fn concurrent_approvals_and_revocations_do_not_lose_updates() {
    let directory = TempDir::new().expect("temp directory");
    let first = PairingStore::open(directory.path()).expect("first store");
    let second = PairingStore::open(directory.path()).expect("second store");
    let barrier = Arc::new(Barrier::new(2));
    let first_barrier = barrier.clone();
    let second_barrier = barrier.clone();
    let first_thread = std::thread::spawn(move || {
        first_barrier.wait();
        first.approve(request(), Utc::now()).expect("first approval")
    });
    let second_thread = std::thread::spawn(move || {
        second_barrier.wait();
        second.approve(request(), Utc::now()).expect("second approval")
    });
    let first_approval = first_thread.join().expect("first join");
    let second_approval = second_thread.join().expect("second join");
    let store = PairingStore::open(directory.path()).expect("reopen");
    assert_eq!(store.list().expect("list").len(), 2);

    let first_store = PairingStore::open(directory.path()).expect("first revoker");
    let second_store = PairingStore::open(directory.path()).expect("second revoker");
    let barrier = Arc::new(Barrier::new(2));
    let first_barrier = barrier.clone();
    let second_barrier = barrier.clone();
    let pairing_id = first_approval.view.pairing_id;
    let first_thread = std::thread::spawn(move || {
        first_barrier.wait();
        first_store.revoke(pairing_id).expect("first revoke")
    });
    let second_thread = std::thread::spawn(move || {
        second_barrier.wait();
        second_store.revoke(pairing_id).expect("second revoke")
    });
    let outcomes = [
        first_thread.join().expect("first revoke join"),
        second_thread.join().expect("second revoke join"),
    ];
    assert_eq!(outcomes.iter().filter(|changed| **changed).count(), 1);
    let reopened = PairingStore::open(directory.path()).expect("reopen after revoke");
    assert!(reopened.get(pairing_id).expect("lookup").expect("tombstone").revoked);
    assert!(reopened.get(second_approval.view.pairing_id).expect("lookup second").is_some());
}

#[test]
fn expiring_an_active_pairing_stops_the_next_authenticated_frame() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approved_at = Utc::now();
    let approval = store.approve(request(), approved_at).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, approved_at);
    let result = bridge.receive(
        &signed(&extension, navigation(2, "https://example.com/")),
        Duration::from_secs(1),
        record.expires_at,
    );
    assert_eq!(
        result,
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Pairing(
            ghostrace::PairingError::RePairingRequired,
        )))
    );
    assert!(admissions.lock().expect("sink lock").is_empty());
}

#[test]
fn accepted_navigation_is_projected_through_the_journal_boundary() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, now);
    let output = bridge
        .receive(
            &signed(
                &extension,
                navigation(2, "https://user:SENTINEL@example.com/reset?q=SENTINEL#SENTINEL"),
            ),
            Duration::from_secs(1),
            now,
        )
        .expect("navigation is admitted");
    let BridgeOutput::Recorded { missing, .. } = output else {
        panic!("expected recorded navigation")
    };
    assert_eq!(missing, 0);

    let admissions = admissions.lock().expect("sink lock");
    assert_eq!(admissions.len(), 1);
    assert_eq!(admissions[0].browser.as_str(), "chrome");
    assert_eq!(admissions[0].navigation.origin(), "https://example.com");
    assert!(admissions[0].navigation.path_segment.is_none());
    let encoded = serde_json::to_string(&admissions[0]).expect("admission JSON");
    assert!(!encoded.contains("SENTINEL"));
}

#[test]
fn sink_uncertainty_is_preserved_for_stable_delivery_retry() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let (mut bridge, extension) = open_bridge(store, UncertainSink, &record, now);
    assert_eq!(
        bridge.receive(
            &signed(&extension, navigation(2, "https://example.com/")),
            Duration::from_secs(1),
            now,
        ),
        Err(NativeBridgeError::JournalUncertain)
    );
}

#[test]
fn private_context_is_refused_before_local_service_admission() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, now);
    let private = ExtensionMessage::Navigation {
        seq: 2,
        url: "https://example.com/private".to_owned(),
        private_context: true,
        transition: ghostrace::NavigationTransition::Committed,
        mac: String::new(),
    };
    let output = bridge
        .receive(&signed(&extension, private), Duration::from_secs(1), now)
        .expect("private refusal is a protocol result");
    assert_eq!(
        output,
        BridgeOutput::NavigationRefused {
            refusal: ghostrace::NavigationRefusal::PrivateContext,
            missing: 0,
        }
    );
    assert!(admissions.lock().expect("sink lock").is_empty());
}

#[test]
fn non_navigation_sequence_gaps_are_refused_as_uncertain_until_a_gap_method_exists() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, now);

    let heartbeat = signed(&extension, ExtensionMessage::Heartbeat { seq: 4, mac: String::new() });
    assert_eq!(
        bridge.receive(&heartbeat, Duration::from_secs(1), now),
        Err(NativeBridgeError::Journal)
    );
    assert!(admissions.lock().expect("sink lock").is_empty());

    let directory = TempDir::new().expect("second temp directory");
    let store = PairingStore::open(directory.path()).expect("second store");
    let approval = store.approve(request(), Utc::now()).expect("second approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let (mut bridge, extension) = open_bridge(store, RecordingSink::default(), &record, now);
    let goodbye = signed(&extension, ExtensionMessage::Goodbye { seq: 4, mac: String::new() });
    assert_eq!(
        bridge.receive(&goodbye, Duration::from_secs(1), now),
        Err(NativeBridgeError::Journal)
    );
}

#[test]
fn sequence_gaps_are_recorded_and_a_restart_rejects_old_transcripts() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, extension) = open_bridge(store, sink, &record, now);
    let first = signed(&extension, navigation(2, "https://example.com/"));
    bridge.receive(&first, Duration::from_secs(1), now).expect("first navigation");
    let gap = bridge
        .receive(
            &signed(&extension, navigation(4, "https://example.org/")),
            Duration::from_secs(2),
            now,
        )
        .expect("gap navigation");
    assert!(matches!(gap, BridgeOutput::Recorded { missing: 1, .. }));
    let admissions = admissions.lock().expect("sink lock");
    assert_eq!(admissions.len(), 2);
    assert_eq!(admissions[0].missing, 0);
    assert_eq!(admissions[1].missing, 1);

    let (mut restarted, _) = open_bridge(
        PairingStore::open(directory.path()).expect("store restart"),
        RecordingSink::default(),
        &record,
        now,
    );
    let replay = restarted.receive(&first, Duration::from_secs(1), now);
    assert!(matches!(
        replay,
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Unauthenticated))
    ));
}

#[test]
fn goodbye_closes_the_session_and_trailing_authenticated_data_is_refused() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let now = Utc::now();
    let (mut bridge, extension) = open_bridge(store, RecordingSink::default(), &record, now);
    let goodbye = signed(&extension, ExtensionMessage::Goodbye { seq: 2, mac: String::new() });
    assert_eq!(
        bridge.receive(&goodbye, Duration::from_secs(1), now).expect("goodbye"),
        BridgeOutput::Closed
    );
    let trailing = signed(&extension, navigation(3, "https://example.com/"));
    assert_eq!(
        bridge.receive(&trailing, Duration::from_secs(2), now),
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Protocol(
            ghostrace::NativeMessagingError::TrailingData,
        )))
    );
}

#[test]
fn caller_origin_must_match_the_hello_extension_before_pairing() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let other = "p".repeat(32);
    let mut other_request = request();
    other_request.extension_id = other.clone();
    let approval = store.approve(other_request, Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let mut bridge = NativeBridge::new(
        store,
        RecordingSink::default(),
        UrlShapePolicy::OriginOnly,
        CALLER_ORIGIN,
    )
    .expect("bridge opens");
    // A valid approval for B still cannot authenticate a process launched by A.
    let hello = hello_for(&record, &other, KEY_DIGEST);
    assert_eq!(
        bridge.receive(&hello, Duration::ZERO, Utc::now()),
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Pairing(
            ghostrace::PairingError::NotPaired,
        )))
    );
}

#[test]
fn prehello_rejections_cannot_extend_the_connection_start_deadline() {
    let mut session = NativeHostSession::with_expected_extension_id_at(
        UrlShapePolicy::OriginOnly,
        Some(EXTENSION.to_owned()),
        Some(Duration::ZERO),
    );
    let heartbeat =
        serde_json::to_vec(&ExtensionMessage::Heartbeat { seq: 1, mac: "00".repeat(32) })
            .expect("heartbeat");
    assert_eq!(
        session.receive(
            &heartbeat,
            ghostrace::NATIVE_SESSION_IDLE_TIMEOUT - Duration::from_secs(1),
            Utc::now(),
            |_| None,
        ),
        Err(ghostrace::NativeSessionError::Protocol(
            ghostrace::NativeMessagingError::HelloRequired,
        ))
    );
    assert_eq!(
        session.receive(&heartbeat, ghostrace::NATIVE_SESSION_IDLE_TIMEOUT, Utc::now(), |_| None,),
        Err(ghostrace::NativeSessionError::Protocol(ghostrace::NativeMessagingError::Timeout,))
    );
}

#[test]
fn stdio_host_emits_only_framed_protocol_replies() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let input = encode_frame(&hello(&record, KEY_DIGEST)).expect("hello frame");
    let (output_writer, mut output_reader) = UnixStream::pair().expect("output pair");
    let summary = run_native_host_stdio(
        input_stream(input),
        output_writer,
        store,
        RecordingSink::default(),
        UrlShapePolicy::OriginOnly,
        CALLER_ORIGIN,
    )
    .expect("stdio run");
    let mut output = Vec::new();
    output_reader.read_to_end(&mut output).expect("read output");
    assert_eq!(summary.frames, 1);
    assert!(!summary.closed);

    let mut decoder = FrameDecoder::new();
    decoder.push(&output).expect("output frame");
    let body = decoder.next_frame().expect("decode output").expect("welcome");
    let message: HostMessage = serde_json::from_slice(&body).expect("host message");
    assert!(matches!(message, HostMessage::Welcome { protocol_version: 1, .. }));
    decoder.finish().expect("no trailing output");
}

#[test]
fn stdio_runner_refuses_a_decoded_frame_after_goodbye() {
    for max_read in [1, 3, ghostrace::MAX_NATIVE_HOST_READ_CHUNK] {
        for trailing in [encode_frame(b"{}").expect("trailing frame"), vec![0xff]] {
            for after_closed in [false, true] {
                assert_stdio_trailing_input(max_read, trailing.clone(), after_closed);
            }
        }
    }
}

fn assert_stdio_trailing_input(max_read: usize, trailing: Vec<u8>, after_closed: bool) {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let (mut input_writer, input_reader) = UnixStream::pair().expect("input pair");
    let (output_writer, mut output_reader) = UnixStream::pair().expect("output pair");
    output_reader.set_read_timeout(Some(Duration::from_secs(5))).expect("output timeout");
    let runner = std::thread::spawn(move || {
        run_native_host_stdio(
            ChunkedInput { reader: input_reader, max_read },
            output_writer,
            store,
            RecordingSink::default(),
            UrlShapePolicy::OriginOnly,
            CALLER_ORIGIN,
        )
    });

    input_writer
        .write_all(&encode_frame(&hello(&record, KEY_DIGEST)).expect("hello frame"))
        .expect("send hello");
    let mut welcome_decoder = FrameDecoder::new();
    let mut prefix = [0_u8; 4];
    output_reader.read_exact(&mut prefix).expect("welcome prefix");
    let length = u32::from_ne_bytes(prefix) as usize;
    let mut welcome_body = vec![0_u8; length];
    output_reader.read_exact(&mut welcome_body).expect("welcome body");
    welcome_decoder
        .push(&encode_frame(&welcome_body).expect("welcome reframe"))
        .expect("welcome decode");
    let welcome = welcome_decoder.next_frame().expect("welcome frame").expect("welcome");
    let HostMessage::Welcome { host_nonce, .. } =
        serde_json::from_slice(&welcome).expect("welcome JSON")
    else {
        panic!("expected welcome")
    };
    let mut host_nonce_bytes = [0_u8; 32];
    for (index, byte) in host_nonce_bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&host_nonce[index * 2..index * 2 + 2], 16)
            .expect("host nonce is hex");
    }
    let extension =
        PairedSession::derive(&record.secret_for_extension(), &host_nonce_bytes, &CLIENT_NONCE);
    input_writer
        .write_all(
            &encode_frame(&signed(
                &extension,
                ExtensionMessage::Goodbye { seq: 2, mac: String::new() },
            ))
            .expect("goodbye frame"),
        )
        .expect("send goodbye");
    let mut trailing_output = Vec::new();
    if after_closed {
        // Wait for the closed reply before sending any trailing byte. This
        // proves the runner checks future input as well as buffered input.
        output_reader.read_exact(&mut prefix).expect("closed prefix");
        let mut body = vec![0_u8; u32::from_ne_bytes(prefix) as usize];
        output_reader.read_exact(&mut body).expect("closed body");
        assert_eq!(
            serde_json::from_slice::<HostMessage>(&body).expect("closed JSON"),
            HostMessage::Closed
        );
        trailing_output.extend_from_slice(&prefix);
        trailing_output.extend_from_slice(&body);
    }
    input_writer.write_all(&trailing).expect("send trailing frame");
    input_writer.shutdown(std::net::Shutdown::Write).expect("close input");

    assert_eq!(
        runner.join().expect("runner join"),
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Protocol(
            ghostrace::NativeMessagingError::TrailingData,
        )))
    );
    output_reader.read_to_end(&mut trailing_output).expect("read trailing output");
    let mut decoder = FrameDecoder::new();
    decoder.push(&trailing_output).expect("decode closed output");
    let body = decoder.next_frame().expect("closed frame").expect("closed");
    assert!(matches!(serde_json::from_slice::<HostMessage>(&body), Ok(HostMessage::Closed)));
    decoder.finish().expect("no refusal after closed");
}

#[cfg(target_os = "macos")]
fn browser_service_policy() -> PolicyProfile {
    let mut policy = PolicyProfile::deny_by_default("browser-pairing-v1");
    policy.enable_source(EventSource::Browser);
    policy
}

#[cfg(target_os = "macos")]
fn signed_browser_relay(
    record: &PairingRecord,
    service_instance: uuid::Uuid,
    request_id: uuid::Uuid,
    mut admission: BrowserNavigationAdmission,
) -> BrowserNavigationRelay {
    let sequence = 2;
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

#[cfg(target_os = "macos")]
#[test]
fn service_retries_return_the_durable_receipt_without_duplicate_gap_or_navigation() {
    let directory = TempDir::new().expect("temp directory");
    let pairing_directory = directory.path().join("pairings");
    let pairing_store = PairingStore::open(&pairing_directory).expect("pairing store");
    let approval = pairing_store.approve(request(), Utc::now()).expect("pairing approval");
    let record = pairing_store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let mut service =
        LocalService::bind(&directory.path().join("service"), [ServiceCapability::Ingest])
            .expect("service bind");
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("browser-retry")).expect("journal");
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        browser_service_policy(),
        PairingStore::open(&pairing_directory).expect("service pairing store"),
    )
    .expect("handler");
    let admission = BrowserNavigationAdmission {
        event_id: uuid::Uuid::nil(),
        browser: "chrome".try_into().expect("browser"),
        navigation: CanonicalNavigation::from_url(
            "https://example.com/account?secret=value",
            false,
            UrlShapePolicy::OriginOnly,
        )
        .expect("canonical navigation"),
        observed_at: Utc::now(),
        missing: 1,
    };

    let submit = |service: &mut LocalService,
                  admission: BrowserNavigationAdmission|
     -> Result<BrowserNavigationAck, ghostrace::ServiceError> {
        let socket = service.socket_path().to_path_buf();
        let instance = service.instance();
        let request_id = uuid::Uuid::new_v4();
        let relay = signed_browser_relay(&record, instance, request_id, admission);
        let client = std::thread::spawn(move || {
            ingest_browser_navigation(&socket, instance, request_id, 5_000, relay)
        });
        service.serve_one(&handler).expect("serve admission");
        client.join().expect("client join")
    };

    let first = submit(&mut service, admission.clone()).expect("first admission");
    let event_id = delivery_event_id(record.pairing_id, &CLIENT_NONCE, 2);
    let retry_admission = BrowserNavigationAdmission {
        observed_at: admission.observed_at + chrono::Duration::seconds(1),
        ..admission.clone()
    };
    let retry = submit(&mut service, retry_admission).expect("retry admission");
    assert_eq!(retry, first, "a committed retry returns its original receipt");
    assert_eq!(first.ingest_sequences.len(), 2, "gap and navigation are both acknowledged");

    let conflict = BrowserNavigationAdmission {
        navigation: CanonicalNavigation::from_url(
            "https://other.example/",
            false,
            UrlShapePolicy::OriginOnly,
        )
        .expect("conflicting canonical navigation"),
        ..admission
    };
    assert_eq!(submit(&mut service, conflict), Err(ghostrace::ServiceError::BrowserIngestRefused));
    let events = journal.events().expect("events");
    assert_eq!(events.len(), 2, "conflicting retry did not append anything");
    assert_eq!(events[1].event.event_id, event_id);
}

#[cfg(target_os = "macos")]
#[test]
fn service_refuses_private_network_projection_without_synthesizing_an_origin() {
    let directory = TempDir::new().expect("temp directory");
    let pairing_directory = directory.path().join("pairings");
    let pairing_store = PairingStore::open(&pairing_directory).expect("pairing store");
    let approval = pairing_store.approve(request(), Utc::now()).expect("pairing approval");
    let record = pairing_store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let service =
        LocalService::bind(&directory.path().join("service"), [ServiceCapability::Ingest])
            .expect("service bind");
    let instance = service.instance();
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("private-network"))
        .expect("journal");
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        browser_service_policy(),
        PairingStore::open(&pairing_directory).expect("service pairing store"),
    )
    .expect("handler");
    let admission = BrowserNavigationAdmission {
        event_id: uuid::Uuid::nil(),
        browser: "chrome".try_into().expect("browser"),
        navigation: CanonicalNavigation::from_url(
            "https://localhost:8443/private",
            false,
            UrlShapePolicy::OriginOnly,
        )
        .expect("private navigation canonicalizes with host withheld"),
        observed_at: Utc::now(),
        missing: 0,
    };
    let request_id = uuid::Uuid::new_v4();
    let request = browser_navigation_request(
        instance,
        request_id,
        5_000,
        signed_browser_relay(&record, instance, request_id, admission),
    )
    .expect("typed request");
    assert_eq!(
        ServiceHandler::handle(&handler, &request),
        Err(ghostrace::ServiceError::BrowserIngestRefused)
    );
    assert!(journal.events().expect("events").is_empty());
    drop(directory);
}

#[cfg(target_os = "macos")]
#[test]
fn service_refuses_a_relay_proof_replayed_under_another_request_id() {
    let directory = TempDir::new().expect("temp directory");
    let pairing_directory = directory.path().join("pairings");
    let pairing_store = PairingStore::open(&pairing_directory).expect("pairing store");
    let approval = pairing_store.approve(request(), Utc::now()).expect("pairing approval");
    let record = pairing_store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("proof-replay")).expect("journal");
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        browser_service_policy(),
        PairingStore::open(&pairing_directory).expect("service pairing store"),
    )
    .expect("handler");
    let service_instance = uuid::Uuid::new_v4();
    let signed_request_id = uuid::Uuid::new_v4();
    let admission = BrowserNavigationAdmission {
        event_id: uuid::Uuid::nil(),
        browser: "chrome".try_into().expect("browser"),
        navigation: CanonicalNavigation::from_url(
            "https://example.com/",
            false,
            UrlShapePolicy::OriginOnly,
        )
        .expect("canonical navigation"),
        observed_at: Utc::now(),
        missing: 0,
    };
    let relay = signed_browser_relay(&record, service_instance, signed_request_id, admission);
    let replay_request =
        browser_navigation_request(service_instance, uuid::Uuid::new_v4(), 5_000, relay)
            .expect("typed replay request");
    assert_eq!(
        ServiceHandler::handle(&handler, &replay_request),
        Err(ghostrace::ServiceError::BrowserIngestRefused)
    );
    assert!(journal.events().expect("events").is_empty());
}

#[cfg(target_os = "macos")]
const NATIVE_E2E_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(target_os = "macos")]
const MAX_NATIVE_E2E_CAPTURE_BYTES: usize = 256 * 1024;

#[cfg(target_os = "macos")]
struct NativeHostChildGuard {
    child: Child,
}

#[cfg(target_os = "macos")]
impl NativeHostChildGuard {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn process_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn wait_until(&mut self, deadline: Instant) -> ExitStatus {
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status,
                Ok(None) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    assert!(!remaining.is_zero(), "native host subprocess deadline expired");
                    std::thread::sleep(remaining.min(Duration::from_millis(5)));
                }
                Err(error) => panic!("poll native host subprocess: {error}"),
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeHostChildGuard {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

#[cfg(target_os = "macos")]
struct ServiceThreadGuard {
    socket_path: PathBuf,
    handle: Option<std::thread::JoinHandle<()>>,
    done: std::sync::mpsc::Receiver<Result<(), ghostrace::ServiceError>>,
    deadline: Instant,
}

#[cfg(target_os = "macos")]
impl ServiceThreadGuard {
    fn new(
        socket_path: PathBuf,
        handle: std::thread::JoinHandle<()>,
        done: std::sync::mpsc::Receiver<Result<(), ghostrace::ServiceError>>,
        deadline: Instant,
    ) -> Self {
        Self { socket_path, handle: Some(handle), done, deadline }
    }

    fn wait_until(&mut self) -> Result<(), ghostrace::ServiceError> {
        let result = self
            .done
            .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
            .expect("service completion deadline");
        while !self.handle.as_ref().expect("service thread handle").is_finished() {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "service thread shutdown deadline expired");
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
        let handle = self.handle.take().expect("service thread handle");
        handle.join().expect("service thread join");
        result
    }
}

#[cfg(target_os = "macos")]
impl Drop for ServiceThreadGuard {
    fn drop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        // Wake a service blocked in accept/read so a failed host test does not
        // leave an unbounded background join behind. A zero-length service
        // message is rejected before any handler or journal work.
        if let Ok(mut stream) = UnixStream::connect(&self.socket_path) {
            let _ = stream.write_all(&0_u32.to_be_bytes());
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if self.done.recv_timeout(remaining).is_ok() && handle.is_finished() {
            let _ = handle.join();
        }
        // If the service did not finish by the test deadline, deliberately
        // detach rather than letting cleanup make the test unbounded.
    }
}

#[cfg(target_os = "macos")]
fn wait_for_readable(fd: RawFd, deadline: Instant) {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "native host pipe deadline expired");
        let timeout_ms = remaining.as_millis().max(1).min(i32::MAX as u128) as libc::c_int;
        let mut descriptor =
            libc::pollfd { fd, events: libc::POLLIN | libc::POLLHUP | libc::POLLERR, revents: 0 };
        // SAFETY: `descriptor` is initialized and remains alive for this call.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if result > 0 {
            return;
        }
        if result == 0 {
            panic!("native host pipe deadline expired");
        }
        if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        panic!("poll native host pipe: {}", std::io::Error::last_os_error());
    }
}

#[cfg(target_os = "macos")]
fn read_exact_until<R: Read + AsRawFd>(reader: &mut R, bytes: &mut [u8], deadline: Instant) {
    let mut offset = 0usize;
    while offset < bytes.len() {
        wait_for_readable(reader.as_raw_fd(), deadline);
        match reader.read(&mut bytes[offset..]) {
            Ok(0) => panic!("native host pipe closed before the expected bytes arrived"),
            Ok(read) => offset += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("read native host pipe: {error}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn read_to_end_until<R: Read + AsRawFd>(
    reader: &mut R,
    deadline: Instant,
    max_bytes: usize,
) -> Vec<u8> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        wait_for_readable(reader.as_raw_fd(), deadline);
        match reader.read(&mut chunk) {
            Ok(0) => return output,
            Ok(read) => {
                assert!(
                    output.len().saturating_add(read) <= max_bytes,
                    "native host output exceeded the test capture bound"
                );
                output.extend_from_slice(&chunk[..read]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("read native host pipe: {error}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn read_host_message_until<R: Read + AsRawFd>(reader: &mut R, deadline: Instant) -> HostMessage {
    let mut prefix = [0_u8; 4];
    read_exact_until(reader, &mut prefix, deadline);
    let length = u32::from_ne_bytes(prefix) as usize;
    assert!(length > 0 && length <= MAX_NATIVE_FRAME_BYTES, "bounded host message");
    let mut body = vec![0_u8; length];
    read_exact_until(reader, &mut body, deadline);
    serde_json::from_slice(&body).expect("host message JSON")
}

#[cfg(target_os = "macos")]
#[test]
fn production_binary_runs_the_full_chromium_bridge_to_a_durable_service_journal() {
    let directory = TempDir::new().expect("temp directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private parent");
    let home = directory.path().join("home");
    let ghostrace_home = home.join("Library/Application Support/GHOSTRACE");
    fs::create_dir_all(&ghostrace_home).expect("GHOSTRACE home");
    fs::set_permissions(&ghostrace_home, fs::Permissions::from_mode(0o700))
        .expect("private GHOSTRACE home");
    let store = PairingStore::open(ghostrace_home.join(NATIVE_HOST_STORE_DIR)).expect("store");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let journal_path = directory.path().join("service-journal.sqlite3");
    let journal = Journal::open_fixture(
        &journal_path,
        DeterministicKeyProvider::from_seed("native-binary-service"),
    )
    .expect("service journal");
    let mut service =
        LocalService::bind(&directory.path().join("service"), [ServiceCapability::Ingest])
            .expect("service bind");
    let service_socket = service.socket_path().to_path_buf();
    let service_instance = service.instance();
    let handler = BrowserIngestService::new_with_pairing_store(
        journal.clone(),
        browser_service_policy(),
        PairingStore::open(ghostrace_home.join(NATIVE_HOST_STORE_DIR)).expect("service store"),
    )
    .expect("service handler");
    write_native_service_endpoint(&ghostrace_home, service_socket.clone(), service_instance)
        .expect("endpoint");
    NativeHostInstaller::new(
        ghostrace_home.parent().expect("Application Support"),
        env!("CARGO_BIN_EXE_ghostrace"),
        EXTENSION,
    )
    .expect("native host installer")
    .install("chrome")
    .expect("native host manifest");

    let deadline = Instant::now() + NATIVE_E2E_TIMEOUT;
    let (service_done_sender, service_done_receiver) = sync_channel(1);
    let service_thread = std::thread::spawn(move || {
        let result = service.serve_one(&handler);
        let _ = service_done_sender.send(result);
    });
    let mut service_guard = ServiceThreadGuard::new(
        service_socket.clone(),
        service_thread,
        service_done_receiver,
        deadline,
    );
    let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .arg(CALLER_ORIGIN)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native host binary");
    let mut child = NativeHostChildGuard::new(child);
    let mut input = child.process_mut().stdin.take().expect("native host stdin");
    let mut output = child.process_mut().stdout.take().expect("native host stdout");
    let mut diagnostics = child.process_mut().stderr.take().expect("native host stderr");
    input
        .write_all(&encode_frame(&hello(&record, KEY_DIGEST)).expect("hello frame"))
        .expect("send hello");
    let HostMessage::Welcome { host_nonce, .. } = read_host_message_until(&mut output, deadline)
    else {
        panic!("expected welcome");
    };
    let mut host_nonce_bytes = [0_u8; 32];
    for (index, byte) in host_nonce_bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&host_nonce[index * 2..index * 2 + 2], 16)
            .expect("host nonce is hex");
    }
    let extension =
        PairedSession::derive(&record.secret_for_extension(), &host_nonce_bytes, &CLIENT_NONCE);
    input
        .write_all(
            &encode_frame(&signed(
                &extension,
                navigation(2, "https://user:SENTINEL@example.com/account?token=SENTINEL"),
            ))
            .expect("navigation frame"),
        )
        .expect("send navigation");
    let accepted = read_host_message_until(&mut output, deadline);
    let event_id = match accepted {
        HostMessage::Accepted { event_id, missing } => {
            assert_eq!(missing, 0);
            event_id
        }
        other => panic!("expected accepted navigation, got {other:?}"),
    };
    service_guard.wait_until().expect("serve browser navigation");

    input
        .write_all(
            &encode_frame(&signed(
                &extension,
                ExtensionMessage::Goodbye { seq: 3, mac: String::new() },
            ))
            .expect("goodbye frame"),
        )
        .expect("send goodbye");
    assert_eq!(read_host_message_until(&mut output, deadline), HostMessage::Closed);
    drop(input);
    let trailing_output = read_to_end_until(&mut output, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let stderr = read_to_end_until(&mut diagnostics, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let status = child.wait_until(deadline);
    assert!(status.success(), "native host failed: {}", String::from_utf8_lossy(&stderr));
    assert!(trailing_output.is_empty(), "native host emitted trailing output");
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("native host stopped after 3 frame(s), 1 recorded event(s)"));
    assert!(!stderr.contains("SENTINEL"));
    assert!(!stderr.contains(&home.to_string_lossy().to_string()));

    drop(journal);
    let reopened = Journal::open_fixture(
        &journal_path,
        DeterministicKeyProvider::from_seed("native-binary-service"),
    )
    .expect("reopen service journal");
    let events = reopened.events().expect("service journal events after reopen");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.event_id, event_id);
    assert_eq!(events[0].event.kind, EventKind::BrowserNavigation);
    let EventPayload::BrowserNavigation(payload) = &events[0].event.payload else {
        panic!("expected browser navigation event");
    };
    assert_eq!(payload.url.as_str(), "https://example.com/");
    assert!(!serde_json::to_string(&events[0].event).expect("event JSON").contains("SENTINEL"));
    reopened.shutdown().expect("checkpoint reopened service journal");
}

#[cfg(target_os = "macos")]
#[test]
fn production_binary_rejects_invalid_origin_before_opening_store_or_stdin() {
    let directory = TempDir::new().expect("temp directory");
    let home = directory.path().join("home");
    let sentinel = "SENTINEL-invalid-origin";
    let deadline = Instant::now() + NATIVE_E2E_TIMEOUT;
    let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .arg("chrome-extension://not-an-extension/")
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native host binary");
    let mut child = NativeHostChildGuard::new(child);
    let _ =
        child.process_mut().stdin.take().expect("native host stdin").write_all(sentinel.as_bytes());
    let mut output = child.process_mut().stdout.take().expect("native host stdout");
    let mut diagnostics = child.process_mut().stderr.take().expect("native host stderr");
    let stdout = read_to_end_until(&mut output, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let stderr = read_to_end_until(&mut diagnostics, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let status = child.wait_until(deadline);
    assert!(!status.success());
    assert!(stdout.is_empty());
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("native host operation failed"));
    assert!(!stderr.contains("not-an-extension"));
    assert!(!stderr.contains(sentinel));
    assert!(!home.join("Library").exists(), "invalid argv opened the native store");
}

#[cfg(target_os = "macos")]
#[test]
fn production_binary_rejects_extra_native_host_argv_without_opening_store() {
    let directory = TempDir::new().expect("temp directory");
    let home = directory.path().join("home");
    let deadline = Instant::now() + NATIVE_E2E_TIMEOUT;
    let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .arg(CALLER_ORIGIN)
        .arg("unexpected-extra-argv")
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native host binary");
    let mut child = NativeHostChildGuard::new(child);
    drop(child.process_mut().stdin.take());
    let mut output = child.process_mut().stdout.take().expect("native host stdout");
    let mut diagnostics = child.process_mut().stderr.take().expect("native host stderr");
    let stdout = read_to_end_until(&mut output, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let stderr = read_to_end_until(&mut diagnostics, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let status = child.wait_until(deadline);
    assert!(!status.success());
    assert!(stdout.is_empty());
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("native host operation failed"));
    assert!(!stderr.contains(CALLER_ORIGIN));
    assert!(!stderr.contains("unexpected-extra-argv"));
    assert!(!home.join("Library").exists(), "extra argv opened the native store");
}

#[cfg(target_os = "macos")]
#[test]
fn production_binary_refuses_a_mismatched_hello_without_echoing_input() {
    let directory = TempDir::new().expect("temp directory");
    let home = directory.path().join("home");
    let ghostrace_home = home.join("Library/Application Support/GHOSTRACE");
    fs::create_dir_all(&ghostrace_home).expect("GHOSTRACE home");
    fs::set_permissions(&ghostrace_home, fs::Permissions::from_mode(0o700))
        .expect("private GHOSTRACE home");
    let store = PairingStore::open(ghostrace_home.join(NATIVE_HOST_STORE_DIR)).expect("store");
    let approval = store.approve(request(), Utc::now()).expect("pairing approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let service_socket = directory.path().join("service/unused.sock");
    write_native_service_endpoint(&ghostrace_home, service_socket.clone(), uuid::Uuid::new_v4())
        .expect("endpoint");
    NativeHostInstaller::new(
        ghostrace_home.parent().expect("Application Support"),
        env!("CARGO_BIN_EXE_ghostrace"),
        EXTENSION,
    )
    .expect("native host installer")
    .install("chrome")
    .expect("native host manifest");
    let deadline = Instant::now() + NATIVE_E2E_TIMEOUT;
    let child = Command::new(env!("CARGO_BIN_EXE_ghostrace"))
        .arg(CALLER_ORIGIN)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native host binary");
    let mut child = NativeHostChildGuard::new(child);
    let mut input = child.process_mut().stdin.take().expect("native host stdin");
    let mut output = child.process_mut().stdout.take().expect("native host stdout");
    input
        .write_all(
            &encode_frame(&hello_for(&record, "ponmlkjihgfedcbaponmlkjihgfedcba", KEY_DIGEST))
                .expect("hello frame"),
        )
        .expect("send mismatched hello");
    drop(input);
    assert_eq!(
        read_host_message_until(&mut output, deadline),
        HostMessage::Refused { reason: "not_paired".to_owned() }
    );
    let trailing_output = read_to_end_until(&mut output, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let mut diagnostics = child.process_mut().stderr.take().expect("native host stderr");
    let stderr = read_to_end_until(&mut diagnostics, deadline, MAX_NATIVE_E2E_CAPTURE_BYTES);
    let status = child.wait_until(deadline);
    assert!(!status.success());
    assert!(trailing_output.is_empty());
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(!stderr.contains("ponmlkjihgfedcbaponmlkjihgfedcba"));
    assert!(!stderr.contains(&service_socket.to_string_lossy().to_string()));
    assert!(!stderr.contains(&approval.extension_secret));
}

#[test]
fn stdio_host_refuses_a_truncated_frame_before_semantic_processing() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let mut input = 5_u32.to_ne_bytes().to_vec();
    input.extend_from_slice(b"{}");
    let (output_writer, mut output_reader) = UnixStream::pair().expect("output pair");
    let result = run_native_host_stdio(
        input_stream(input),
        output_writer,
        store,
        RecordingSink::default(),
        UrlShapePolicy::OriginOnly,
        CALLER_ORIGIN,
    );
    let mut output = Vec::new();
    output_reader.read_to_end(&mut output).expect("read output");
    assert_eq!(result, Err(NativeBridgeError::Framing));
    assert!(output.is_empty());
}

#[test]
fn stdio_host_times_out_without_a_hello_or_with_a_partial_frame() {
    let directory = TempDir::new().expect("temp directory");
    let store = PairingStore::open(directory.path()).expect("store opens");
    let (reader, _writer) = UnixStream::pair().expect("idle pair");
    let (output_writer, _output_reader) = UnixStream::pair().expect("idle output pair");
    let result = ghostrace::native_bridge::run_stdio_with_timeout(
        reader,
        output_writer,
        store,
        RecordingSink::default(),
        UrlShapePolicy::OriginOnly,
        CALLER_ORIGIN,
        Duration::from_millis(20),
    );
    assert_eq!(
        result,
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Protocol(
            ghostrace::NativeMessagingError::Timeout,
        )))
    );

    let directory = TempDir::new().expect("partial directory");
    let store = PairingStore::open(directory.path()).expect("partial store");
    let (mut writer, reader) = UnixStream::pair().expect("partial pair");
    writer.write_all(&100_u32.to_ne_bytes()).expect("partial prefix");
    let (output_writer, _output_reader) = UnixStream::pair().expect("partial output pair");
    let result = ghostrace::native_bridge::run_stdio_with_timeout(
        reader,
        output_writer,
        store,
        RecordingSink::default(),
        UrlShapePolicy::OriginOnly,
        CALLER_ORIGIN,
        Duration::from_millis(20),
    );
    assert_eq!(
        result,
        Err(NativeBridgeError::Session(ghostrace::NativeSessionError::Protocol(
            ghostrace::NativeMessagingError::Timeout,
        )))
    );
}

#[test]
fn unpaired_hello_and_unauthenticated_navigation_never_reach_the_sink() {
    let directory = TempDir::new().expect("directory");
    let store = PairingStore::open(directory.path()).expect("store");
    let record = PairingRecord::approve(request(), Utc::now()).expect("unapproved record");
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let mut bridge =
        NativeBridge::new(store, sink, UrlShapePolicy::OriginOnly, CALLER_ORIGIN).expect("bridge");
    let error = bridge
        .receive(&hello(&record, KEY_DIGEST), Duration::ZERO, Utc::now())
        .expect_err("unpaired hello refused");
    assert_eq!(error.code(), "not_paired");
    assert!(admissions.lock().expect("sink").is_empty());

    let store = PairingStore::open(directory.path()).expect("store");
    let approval = store.approve(request(), Utc::now()).expect("approval");
    let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
    let sink = RecordingSink::default();
    let admissions = sink.admissions.clone();
    let (mut bridge, _) = open_bridge(store, sink, &record, Utc::now());
    let mut message =
        serde_json::to_value(navigation(2, "https://SENTINEL.example/secret?q=SENTINEL"))
            .expect("unauthed navigation");
    message["mac"] = serde_json::json!("00".repeat(32));
    let body = serde_json::to_vec(&message).expect("body");
    let error = bridge
        .receive(&body, Duration::from_secs(1), Utc::now())
        .expect_err("unauthed navigation refused");
    assert_eq!(error.code(), "unauthenticated");
    assert!(!error.to_string().contains("SENTINEL"));
    assert!(admissions.lock().expect("sink").is_empty());
}

#[test]
fn malformed_commands_paths_and_oversized_messages_have_no_sink_or_file_effect() {
    let directory = TempDir::new().expect("directory");
    let marker = directory.path().join("outside-journal-marker");
    for variant in ["command", "journal_path", "oversized", "unknown_type"] {
        let store = PairingStore::open(directory.path()).expect("store");
        let approval = store.approve(request(), Utc::now()).expect("approval");
        let record = store.get(approval.view.pairing_id).expect("lookup").expect("record");
        let sink = RecordingSink::default();
        let admissions = sink.admissions.clone();
        let (mut bridge, extension) = open_bridge(store, sink, &record, Utc::now());
        let valid = signed(&extension, navigation(2, "https://example.com/"));
        let mut body: serde_json::Value = serde_json::from_slice(&valid).expect("body");
        match variant {
            "command" => body["command"] = serde_json::json!(["touch", marker]),
            "journal_path" => {
                body["journal_path"] = serde_json::json!("../../outside-journal-marker")
            }
            "oversized" => body["url"] = serde_json::json!("SENTINEL".repeat(10_000)),
            "unknown_type" => body["type"] = serde_json::json!("execute"),
            _ => unreachable!(),
        }
        let error = bridge
            .receive(
                &serde_json::to_vec(&body).expect("encode"),
                Duration::from_secs(1),
                Utc::now(),
            )
            .expect_err("malformed frame refused");
        assert_eq!(error.code(), "protocol_error", "{variant}");
        assert!(!error.to_string().contains("SENTINEL"));
        assert!(!error.to_string().contains("outside-journal"));
        assert!(admissions.lock().expect("sink").is_empty(), "{variant}");
        assert!(!marker.exists(), "{variant}");
    }
}
