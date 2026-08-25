//! Volume identity and mount-continuity evidence.
//!
//! A path string is not a volume identity.  The cursor boundary therefore
//! carries device and filesystem identifiers, with an optional digest of a
//! platform volume UUID when a caller can obtain one.  Mutable display names
//! are deliberately not represented.  Mount observations also retain a
//! caller-owned mount generation and an optional APFS snapshot digest so a
//! detach, replacement, restore, or path reuse becomes an explicit
//! discontinuity instead of an inferred continuation.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::model::SnapshotDigest;

/// Version of the in-memory volume identity contract.
pub const VOLUME_IDENTITY_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum VolumeIdentityError {
    #[error("volume device identity is unavailable")]
    DeviceUnavailable,
    #[error("volume filesystem identity is unavailable")]
    FilesystemUnavailable,
}

/// Stable volume evidence used to bind a selected root or cursor.
///
/// `device_id` and `filesystem_id` come from operating-system metadata.  The
/// optional `volume_uuid_digest` is a SHA-256 digest of a platform volume UUID
/// supplied by a privileged platform adapter; the raw UUID is never retained.
/// A volume display name is intentionally absent because it can change while
/// the underlying volume remains the same (or be reused by another volume).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeIdentity {
    pub contract_version: u32,
    pub device_id: u64,
    pub filesystem_id: u64,
    #[serde(default)]
    pub volume_uuid_digest: Option<SnapshotDigest>,
}

impl VolumeIdentity {
    /// Construct identity from stable platform fields.
    pub fn new(
        device_id: u64,
        filesystem_id: u64,
        volume_uuid_digest: Option<SnapshotDigest>,
    ) -> Result<Self, VolumeIdentityError> {
        if device_id == 0 {
            return Err(VolumeIdentityError::DeviceUnavailable);
        }
        if filesystem_id == 0 {
            return Err(VolumeIdentityError::FilesystemUnavailable);
        }
        Ok(Self {
            contract_version: VOLUME_IDENTITY_CONTRACT_VERSION,
            device_id,
            filesystem_id,
            volume_uuid_digest,
        })
    }

    /// Derive a deterministic synthetic identity for fixture and adversarial
    /// transition tests.  The label is input only; it is not retained.
    pub fn synthetic(label: &str) -> Self {
        let digest = Sha256::digest(label.as_bytes());
        let mut device_bytes = [0_u8; 8];
        device_bytes.copy_from_slice(&digest[..8]);
        let mut filesystem_bytes = [0_u8; 8];
        filesystem_bytes.copy_from_slice(&digest[8..16]);
        let device_id = u64::from_le_bytes(device_bytes).max(1);
        let filesystem_id = u64::from_le_bytes(filesystem_bytes).max(1);
        let uuid_digest = digest_bytes(b"ghostrace-volume-uuid-v1\0", label.as_bytes());
        Self::new(device_id, filesystem_id, Some(uuid_digest))
            .expect("synthetic volume fields are non-zero")
    }

    /// Read device and filesystem identity without retaining a path or mount
    /// display name.  A platform adapter may enrich the result with a volume
    /// UUID digest using [`Self::new`].
    pub fn from_path(path: &Path) -> Result<Self, VolumeIdentityError> {
        let metadata = fs::metadata(path).map_err(|_| VolumeIdentityError::DeviceUnavailable)?;
        #[cfg(unix)]
        let device_id = {
            use std::os::unix::fs::MetadataExt;
            metadata.dev()
        };
        #[cfg(not(unix))]
        let device_id = 1_u64;
        if device_id == 0 {
            return Err(VolumeIdentityError::DeviceUnavailable);
        }
        let filesystem_id = filesystem_id(path, device_id)?;
        Self::new(device_id, filesystem_id, None)
    }

    /// Hash the identity for scope binding without exporting individual
    /// device fields.
    pub fn fingerprint(&self) -> SnapshotDigest {
        let mut bytes = Vec::with_capacity(4 + 8 + 8 + 32);
        bytes.extend_from_slice(&self.contract_version.to_le_bytes());
        bytes.extend_from_slice(&self.device_id.to_le_bytes());
        bytes.extend_from_slice(&self.filesystem_id.to_le_bytes());
        if let Some(uuid) = &self.volume_uuid_digest {
            bytes.extend_from_slice(uuid.as_str().as_bytes());
        }
        digest_bytes(b"ghostrace-volume-identity-v1\0", &bytes)
    }
}

/// Mount state captured by a platform volume observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountState {
    Mounted,
    Unmounted,
}

/// Path-free observation used to classify continuity between two mounts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeObservation {
    pub volume: VolumeIdentity,
    /// Digest of the selected-root scope; the path itself is never retained.
    pub scope_digest: SnapshotDigest,
    /// Monotonic instance assigned by the mount observer.  A new value means
    /// the volume was detached and mounted again, even if identity matches.
    pub mount_generation: u64,
    pub mount_state: MountState,
    /// Optional APFS snapshot identity supplied by a snapshot-aware adapter.
    #[serde(default)]
    pub snapshot_digest: Option<SnapshotDigest>,
}

impl VolumeObservation {
    pub fn new(
        volume: VolumeIdentity,
        scope_digest: SnapshotDigest,
        mount_generation: u64,
        mount_state: MountState,
        snapshot_digest: Option<SnapshotDigest>,
    ) -> Self {
        Self { volume, scope_digest, mount_generation, mount_state, snapshot_digest }
    }

    pub fn transition_from(&self, previous: &Self) -> VolumeTransition {
        match (previous.mount_state, self.mount_state) {
            (MountState::Mounted, MountState::Unmounted) => return VolumeTransition::Unmounted,
            (MountState::Unmounted, MountState::Mounted) => return VolumeTransition::Mounted,
            _ => {}
        }
        if previous.volume != self.volume {
            return if previous.scope_digest == self.scope_digest {
                VolumeTransition::PathReused
            } else {
                VolumeTransition::DeviceReplaced
            };
        }
        if previous.snapshot_digest != self.snapshot_digest && self.snapshot_digest.is_some() {
            return VolumeTransition::SnapshotRestored;
        }
        if previous.mount_generation != self.mount_generation {
            return VolumeTransition::Remounted;
        }
        VolumeTransition::Continuous
    }
}

/// Explicit outcomes of comparing two volume observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeTransition {
    Initial,
    Continuous,
    Mounted,
    Unmounted,
    Remounted,
    DeviceReplaced,
    SnapshotRestored,
    PathReused,
}

impl VolumeTransition {
    pub fn is_discontinuity(self) -> bool {
        !matches!(self, Self::Initial | Self::Continuous)
    }
}

fn digest_bytes(domain: &[u8], value: &[u8]) -> SnapshotDigest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(value);
    let digest = hasher.finalize();
    let encoded = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    SnapshotDigest::try_from(format!("sha256:{encoded}"))
        .expect("sha256 digest is valid SnapshotDigest")
}

#[cfg(unix)]
fn filesystem_id(path: &Path, device_id: u64) -> Result<u64, VolumeIdentityError> {
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt, slice};

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| VolumeIdentityError::FilesystemUnavailable)?;
    let mut stats = MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: `path` is a NUL-terminated path and `stats` points to writable
    // storage of the exact platform `statfs` layout.
    let result = unsafe { libc::statfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(VolumeIdentityError::FilesystemUnavailable);
    }
    // SAFETY: statfs initialized the structure on success.  The fsid bytes are
    // copied as an opaque value so platform-private fields and display names
    // are never interpreted or retained.
    let stats = unsafe { stats.assume_init() };
    let fsid = unsafe {
        slice::from_raw_parts(
            std::ptr::addr_of!(stats.f_fsid).cast::<u8>(),
            std::mem::size_of::<libc::fsid_t>(),
        )
    };
    let digest = Sha256::digest(fsid);
    let mut id_bytes = [0_u8; 8];
    id_bytes.copy_from_slice(&digest[..8]);
    Ok(u64::from_le_bytes(id_bytes).max(device_id))
}

#[cfg(not(unix))]
fn filesystem_id(_path: &Path, device_id: u64) -> Result<u64, VolumeIdentityError> {
    Ok(device_id)
}
