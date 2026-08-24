//! Explicit policy and evidence types for the local SQLite WAL.

use std::time::Duration;

use crate::error::GhostraceError;

pub const DEFAULT_AUTOCHECKPOINT_PAGES: u32 = 1_000;
pub const DEFAULT_BUSY_TIMEOUT_MS: u64 = 250;
pub const DEFAULT_MAX_READER_MS: u64 = 30_000;
pub const DEFAULT_MAX_WAL_BYTES: u64 = 64 * 1024 * 1024;

/// Bounds that apply to every file-backed journal connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalPolicy {
    pub autocheckpoint_pages: u32,
    pub busy_timeout_ms: u64,
    pub max_reader_ms: u64,
    pub max_wal_bytes: u64,
}

impl WalPolicy {
    pub fn new(
        autocheckpoint_pages: u32,
        busy_timeout_ms: u64,
        max_reader_ms: u64,
        max_wal_bytes: u64,
    ) -> Result<Self, GhostraceError> {
        let policy = Self { autocheckpoint_pages, busy_timeout_ms, max_reader_ms, max_wal_bytes };
        policy.validate()?;
        Ok(policy)
    }

    pub fn reader_limit(self) -> Duration {
        Duration::from_millis(self.max_reader_ms)
    }

    pub fn busy_timeout(self) -> Duration {
        Duration::from_millis(self.busy_timeout_ms)
    }

    pub fn validate(self) -> Result<(), GhostraceError> {
        if self.autocheckpoint_pages == 0 {
            return Err(GhostraceError::InvalidWalPolicy(
                "autocheckpoint_pages must be greater than zero".to_owned(),
            ));
        }
        if self.busy_timeout_ms > 30_000 {
            return Err(GhostraceError::InvalidWalPolicy(
                "busy_timeout_ms must be at most 30000".to_owned(),
            ));
        }
        if self.max_reader_ms == 0 {
            return Err(GhostraceError::InvalidWalPolicy(
                "max_reader_ms must be greater than zero".to_owned(),
            ));
        }
        if self.max_wal_bytes < 4 * 1024 {
            return Err(GhostraceError::InvalidWalPolicy(
                "max_wal_bytes must be at least 4096".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for WalPolicy {
    fn default() -> Self {
        Self {
            autocheckpoint_pages: DEFAULT_AUTOCHECKPOINT_PAGES,
            busy_timeout_ms: DEFAULT_BUSY_TIMEOUT_MS,
            max_reader_ms: DEFAULT_MAX_READER_MS,
            max_wal_bytes: DEFAULT_MAX_WAL_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointMode {
    Passive,
    Truncate,
}

impl CheckpointMode {
    pub(crate) const fn pragma_name(self) -> &'static str {
        match self {
            Self::Passive => "PASSIVE",
            Self::Truncate => "TRUNCATE",
        }
    }
}

/// The SQLite checkpoint tuple plus the observed sidecar size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalCheckpointReport {
    pub mode: CheckpointMode,
    pub busy: bool,
    pub frames_in_wal: u64,
    pub frames_checkpointed: u64,
    pub frames_remaining: u64,
    pub wal_bytes: u64,
    pub max_wal_bytes: u64,
}

impl WalCheckpointReport {
    pub(crate) fn memory(mode: CheckpointMode, max_wal_bytes: u64) -> Self {
        Self {
            mode,
            busy: false,
            frames_in_wal: 0,
            frames_checkpointed: 0,
            frames_remaining: 0,
            wal_bytes: 0,
            max_wal_bytes,
        }
    }

    pub fn within_policy(self) -> bool {
        !self.busy && self.frames_remaining == 0 && self.wal_bytes <= self.max_wal_bytes
    }
}
