//! Strict metadata-only contract for an explicitly invoked shell wrapper.
//!
//! This module is deliberately separate from the ambient collector and from
//! command execution. It describes the small record a future wrapper may
//! submit after a user deliberately routes a command through it. Raw command
//! text, arguments, environment, terminal streams, and shell state have no
//! representation in these types or in the checked-in schema.

use std::{fmt, str::FromStr};

use chrono::{DateTime, Duration, Utc};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    error::GhostraceError,
    model::{PathClass, PathDigest, SessionId, ShellStatus},
};

pub const SHELL_METADATA_SCHEMA_VERSION: u32 = 1;
pub const SHELL_METADATA_SCHEMA_ID: &str = "ghostrace.shell-execution-metadata";
pub const MAX_SHELL_METADATA_BYTES: usize = 16 * 1024;
pub const MAX_SHELL_EXECUTION_DURATION_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
pub const MAX_SHELL_SIGNAL: u8 = 64;

pub const SHELL_METADATA_SCHEMA_JSON: &str =
    include_str!("../schemas/shell-execution-metadata-v1.json");
pub const SHELL_METADATA_GOLDEN_JSON: &str =
    include_str!("../fixtures/shell-execution-metadata-v1.golden.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellMetadataSensitivity {
    Contract,
    Identifying,
    SensitiveMetadata,
    TimingEvidence,
    Outcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ShellMetadataFieldDescriptor {
    pub name: &'static str,
    pub semantic: &'static str,
    pub sensitivity: ShellMetadataSensitivity,
    pub required: bool,
}

pub const SHELL_METADATA_FIELDS: &[ShellMetadataFieldDescriptor] = &[
    ShellMetadataFieldDescriptor {
        name: "schema_version",
        semantic: "contract_revision",
        sensitivity: ShellMetadataSensitivity::Contract,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "wrapper_session_id",
        semantic: "opaque_explicit_wrapper_session",
        sensitivity: ShellMetadataSensitivity::Identifying,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "executable_id",
        semantic: "normalized_executable_identity_without_path",
        sensitivity: ShellMetadataSensitivity::SensitiveMetadata,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "working_directory.path_class",
        semantic: "sanitized_working_directory_scope",
        sensitivity: ShellMetadataSensitivity::SensitiveMetadata,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "working_directory.path_digest",
        semantic: "root_scoped_working_directory_digest",
        sensitivity: ShellMetadataSensitivity::SensitiveMetadata,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "started_at",
        semantic: "wrapper_start_time",
        sensitivity: ShellMetadataSensitivity::TimingEvidence,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "ended_at",
        semantic: "wrapper_end_time",
        sensitivity: ShellMetadataSensitivity::TimingEvidence,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "status",
        semantic: "process_outcome_class",
        sensitivity: ShellMetadataSensitivity::Outcome,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "exit_code",
        semantic: "process_exit_status",
        sensitivity: ShellMetadataSensitivity::Outcome,
        required: true,
    },
    ShellMetadataFieldDescriptor {
        name: "signal",
        semantic: "process_termination_signal",
        sensitivity: ShellMetadataSensitivity::Outcome,
        required: true,
    },
];

pub const fn shell_metadata_fields() -> &'static [ShellMetadataFieldDescriptor] {
    SHELL_METADATA_FIELDS
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ShellExecutableId(String);

impl ShellExecutableId {
    pub fn new(value: impl Into<String>) -> Result<Self, GhostraceError> {
        let value = value.into();
        validate_executable_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ShellExecutableId {
    type Error = GhostraceError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ShellExecutableId {
    type Error = GhostraceError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for ShellExecutableId {
    type Err = GhostraceError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value)
    }
}

impl AsRef<str> for ShellExecutableId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ShellExecutableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for ShellExecutableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ShellExecutableId").field(&"<redacted>").finish()
    }
}

impl Serialize for ShellExecutableId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShellExecutableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellWorkingDirectory {
    pub path_class: PathClass,
    pub path_digest: PathDigest,
}

impl ShellWorkingDirectory {
    pub fn new(path_class: PathClass, path_digest: PathDigest) -> Self {
        Self { path_class, path_digest }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShellExecutionMetadata {
    pub schema_version: u32,
    pub wrapper_session_id: SessionId,
    pub executable_id: ShellExecutableId,
    pub working_directory: ShellWorkingDirectory,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub status: ShellStatus,
    pub exit_code: Option<i32>,
    pub signal: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellExecutionMetadataWire {
    schema_version: u32,
    wrapper_session_id: SessionId,
    executable_id: ShellExecutableId,
    working_directory: ShellWorkingDirectory,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    status: ShellStatus,
    exit_code: Option<i32>,
    signal: Option<u8>,
}

impl<'de> Deserialize<'de> for ShellExecutionMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ShellExecutionMetadataWire::deserialize(deserializer)?;
        let metadata = Self {
            schema_version: wire.schema_version,
            wrapper_session_id: wire.wrapper_session_id,
            executable_id: wire.executable_id,
            working_directory: wire.working_directory,
            started_at: wire.started_at,
            ended_at: wire.ended_at,
            status: wire.status,
            exit_code: wire.exit_code,
            signal: wire.signal,
        };
        metadata.validate().map_err(D::Error::custom)?;
        Ok(metadata)
    }
}

impl ShellExecutionMetadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        wrapper_session_id: SessionId,
        executable_id: ShellExecutableId,
        working_directory: ShellWorkingDirectory,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
        status: ShellStatus,
        exit_code: Option<i32>,
        signal: Option<u8>,
    ) -> Result<Self, GhostraceError> {
        let metadata = Self {
            schema_version: SHELL_METADATA_SCHEMA_VERSION,
            wrapper_session_id,
            executable_id,
            working_directory,
            started_at,
            ended_at,
            status,
            exit_code,
            signal,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != SHELL_METADATA_SCHEMA_VERSION {
            return Err(shell_metadata_error("unsupported schema version"));
        }
        validate_executable_id(self.executable_id.as_str())?;
        if self.working_directory.path_digest.as_str().len() != 71 {
            return Err(shell_metadata_error("working-directory digest is invalid"));
        }
        let duration = self.ended_at.signed_duration_since(self.started_at);
        if duration < Duration::zero()
            || duration.num_milliseconds() > MAX_SHELL_EXECUTION_DURATION_MS
        {
            return Err(shell_metadata_error("execution time range is invalid"));
        }
        match self.status {
            ShellStatus::Succeeded => {
                if self.exit_code != Some(0) || self.signal.is_some() {
                    return Err(shell_metadata_error("succeeded outcome is inconsistent"));
                }
            }
            ShellStatus::Failed => {
                if self.exit_code.is_none_or(|code| code == 0) || self.signal.is_some() {
                    return Err(shell_metadata_error("failed outcome is inconsistent"));
                }
            }
            ShellStatus::Signaled => {
                if self.exit_code.is_some()
                    || self.signal.is_none_or(|signal| signal == 0 || signal > MAX_SHELL_SIGNAL)
                {
                    return Err(shell_metadata_error("signaled outcome is inconsistent"));
                }
            }
            ShellStatus::Unknown => {
                if self.exit_code.is_some() || self.signal.is_some() {
                    return Err(shell_metadata_error("unknown outcome is inconsistent"));
                }
            }
        }
        Ok(())
    }
}

pub fn validate_shell_metadata(input: &str) -> Result<ShellExecutionMetadata, GhostraceError> {
    if input.len() > MAX_SHELL_METADATA_BYTES {
        return Err(shell_metadata_error("metadata exceeds the byte bound"));
    }
    let metadata: ShellExecutionMetadata = serde_json::from_str(input)?;
    metadata.validate()?;
    Ok(metadata)
}

pub fn checked_in_shell_metadata() -> Result<ShellExecutionMetadata, GhostraceError> {
    validate_shell_metadata(SHELL_METADATA_GOLDEN_JSON)
}

fn validate_executable_id(value: &str) -> Result<(), GhostraceError> {
    if value.is_empty()
        || value.len() > 128
        || !value.is_ascii()
        || !value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !value.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        || value.contains("..")
        || value.contains("password")
        || value.contains("passwd")
        || value.contains("secret")
        || value.contains("credential")
        || value.contains("authorization")
        || value.contains("bearer")
        || value.contains("private-key")
    {
        return Err(shell_metadata_error("executable identity is not a safe opaque token"));
    }
    Ok(())
}

fn shell_metadata_error(message: &str) -> GhostraceError {
    GhostraceError::InvalidEvent(format!("shell metadata: {message}"))
}
