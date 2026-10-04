//! The authenticated local service socket.

#![cfg(unix)]

use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{symlink, MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    thread,
    time::{Duration, Instant},
};

use ghostrace::{
    service_request, LocalService, ServiceCapability, ServiceError, ServiceHandler, ServiceRequest,
    ServiceResponse, LOCAL_SERVICE_PROTOCOL_VERSION, MAX_SERVICE_MESSAGE_BYTES,
};
use serde_json::{json, Value};
use uuid::Uuid;

struct Echo;

impl ServiceHandler for Echo {
    fn handle(&self, request: &ServiceRequest) -> Result<Value, ServiceError> {
        Ok(json!({"method": request.method}))
    }
}

struct DeadlineEcho;

impl ServiceHandler for DeadlineEcho {
    fn handle(&self, request: &ServiceRequest) -> Result<Value, ServiceError> {
        Ok(json!({"method": request.method}))
    }

    fn handle_with_deadline(
        &self,
        request: &ServiceRequest,
        context: ghostrace::local_service::ServiceRequestContext,
    ) -> Result<Value, ServiceError> {
        assert!(!context.is_expired());
        self.handle(request)
    }
}

fn private_parent() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("tempdir");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).expect("chmod");
    directory
}

fn request_for(service: &LocalService, capability: ServiceCapability) -> ServiceRequest {
    ServiceRequest {
        protocol_version: LOCAL_SERVICE_PROTOCOL_VERSION,
        service_instance: service.instance(),
        request_id: Uuid::new_v4(),
        deadline_ms: 5_000,
        capability,
        method: "status".to_owned(),
        params: Value::Null,
    }
}

/// Serve one connection on a thread and send `request` to it. The client
/// waits a fixed time rather than the request's own deadline, so a request
/// the server must refuse for its deadline still gets its answer read.
fn roundtrip(service: &mut LocalService, request: &ServiceRequest) -> ServiceResponse {
    roundtrip_with(service, request, &Echo)
}

fn roundtrip_with<H: ServiceHandler>(
    service: &mut LocalService,
    request: &ServiceRequest,
    handler: &H,
) -> ServiceResponse {
    let path = service.socket_path().to_path_buf();
    let body = serde_json::to_vec(request).expect("request JSON");
    let client = thread::spawn(move || {
        let mut stream = UnixStream::connect(path).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        stream.write_all(&(body.len() as u32).to_be_bytes()).expect("length");
        stream.write_all(&body).expect("body");
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).expect("response length");
        let mut response = vec![0u8; u32::from_be_bytes(length) as usize];
        stream.read_exact(&mut response).expect("response body");
        serde_json::from_slice::<ServiceResponse>(&response).expect("response JSON")
    });
    service.serve_one(handler).expect("serve");
    client.join().expect("client")
}

fn refused(response: ServiceResponse) -> ServiceError {
    match response {
        ServiceResponse::Refused { error } => error,
        ServiceResponse::Ok { .. } => panic!("request was admitted"),
    }
}

#[test]
fn socket_is_private_and_a_granted_request_is_answered() {
    let parent = private_parent();
    let directory = parent.path().join("service");
    let mut service = LocalService::bind(&directory, [ServiceCapability::Read]).expect("bind");
    let directory_mode = fs::symlink_metadata(&directory).expect("dir").permissions().mode();
    assert_eq!(directory_mode & 0o777, 0o700);
    let socket = fs::symlink_metadata(service.socket_path()).expect("socket");
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
    // SAFETY: geteuid has no preconditions.
    assert_eq!(socket.uid(), unsafe { libc::geteuid() });

    let request = request_for(&service, ServiceCapability::Read);
    match roundtrip(&mut service, &request) {
        ServiceResponse::Ok { request_id, result } => {
            assert_eq!(request_id, request.request_id);
            assert_eq!(result, json!({"method": "status"}));
        }
        ServiceResponse::Refused { error } => panic!("refused: {error}"),
    }
}

#[test]
fn capabilities_are_separate_and_denied_by_default() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    for capability in [
        ServiceCapability::Ingest,
        ServiceCapability::Export,
        ServiceCapability::Policy,
        ServiceCapability::Lifecycle,
        ServiceCapability::Admin,
    ] {
        let request = request_for(&service, capability);
        assert_eq!(refused(roundtrip(&mut service, &request)), ServiceError::CapabilityDenied);
    }
    let parent = private_parent();
    let mut nothing = LocalService::bind(&parent.path().join("svc"), []).expect("bind");
    let request = request_for(&nothing, ServiceCapability::Read);
    assert_eq!(refused(roundtrip(&mut nothing, &request)), ServiceError::CapabilityDenied);
}

#[test]
fn compatible_handler_receives_a_monotonic_deadline_context() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    let request = request_for(&service, ServiceCapability::Read);
    assert!(matches!(
        roundtrip_with(&mut service, &request, &DeadlineEcho),
        ServiceResponse::Ok { .. }
    ));
}

#[test]
fn the_client_refuses_an_invalid_deadline_before_connecting() {
    let parent = private_parent();
    let service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    let base = request_for(&service, ServiceCapability::Read);
    for deadline_ms in [0, 60 * 60 * 1000] {
        let request = ServiceRequest { deadline_ms, ..base.clone() };
        assert_eq!(
            service_request(service.socket_path(), &request),
            Err(ServiceError::InvalidDeadline)
        );
    }
}

#[test]
fn client_budget_covers_a_connected_socket_that_never_answers() {
    let parent = private_parent();
    let service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    let request =
        ServiceRequest { deadline_ms: 25, ..request_for(&service, ServiceCapability::Read) };
    let started = Instant::now();
    assert_eq!(
        service_request(service.socket_path(), &request),
        Err(ServiceError::DeadlineExceeded)
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn version_instance_deadline_and_replay_are_checked_before_dispatch() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    let base = request_for(&service, ServiceCapability::Read);
    let cases = [
        (ServiceRequest { protocol_version: 2, ..base.clone() }, ServiceError::UnsupportedVersion),
        (ServiceRequest { protocol_version: 0, ..base.clone() }, ServiceError::UnsupportedVersion),
        (
            ServiceRequest { service_instance: Uuid::new_v4(), ..base.clone() },
            ServiceError::WrongInstance,
        ),
        (ServiceRequest { deadline_ms: 0, ..base.clone() }, ServiceError::InvalidDeadline),
        (ServiceRequest { deadline_ms: 31_000, ..base.clone() }, ServiceError::InvalidDeadline),
        (ServiceRequest { method: String::new(), ..base.clone() }, ServiceError::Malformed),
    ];
    for (request, expected) in cases {
        let request = ServiceRequest { request_id: Uuid::new_v4(), ..request };
        assert_eq!(refused(roundtrip(&mut service, &request)), expected);
    }
    assert!(matches!(roundtrip(&mut service, &base), ServiceResponse::Ok { .. }));
    assert_eq!(refused(roundtrip(&mut service, &base)), ServiceError::Replay);
}

#[test]
fn oversized_malformed_and_unknown_field_requests_are_refused() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    let path = service.socket_path().to_path_buf();
    let raw = |path: &Path, bytes: Vec<u8>| {
        let path = path.to_path_buf();
        thread::spawn(move || {
            let mut stream = UnixStream::connect(&path).expect("connect");
            let _ = stream.write_all(&bytes);
            let mut prefix = [0u8; 4];
            stream.read_exact(&mut prefix).expect("response prefix");
            let mut body = vec![0u8; u32::from_be_bytes(prefix) as usize];
            stream.read_exact(&mut body).expect("response body");
            serde_json::from_slice::<ServiceResponse>(&body).expect("response")
        })
    };
    let oversized = ((MAX_SERVICE_MESSAGE_BYTES + 1) as u32).to_be_bytes().to_vec();
    let client = raw(&path, oversized);
    service.serve_one(&Echo).expect("serve");
    assert_eq!(refused(client.join().expect("client")), ServiceError::TooLarge);

    let mut request =
        serde_json::to_value(request_for(&service, ServiceCapability::Read)).expect("json");
    request["admin_override"] = json!(true);
    let body = serde_json::to_vec(&request).expect("body");
    let mut framed = (body.len() as u32).to_be_bytes().to_vec();
    framed.extend(body);
    let client = raw(&path, framed);
    service.serve_one(&Echo).expect("serve");
    assert_eq!(refused(client.join().expect("client")), ServiceError::Malformed);
}

#[test]
fn a_mismatched_peer_is_rejected_before_the_request_is_parsed() {
    let parent = private_parent();
    let mut service =
        LocalService::bind(&parent.path().join("svc"), [ServiceCapability::Read]).expect("bind");
    // SAFETY: geteuid has no preconditions.
    service.set_expected_uid_for_test(unsafe { libc::geteuid() }.wrapping_add(1));
    let request = request_for(&service, ServiceCapability::Read);
    assert_eq!(refused(roundtrip(&mut service, &request)), ServiceError::PeerRejected);
}

#[test]
fn unsafe_directories_and_socket_paths_are_refused_without_following_links() {
    let parent = private_parent();
    let open = parent.path().join("open");
    fs::create_dir(&open).expect("dir");
    fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(LocalService::bind(&open, []).err(), Some(ServiceError::UnsafeDirectory));

    let target = parent.path().join("target");
    fs::create_dir(&target).expect("target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).expect("chmod");
    let link = parent.path().join("link");
    symlink(&target, &link).expect("symlink");
    assert_eq!(LocalService::bind(&link, []).err(), Some(ServiceError::UnsafeDirectory));

    let squatted = parent.path().join("squatted");
    fs::create_dir(&squatted).expect("dir");
    fs::set_permissions(&squatted, fs::Permissions::from_mode(0o700)).expect("chmod");
    fs::write(squatted.join("ghostrace.sock"), "not a socket").expect("file");
    assert_eq!(LocalService::bind(&squatted, []).err(), Some(ServiceError::UnsafeSocketPath));
    assert_eq!(fs::read_to_string(squatted.join("ghostrace.sock")).expect("kept"), "not a socket");
}

#[test]
fn stale_socket_is_reclaimed_but_a_second_active_binder_is_refused() {
    let parent = private_parent();
    let directory = parent.path().join("svc");
    fs::create_dir(&directory).expect("dir");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("chmod");
    let socket = directory.join("ghostrace.sock");
    let stale = UnixListener::bind(&socket).expect("stale listener");
    drop(stale);
    let first = LocalService::bind(&directory, []).expect("reclaim stale socket");
    assert!(socket.exists());
    assert_eq!(LocalService::bind(&directory, []).err(), Some(ServiceError::AlreadyRunning));
    drop(first);
}

#[test]
fn an_active_unmanaged_socket_is_not_unlinked() {
    let parent = private_parent();
    let directory = parent.path().join("svc");
    fs::create_dir(&directory).expect("dir");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("chmod");
    let socket = directory.join("ghostrace.sock");
    let active = UnixListener::bind(&socket).expect("active listener");
    assert_eq!(LocalService::bind(&directory, []).err(), Some(ServiceError::AlreadyRunning));
    assert!(socket.exists(), "active listener path must not be unlinked");
    drop(active);
}

#[test]
fn stale_socket_probe_is_bounded_nonblocking_and_refuses_unknown_results() {
    let source = include_str!("../src/local_service.rs");
    assert!(source.contains("STALE_SOCKET_PROBE_BUDGET"));
    assert!(source.contains("connect_nonblocking_outcome"));
    assert!(source.contains("wait_for_connect"));
    assert!(source.contains("NonblockingConnect::TimedOut | NonblockingConnect::Unknown"));
    assert!(!source.contains("UnixStream::connect(socket_path)"));
}

#[test]
fn dropping_an_old_owner_does_not_unlink_a_replacement_inode() {
    let parent = private_parent();
    let directory = parent.path().join("svc");
    let first = LocalService::bind(&directory, []).expect("first");
    let socket = first.socket_path().to_path_buf();
    fs::remove_file(&socket).expect("simulate displacement");
    let replacement = UnixListener::bind(&socket).expect("replacement listener");
    drop(first);
    assert!(socket.exists(), "old owner must not unlink replacement");
    drop(replacement);
}

#[test]
fn the_module_opens_no_network_listener() {
    let source = include_str!("../src/local_service.rs");
    for forbidden in ["TcpListener", "TcpStream", "UdpSocket", "SocketAddr"] {
        assert!(!source.contains(forbidden), "{forbidden} appears in the service");
    }
}
