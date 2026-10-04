//! Authenticated local service socket.
//!
//! The GHOSTRACE local service is reachable only through a Unix-domain socket
//! in a directory owned by the current user with mode 0700; the socket itself
//! is mode 0600. No TCP listener exists. Every connection is checked for peer
//! credentials (same effective user), and every request for protocol version,
//! service instance, byte size, deadline, replay, and capability before the
//! handler sees it. Capabilities (browser ingest, read, export, policy,
//! lifecycle, admin) are separate and denied unless the service granted them.

use std::{
    collections::{BTreeSet, HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::{AsRawFd, FromRawFd, RawFd},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    browser_origin::{CanonicalNavigation, NavigationHostClass, UrlShapePolicy},
    model::BrowserName,
};

/// Version of the local service protocol.
pub const LOCAL_SERVICE_PROTOCOL_VERSION: u32 = 1;
/// Largest request or response body.
pub const MAX_SERVICE_MESSAGE_BYTES: usize = 64 * 1024;
/// Longest deadline a client may request.
pub const MAX_SERVICE_DEADLINE: Duration = Duration::from_secs(30);
const STALE_SOCKET_PROBE_BUDGET: Duration = Duration::from_millis(100);
/// Request IDs remembered for replay detection per service instance.
pub const SERVICE_REPLAY_WINDOW: usize = 4096;
/// File name of the socket inside the service directory.
pub const SERVICE_SOCKET_NAME: &str = "ghostrace.sock";
/// The only browser-ingestion method exposed by the local service.
pub const BROWSER_NAVIGATION_INGEST_METHOD: &str = "browser_navigation_v1";
/// A missing-message count is metadata, not a loop bound for arbitrary work.
pub const MAX_BROWSER_INGEST_MISSING: u64 = 1_000_000;
/// The native bridge can submit one gap and one navigation in one admission.
pub const MAX_BROWSER_INGEST_SEQUENCES: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceError {
    #[error("the service directory is not a private directory owned by this user")]
    UnsafeDirectory,
    #[error("an existing path at the socket location is not a stale socket owned by this user")]
    UnsafeSocketPath,
    #[error("a local socket operation failed")]
    Io,
    #[error("the peer is not the current user")]
    PeerRejected,
    #[error("the request exceeds the size bound")]
    TooLarge,
    #[error("the request is malformed")]
    Malformed,
    #[error("the protocol version is not supported")]
    UnsupportedVersion,
    #[error("the request is addressed to another service instance")]
    WrongInstance,
    #[error("the request deadline is missing, expired, or too long")]
    InvalidDeadline,
    #[error("the request exceeded its monotonic time budget")]
    DeadlineExceeded,
    #[error("another local service owns this socket")]
    AlreadyRunning,
    #[error("the request ID was already used")]
    Replay,
    #[error("the capability is not granted")]
    CapabilityDenied,
    #[error("the browser ingestion request is malformed")]
    BrowserIngestMalformed,
    #[error("the browser ingestion request was refused")]
    BrowserIngestRefused,
}

/// Separately grantable capabilities. Nothing is granted by default.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCapability {
    /// Admission of canonicalized, metadata-only browser navigation records.
    Ingest,
    Read,
    Export,
    Policy,
    Lifecycle,
    Admin,
}

/// The typed, already-canonicalized payload accepted by the browser-ingestion
/// service method. It deliberately has no raw URL, query, fragment, userinfo,
/// extension secret, or filesystem path. This metadata is not paired-browser
/// authentication by itself; only [`BrowserNavigationRelay`] is admissible at
/// the request boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNavigationAdmission {
    pub event_id: Uuid,
    pub browser: BrowserName,
    pub navigation: CanonicalNavigation,
    pub observed_at: DateTime<Utc>,
    pub missing: u64,
}

impl BrowserNavigationAdmission {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.event_id.is_nil()
            || self.navigation.path_segment.is_some()
            || self.missing > MAX_BROWSER_INGEST_MISSING
            || !is_canonical_origin(&self.navigation)
        {
            return Err(ServiceError::BrowserIngestMalformed);
        }
        Ok(())
    }
}

/// The credential carried with one native-host-to-service browser relay.
///
/// This is a typed transport shape, not an authentication check. The native
/// bridge owns the pairing store and must verify that `mac` covers the service
/// instance, request ID, canonical admission, and this pairing/nonce/sequence
/// tuple before the service treats the admission as paired-browser input.
/// Fixed-size byte arrays keep the wire value bounded and avoid accepting
/// arbitrary credential strings at this boundary.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRelayProof {
    pub pairing_id: Uuid,
    pub client_nonce: [u8; 32],
    pub sequence: u64,
    pub mac: [u8; 32],
}

impl std::fmt::Debug for BrowserRelayProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserRelayProof")
            .field("pairing_id", &self.pairing_id)
            .field("client_nonce", &"<redacted>")
            .field("sequence", &self.sequence)
            .field("mac", &"<redacted>")
            .finish()
    }
}

impl BrowserRelayProof {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.pairing_id.is_nil() || self.sequence == 0 {
            return Err(ServiceError::BrowserIngestMalformed);
        }
        Ok(())
    }
}

/// Canonical admission plus the mandatory paired-browser relay credential.
///
/// Keeping the proof outside [`BrowserNavigationAdmission`] prevents callers
/// from accidentally treating the metadata as authenticated merely because it
/// passed origin canonicalization. A request without this wrapper is refused
/// before a service handler receives it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNavigationRelay {
    pub admission: BrowserNavigationAdmission,
    pub proof: BrowserRelayProof,
}

impl BrowserNavigationRelay {
    pub fn new(admission: BrowserNavigationAdmission, proof: BrowserRelayProof) -> Self {
        Self { admission, proof }
    }

    /// The stable delivery identity represented by this proof tuple. The
    /// native bridge still authenticates the tuple with its paired-session
    /// MAC; this deterministic relation only prevents a structurally valid
    /// request from carrying an unrelated event ID.
    pub fn expected_event_id(&self) -> Uuid {
        crate::browser_pairing::delivery_event_id(
            self.proof.pairing_id,
            &self.proof.client_nonce,
            self.proof.sequence,
        )
    }

    pub fn validate(&self) -> Result<(), ServiceError> {
        self.admission.validate()?;
        self.proof.validate()?;
        if self.admission.event_id != self.expected_event_id() {
            return Err(ServiceError::BrowserIngestMalformed);
        }
        Ok(())
    }
}

fn is_canonical_origin(navigation: &CanonicalNavigation) -> bool {
    // Private-network hosts intentionally have no retained host. Reconstruct
    // the same withheld class with a local-only sentinel rather than parsing
    // `private-network` back as an ordinary public domain.
    let origin = if navigation.host_class == NavigationHostClass::PrivateNetwork
        && navigation.host.is_none()
    {
        match navigation.port {
            Some(port) => format!("{}://localhost:{port}", navigation.scheme),
            None => format!("{}://localhost", navigation.scheme),
        }
    } else {
        navigation.origin()
    };
    CanonicalNavigation::from_url(&origin, false, UrlShapePolicy::OriginOnly)
        .map(|canonical| canonical == *navigation)
        .unwrap_or(false)
}

/// The service acknowledgement for one browser admission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNavigationAck {
    pub event_id: Uuid,
    pub ingest_sequences: Vec<u64>,
    pub missing: u64,
}

impl BrowserNavigationAck {
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.event_id.is_nil()
            || self.ingest_sequences.is_empty()
            || self.ingest_sequences.len() > MAX_BROWSER_INGEST_SEQUENCES
            || self.missing > MAX_BROWSER_INGEST_MISSING
        {
            return Err(ServiceError::BrowserIngestMalformed);
        }
        Ok(())
    }
}

/// A request as sent by a client.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRequest {
    pub protocol_version: u32,
    pub service_instance: Uuid,
    pub request_id: Uuid,
    /// Remaining time the client will wait, in milliseconds.
    pub deadline_ms: u64,
    pub capability: ServiceCapability,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl std::fmt::Debug for ServiceRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServiceRequest")
            .field("protocol_version", &self.protocol_version)
            .field("service_instance", &self.service_instance)
            .field("request_id", &self.request_id)
            .field("deadline_ms", &self.deadline_ms)
            .field("capability", &self.capability)
            .field("method", &self.method)
            // Params may carry a relay MAC. Keep generic request diagnostics
            // from becoming a credential log sink.
            .field("params", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceResponse {
    Ok { request_id: Uuid, result: Value },
    Refused { error: ServiceError },
}

/// A dispatched method. It receives only requests that passed every check.
#[derive(Clone, Copy, Debug)]
pub struct ServiceRequestContext {
    deadline: Instant,
}

impl ServiceRequestContext {
    fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    /// Remaining cooperative time for the handler. This is a snapshot and
    /// must be checked again before committing externally visible work.
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Whether the request's cooperative deadline has elapsed.
    pub fn is_expired(&self) -> bool {
        self.remaining().is_zero()
    }

    /// Absolute monotonic deadline for code that needs to pass it through.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

pub trait ServiceHandler {
    fn handle(&self, request: &ServiceRequest) -> Result<Value, ServiceError>;

    /// Deadline-aware dispatch seam. Existing handlers retain source
    /// compatibility through the default implementation. The default method
    /// cannot cancel an already-running handler; handlers that can cancel or
    /// fence their own work should override this method.
    fn handle_with_deadline(
        &self,
        request: &ServiceRequest,
        context: ServiceRequestContext,
    ) -> Result<Value, ServiceError> {
        let _ = context;
        self.handle(request)
    }
}

/// Decode and validate the one typed browser-ingestion request. Keeping this
/// at the service boundary prevents a handler from accidentally accepting a
/// raw URL or a path-bearing navigation in a future call site.
pub fn browser_navigation_from_request(
    request: &ServiceRequest,
) -> Result<BrowserNavigationRelay, ServiceError> {
    if request.capability != ServiceCapability::Ingest
        || request.method != BROWSER_NAVIGATION_INGEST_METHOD
    {
        return Err(ServiceError::BrowserIngestMalformed);
    }
    let relay: BrowserNavigationRelay = serde_json::from_value(request.params.clone())
        .map_err(|_| ServiceError::BrowserIngestMalformed)?;
    relay.validate()?;
    Ok(relay)
}

/// Build a bounded service request for a canonical browser admission and its
/// mandatory relay proof.
pub fn browser_navigation_request(
    service_instance: Uuid,
    request_id: Uuid,
    deadline_ms: u64,
    relay: BrowserNavigationRelay,
) -> Result<ServiceRequest, ServiceError> {
    relay.validate()?;
    if service_instance.is_nil() || request_id.is_nil() {
        return Err(ServiceError::BrowserIngestMalformed);
    }
    if deadline_ms == 0 || Duration::from_millis(deadline_ms) > MAX_SERVICE_DEADLINE {
        return Err(ServiceError::InvalidDeadline);
    }
    let params = serde_json::to_value(relay).map_err(|_| ServiceError::BrowserIngestMalformed)?;
    Ok(ServiceRequest {
        protocol_version: LOCAL_SERVICE_PROTOCOL_VERSION,
        service_instance,
        request_id,
        deadline_ms,
        capability: ServiceCapability::Ingest,
        method: BROWSER_NAVIGATION_INGEST_METHOD.to_owned(),
        params,
    })
}

/// Send one typed browser admission through the authenticated local service.
pub fn ingest_browser_navigation(
    socket_path: &Path,
    service_instance: Uuid,
    request_id: Uuid,
    deadline_ms: u64,
    relay: BrowserNavigationRelay,
) -> Result<BrowserNavigationAck, ServiceError> {
    let service_request =
        browser_navigation_request(service_instance, request_id, deadline_ms, relay)?;
    match request(socket_path, &service_request)? {
        ServiceResponse::Ok { request_id: response_id, result }
            if response_id == service_request.request_id =>
        {
            let acknowledgement: BrowserNavigationAck =
                serde_json::from_value(result).map_err(|_| ServiceError::BrowserIngestMalformed)?;
            acknowledgement.validate()?;
            Ok(acknowledgement)
        }
        ServiceResponse::Ok { .. } => Err(ServiceError::Malformed),
        ServiceResponse::Refused { error } => Err(error),
    }
}

/// The bound service socket and its admission state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

fn socket_identity(metadata: &fs::Metadata) -> SocketIdentity {
    SocketIdentity { device: metadata.dev(), inode: metadata.ino() }
}

pub struct LocalService {
    listener: UnixListener,
    socket_path: PathBuf,
    socket_identity: SocketIdentity,
    _owner_lock: File,
    instance: Uuid,
    granted: BTreeSet<ServiceCapability>,
    expected_uid: u32,
    seen: HashSet<Uuid>,
    seen_order: VecDeque<Uuid>,
}

impl LocalService {
    /// Bind the socket in `directory`, creating it with mode 0700 if absent.
    /// An existing directory must be a real directory owned by this user with
    /// no group or other access; symbolic links are never followed.
    pub fn bind<I>(directory: &Path, granted: I) -> Result<Self, ServiceError>
    where
        I: IntoIterator<Item = ServiceCapability>,
    {
        let uid = current_uid();
        prepare_directory(directory, uid)?;
        let socket_path = directory.join(SERVICE_SOCKET_NAME);
        let owner_lock = open_owner_lock(&socket_path, uid)?;
        acquire_owner_lock(&owner_lock)?;
        remove_owned_stale_socket(&socket_path, uid)?;
        let listener = UnixListener::bind(&socket_path).map_err(|_| ServiceError::Io)?;
        // A Unix socket descriptor's fstat identity is not the identity of the
        // pathname entry on macOS. Capture the filesystem identity from the
        // bound pathname while the owner lock is held, then re-check it after
        // chmod before retaining it for the lifetime/drop guard.
        let metadata = fs::symlink_metadata(&socket_path).map_err(|_| ServiceError::Io)?;
        if !metadata.file_type_is_socket() || metadata.uid() != uid || metadata.nlink() != 1 {
            return Err(ServiceError::UnsafeSocketPath);
        }
        let bound_identity = socket_identity(&metadata);
        listener.set_nonblocking(true).map_err(|_| ServiceError::Io)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| ServiceError::Io)?;
        let metadata = fs::symlink_metadata(&socket_path).map_err(|_| ServiceError::Io)?;
        if !metadata.file_type_is_socket()
            || metadata.uid() != uid
            || metadata.nlink() != 1
            || socket_identity(&metadata) != bound_identity
        {
            return Err(ServiceError::UnsafeSocketPath);
        }
        Ok(Self {
            listener,
            socket_path,
            socket_identity: bound_identity,
            _owner_lock: owner_lock,
            instance: Uuid::new_v4(),
            granted: granted.into_iter().collect(),
            expected_uid: uid,
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// The instance a client must address; published to local clients out of
    /// band (for example in a 0600 file beside the socket).
    pub fn instance(&self) -> Uuid {
        self.instance
    }

    #[doc(hidden)]
    pub fn set_expected_uid_for_test(&mut self, uid: u32) {
        self.expected_uid = uid;
    }

    /// Accept one connection and answer one request.
    pub fn serve_one<H: ServiceHandler>(&mut self, handler: &H) -> Result<(), ServiceError> {
        let ingress_deadline = Instant::now() + MAX_SERVICE_DEADLINE;
        let mut stream = accept_until(&self.listener, ingress_deadline)?;
        set_nonblocking(&stream)?;
        let (response, response_deadline) = match self.admit(&mut stream, ingress_deadline) {
            Ok(request) => {
                let budget = Duration::from_millis(request.deadline_ms).min(MAX_SERVICE_DEADLINE);
                let context =
                    ServiceRequestContext::new((Instant::now() + budget).min(ingress_deadline));
                let result = if context.is_expired() {
                    Err(ServiceError::DeadlineExceeded)
                } else {
                    handler.handle_with_deadline(&request, context)
                };
                let response = if context.is_expired() {
                    ServiceResponse::Refused { error: ServiceError::DeadlineExceeded }
                } else {
                    match result {
                        Ok(result) => {
                            ServiceResponse::Ok { request_id: request.request_id, result }
                        }
                        Err(error) => ServiceResponse::Refused { error },
                    }
                };
                (response, context.deadline())
            }
            Err(error) => (ServiceResponse::Refused { error }, ingress_deadline),
        };
        let body = serde_json::to_vec(&response).map_err(|_| ServiceError::Io)?;
        let body = if body.len() > MAX_SERVICE_MESSAGE_BYTES {
            serde_json::to_vec(&ServiceResponse::Refused { error: ServiceError::TooLarge })
                .map_err(|_| ServiceError::Io)?
        } else {
            body
        };
        let result = write_message_until(&mut stream, &body, response_deadline);
        finish_until(&mut stream, response_deadline);
        result
    }

    fn admit(
        &mut self,
        stream: &mut UnixStream,
        deadline: Instant,
    ) -> Result<ServiceRequest, ServiceError> {
        if peer_uid(stream)? != self.expected_uid {
            return Err(ServiceError::PeerRejected);
        }
        let body = read_message_until(stream, deadline)?;
        let request: ServiceRequest =
            serde_json::from_slice(&body).map_err(|_| ServiceError::Malformed)?;
        if request.protocol_version != LOCAL_SERVICE_PROTOCOL_VERSION {
            return Err(ServiceError::UnsupportedVersion);
        }
        if request.service_instance != self.instance {
            return Err(ServiceError::WrongInstance);
        }
        if request.deadline_ms == 0
            || Duration::from_millis(request.deadline_ms) > MAX_SERVICE_DEADLINE
        {
            return Err(ServiceError::InvalidDeadline);
        }
        if request.request_id.is_nil() {
            return Err(ServiceError::Malformed);
        }
        if request.method.is_empty() || request.method.len() > 64 {
            return Err(ServiceError::Malformed);
        }
        if !self.seen.insert(request.request_id) {
            return Err(ServiceError::Replay);
        }
        self.seen_order.push_back(request.request_id);
        if self.seen_order.len() > SERVICE_REPLAY_WINDOW {
            if let Some(oldest) = self.seen_order.pop_front() {
                self.seen.remove(&oldest);
            }
        }
        if !self.granted.contains(&request.capability) {
            return Err(ServiceError::CapabilityDenied);
        }
        Ok(request)
    }
}

impl Drop for LocalService {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.socket_path) else {
            return;
        };
        if metadata.file_type_is_socket()
            && metadata.uid() == current_uid()
            && metadata.nlink() == 1
            && socket_identity(&metadata) == self.socket_identity
        {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

/// Close the write side, then discard unread input (bounded) so the kernel
/// does not reset the connection before the client reads a refusal that was
/// sent without parsing its request.
fn finish_until(stream: &mut UnixStream, deadline: Instant) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let drain_deadline = (Instant::now() + Duration::from_millis(200)).min(deadline);
    let mut sink = [0u8; 4096];
    let mut drained = 0usize;
    while drained <= MAX_SERVICE_MESSAGE_BYTES + 4 {
        if wait_for_fd(stream.as_raw_fd(), libc::POLLIN, drain_deadline).is_err() {
            break;
        }
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(read) => drained += read,
        }
    }
}

/// Send one request and read one response.
pub fn request(
    socket_path: &Path,
    request: &ServiceRequest,
) -> Result<ServiceResponse, ServiceError> {
    if request.deadline_ms == 0 || Duration::from_millis(request.deadline_ms) > MAX_SERVICE_DEADLINE
    {
        return Err(ServiceError::InvalidDeadline);
    }
    let deadline = Instant::now() + Duration::from_millis(request.deadline_ms);
    let body = serde_json::to_vec(request).map_err(|_| ServiceError::Malformed)?;
    let mut stream = connect_nonblocking(socket_path, deadline)?;
    write_message_until(&mut stream, &body, deadline)?;
    let body = read_message_until(&mut stream, deadline)?;
    if Instant::now() >= deadline {
        return Err(ServiceError::DeadlineExceeded);
    }
    serde_json::from_slice(&body).map_err(|_| ServiceError::Malformed)
}

/// Write a length-prefixed (big-endian u32) message.
pub fn write_message(stream: &mut UnixStream, body: &[u8]) -> Result<(), ServiceError> {
    if body.len() > MAX_SERVICE_MESSAGE_BYTES {
        return Err(ServiceError::TooLarge);
    }
    stream.write_all(&(body.len() as u32).to_be_bytes()).map_err(|_| ServiceError::Io)?;
    stream.write_all(body).map_err(|_| ServiceError::Io)
}

fn write_message_until(
    stream: &mut UnixStream,
    body: &[u8],
    deadline: Instant,
) -> Result<(), ServiceError> {
    if body.len() > MAX_SERVICE_MESSAGE_BYTES {
        return Err(ServiceError::TooLarge);
    }
    write_all_until(stream, &(body.len() as u32).to_be_bytes(), deadline)?;
    write_all_until(stream, body, deadline)
}

fn read_message_until(stream: &mut UnixStream, deadline: Instant) -> Result<Vec<u8>, ServiceError> {
    let mut prefix = [0u8; 4];
    read_exact_until(stream, &mut prefix, deadline).map_err(|error| {
        if error == ServiceError::DeadlineExceeded {
            error
        } else {
            ServiceError::Malformed
        }
    })?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(ServiceError::Malformed);
    }
    if length > MAX_SERVICE_MESSAGE_BYTES {
        return Err(ServiceError::TooLarge);
    }
    let mut body = vec![0u8; length];
    read_exact_until(stream, &mut body, deadline).map_err(|error| {
        if error == ServiceError::DeadlineExceeded {
            error
        } else {
            ServiceError::Malformed
        }
    })?;
    Ok(body)
}

fn write_all_until(
    stream: &mut UnixStream,
    mut bytes: &[u8],
    deadline: Instant,
) -> Result<(), ServiceError> {
    while !bytes.is_empty() {
        wait_for_fd(stream.as_raw_fd(), libc::POLLOUT, deadline)?;
        match stream.write(bytes) {
            Ok(0) => return Err(ServiceError::Io),
            Ok(written) => bytes = &bytes[written..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => return Err(ServiceError::Io),
        }
    }
    Ok(())
}

fn read_exact_until(
    stream: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), ServiceError> {
    while !bytes.is_empty() {
        wait_for_fd(stream.as_raw_fd(), libc::POLLIN, deadline)?;
        match stream.read(bytes) {
            Ok(0) => return Err(ServiceError::Io),
            Ok(read) => {
                let (_, remainder) = bytes.split_at_mut(read);
                bytes = remainder;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => return Err(ServiceError::Io),
        }
    }
    Ok(())
}

fn accept_until(listener: &UnixListener, deadline: Instant) -> Result<UnixStream, ServiceError> {
    loop {
        wait_for_fd(listener.as_raw_fd(), libc::POLLIN, deadline)?;
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => return Err(ServiceError::Io),
        }
    }
}

fn set_nonblocking(stream: &UnixStream) -> Result<(), ServiceError> {
    set_fd_nonblocking(stream.as_raw_fd())
}

fn set_fd_nonblocking(fd: RawFd) -> Result<(), ServiceError> {
    // SAFETY: fcntl operates on this live descriptor and does not retain any
    // pointer supplied by the caller.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(ServiceError::Io);
    }
    // SAFETY: the flags are read from the same descriptor immediately above.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(ServiceError::Io);
    }
    // SAFETY: setting close-on-exec on the same descriptor has no pointer
    // preconditions and prevents inherited service connections.
    let descriptor_flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if descriptor_flags < 0
        || unsafe { libc::fcntl(fd, libc::F_SETFD, descriptor_flags | libc::FD_CLOEXEC) } < 0
    {
        return Err(ServiceError::Io);
    }
    Ok(())
}

fn wait_for_fd(fd: RawFd, events: libc::c_short, deadline: Instant) -> Result<(), ServiceError> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ServiceError::DeadlineExceeded);
        }
        let timeout = remaining.as_millis().min(i32::MAX as u128).max(1) as libc::c_int;
        let mut pollfd = libc::pollfd { fd, events, revents: 0 };
        // SAFETY: poll receives a valid pointer to one initialized pollfd.
        let result = unsafe { libc::poll(&mut pollfd, 1, timeout) };
        if result > 0 {
            if pollfd.revents & events != 0 {
                return Ok(());
            }
            if pollfd.revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
                return Err(ServiceError::Io);
            }
            continue;
        }
        if result == 0 {
            continue;
        }
        if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(ServiceError::Io);
    }
}

#[derive(Debug)]
enum NonblockingConnect {
    Connected(UnixStream),
    Refused,
    Unknown,
    TimedOut,
}

fn connect_nonblocking(socket_path: &Path, deadline: Instant) -> Result<UnixStream, ServiceError> {
    match connect_nonblocking_outcome(socket_path, deadline)? {
        NonblockingConnect::Connected(stream) => Ok(stream),
        NonblockingConnect::TimedOut => Err(ServiceError::DeadlineExceeded),
        NonblockingConnect::Refused | NonblockingConnect::Unknown => Err(ServiceError::Io),
    }
}

fn connect_nonblocking_outcome(
    socket_path: &Path,
    deadline: Instant,
) -> Result<NonblockingConnect, ServiceError> {
    let (address, address_length) = unix_socket_address(socket_path)?;
    // SAFETY: socket has no Rust aliasing requirements and returns an owned
    // descriptor on success. All error paths below close it before returning.
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(ServiceError::Io);
    }
    if set_fd_nonblocking(fd).is_err() {
        // SAFETY: fd was returned by socket and has not been transferred.
        unsafe { libc::close(fd) };
        return Err(ServiceError::Io);
    }
    // SAFETY: address points to a fully initialized sockaddr_un whose length
    // was computed by unix_socket_address.
    let result = unsafe {
        libc::connect(
            fd,
            (&address as *const libc::sockaddr_un).cast::<libc::sockaddr>(),
            address_length,
        )
    };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        let in_progress = matches!(error.raw_os_error(), Some(code)
            if code == libc::EINPROGRESS
                || code == libc::EALREADY
                || code == libc::EINTR
                || code == libc::EWOULDBLOCK);
        if !in_progress {
            // SAFETY: fd remains owned by this function.
            unsafe { libc::close(fd) };
            return Ok(if error.raw_os_error() == Some(libc::ECONNREFUSED) {
                NonblockingConnect::Refused
            } else {
                NonblockingConnect::Unknown
            });
        }
        match wait_for_connect(fd, deadline) {
            Ok(()) => {}
            Err(ServiceError::DeadlineExceeded) => {
                // SAFETY: fd remains owned by this function.
                unsafe { libc::close(fd) };
                return Ok(NonblockingConnect::TimedOut);
            }
            Err(_) => {
                // SAFETY: fd remains owned by this function.
                unsafe { libc::close(fd) };
                return Ok(NonblockingConnect::Unknown);
            }
        }
    }
    let mut socket_error: libc::c_int = 0;
    let mut socket_error_length = std::mem::size_of_val(&socket_error) as libc::socklen_t;
    // SAFETY: getsockopt writes one c_int into the valid output buffer.
    let option_result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            (&mut socket_error as *mut libc::c_int).cast(),
            &mut socket_error_length,
        )
    };
    if option_result < 0 || socket_error != 0 {
        // SAFETY: fd remains owned by this function.
        unsafe { libc::close(fd) };
        if option_result < 0 {
            return Ok(NonblockingConnect::Unknown);
        }
        return Ok(if socket_error == libc::ECONNREFUSED {
            NonblockingConnect::Refused
        } else {
            NonblockingConnect::Unknown
        });
    }
    // SAFETY: the successful connect transfers fd ownership to UnixStream.
    Ok(NonblockingConnect::Connected(unsafe { UnixStream::from_raw_fd(fd) }))
}

fn wait_for_connect(fd: RawFd, deadline: Instant) -> Result<(), ServiceError> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ServiceError::DeadlineExceeded);
        }
        let timeout = remaining.as_millis().min(i32::MAX as u128).max(1) as libc::c_int;
        let mut pollfd =
            libc::pollfd { fd, events: libc::POLLOUT | libc::POLLERR | libc::POLLHUP, revents: 0 };
        // SAFETY: poll receives a valid pointer to one initialized pollfd.
        let result = unsafe { libc::poll(&mut pollfd, 1, timeout) };
        if result > 0 {
            if pollfd.revents & libc::POLLNVAL != 0 {
                return Err(ServiceError::Io);
            }
            // SO_ERROR below distinguishes a refused stale socket from other
            // failures. POLLERR/POLLHUP are therefore wakeups, not proof of
            // an active or stale owner by themselves.
            if pollfd.revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0 {
                return Ok(());
            }
            continue;
        }
        if result == 0 {
            continue;
        }
        if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(ServiceError::Io);
    }
}

fn unix_socket_address(
    socket_path: &Path,
) -> Result<(libc::sockaddr_un, libc::socklen_t), ServiceError> {
    let bytes = socket_path.as_os_str().as_bytes();
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(ServiceError::Io);
    }
    address.sun_family = libc::AF_UNIX as _;
    // SAFETY: bytes and sun_path are non-overlapping, and the length was
    // checked above to leave a trailing NUL in the zeroed address.
    unsafe {
        ptr::copy_nonoverlapping(
            bytes.as_ptr().cast::<libc::c_char>(),
            address.sun_path.as_mut_ptr(),
            bytes.len(),
        );
    }
    let address_length =
        (std::mem::size_of_val(&address.sun_family) + bytes.len() + 1) as libc::socklen_t;
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        address.sun_len = address_length as _;
    }
    Ok((address, address_length))
}

fn owner_lock_path(socket_path: &Path) -> PathBuf {
    socket_path.with_file_name(format!("{SERVICE_SOCKET_NAME}.lock"))
}

fn open_owner_lock(socket_path: &Path, uid: u32) -> Result<File, ServiceError> {
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(owner_lock_path(socket_path))
        .map_err(|_| ServiceError::UnsafeSocketPath)?;
    let metadata = lock.metadata().map_err(|_| ServiceError::UnsafeSocketPath)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(ServiceError::UnsafeSocketPath);
    }
    Ok(lock)
}

fn acquire_owner_lock(lock: &File) -> Result<(), ServiceError> {
    // SAFETY: flock acts on the live descriptor and does not retain pointers.
    let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(());
    }
    if std::io::Error::last_os_error().raw_os_error() == Some(libc::EWOULDBLOCK) {
        Err(ServiceError::AlreadyRunning)
    } else {
        Err(ServiceError::Io)
    }
}

fn remove_owned_stale_socket(socket_path: &Path, uid: u32) -> Result<(), ServiceError> {
    // The lock serializes cooperating binders. The liveness probe and identity
    // recheck cover stale sockets left by older versions. An uncooperative
    // same-UID process can still race pathname operations; the lock is not a
    // kernel-level ownership proof for that actor, so unknown probes are
    // refused rather than unlinked.
    for _ in 0..4 {
        let metadata = match fs::symlink_metadata(socket_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(ServiceError::Io),
        };
        if !metadata.file_type_is_socket() || metadata.uid() != uid || metadata.nlink() != 1 {
            return Err(ServiceError::UnsafeSocketPath);
        }
        let identity = socket_identity(&metadata);
        match connect_nonblocking_outcome(socket_path, Instant::now() + STALE_SOCKET_PROBE_BUDGET)?
        {
            NonblockingConnect::Connected(stream) => {
                drop(stream);
                return Err(ServiceError::AlreadyRunning);
            }
            NonblockingConnect::Refused => {}
            NonblockingConnect::TimedOut | NonblockingConnect::Unknown => {
                // A timeout or an unclassified connect failure does not prove
                // that the pathname is stale. Never unlink it in that case.
                return Err(ServiceError::UnsafeSocketPath);
            }
        }
        let current = match fs::symlink_metadata(socket_path) {
            Ok(current) => current,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(ServiceError::Io),
        };
        if !current.file_type_is_socket()
            || current.uid() != uid
            || current.nlink() != 1
            || socket_identity(&current) != identity
        {
            continue;
        }
        match fs::remove_file(socket_path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(ServiceError::Io),
        }
    }
    Err(ServiceError::AlreadyRunning)
}

fn prepare_directory(directory: &Path, uid: u32) -> Result<(), ServiceError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.uid() != uid
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(ServiceError::UnsafeDirectory);
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(directory).map_err(|_| ServiceError::Io)?;
            let metadata = fs::symlink_metadata(directory).map_err(|_| ServiceError::Io)?;
            if !metadata.is_dir() || metadata.uid() != uid {
                return Err(ServiceError::UnsafeDirectory);
            }
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| ServiceError::Io)
        }
        Err(_) => Err(ServiceError::Io),
    }
}

trait SocketType {
    fn file_type_is_socket(&self) -> bool;
}

impl SocketType for fs::Metadata {
    fn file_type_is_socket(&self) -> bool {
        use std::os::unix::fs::FileTypeExt;
        self.file_type().is_socket()
    }
}

fn current_uid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn peer_uid(stream: &UnixStream) -> Result<u32, ServiceError> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: the descriptor is a connected Unix socket and both out
    // pointers are valid for writes.
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result == 0 {
        Ok(uid)
    } else {
        Err(ServiceError::PeerRejected)
    }
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Result<u32, ServiceError> {
    let mut credentials = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: SO_PEERCRED writes a ucred into a buffer of the given length.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if result == 0 {
        Ok(credentials.uid)
    } else {
        Err(ServiceError::PeerRejected)
    }
}
