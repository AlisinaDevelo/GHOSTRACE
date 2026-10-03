//! Executable native-host bridge and durable browser pairing boundary.
//!
//! The browser-facing process owns only two capabilities: it authenticates an
//! explicitly approved extension and relays an already-minimized navigation to
//! the LocalService admission method. The service-side handler owns the
//! existing journal writer. Pairing state is kept in a private, authenticated
//! sidecar with a key that is independent from the journal key. The extension
//! never supplies a path, command, policy, or key to this module.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    os::unix::io::{AsRawFd, RawFd},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    browser_origin::{NavigationRefusal, UrlShapePolicy},
    browser_pairing::{BrowserEventClass, PairingError, PairingRecord, PairingRequest},
    crypto::CiphertextEnvelope,
    local_service::{
        browser_navigation_from_request, ingest_browser_navigation, BrowserNavigationAck,
        BrowserNavigationAdmission, ServiceCapability, ServiceError, ServiceHandler,
        ServiceRequest, BROWSER_NAVIGATION_INGEST_METHOD, MAX_BROWSER_INGEST_SEQUENCES,
    },
    model::{
        BrowserName, BrowserNavigationPayload, EventEnvelope, EventKind, EventPayload, EventSource,
        Evidence, GapPayload, IngestionOrigin, ReasonCode,
    },
    native_host::{HostMessage, NativeHostSession, NativeSessionError},
    native_messaging::{
        encode_frame, parse_message, ExtensionMessage, FrameDecoder, NativeMessagingError,
        NATIVE_SESSION_IDLE_TIMEOUT,
    },
    policy::PolicyProfile,
    writer::{Writer, WriterConfig, WriterOutcome},
};

/// Directory name beneath a user-selected GHOSTRACE home.
pub const NATIVE_HOST_STORE_DIR: &str = "native-host";
/// Independent key used to encrypt persisted pairing records.
pub const PAIRING_KEY_FILE: &str = "pairing.key";
/// Authenticated ciphertext containing pairing records.
pub const PAIRING_STATE_FILE: &str = "pairings.enc";
/// Lock serializing all pairing-store read-modify-write operations.
pub const PAIRING_LOCK_FILE: &str = "pairings.lock";
/// Maximum time a pairing operation waits for another process's lock holder.
pub const PAIRING_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
/// Endpoint receipt consumed by the browser-launched binary. The LocalService
/// owner updates this file whenever its per-start instance changes.
pub const NATIVE_SERVICE_ENDPOINT_FILE: &str = "service.endpoint";
pub const NATIVE_SERVICE_ENDPOINT_SCHEMA_VERSION: u32 = 1;
pub const MAX_NATIVE_SERVICE_ENDPOINT_BYTES: usize = 4096;
/// The pairing store has its own domain and never reuses the journal key.
pub const PAIRING_STORE_DOMAIN: &[u8] = b"ghostrace:browser-pairings:v1\0";
pub const PAIRING_STORE_SCHEMA_VERSION: u32 = 1;
pub const MAX_PAIRING_RECORDS: usize = 64;
pub const MAX_PAIRING_STORE_BYTES: usize = 1024 * 1024;
pub const MAX_NATIVE_HOST_READ_CHUNK: usize = 16 * 1024;
/// Maximum time a native host write may wait for a stalled browser peer.
pub const NATIVE_HOST_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

const SUPPORTED_BROWSER_CHANNELS: [&str; 4] = ["chrome", "chrome-beta", "chromium", "edge"];
const RETAINED_ORIGIN: &str = "origin";
const PRIVATE_CONTEXT_REFUSAL: &str = "refuse_private_context";
const CHROMIUM_EXTENSION_ORIGIN_PREFIX: &str = "chrome-extension://";

/// Fixed, path-free errors for a native host boundary. Untrusted extension
/// input must never be copied into an error returned by this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum NativeBridgeError {
    #[error("native bridge storage is unavailable")]
    Storage,
    #[error("native bridge pairing state is busy")]
    PairingBusy,
    #[error("pairing request is not supported")]
    PairingRequestInvalid,
    #[error("native session refused")]
    Session(NativeSessionError),
    #[error("native bridge event class is not approved")]
    EventClassNotApproved,
    #[error("native bridge journal admission refused")]
    Journal,
    #[error("native host input/output failed")]
    Io,
    #[error("native host framing failed")]
    Framing,
    #[error("native host response serialization failed")]
    Serialization,
    #[error("native bridge state is invalid")]
    State,
}

impl NativeBridgeError {
    /// Fixed refusal code safe to send over the extension channel.
    pub fn code(self) -> &'static str {
        match self {
            Self::Storage | Self::State => "storage_error",
            Self::PairingBusy => "pairing_busy",
            Self::PairingRequestInvalid | Self::EventClassNotApproved => "not_authorized",
            Self::Session(error) => error.code(),
            Self::Journal => "journal_error",
            Self::Io | Self::Framing | Self::Serialization => "protocol_error",
        }
    }
}

/// A secret-free view suitable for a list command or approval receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PairingView {
    pub pairing_id: Uuid,
    pub request: PairingRequest,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked: bool,
}

/// The only out-of-band data needed by a browser-launched host. It contains no
/// journal path, journal key, credential, URL, or browser payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeServiceEndpoint {
    pub schema_version: u32,
    pub socket_path: PathBuf,
    pub service_instance: Uuid,
}

impl NativeServiceEndpoint {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        service_instance: Uuid,
    ) -> Result<Self, NativeBridgeError> {
        let endpoint = Self {
            schema_version: NATIVE_SERVICE_ENDPOINT_SCHEMA_VERSION,
            socket_path: socket_path.into(),
            service_instance,
        };
        endpoint.validate()?;
        Ok(endpoint)
    }

    pub fn validate(&self) -> Result<(), NativeBridgeError> {
        if self.schema_version != NATIVE_SERVICE_ENDPOINT_SCHEMA_VERSION
            || self.service_instance.is_nil()
            || !self.socket_path.is_absolute()
            || self.socket_path.as_os_str().len() > 1024
            || self.socket_path.to_str().is_none_or(|path| path.chars().any(char::is_control))
        {
            return Err(NativeBridgeError::State);
        }
        Ok(())
    }
}

/// Publish a fresh LocalService endpoint under the private native-host store.
/// The service process owns the socket and instance; this helper only writes
/// its bounded discovery receipt for the next browser-launched host.
pub fn write_native_service_endpoint(
    home: impl AsRef<Path>,
    socket_path: impl Into<PathBuf>,
    service_instance: Uuid,
) -> Result<(), NativeBridgeError> {
    let directory = home.as_ref().join(NATIVE_HOST_STORE_DIR);
    ensure_private_directory(&directory)?;
    let endpoint = NativeServiceEndpoint::new(socket_path, service_instance)?;
    let encoded = serde_json::to_vec(&endpoint).map_err(|_| NativeBridgeError::Storage)?;
    if encoded.len() > MAX_NATIVE_SERVICE_ENDPOINT_BYTES {
        return Err(NativeBridgeError::Storage);
    }
    write_private_atomic(&directory.join(NATIVE_SERVICE_ENDPOINT_FILE), &encoded)
}

/// Read the service endpoint before consuming browser stdin.
pub fn read_native_service_endpoint(
    home: impl AsRef<Path>,
) -> Result<NativeServiceEndpoint, NativeBridgeError> {
    let path = home.as_ref().join(NATIVE_HOST_STORE_DIR).join(NATIVE_SERVICE_ENDPOINT_FILE);
    let encoded = read_private_file(&path)?.ok_or(NativeBridgeError::Storage)?;
    if encoded.len() > MAX_NATIVE_SERVICE_ENDPOINT_BYTES {
        return Err(NativeBridgeError::Storage);
    }
    let endpoint: NativeServiceEndpoint =
        serde_json::from_slice(&encoded).map_err(|_| NativeBridgeError::Storage)?;
    endpoint.validate()?;
    Ok(endpoint)
}

impl From<&PairingRecord> for PairingView {
    fn from(record: &PairingRecord) -> Self {
        Self {
            pairing_id: record.pairing_id,
            request: record.request.clone(),
            approved_at: record.approved_at,
            expires_at: record.expires_at,
            revoked: record.revoked,
        }
    }
}

/// The one-time delivery produced by an explicit pairing approval. The secret
/// is never part of [`PairingView`] or unencrypted CLI/list output; the host
/// retains its copy only inside the authenticated ciphertext.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct PairingApproval {
    pub view: PairingView,
    pub extension_secret: String,
}

impl std::fmt::Debug for PairingApproval {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingApproval")
            .field("view", &self.view)
            .field("extension_secret", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedPairingState {
    schema_version: u32,
    records: Vec<PairingRecord>,
}

/// Private, authenticated pairing storage. The key file and encrypted state
/// are intentionally separate from the journal SQLite file and its key.
pub struct PairingStore {
    directory: PathBuf,
    key: [u8; 32],
}

impl std::fmt::Debug for PairingStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingStore")
            .field("directory", &self.directory)
            .field("key", &"<redacted>")
            .finish()
    }
}

impl PairingStore {
    /// Open a private directory containing `pairing.key` and `pairings.enc`.
    /// The caller chooses this directory explicitly; extension messages cannot.
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, NativeBridgeError> {
        let directory = directory.as_ref().to_path_buf();
        ensure_private_directory(&directory)?;
        ensure_private_lock(&directory.join(PAIRING_LOCK_FILE))?;
        let open_lock = PairingFileLock::open(&directory.join(PAIRING_LOCK_FILE))?;
        open_lock.exclusive()?;
        let key_path = directory.join(PAIRING_KEY_FILE);
        let state_path = directory.join(PAIRING_STATE_FILE);
        let key = match read_private_file(&key_path)? {
            Some(bytes) => decode_pairing_key(&bytes)?,
            None => {
                if private_metadata_exists(&state_path)? {
                    return Err(NativeBridgeError::Storage);
                }
                create_pairing_key(&key_path)?
            }
        };
        let store = Self { directory, key };
        // Opening validates the authenticated record so a restart cannot use a
        // partially written, copied, or tampered approval set.
        let _ = store.records()?;
        Ok(store)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Approve a request after the caller has shown it to and received consent
    /// from the user. The extension secret is returned once for delivery.
    pub fn approve(
        &self,
        request: PairingRequest,
        now: DateTime<Utc>,
    ) -> Result<PairingApproval, NativeBridgeError> {
        validate_pairing_request(&request)?;
        self.with_exclusive_lock(|| {
            let mut records = self.records()?;
            if records.len() >= MAX_PAIRING_RECORDS {
                return Err(NativeBridgeError::Storage);
            }
            let record =
                PairingRecord::approve(request, now).map_err(|_| NativeBridgeError::Storage)?;
            let approval = PairingApproval {
                view: PairingView::from(&record),
                extension_secret: crate::native_host::encode_hex(&record.secret_for_extension()),
            };
            records.push(record);
            self.write_records(&records)?;
            Ok(approval)
        })
    }

    pub fn list(&self) -> Result<Vec<PairingView>, NativeBridgeError> {
        self.with_shared_lock(|| Ok(self.records()?.iter().map(PairingView::from).collect()))
    }

    pub fn get(&self, pairing_id: Uuid) -> Result<Option<PairingRecord>, NativeBridgeError> {
        self.with_shared_lock(|| {
            Ok(self.records()?.into_iter().find(|record| record.pairing_id == pairing_id))
        })
    }

    /// Admit one bridge frame under a shared lock that remains held until the
    /// caller drops the lease. In particular, an accepted navigation keeps
    /// the lease through the LocalService/journal receipt, so a concurrent
    /// revoke cannot return success before that receipt is complete.
    fn admission(&self) -> Result<PairingAdmissionLease, NativeBridgeError> {
        let lock = PairingFileLock::open(&self.lock_path())?;
        lock.shared()?;
        let records = self.records()?;
        Ok(PairingAdmissionLease { _lock: lock, records })
    }

    /// Revoke one pairing. `Ok(true)` is a durable tombstone receipt after all
    /// earlier admission leases have ended; `PairingBusy` means no revocation
    /// receipt was issued and the caller must retry or inspect state. The
    /// record remains as a bounded tombstone so a replayed pairing ID cannot
    /// become valid after restart.
    pub fn revoke(&self, pairing_id: Uuid) -> Result<bool, NativeBridgeError> {
        self.with_exclusive_lock(|| {
            let mut records = self.records()?;
            let Some(record) = records.iter_mut().find(|record| record.pairing_id == pairing_id)
            else {
                return Ok(false);
            };
            if record.revoked {
                return Ok(false);
            }
            record.revoke();
            self.write_records(&records)?;
            Ok(true)
        })
    }

    fn state_path(&self) -> PathBuf {
        self.directory.join(PAIRING_STATE_FILE)
    }

    fn lock_path(&self) -> PathBuf {
        self.directory.join(PAIRING_LOCK_FILE)
    }

    fn with_exclusive_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, NativeBridgeError>,
    ) -> Result<T, NativeBridgeError> {
        let lock = PairingFileLock::open(&self.lock_path())?;
        lock.exclusive()?;
        operation()
    }

    fn with_shared_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, NativeBridgeError>,
    ) -> Result<T, NativeBridgeError> {
        let lock = PairingFileLock::open(&self.lock_path())?;
        lock.shared()?;
        operation()
    }

    fn records(&self) -> Result<Vec<PairingRecord>, NativeBridgeError> {
        let Some(encoded) = read_private_file(&self.state_path())? else {
            return Ok(Vec::new());
        };
        if encoded.len() > MAX_PAIRING_STORE_BYTES {
            return Err(NativeBridgeError::Storage);
        }
        let envelope =
            CiphertextEnvelope::decode(&encoded).map_err(|_| NativeBridgeError::Storage)?;
        if envelope.key_generation != 1 {
            return Err(NativeBridgeError::Storage);
        }
        let plaintext = envelope
            .decrypt_with_key(self.key, PAIRING_STORE_DOMAIN)
            .map_err(|_| NativeBridgeError::Storage)?;
        if plaintext.len() > MAX_PAIRING_STORE_BYTES {
            return Err(NativeBridgeError::Storage);
        }
        let state: PersistedPairingState =
            serde_json::from_slice(&plaintext).map_err(|_| NativeBridgeError::Storage)?;
        if state.schema_version != PAIRING_STORE_SCHEMA_VERSION
            || state.records.len() > MAX_PAIRING_RECORDS
        {
            return Err(NativeBridgeError::Storage);
        }
        let mut ids = BTreeSet::new();
        for record in &state.records {
            validate_pairing_request(&record.request)?;
            if record.pairing_id.is_nil()
                || record.expires_at <= record.approved_at
                || record.secret_for_extension() == [0; 32]
                || !ids.insert(record.pairing_id)
            {
                return Err(NativeBridgeError::Storage);
            }
        }
        Ok(state.records)
    }

    fn write_records(&self, records: &[PairingRecord]) -> Result<(), NativeBridgeError> {
        if records.len() > MAX_PAIRING_RECORDS {
            return Err(NativeBridgeError::Storage);
        }
        let plaintext = serde_json::to_vec(&PersistedPairingState {
            schema_version: PAIRING_STORE_SCHEMA_VERSION,
            records: records.to_owned(),
        })
        .map_err(|_| NativeBridgeError::Storage)?;
        let envelope =
            CiphertextEnvelope::encrypt_with_key(1, self.key, PAIRING_STORE_DOMAIN, &plaintext)
                .map_err(|_| NativeBridgeError::Storage)?;
        let encoded = envelope.encode().map_err(|_| NativeBridgeError::Storage)?;
        if encoded.len() > MAX_PAIRING_STORE_BYTES {
            return Err(NativeBridgeError::Storage);
        }
        write_private_atomic(&self.state_path(), &encoded)
    }
}

struct PairingAdmissionLease {
    _lock: PairingFileLock,
    records: Vec<PairingRecord>,
}

impl PairingAdmissionLease {
    fn get(&self, pairing_id: Uuid) -> Option<PairingRecord> {
        self.records.iter().find(|record| record.pairing_id == pairing_id).cloned()
    }
}

/// A process-level advisory lock. The lock file is separate from the
/// authenticated state so an interrupted write cannot leave a half-written
/// record and concurrent approve/revoke operations cannot lose updates.
struct PairingFileLock {
    file: File,
}

impl PairingFileLock {
    fn open(path: &Path) -> Result<Self, NativeBridgeError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| NativeBridgeError::Storage)?;
        let metadata = file.metadata().map_err(|_| NativeBridgeError::Storage)?;
        if !metadata.is_file()
            || metadata.uid() != current_uid()
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(NativeBridgeError::Storage);
        }
        Ok(Self { file })
    }

    fn shared(&self) -> Result<(), NativeBridgeError> {
        flock_bounded(&self.file, libc::LOCK_SH, PAIRING_LOCK_TIMEOUT)
            .map_err(NativeBridgeError::from)
    }

    fn exclusive(&self) -> Result<(), NativeBridgeError> {
        flock_bounded(&self.file, libc::LOCK_EX, PAIRING_LOCK_TIMEOUT)
            .map_err(NativeBridgeError::from)
    }

    #[cfg(test)]
    fn exclusive_with_timeout(&self, timeout: Duration) -> Result<(), NativeBridgeError> {
        flock_bounded(&self.file, libc::LOCK_EX, timeout).map_err(NativeBridgeError::from)
    }
}

impl Drop for PairingFileLock {
    fn drop(&mut self) {
        // Closing the descriptor releases the lock. An explicit unlock keeps
        // the lifetime obvious and is harmless if the descriptor is closing.
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairingLockError {
    Storage,
    Busy,
}

impl From<PairingLockError> for NativeBridgeError {
    fn from(error: PairingLockError) -> Self {
        match error {
            PairingLockError::Storage => Self::Storage,
            PairingLockError::Busy => Self::PairingBusy,
        }
    }
}

fn flock_bounded(
    file: &File,
    operation: libc::c_int,
    timeout: Duration,
) -> Result<(), PairingLockError> {
    let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
    loop {
        let result = unsafe { libc::flock(file.as_raw_fd(), operation | libc::LOCK_NB) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if !matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted)
        {
            return Err(PairingLockError::Storage);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PairingLockError::Busy);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
    }
}

/// Validate the user-visible request before any secret is generated.
pub fn validate_pairing_request(request: &PairingRequest) -> Result<(), NativeBridgeError> {
    if !SUPPORTED_BROWSER_CHANNELS.contains(&request.browser_channel.as_str())
        || matches!(request.profile_class, crate::browser_pairing::ProfileClass::Unknown)
        || !is_extension_id(&request.extension_id)
        || !is_hex_digest(&request.extension_key_digest)
        || !is_hex_digest(&request.permissions_digest)
        || request.event_classes.is_empty()
        || request.event_classes.iter().any(|class| *class != BrowserEventClass::TopLevelNavigation)
        || request.retained_fields.len() != 1
        || request.retained_fields.first().map(String::as_str) != Some(RETAINED_ORIGIN)
        || request.private_context_policy != PRIVATE_CONTEXT_REFUSAL
    {
        return Err(NativeBridgeError::PairingRequestInvalid);
    }
    Ok(())
}

/// Validate the exact caller origin supplied out of band by Chromium before
/// consuming any extension bytes. The manifest allowlist is not sufficient:
/// the per-process caller identity must agree with the first authenticated
/// hello as well.
pub fn validate_caller_origin(origin: &str) -> Result<String, NativeBridgeError> {
    let Some(extension_id) = origin
        .strip_prefix(CHROMIUM_EXTENSION_ORIGIN_PREFIX)
        .and_then(|value| value.strip_suffix('/'))
    else {
        return Err(NativeBridgeError::PairingRequestInvalid);
    };
    if !is_extension_id(extension_id) {
        return Err(NativeBridgeError::PairingRequestInvalid);
    }
    Ok(extension_id.to_owned())
}

fn is_extension_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Result of one authenticated, policy-gated bridge input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeOutput {
    Reply(HostMessage),
    Recorded { event_id: Uuid, missing: u64 },
    NavigationRefused { refusal: NavigationRefusal, missing: u64 },
    Heartbeat { missing: u64 },
    Closed,
}

/// The narrow native-host-to-service handoff. A native host has no journal
/// writer capability; it can only submit the typed, canonical admission.
pub trait NativeBridgeSink {
    fn ingest_browser_navigation(
        &self,
        admission: BrowserNavigationAdmission,
    ) -> Result<BrowserNavigationAck, NativeBridgeError>;
}

/// A client for the authenticated LocalService browser-ingestion method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalServiceClient {
    socket_path: PathBuf,
    service_instance: Uuid,
    deadline_ms: u64,
}

impl LocalServiceClient {
    pub fn new(socket_path: impl Into<PathBuf>, service_instance: Uuid) -> Self {
        Self { socket_path: socket_path.into(), service_instance, deadline_ms: 5_000 }
    }

    pub fn with_deadline_ms(mut self, deadline_ms: u64) -> Self {
        self.deadline_ms = deadline_ms;
        self
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn service_instance(&self) -> Uuid {
        self.service_instance
    }

    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

impl NativeBridgeSink for LocalServiceClient {
    fn ingest_browser_navigation(
        &self,
        admission: BrowserNavigationAdmission,
    ) -> Result<BrowserNavigationAck, NativeBridgeError> {
        ingest_browser_navigation(
            &self.socket_path,
            self.service_instance,
            Uuid::new_v4(),
            self.deadline_ms,
            admission,
        )
        .map_err(|_| NativeBridgeError::Journal)
    }
}

/// The LocalService handler that projects canonical browser admissions into
/// the single existing journal writer. It is deliberately separate from the
/// native host process and is the only bridge component with journal access.
pub struct BrowserIngestService {
    writer: Writer,
    policy: PolicyProfile,
    origin: IngestionOrigin,
}

impl std::fmt::Debug for BrowserIngestService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("BrowserIngestService").finish_non_exhaustive()
    }
}

impl BrowserIngestService {
    pub fn new(
        journal: crate::journal::Journal,
        policy: PolicyProfile,
    ) -> Result<Self, ServiceError> {
        let origin = IngestionOrigin::live("live-browser-native")
            .map_err(|_| ServiceError::BrowserIngestRefused)?;
        let writer = Writer::new(journal, WriterConfig::default())
            .map_err(|_| ServiceError::BrowserIngestRefused)?;
        Ok(Self { writer, policy, origin })
    }

    fn gap_event(
        &self,
        missing: u64,
        wall_clock: DateTime<Utc>,
    ) -> Result<EventEnvelope, ServiceError> {
        let reason_code = ReasonCode::try_from("native_message_gap")
            .map_err(|_| ServiceError::BrowserIngestRefused)?;
        EventEnvelope::new(
            &self.origin,
            Uuid::new_v4(),
            wall_clock,
            wall_clock,
            EventSource::Browser,
            EventKind::Gap,
            EventPayload::Gap(GapPayload {
                source: EventSource::Browser,
                reason_code,
                dropped_count: missing,
                from_cursor: None,
                to_cursor: None,
                volume_digest: None,
                root_ids: Vec::new(),
                remediation: None,
            }),
            None,
            self.policy.id.clone(),
            self.policy.version,
            Evidence::Unknown,
            None,
        )
        .map_err(|_| ServiceError::BrowserIngestRefused)
    }
}

impl ServiceHandler for BrowserIngestService {
    fn handle(&self, request: &ServiceRequest) -> Result<serde_json::Value, ServiceError> {
        if request.capability != ServiceCapability::Ingest
            || request.method != BROWSER_NAVIGATION_INGEST_METHOD
        {
            return Err(ServiceError::BrowserIngestMalformed);
        }
        let admission = browser_navigation_from_request(request)?;
        if !SUPPORTED_BROWSER_CHANNELS.contains(&admission.browser.as_str()) {
            return Err(ServiceError::BrowserIngestRefused);
        }
        let payload = BrowserNavigationPayload::new(
            admission.browser.as_str(),
            &admission.navigation.origin(),
            false,
        )
        .map_err(|_| ServiceError::BrowserIngestRefused)?;
        let navigation_id = admission.event_id;
        let mut events = Vec::with_capacity(if admission.missing > 0 { 2 } else { 1 });
        if admission.missing > 0 {
            events.push(self.gap_event(admission.missing, admission.observed_at)?);
        }
        events.push(
            EventEnvelope::new(
                &self.origin,
                navigation_id,
                admission.observed_at,
                admission.observed_at,
                EventSource::Browser,
                EventKind::BrowserNavigation,
                EventPayload::BrowserNavigation(payload),
                None,
                self.policy.id.clone(),
                self.policy.version,
                Evidence::Direct,
                None,
            )
            .map_err(|_| ServiceError::BrowserIngestRefused)?,
        );
        match self
            .writer
            .submit(self.origin.clone(), events, self.policy.clone(), Vec::new())
            .map_err(|_| ServiceError::BrowserIngestRefused)?
        {
            WriterOutcome::Committed(ack)
                if ack.event_ids.last().copied() == Some(navigation_id) =>
            {
                let response = BrowserNavigationAck {
                    event_id: navigation_id,
                    ingest_sequences: ack.ingest_sequences,
                    missing: admission.missing,
                };
                response.validate()?;
                serde_json::to_value(response).map_err(|_| ServiceError::BrowserIngestRefused)
            }
            WriterOutcome::Committed(_) | WriterOutcome::Gap(_) => {
                Err(ServiceError::BrowserIngestRefused)
            }
        }
    }
}

/// Native host state for one browser-launched connection.
pub struct NativeBridge<S: NativeBridgeSink> {
    session: NativeHostSession,
    store: PairingStore,
    sink: S,
    active_pairing: Option<PairingRecord>,
}

impl<S: NativeBridgeSink> NativeBridge<S> {
    /// Construct the bridge with the only currently approved retention policy.
    /// A future path-retaining policy must first add a matching approval and
    /// service schema; it cannot be enabled by a caller of this boundary.
    pub fn new(
        store: PairingStore,
        sink: S,
        url_policy: UrlShapePolicy,
        caller_origin: &str,
    ) -> Result<Self, NativeBridgeError> {
        Self::new_with_start(store, sink, url_policy, caller_origin, None)
    }

    /// Construct a bridge whose protocol deadline is anchored to the process
    /// connection start. The stdio runner uses this to bound the handshake even
    /// when a peer keeps sending rejected frames.
    pub fn new_at(
        store: PairingStore,
        sink: S,
        url_policy: UrlShapePolicy,
        caller_origin: &str,
        started_at: Duration,
    ) -> Result<Self, NativeBridgeError> {
        Self::new_with_start(store, sink, url_policy, caller_origin, Some(started_at))
    }

    fn new_with_start(
        store: PairingStore,
        sink: S,
        url_policy: UrlShapePolicy,
        caller_origin: &str,
        started_at: Option<Duration>,
    ) -> Result<Self, NativeBridgeError> {
        if url_policy != UrlShapePolicy::OriginOnly {
            return Err(NativeBridgeError::PairingRequestInvalid);
        }
        let caller_extension_id = validate_caller_origin(caller_origin)?;
        Ok(Self {
            session: NativeHostSession::with_expected_extension_id_at(
                url_policy,
                Some(caller_extension_id),
                started_at,
            ),
            store,
            sink,
            active_pairing: None,
        })
    }

    /// Authenticate, minimize, authorize, and (for navigation) persist one
    /// message. `now` is monotonic session time; `wall_clock` is supplied by
    /// the caller so tests can prove expiry and restart behavior.
    pub fn receive(
        &mut self,
        body: &[u8],
        now: Duration,
        wall_clock: DateTime<Utc>,
    ) -> Result<BridgeOutput, NativeBridgeError> {
        // Keep the shared admission lease until the complete frame has been
        // handled. For navigation this includes the LocalService/journal
        // receipt; revoke's exclusive lock therefore cannot return success
        // while an already-admitted frame is still in flight.
        let admission = self.store.admission()?;
        // Re-check mutable approval state before every post-handshake frame.
        // The record is read under the lease that also covers the sink call.
        if let Some(active) = &self.active_pairing {
            let current = admission.get(active.pairing_id).ok_or(NativeBridgeError::State)?;
            if current.revoked {
                return Err(NativeBridgeError::Session(PairingError::Revoked.into()));
            }
            if wall_clock >= current.expires_at {
                return Err(NativeBridgeError::Session(PairingError::RePairingRequired.into()));
            }
        }
        let output = self
            .session
            .receive(body, now, wall_clock, |pairing_id| admission.get(pairing_id))
            .map_err(|error| NativeBridgeError::Session(error))?;
        match output {
            crate::native_host::HostOutput::Reply(reply) => {
                // The session has already authenticated the hello. Decode it
                // only after that admission to remember the approved record;
                // this second bounded parse cannot influence authentication or
                // protocol state.
                let parsed = parse_message(body)
                    .map_err(|error| NativeBridgeError::Session(error.into()))?;
                let ExtensionMessage::Hello { pairing_id, .. } = parsed else {
                    return Err(NativeBridgeError::State);
                };
                self.active_pairing = admission.get(pairing_id);
                if self.active_pairing.is_none() {
                    return Err(NativeBridgeError::State);
                }
                Ok(BridgeOutput::Reply(reply))
            }
            crate::native_host::HostOutput::Navigation { navigation, missing, .. } => {
                let active = self.active_pairing.as_ref().ok_or(NativeBridgeError::State)?;
                let record = admission.get(active.pairing_id).ok_or(NativeBridgeError::State)?;
                if !record.request.event_classes.contains(&BrowserEventClass::TopLevelNavigation) {
                    return Err(NativeBridgeError::EventClassNotApproved);
                }
                let browser = BrowserName::try_from(record.request.browser_channel.clone())
                    .map_err(|_| NativeBridgeError::State)?;
                let navigation_id = Uuid::new_v4();
                let acknowledgement = self
                    .sink
                    .ingest_browser_navigation(BrowserNavigationAdmission {
                        event_id: navigation_id,
                        browser,
                        navigation,
                        observed_at: wall_clock,
                        missing,
                    })
                    .map_err(|_| NativeBridgeError::Journal)?;
                if acknowledgement.event_id != navigation_id
                    || acknowledgement.missing != missing
                    || acknowledgement.ingest_sequences.is_empty()
                    || acknowledgement.ingest_sequences.len() > MAX_BROWSER_INGEST_SEQUENCES
                {
                    return Err(NativeBridgeError::State);
                }
                Ok(BridgeOutput::Recorded { event_id: navigation_id, missing })
            }
            crate::native_host::HostOutput::NavigationRefused { refusal, missing } => {
                Ok(BridgeOutput::NavigationRefused { refusal, missing })
            }
            crate::native_host::HostOutput::Heartbeat { missing } => {
                Ok(BridgeOutput::Heartbeat { missing })
            }
            crate::native_host::HostOutput::Closed => {
                self.active_pairing = None;
                Ok(BridgeOutput::Closed)
            }
        }
    }
}

/// Summary returned after the stdio stream reaches EOF or a clean goodbye.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeHostRunSummary {
    pub frames: u64,
    pub recorded_events: u64,
    pub closed: bool,
}

/// Run one native-messaging process over stdin/stdout. stdout receives only
/// framed JSON replies; diagnostics belong to the caller's stderr.
pub fn run_stdio<R: Read + AsRawFd, W: Write + AsRawFd, S: NativeBridgeSink>(
    reader: R,
    writer: W,
    store: PairingStore,
    sink: S,
    url_policy: UrlShapePolicy,
    caller_origin: &str,
) -> Result<NativeHostRunSummary, NativeBridgeError> {
    run_stdio_with_timeout(
        reader,
        writer,
        store,
        sink,
        url_policy,
        caller_origin,
        NATIVE_SESSION_IDLE_TIMEOUT,
    )
}

/// Testable native-host runner with an explicit idle deadline. Production
/// callers use [`run_stdio`] with the protocol's 120-second limit; tests use a
/// short value while still exercising the same `poll`-guarded file descriptor
/// path as the real binary.
pub fn run_stdio_with_timeout<R: Read + AsRawFd, W: Write + AsRawFd, S: NativeBridgeSink>(
    mut reader: R,
    mut writer: W,
    store: PairingStore,
    sink: S,
    url_policy: UrlShapePolicy,
    caller_origin: &str,
    timeout: Duration,
) -> Result<NativeHostRunSummary, NativeBridgeError> {
    let started = Instant::now();
    let mut bridge = NativeBridge::new_at(store, sink, url_policy, caller_origin, Duration::ZERO)?;
    let mut decoder = FrameDecoder::new();
    let mut chunk = [0_u8; MAX_NATIVE_HOST_READ_CHUNK];
    let mut idle_deadline = Instant::now() + timeout;
    let mut summary = NativeHostRunSummary { frames: 0, recorded_events: 0, closed: false };
    loop {
        let remaining = idle_deadline.saturating_duration_since(Instant::now());
        if !wait_for_input(reader.as_raw_fd(), remaining)? {
            // Do not emit a response for a missing or partial frame. The
            // browser sees a closed host, while stderr receives only the
            // caller's fixed high-level error.
            return Err(NativeBridgeError::Session(NativeSessionError::Protocol(
                NativeMessagingError::Timeout,
            )));
        }
        let read = reader.read(&mut chunk).map_err(|_| NativeBridgeError::Io)?;
        if read == 0 {
            decoder.finish().map_err(|_| NativeBridgeError::Framing)?;
            return Ok(summary);
        }
        decoder.push(&chunk[..read]).map_err(|_| NativeBridgeError::Framing)?;
        while let Some(body) = decoder.next_frame().map_err(|_| NativeBridgeError::Framing)? {
            summary.frames = summary.frames.saturating_add(1);
            if summary.closed {
                // A complete frame was already decoded after `goodbye`. Do
                // not hand it back to the bridge (which would otherwise run
                // pairing/MAC logic again); the protocol is closed and the
                // trailing frame is refused without a second response.
                return Err(NativeBridgeError::Session(NativeSessionError::Protocol(
                    NativeMessagingError::TrailingData,
                )));
            }
            let result = bridge.receive(&body, started.elapsed(), Utc::now());
            let output = match result {
                Ok(output) => output,
                Err(error) => {
                    write_host_message_with_timeout(
                        &mut writer,
                        &HostMessage::Refused { reason: error.code().to_owned() },
                        timeout.min(NATIVE_HOST_WRITE_TIMEOUT),
                    )?;
                    return Err(error);
                }
            };
            let message = match output {
                BridgeOutput::Reply(message) => message,
                BridgeOutput::Recorded { event_id, missing } => {
                    summary.recorded_events = summary.recorded_events.saturating_add(1);
                    HostMessage::Accepted { event_id, missing }
                }
                BridgeOutput::NavigationRefused { refusal, .. } => {
                    HostMessage::Refused { reason: navigation_refusal_code(refusal).to_owned() }
                }
                BridgeOutput::Heartbeat { missing } => HostMessage::Heartbeat { missing },
                BridgeOutput::Closed => {
                    summary.closed = true;
                    HostMessage::Closed
                }
            };
            write_host_message_with_timeout(
                &mut writer,
                &message,
                timeout.min(NATIVE_HOST_WRITE_TIMEOUT),
            )?;
            idle_deadline = Instant::now() + timeout;
        }
        if summary.closed {
            if decoder.has_buffered_data() {
                return Err(NativeBridgeError::Framing);
            }
            return Ok(summary);
        }
    }
}

fn wait_for_input(fd: RawFd, timeout: Duration) -> Result<bool, NativeBridgeError> {
    let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
    wait_for_fd(fd, libc::POLLIN | libc::POLLHUP, deadline)
}

fn write_host_message_with_timeout<W: Write + AsRawFd>(
    writer: &mut W,
    message: &HostMessage,
    timeout: Duration,
) -> Result<(), NativeBridgeError> {
    let body = serde_json::to_vec(message).map_err(|_| NativeBridgeError::Serialization)?;
    let frame = encode_frame(&body).map_err(|_| NativeBridgeError::Framing)?;
    write_all_with_timeout(writer.as_raw_fd(), &frame, timeout)
}

fn wait_for_fd(
    fd: RawFd,
    events: libc::c_short,
    deadline: Instant,
) -> Result<bool, NativeBridgeError> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        let timeout_ms = poll_timeout_ms(remaining);
        let mut descriptor = libc::pollfd { fd, events, revents: 0 };
        // SAFETY: `descriptor` points to one initialized pollfd and remains
        // alive for the duration of the call.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if result > 0 {
            return Ok(true);
        }
        if result == 0 {
            if deadline.saturating_duration_since(Instant::now()).is_zero() {
                return Ok(false);
            }
            continue;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return Err(NativeBridgeError::Io);
    }
}

fn poll_timeout_ms(duration: Duration) -> libc::c_int {
    let millis = duration.as_millis();
    let rounded = millis.saturating_add(u128::from(duration.subsec_nanos() % 1_000_000 != 0));
    rounded.min(i32::MAX as u128) as libc::c_int
}

fn write_all_with_timeout(
    fd: RawFd,
    bytes: &[u8],
    timeout: Duration,
) -> Result<(), NativeBridgeError> {
    let original_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if original_flags < 0 {
        return Err(NativeBridgeError::Io);
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, original_flags | libc::O_NONBLOCK) } < 0 {
        return Err(NativeBridgeError::Io);
    }
    let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
    let mut written = 0usize;
    let result = loop {
        if written == bytes.len() {
            break Ok(());
        }
        match wait_for_fd(fd, libc::POLLOUT | libc::POLLERR | libc::POLLHUP, deadline) {
            Ok(true) => {}
            Ok(false) => break Err(NativeBridgeError::Io),
            Err(error) => break Err(error),
        }
        let count =
            unsafe { libc::write(fd, bytes[written..].as_ptr().cast(), bytes.len() - written) };
        if count > 0 {
            written = written.saturating_add(count as usize);
            continue;
        }
        if count == 0 {
            break Err(NativeBridgeError::Io);
        }
        let error = std::io::Error::last_os_error();
        if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted)
        {
            continue;
        }
        break Err(NativeBridgeError::Io);
    };
    let restored = unsafe { libc::fcntl(fd, libc::F_SETFL, original_flags) } == 0;
    if !restored {
        return Err(NativeBridgeError::Io);
    }
    result
}

fn navigation_refusal_code(refusal: NavigationRefusal) -> &'static str {
    match refusal {
        NavigationRefusal::TooLong => "navigation_too_long",
        NavigationRefusal::Invalid => "navigation_invalid",
        NavigationRefusal::FileScheme => "navigation_file_scheme",
        NavigationRefusal::BlobScheme => "navigation_blob_scheme",
        NavigationRefusal::DataScheme => "navigation_data_scheme",
        NavigationRefusal::InternalPage => "navigation_internal_page",
        NavigationRefusal::ExtensionScheme => "navigation_extension_scheme",
        NavigationRefusal::ScriptScheme => "navigation_script_scheme",
        NavigationRefusal::Opaque => "navigation_opaque_scheme",
        NavigationRefusal::PrivateContext => "private_context",
    }
}

fn sync_directory(path: &Path) -> Result<(), NativeBridgeError> {
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| NativeBridgeError::Storage)?;
    directory.sync_all().map_err(|_| NativeBridgeError::Storage)
}

fn ensure_private_directory(path: &Path) -> Result<(), NativeBridgeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.uid() != current_uid() {
                return Err(NativeBridgeError::Storage);
            }
            // Tighten an owner-created directory before placing key material
            // inside it. We never broaden permissions or follow a symlink.
            if metadata.permissions().mode() & 0o077 != 0 {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                    .map_err(|_| NativeBridgeError::Storage)?;
                sync_directory(path)?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(NativeBridgeError::Storage)?;
            let metadata = fs::symlink_metadata(parent).map_err(|_| NativeBridgeError::Storage)?;
            if !metadata.is_dir() || metadata.uid() != current_uid() {
                return Err(NativeBridgeError::Storage);
            }
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| NativeBridgeError::Storage)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| NativeBridgeError::Storage)?;
            sync_directory(parent)?;
            sync_directory(path)
        }
        Err(_) => Err(NativeBridgeError::Storage),
    }
}

fn current_uid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

fn private_metadata_exists(path: &Path) -> Result<bool, NativeBridgeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.uid() != current_uid()
                || metadata.nlink() != 1
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(NativeBridgeError::Storage);
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(NativeBridgeError::Storage),
    }
}

fn ensure_private_lock(path: &Path) -> Result<(), NativeBridgeError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.uid() != current_uid()
                || metadata.nlink() != 1
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err(NativeBridgeError::Storage);
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)
            {
                Ok(file) => {
                    file.sync_all().map_err(|_| NativeBridgeError::Storage)?;
                    let parent = path.parent().ok_or(NativeBridgeError::Storage)?;
                    sync_directory(parent)
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    validate_private_file(path)
                }
                Err(_) => Err(NativeBridgeError::Storage),
            }
        }
        Err(_) => Err(NativeBridgeError::Storage),
    }
}

fn validate_private_file(path: &Path) -> Result<(), NativeBridgeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| NativeBridgeError::Storage)?;
    if !metadata.is_file()
        || metadata.uid() != current_uid()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(NativeBridgeError::Storage);
    }
    Ok(())
}

fn read_private_file(path: &Path) -> Result<Option<Vec<u8>>, NativeBridgeError> {
    if !private_metadata_exists(path)? {
        return Ok(None);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| NativeBridgeError::Storage)?;
    let metadata = file.metadata().map_err(|_| NativeBridgeError::Storage)?;
    if !metadata.is_file()
        || metadata.uid() != current_uid()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(NativeBridgeError::Storage);
    }
    let mut bytes = Vec::new();
    file.take((MAX_PAIRING_STORE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| NativeBridgeError::Storage)?;
    if bytes.len() > MAX_PAIRING_STORE_BYTES {
        return Err(NativeBridgeError::Storage);
    }
    Ok(Some(bytes))
}

fn decode_pairing_key(bytes: &[u8]) -> Result<[u8; 32], NativeBridgeError> {
    bytes.try_into().map_err(|_| NativeBridgeError::Storage)
}

fn create_pairing_key(path: &Path) -> Result<[u8; 32], NativeBridgeError> {
    let mut key = [0_u8; 32];
    rand_core::OsRng.try_fill_bytes(&mut key).map_err(|_| NativeBridgeError::Storage)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| NativeBridgeError::Storage)?;
    file.write_all(&key).map_err(|_| NativeBridgeError::Storage)?;
    file.sync_all().map_err(|_| NativeBridgeError::Storage)?;
    let parent = path.parent().ok_or(NativeBridgeError::Storage)?;
    sync_directory(parent)?;
    Ok(key)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), NativeBridgeError> {
    let parent = path.parent().ok_or(NativeBridgeError::Storage)?;
    let temporary = parent.join(format!(".pairings.{}.tmp", Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| NativeBridgeError::Storage)?;
        file.write_all(bytes).map_err(|_| NativeBridgeError::Storage)?;
        file.sync_all().map_err(|_| NativeBridgeError::Storage)?;
        fs::rename(&temporary, path).map_err(|_| NativeBridgeError::Storage)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;

    use super::*;

    #[test]
    fn pairing_request_rejects_non_exact_extension_ids_and_unsupported_fields() {
        let mut request = PairingRequest {
            browser_channel: "chrome".to_owned(),
            profile_class: crate::browser_pairing::ProfileClass::Default,
            extension_id: "a".repeat(32),
            extension_key_digest: "a".repeat(64),
            permissions_digest: "b".repeat(64),
            event_classes: BTreeSet::from([BrowserEventClass::TopLevelNavigation]),
            retained_fields: vec!["origin".to_owned()],
            private_context_policy: PRIVATE_CONTEXT_REFUSAL.to_owned(),
        };
        assert!(validate_pairing_request(&request).is_ok());
        request.extension_id = "A".repeat(32);
        assert_eq!(
            validate_pairing_request(&request),
            Err(NativeBridgeError::PairingRequestInvalid)
        );
        request.extension_id = "a".repeat(32);
        request.retained_fields = vec!["url".to_owned()];
        assert_eq!(
            validate_pairing_request(&request),
            Err(NativeBridgeError::PairingRequestInvalid)
        );
    }

    #[test]
    fn pairing_lock_contention_has_a_bounded_fd_wait() {
        let directory = tempfile::tempdir().expect("tempdir");
        ensure_private_directory(directory.path()).expect("private directory");
        let path = directory.path().join(PAIRING_LOCK_FILE);
        ensure_private_lock(&path).expect("lock file");
        let first = PairingFileLock::open(&path).expect("first lock");
        first.exclusive().expect("first exclusive lock");
        let second = PairingFileLock::open(&path).expect("second lock");
        let started = Instant::now();
        assert_eq!(
            second.exclusive_with_timeout(Duration::from_millis(20)),
            Err(NativeBridgeError::PairingBusy)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn stalled_native_host_output_has_a_bounded_fd_wait() {
        let (mut writer, _reader) = UnixStream::pair().expect("socket pair");
        let buffer_size: libc::c_int = 1024;
        let result = unsafe {
            libc::setsockopt(
                writer.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                (&buffer_size as *const libc::c_int).cast(),
                std::mem::size_of_val(&buffer_size) as libc::socklen_t,
            )
        };
        assert_eq!(result, 0, "set a small socket send buffer");
        let message = HostMessage::Refused { reason: "x".repeat(60 * 1024) };
        let started = Instant::now();
        assert_eq!(
            write_host_message_with_timeout(&mut writer, &message, Duration::from_millis(20)),
            Err(NativeBridgeError::Io)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
