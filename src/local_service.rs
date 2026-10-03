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
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::Duration,
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
/// extension secret, or filesystem path.
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceResponse {
    Ok { request_id: Uuid, result: Value },
    Refused { error: ServiceError },
}

/// A dispatched method. It receives only requests that passed every check.
pub trait ServiceHandler {
    fn handle(&self, request: &ServiceRequest) -> Result<Value, ServiceError>;
}

/// Decode and validate the one typed browser-ingestion request. Keeping this
/// at the service boundary prevents a handler from accidentally accepting a
/// raw URL or a path-bearing navigation in a future call site.
pub fn browser_navigation_from_request(
    request: &ServiceRequest,
) -> Result<BrowserNavigationAdmission, ServiceError> {
    if request.capability != ServiceCapability::Ingest
        || request.method != BROWSER_NAVIGATION_INGEST_METHOD
    {
        return Err(ServiceError::BrowserIngestMalformed);
    }
    let admission: BrowserNavigationAdmission = serde_json::from_value(request.params.clone())
        .map_err(|_| ServiceError::BrowserIngestMalformed)?;
    admission.validate()?;
    Ok(admission)
}

/// Build a bounded service request for a canonical browser admission.
pub fn browser_navigation_request(
    service_instance: Uuid,
    request_id: Uuid,
    deadline_ms: u64,
    admission: BrowserNavigationAdmission,
) -> Result<ServiceRequest, ServiceError> {
    admission.validate()?;
    if service_instance.is_nil() || request_id.is_nil() {
        return Err(ServiceError::BrowserIngestMalformed);
    }
    if deadline_ms == 0 || Duration::from_millis(deadline_ms) > MAX_SERVICE_DEADLINE {
        return Err(ServiceError::InvalidDeadline);
    }
    let params =
        serde_json::to_value(admission).map_err(|_| ServiceError::BrowserIngestMalformed)?;
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
    admission: BrowserNavigationAdmission,
) -> Result<BrowserNavigationAck, ServiceError> {
    let service_request =
        browser_navigation_request(service_instance, request_id, deadline_ms, admission)?;
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
pub struct LocalService {
    listener: UnixListener,
    socket_path: PathBuf,
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
        match fs::symlink_metadata(&socket_path) {
            Ok(metadata)
                if metadata.file_type_is_socket()
                    && metadata.uid() == uid
                    && metadata.nlink() == 1 =>
            {
                fs::remove_file(&socket_path).map_err(|_| ServiceError::Io)?;
            }
            Ok(_) => return Err(ServiceError::UnsafeSocketPath),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ServiceError::Io),
        }
        let listener = UnixListener::bind(&socket_path).map_err(|_| ServiceError::Io)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| ServiceError::Io)?;
        Ok(Self {
            listener,
            socket_path,
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
        let (mut stream, _) = self.listener.accept().map_err(|_| ServiceError::Io)?;
        stream.set_read_timeout(Some(MAX_SERVICE_DEADLINE)).map_err(|_| ServiceError::Io)?;
        stream.set_write_timeout(Some(MAX_SERVICE_DEADLINE)).map_err(|_| ServiceError::Io)?;
        let response = match self.admit(&mut stream) {
            Ok(request) => match handler.handle(&request) {
                Ok(result) => ServiceResponse::Ok { request_id: request.request_id, result },
                Err(error) => ServiceResponse::Refused { error },
            },
            Err(error) => ServiceResponse::Refused { error },
        };
        let body = serde_json::to_vec(&response).map_err(|_| ServiceError::Io)?;
        if body.len() > MAX_SERVICE_MESSAGE_BYTES {
            return write_message(
                &mut stream,
                &serde_json::to_vec(&ServiceResponse::Refused { error: ServiceError::TooLarge })
                    .map_err(|_| ServiceError::Io)?,
            );
        }
        write_message(&mut stream, &body)?;
        finish(&mut stream);
        Ok(())
    }

    fn admit(&mut self, stream: &mut UnixStream) -> Result<ServiceRequest, ServiceError> {
        if peer_uid(stream)? != self.expected_uid {
            return Err(ServiceError::PeerRejected);
        }
        let body = read_message(stream)?;
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
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Close the write side, then discard unread input (bounded) so the kernel
/// does not reset the connection before the client reads a refusal that was
/// sent without parsing its request.
fn finish(stream: &mut UnixStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut sink = [0u8; 4096];
    let mut drained = 0usize;
    while drained <= MAX_SERVICE_MESSAGE_BYTES + 4 {
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
    let mut stream = UnixStream::connect(socket_path).map_err(|_| ServiceError::Io)?;
    let deadline = Duration::from_millis(request.deadline_ms);
    stream.set_read_timeout(Some(deadline)).map_err(|_| ServiceError::Io)?;
    stream.set_write_timeout(Some(deadline)).map_err(|_| ServiceError::Io)?;
    write_message(&mut stream, &serde_json::to_vec(request).map_err(|_| ServiceError::Malformed)?)?;
    let body = read_message(&mut stream)?;
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

fn read_message(stream: &mut UnixStream) -> Result<Vec<u8>, ServiceError> {
    let mut prefix = [0u8; 4];
    stream.read_exact(&mut prefix).map_err(|_| ServiceError::Malformed)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(ServiceError::Malformed);
    }
    if length > MAX_SERVICE_MESSAGE_BYTES {
        return Err(ServiceError::TooLarge);
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).map_err(|_| ServiceError::Malformed)?;
    Ok(body)
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
