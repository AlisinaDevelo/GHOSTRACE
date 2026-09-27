//! Authenticated local service socket.
//!
//! The GHOSTRACE local service is reachable only through a Unix-domain socket
//! in a directory owned by the current user with mode 0700; the socket itself
//! is mode 0600. No TCP listener exists. Every connection is checked for peer
//! credentials (same effective user), and every request for protocol version,
//! service instance, byte size, deadline, replay, and capability before the
//! handler sees it. Capabilities (read, export, policy, lifecycle, admin) are
//! separate and denied unless the service granted them.

#![cfg(unix)]

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

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

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
}

/// Separately grantable capabilities. Nothing is granted by default.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCapability {
    Read,
    Export,
    Policy,
    Lifecycle,
    Admin,
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
        write_message(&mut stream, &body)
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

/// Send one request and read one response.
pub fn request(
    socket_path: &Path,
    request: &ServiceRequest,
) -> Result<ServiceResponse, ServiceError> {
    let mut stream = UnixStream::connect(socket_path).map_err(|_| ServiceError::Io)?;
    let deadline = Duration::from_millis(request.deadline_ms.max(1)).min(MAX_SERVICE_DEADLINE);
    stream.set_read_timeout(Some(deadline)).map_err(|_| ServiceError::Io)?;
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
