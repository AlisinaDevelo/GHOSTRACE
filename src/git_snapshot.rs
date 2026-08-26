//! Privacy-bounded Git snapshot metadata.
//!
//! This module is the input contract for a future, explicitly requested Git
//! snapshot adapter.  It accepts already-normalized metadata and never accepts
//! a path, a ref name, a remote, a command line, or an object reader.  In
//! particular, constructing or validating a snapshot cannot open a Git object
//! or inspect its contents.  An adapter must ask Git only for the bounded facts
//! represented here and discard every other value before calling this module.

use std::{fmt, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    git_identity::{GitIdentity, GitRepositoryKind},
    model::{RepositoryId, SnapshotDigest},
};

/// Version of the metadata-only Git snapshot contract.
pub const GIT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
/// Stable schema identifier for the metadata-only snapshot.
pub const GIT_SNAPSHOT_SCHEMA_ID: &str = "ghostrace.git-snapshot-metadata";
/// Maximum serialized snapshot size accepted by the boundary.
pub const MAX_GIT_SNAPSHOT_BYTES: usize = 16 * 1024;
/// Maximum count retained for any one status class.
pub const MAX_GIT_STATUS_COUNT: u32 = 1_000_000;
/// Maximum sum of status counts retained in one snapshot.
pub const MAX_GIT_STATUS_TOTAL: u64 = 4_000_000;
/// Object reads are not part of this contract.
pub const GIT_OBJECT_READ_POLICY: &str = "metadata_only";

/// Checked-in JSON Schema for the snapshot contract.
pub const GIT_SNAPSHOT_SCHEMA_JSON: &str = include_str!("../schemas/git-snapshot-metadata-v1.json");
/// Checked-in deterministic snapshot example.
pub const GIT_SNAPSHOT_GOLDEN_JSON: &str =
    include_str!("../fixtures/git-snapshot-metadata-v1.golden.json");

/// Errors at the Git snapshot boundary never include untrusted Git text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum GitSnapshotError {
    #[error("Git snapshot metadata is malformed")]
    Malformed,
    #[error("Git snapshot metadata exceeds its byte bound")]
    MetadataTooLarge,
    #[error("unsupported Git snapshot contract version")]
    UnsupportedVersion,
    #[error("Git snapshot object identifier is invalid")]
    InvalidObjectId,
    #[error("Git snapshot object identifier does not match its algorithm")]
    ObjectFormatMismatch,
    #[error("Git snapshot status counts exceed their bound")]
    StatusCountExceeded,
    #[error("Git snapshot worktree state does not match its status counts")]
    WorktreeStateMismatch,
    #[error("Git snapshot repository kind does not match its worktree state")]
    RepositoryKindMismatch,
    #[error("Git snapshot digest does not match its metadata")]
    DigestMismatch,
    #[error("Git snapshot identity is invalid")]
    InvalidIdentity,
}

/// The hash algorithm used by a Git object database.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitObjectFormat {
    Sha1,
    Sha256,
}

impl GitObjectFormat {
    const fn hex_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }

    const fn prefix(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1:",
            Self::Sha256 => "sha256:",
        }
    }
}

impl fmt::Display for GitObjectFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        })
    }
}

/// An algorithm-tagged Git object ID.
///
/// The tagged representation (`sha1:<40 lowercase hex>` or
/// `sha256:<64 lowercase hex>`) prevents a SHA-1 and SHA-256 object from being
/// silently treated as the same identifier.  The type contains no object
/// content and does not provide an object-reading operation.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct GitObjectIdRef {
    algorithm: GitObjectFormat,
    hex: String,
}

impl GitObjectIdRef {
    /// Construct an algorithm-aware object ID from its bare hexadecimal form.
    pub fn new(
        algorithm: GitObjectFormat,
        hex: impl Into<String>,
    ) -> Result<Self, GitSnapshotError> {
        let hex = hex.into();
        if hex.len() != algorithm.hex_len()
            || !hex.is_ascii()
            || !hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(GitSnapshotError::InvalidObjectId);
        }
        Ok(Self { algorithm, hex })
    }

    /// Parse the canonical tagged form.
    pub fn parse_tagged(value: &str) -> Result<Self, GitSnapshotError> {
        let (prefix, hex) = value.split_once(':').ok_or(GitSnapshotError::InvalidObjectId)?;
        let algorithm = match prefix {
            "sha1" => GitObjectFormat::Sha1,
            "sha256" => GitObjectFormat::Sha256,
            _ => return Err(GitSnapshotError::InvalidObjectId),
        };
        Self::new(algorithm, hex)
    }

    /// Return the explicit object hash algorithm.
    pub const fn algorithm(&self) -> GitObjectFormat {
        self.algorithm
    }

    /// Return the bare hexadecimal object ID.
    pub fn hex(&self) -> &str {
        &self.hex
    }

    /// Return the canonical algorithm-tagged representation.
    pub fn tagged(&self) -> String {
        format!("{}{}", self.algorithm.prefix(), self.hex)
    }
}

impl fmt::Debug for GitObjectIdRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitObjectIdRef(<redacted>)")
    }
}

impl fmt::Display for GitObjectIdRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.tagged())
    }
}

impl FromStr for GitObjectIdRef {
    type Err = GitSnapshotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_tagged(value)
    }
}

impl Serialize for GitObjectIdRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            algorithm: GitObjectFormat,
            hex: &'a str,
        }
        Wire { algorithm: self.algorithm, hex: &self.hex }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GitObjectIdRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            algorithm: GitObjectFormat,
            hex: String,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.algorithm, wire.hex).map_err(D::Error::custom)
    }
}

/// The bounded class of the current ref without retaining its name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitBranchClass {
    Local,
    RemoteTracking,
    DetachedHead,
    Tag,
    Unborn,
    NoWorktree,
    Unknown,
}

/// Bounded worktree status without filenames or file contents.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitWorktreeState {
    Clean,
    Modified,
    UntrackedOnly,
    Conflicted,
    NotApplicable,
    Unknown,
}

/// The current Git operation class; operation paths and messages are omitted.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOperation {
    Idle,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    Apply,
    Sequencer,
    Unknown,
}

/// Counts of status classes, bounded before they enter a snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitStatusCounts {
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

impl GitStatusCounts {
    pub fn new(
        staged: u32,
        unstaged: u32,
        untracked: u32,
        conflicted: u32,
    ) -> Result<Self, GitSnapshotError> {
        let counts = Self { staged, unstaged, untracked, conflicted };
        counts.validate()?;
        Ok(counts)
    }

    fn validate(&self) -> Result<(), GitSnapshotError> {
        let values = [self.staged, self.unstaged, self.untracked, self.conflicted];
        if values.iter().any(|value| *value > MAX_GIT_STATUS_COUNT)
            || values.iter().map(|value| u64::from(*value)).sum::<u64>() > MAX_GIT_STATUS_TOTAL
        {
            return Err(GitSnapshotError::StatusCountExceeded);
        }
        Ok(())
    }

    fn total(&self) -> u64 {
        u64::from(self.staged)
            + u64::from(self.unstaged)
            + u64::from(self.untracked)
            + u64::from(self.conflicted)
    }
}

/// Source limitations that must be visible instead of silently inferred away.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitPartialCloneState {
    Full,
    Partial,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitReplaceRefsState {
    None,
    Active,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitShallowHistoryState {
    Complete,
    Shallow,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitSubmoduleState {
    None,
    Present,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitAlternateObjectDatabaseState {
    None,
    Present,
    Unknown,
}

/// Explicit limitations for the source from which a snapshot was obtained.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSourceLimitations {
    pub partial_clone: GitPartialCloneState,
    pub replace_refs: GitReplaceRefsState,
    pub shallow_history: GitShallowHistoryState,
    pub submodules: GitSubmoduleState,
    pub alternate_object_database: GitAlternateObjectDatabaseState,
}

/// The complete metadata-only Git snapshot.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSnapshotMetadata {
    pub schema_version: u32,
    pub repository_id: RepositoryId,
    pub repository_kind: GitRepositoryKind,
    pub object_format: GitObjectFormat,
    pub head: Option<GitObjectIdRef>,
    pub tree: Option<GitObjectIdRef>,
    pub index: Option<GitObjectIdRef>,
    pub worktree_state: GitWorktreeState,
    pub branch_class: GitBranchClass,
    pub operation: GitOperation,
    pub status: GitStatusCounts,
    pub limitations: GitSourceLimitations,
    pub snapshot_digest: SnapshotDigest,
}

impl GitSnapshotMetadata {
    /// Build a snapshot from a previously validated path-free Git identity.
    ///
    /// This function accepts no path and performs no Git, filesystem, or object
    /// database I/O. All inputs are already-normalized metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn from_identity(
        identity: &GitIdentity,
        object_format: GitObjectFormat,
        head: Option<GitObjectIdRef>,
        tree: Option<GitObjectIdRef>,
        index: Option<GitObjectIdRef>,
        worktree_state: GitWorktreeState,
        branch_class: GitBranchClass,
        operation: GitOperation,
        status: GitStatusCounts,
        limitations: GitSourceLimitations,
    ) -> Result<Self, GitSnapshotError> {
        identity.validate().map_err(|_| GitSnapshotError::InvalidIdentity)?;
        Self::new(
            identity.repository_id(),
            identity.repository_kind,
            object_format,
            head,
            tree,
            index,
            worktree_state,
            branch_class,
            operation,
            status,
            limitations,
        )
    }

    /// Build a snapshot from an opaque repository ID and normalized metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repository_id: RepositoryId,
        repository_kind: GitRepositoryKind,
        object_format: GitObjectFormat,
        head: Option<GitObjectIdRef>,
        tree: Option<GitObjectIdRef>,
        index: Option<GitObjectIdRef>,
        worktree_state: GitWorktreeState,
        branch_class: GitBranchClass,
        operation: GitOperation,
        status: GitStatusCounts,
        limitations: GitSourceLimitations,
    ) -> Result<Self, GitSnapshotError> {
        let mut snapshot = Self {
            schema_version: GIT_SNAPSHOT_SCHEMA_VERSION,
            repository_id,
            repository_kind,
            object_format,
            head,
            tree,
            index,
            worktree_state,
            branch_class,
            operation,
            status,
            limitations,
            snapshot_digest: zero_digest(),
        };
        snapshot.validate_without_digest()?;
        snapshot.snapshot_digest = snapshot.compute_digest()?;
        Ok(snapshot)
    }

    /// Parse and validate a bounded JSON snapshot.
    pub fn parse(input: &str) -> Result<Self, GitSnapshotError> {
        if input.len() > MAX_GIT_SNAPSHOT_BYTES {
            return Err(GitSnapshotError::MetadataTooLarge);
        }
        let snapshot: Self =
            serde_json::from_str(input).map_err(|_| GitSnapshotError::Malformed)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Parse the checked-in deterministic example.
    pub fn checked_in() -> Result<Self, GitSnapshotError> {
        Self::parse(GIT_SNAPSHOT_GOLDEN_JSON)
    }

    /// Validate all semantic, privacy, bound, and digest invariants.
    pub fn validate(&self) -> Result<(), GitSnapshotError> {
        self.validate_without_digest()?;
        if self.snapshot_digest != self.compute_digest()? {
            return Err(GitSnapshotError::DigestMismatch);
        }
        Ok(())
    }

    /// Recompute the digest over metadata only.
    pub fn digest(&self) -> Result<SnapshotDigest, GitSnapshotError> {
        self.compute_digest()
    }

    fn validate_without_digest(&self) -> Result<(), GitSnapshotError> {
        if self.schema_version != GIT_SNAPSHOT_SCHEMA_VERSION {
            return Err(GitSnapshotError::UnsupportedVersion);
        }
        self.status.validate()?;
        for object in
            [self.head.as_ref(), self.tree.as_ref(), self.index.as_ref()].into_iter().flatten()
        {
            if object.algorithm() != self.object_format {
                return Err(GitSnapshotError::ObjectFormatMismatch);
            }
        }
        if self.repository_kind == GitRepositoryKind::Bare {
            if self.worktree_state != GitWorktreeState::NotApplicable
                || self.branch_class != GitBranchClass::NoWorktree
                || self.status.total() != 0
            {
                return Err(GitSnapshotError::RepositoryKindMismatch);
            }
        } else if self.worktree_state == GitWorktreeState::NotApplicable
            || self.branch_class == GitBranchClass::NoWorktree
        {
            return Err(GitSnapshotError::RepositoryKindMismatch);
        }
        if self.repository_kind == GitRepositoryKind::Submodule
            && self.limitations.submodules == GitSubmoduleState::None
        {
            return Err(GitSnapshotError::RepositoryKindMismatch);
        }
        if self.worktree_state == GitWorktreeState::Clean && self.status.total() != 0 {
            return Err(GitSnapshotError::WorktreeStateMismatch);
        }
        if self.worktree_state == GitWorktreeState::Conflicted && self.status.conflicted == 0 {
            return Err(GitSnapshotError::WorktreeStateMismatch);
        }
        if self.worktree_state == GitWorktreeState::UntrackedOnly
            && (self.status.untracked == 0
                || self.status.staged != 0
                || self.status.unstaged != 0
                || self.status.conflicted != 0)
        {
            return Err(GitSnapshotError::WorktreeStateMismatch);
        }
        Ok(())
    }

    fn compute_digest(&self) -> Result<SnapshotDigest, GitSnapshotError> {
        #[derive(Serialize)]
        #[serde(deny_unknown_fields)]
        struct DigestInput<'a> {
            schema_version: u32,
            repository_id: &'a RepositoryId,
            repository_kind: GitRepositoryKind,
            object_format: GitObjectFormat,
            head: &'a Option<GitObjectIdRef>,
            tree: &'a Option<GitObjectIdRef>,
            index: &'a Option<GitObjectIdRef>,
            worktree_state: GitWorktreeState,
            branch_class: GitBranchClass,
            operation: GitOperation,
            status: &'a GitStatusCounts,
            limitations: GitSourceLimitations,
        }
        let input = DigestInput {
            schema_version: self.schema_version,
            repository_id: &self.repository_id,
            repository_kind: self.repository_kind,
            object_format: self.object_format,
            head: &self.head,
            tree: &self.tree,
            index: &self.index,
            worktree_state: self.worktree_state,
            branch_class: self.branch_class,
            operation: self.operation,
            status: &self.status,
            limitations: self.limitations,
        };
        let encoded = serde_json::to_vec(&input).map_err(|_| GitSnapshotError::Malformed)?;
        let mut hasher = Sha256::new();
        hasher.update(b"ghostrace-git-snapshot-metadata-v1\0");
        hasher.update(encoded);
        let digest = hasher.finalize();
        let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        SnapshotDigest::try_from(format!("sha256:{hex}")).map_err(|_| GitSnapshotError::Malformed)
    }
}

fn zero_digest() -> SnapshotDigest {
    SnapshotDigest::try_from(format!("sha256:{}", "0".repeat(64)))
        .expect("zero SHA-256 digest is valid")
}
