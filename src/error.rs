use std::path::PathBuf;

use thiserror::Error;

/// Errors returned by the fixture-only vertical slice.
#[derive(Debug, Error)]
pub enum GhostraceError {
    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("invalid browser URL: {0}")]
    InvalidUrl(String),

    #[error("private browser context is not accepted by default")]
    PrivateContext,

    #[error("policy denied event: {reason}")]
    PolicyDenied { reason: String },

    #[error("encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O operation failed")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("fixture line {line}: {message}")]
    FixtureLine { line: usize, message: String },

    #[error("event not found: {0}")]
    EventNotFound(uuid::Uuid),

    #[error("export destination already exists; pass --force to overwrite")]
    ExportExists(PathBuf),

    #[error("journal directory permissions are not private (expected no group/world access)")]
    InsecurePermissions(PathBuf),

    #[error("journal path is unsafe")]
    UnsafePath,

    #[error("journal path changed during secure open")]
    PathRace,

    #[error("journal path owner is unexpected")]
    UnexpectedOwner,

    #[error("journal path has unexpected hard links")]
    UnexpectedHardLinks,

    #[error("fixture event provenance is invalid")]
    FixtureProvenance,

    #[error("ingestion origin does not authorize event")]
    OriginRejected,

    #[error("ingestion origin capability is not bound to this event")]
    OriginCapabilityMismatch,

    #[error("ingestion origin does not allow this event class")]
    OriginEventClass,

    #[error("an origin instance is required to construct an event")]
    OriginInstanceRequired,

    #[error("live capture is intentionally disabled until policy/cursor/Keychain gates land")]
    LiveCaptureDisabled,

    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),

    #[error("unsupported policy document schema version: {0}")]
    UnsupportedPolicySchema(u32),

    #[error("policy document migration rejected: {0}")]
    PolicyMigration(String),

    #[error("consent state transition rejected: {0}")]
    ConsentTransition(String),

    #[error("migration error: {0}")]
    Migration(String),
}

/// Errors from payload encryption and authenticated decryption.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("key provider failed: {0}")]
    KeyProvider(String),

    #[error("ciphertext is truncated")]
    Truncated,

    #[error("ciphertext authentication failed")]
    AuthenticationFailed,

    #[error("ciphertext encoding error: {0}")]
    Encoding(String),

    #[error("operating-system randomness failed")]
    Random,
}

impl From<chacha20poly1305::aead::Error> for CryptoError {
    fn from(_: chacha20poly1305::aead::Error) -> Self {
        Self::AuthenticationFailed
    }
}
