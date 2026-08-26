//! Path-free Git repository and worktree identity.
//!
//! A Git path is an address, not an identity.  This module accepts only the
//! stable filesystem identity of the Git object database and (when present) a
//! worktree.  It immediately projects those values into domain-separated
//! SHA-256 digests.  Remote URLs, credential helpers, config values, reflog
//! messages, and filesystem paths have no representation in the output type.
//!
//! The module is an identity contract for a future Git adapter, not a Git
//! command runner.  An adapter may resolve Git's common object directory and
//! worktree metadata, but it must pass only the resulting device/file
//! identities here and discard the source strings before persistence.

use std::{fmt, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::model::{RepositoryId, RootId, SnapshotDigest};

/// Version of the path-free Git identity contract.
pub const GIT_IDENTITY_CONTRACT_VERSION: u32 = 1;
/// Stable schema identifier for the identity contract.
pub const GIT_IDENTITY_SCHEMA_ID: &str = "ghostrace.git-repository-worktree-identity";
/// Checked-in schema for the fixture contract.
pub const GIT_IDENTITY_SCHEMA_JSON: &str =
    include_str!("../schemas/git-repository-worktree-identity-v1.json");

const OBJECT_DATABASE_DOMAIN: &[u8] = b"ghostrace-git-object-database-identity-v1\0";
const WORKTREE_DOMAIN: &[u8] = b"ghostrace-git-worktree-identity-v1\0";

/// Errors at the Git identity boundary are deliberately path-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum GitIdentityError {
    #[error("Git filesystem identity fields must be non-zero")]
    InvalidFilesystemIdentity,
    #[error("Git filesystem identity is unavailable")]
    FilesystemIdentityUnavailable,
    #[error("Git identity scope and repository kind are inconsistent")]
    InvalidScope,
    #[error("unsupported Git identity contract version: {0}")]
    UnsupportedContractVersion(u32),
}

/// The stable operating-system identity of a Git directory.
///
/// The fields are intentionally private and this type does not implement
/// `Serialize`.  Callers can construct it from metadata, but only the digest
/// produced by [`GitIdentity`] is serializable.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct GitFilesystemIdentity {
    device_id: u64,
    file_id: u64,
}

impl GitFilesystemIdentity {
    /// Construct an identity from platform device and file/inode values.
    pub fn new(device_id: u64, file_id: u64) -> Result<Self, GitIdentityError> {
        if device_id == 0 || file_id == 0 {
            return Err(GitIdentityError::InvalidFilesystemIdentity);
        }
        Ok(Self { device_id, file_id })
    }

    /// Read directory metadata and discard the supplied path after hashing.
    ///
    /// On Unix, `dev` and `ino` remain stable when a directory is moved within
    /// a filesystem.  Non-Unix platforms return an explicit no-go until a
    /// platform adapter can provide equivalent stable fields.
    #[cfg(unix)]
    pub fn from_path(path: &Path) -> Result<Self, GitIdentityError> {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::symlink_metadata(path)
            .map_err(|_| GitIdentityError::FilesystemIdentityUnavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(GitIdentityError::FilesystemIdentityUnavailable);
        }
        Self::new(metadata.dev(), metadata.ino())
    }

    /// Explicit platform no-go when stable directory identity fields are not
    /// available.  No path is included in the error.
    #[cfg(not(unix))]
    pub fn from_path(_path: &Path) -> Result<Self, GitIdentityError> {
        Err(GitIdentityError::FilesystemIdentityUnavailable)
    }
}

impl fmt::Debug for GitFilesystemIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitFilesystemIdentity(<redacted>)")
    }
}

/// Whether an identity describes a normal worktree repository, a bare object
/// database, or a repository nested as a submodule.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitRepositoryKind {
    Standard,
    Bare,
    Submodule,
}

/// The explicit source scope under which a Git identity was observed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitSourceScope {
    SelectedRoot,
    Repository,
    Worktree,
    Submodule,
}

/// Result of comparing two identities.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitContinuity {
    Continuous,
    WorktreeChanged,
    RepositoryChanged,
    ScopeChanged,
    Incomparable,
}

/// The serializable, privacy-minimized identity of one Git observation.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitIdentity {
    pub contract_version: u32,
    /// Digest of the Git common object database directory identity.
    pub object_database_digest: SnapshotDigest,
    /// Digest of the worktree directory identity; absent for object-only
    /// observations such as a bare repository.
    #[serde(default)]
    pub worktree_digest: Option<SnapshotDigest>,
    /// The caller-owned selected-root identity; the root path is never stored.
    pub selected_root_id: RootId,
    pub source_scope: GitSourceScope,
    pub repository_kind: GitRepositoryKind,
}

impl GitIdentity {
    /// Build a path-free identity from stable filesystem metadata.
    pub fn from_stable_parts(
        object_database: GitFilesystemIdentity,
        worktree: Option<GitFilesystemIdentity>,
        selected_root_id: RootId,
        source_scope: GitSourceScope,
        repository_kind: GitRepositoryKind,
    ) -> Result<Self, GitIdentityError> {
        validate_scope(source_scope, repository_kind)?;
        Ok(Self {
            contract_version: GIT_IDENTITY_CONTRACT_VERSION,
            object_database_digest: digest_identity(OBJECT_DATABASE_DOMAIN, object_database),
            worktree_digest: worktree.map(|identity| digest_identity(WORKTREE_DOMAIN, identity)),
            selected_root_id,
            source_scope,
            repository_kind,
        })
    }

    /// Build an identity by reading only directory metadata.  No path is
    /// copied into the returned value or any error string.
    pub fn from_paths(
        object_database_path: &Path,
        worktree_path: Option<&Path>,
        selected_root_id: RootId,
        source_scope: GitSourceScope,
        repository_kind: GitRepositoryKind,
    ) -> Result<Self, GitIdentityError> {
        let object_database = GitFilesystemIdentity::from_path(object_database_path)?;
        let worktree = worktree_path.map(GitFilesystemIdentity::from_path).transpose()?;
        Self::from_stable_parts(
            object_database,
            worktree,
            selected_root_id,
            source_scope,
            repository_kind,
        )
    }

    /// Validate the version and the cross-field scope contract after
    /// deserialization or an external field update.
    pub fn validate(&self) -> Result<(), GitIdentityError> {
        if self.contract_version != GIT_IDENTITY_CONTRACT_VERSION {
            return Err(GitIdentityError::UnsupportedContractVersion(self.contract_version));
        }
        validate_scope(self.source_scope, self.repository_kind)
    }

    /// Return the event-model repository identifier derived only from the
    /// object-database digest.  It is stable across worktree additions and
    /// selected-root changes while remaining an opaque identifier.
    pub fn repository_id(&self) -> RepositoryId {
        let hex = self
            .object_database_digest
            .as_str()
            .strip_prefix("sha256:")
            .expect("Git identity stores a tagged SHA-256 digest");
        RepositoryId::try_from(format!("git-{hex}")).expect("derived Git repository ID is valid")
    }

    /// Compare this observation with a previous observation.
    ///
    /// Repository identity is checked before worktree and scope identity, so
    /// a clone or reinitialization cannot be mistaken for a moved worktree.
    pub fn continuity_from(&self, previous: &Self) -> GitContinuity {
        if self.validate().is_err() || previous.validate().is_err() {
            return GitContinuity::Incomparable;
        }
        if self.object_database_digest != previous.object_database_digest
            || self.repository_kind != previous.repository_kind
        {
            return GitContinuity::RepositoryChanged;
        }
        if self.worktree_digest != previous.worktree_digest {
            return GitContinuity::WorktreeChanged;
        }
        if self.selected_root_id != previous.selected_root_id
            || self.source_scope != previous.source_scope
        {
            return GitContinuity::ScopeChanged;
        }
        GitContinuity::Continuous
    }
}

fn validate_scope(
    source_scope: GitSourceScope,
    repository_kind: GitRepositoryKind,
) -> Result<(), GitIdentityError> {
    let valid = match (source_scope, repository_kind) {
        (GitSourceScope::Submodule, GitRepositoryKind::Submodule) => true,
        (GitSourceScope::Submodule, _) | (_, GitRepositoryKind::Submodule) => false,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(GitIdentityError::InvalidScope)
    }
}

fn digest_identity(domain: &[u8], identity: GitFilesystemIdentity) -> SnapshotDigest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(identity.device_id.to_be_bytes());
    hasher.update(identity.file_id.to_be_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing a String cannot fail");
    }
    SnapshotDigest::try_from(format!("sha256:{encoded}")).expect("SHA-256 identity digest is valid")
}
