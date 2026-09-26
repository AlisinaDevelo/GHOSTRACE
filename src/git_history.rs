//! Git history transitions between two metadata-only snapshots.
//!
//! Local Git history is mutable: a rebase, reset, amend, or force update moves
//! a ref, and garbage collection or a shallow boundary can remove the objects
//! that would prove what came before. A later graph therefore cannot
//! retroactively establish earlier ancestry. This module compares two
//! validated [`GitSnapshotMetadata`] values and one bounded ancestry probe and
//! keeps three facts separate:
//!
//! - the observed ref movement, derived only from the two snapshots;
//! - the ancestry inference, which exists only when the probe answered it and
//!   no source limitation could have substituted the answer;
//! - a typed gap carrying the last known and current bounded state whenever
//!   history was rewritten, missing, truncated, replaced, or not probed.
//!
//! Nothing here runs Git, reads objects, or accepts a ref name or path.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    git_snapshot::{
        GitBranchClass, GitObjectIdRef, GitPartialCloneState, GitReplaceRefsState,
        GitShallowHistoryState, GitSnapshotError, GitSnapshotMetadata,
    },
    model::RepositoryId,
};

/// Version of the Git history transition contract.
pub const GIT_HISTORY_TRANSITION_SCHEMA_VERSION: u32 = 1;

/// Checked-in synthetic transition corpus.
pub const GIT_HISTORY_TRANSITIONS_JSON: &str =
    include_str!("../fixtures/git-history-transitions-v1.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum GitHistoryError {
    #[error("Git history snapshots are invalid")]
    InvalidSnapshot,
    #[error("Git history snapshots belong to different repositories")]
    RepositoryMismatch,
    #[error("Git history snapshots use different object formats")]
    ObjectFormatMismatch,
}

impl From<GitSnapshotError> for GitHistoryError {
    fn from(_: GitSnapshotError) -> Self {
        Self::InvalidSnapshot
    }
}

/// The one ancestry question an adapter may ask Git between two snapshots:
/// whether the previous HEAD is an ancestor of the current HEAD. The adapter
/// reports only the class of answer, never command output.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitAncestryProbe {
    /// Both objects exist and the previous HEAD is an ancestor.
    PreviousIsAncestor,
    /// Both objects exist and the previous HEAD is not an ancestor.
    PreviousNotAncestor,
    /// The previous HEAD object is no longer in the object database.
    PreviousObjectMissing,
    /// The walk reached a shallow boundary before answering.
    ShallowBoundaryReached,
    /// The adapter did not or could not ask.
    NotProbed,
}

/// Ref movement observed from the two snapshots alone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitRefMovement {
    Unchanged,
    HeadMoved,
    /// A branch checkout became a detached HEAD.
    Detached,
    /// A detached HEAD became a branch checkout.
    Attached,
    /// HEAD now has no commit.
    BecameUnborn,
    /// An unborn HEAD received its first commit.
    FirstCommit,
    /// The ref class changed in a way the snapshots cannot classify.
    Unknown,
}

/// Ancestry inferred from the probe. `Unknown` is never promoted to either
/// positive answer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitAncestryInference {
    /// HEAD did not move; no ancestry question arises.
    NotApplicable,
    /// The current HEAD descends from the previous HEAD.
    Descendant,
    /// The previous HEAD is not in the current HEAD's history.
    NotDescendant,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHistoryGapReason {
    /// The previous HEAD is no longer reachable from the current HEAD.
    HistoryRewritten,
    /// An object needed to relate the two states is gone.
    ObjectMissing,
    /// The shallow boundary moved or blocked the ancestry walk.
    ShallowBoundary,
    /// Replace refs are active, so Git may have answered with substitutes.
    ReplacedObjects,
    /// HEAD moved but ancestry was not probed.
    AncestryNotProbed,
}

/// Bounded state retained on both sides of a gap.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHistoryState {
    pub head: Option<GitObjectIdRef>,
    pub branch_class: GitBranchClass,
    pub shallow_history: GitShallowHistoryState,
    pub replace_refs: GitReplaceRefsState,
    pub partial_clone: GitPartialCloneState,
}

impl GitHistoryState {
    fn of(snapshot: &GitSnapshotMetadata) -> Self {
        Self {
            head: snapshot.head.clone(),
            branch_class: snapshot.branch_class,
            shallow_history: snapshot.limitations.shallow_history,
            replace_refs: snapshot.limitations.replace_refs,
            partial_clone: snapshot.limitations.partial_clone,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHistoryGap {
    pub reason: GitHistoryGapReason,
    pub last_known: GitHistoryState,
    pub current: GitHistoryState,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHistoryTransition {
    pub schema_version: u32,
    pub repository_id: RepositoryId,
    pub ref_movement: GitRefMovement,
    pub ancestry: GitAncestryInference,
    pub probe: GitAncestryProbe,
    pub gap: Option<GitHistoryGap>,
}

impl GitHistoryTransition {
    /// Classify the transition from `previous` to `current`.
    pub fn classify(
        previous: &GitSnapshotMetadata,
        current: &GitSnapshotMetadata,
        probe: GitAncestryProbe,
    ) -> Result<Self, GitHistoryError> {
        previous.validate()?;
        current.validate()?;
        if previous.repository_id != current.repository_id {
            return Err(GitHistoryError::RepositoryMismatch);
        }
        if previous.object_format != current.object_format {
            return Err(GitHistoryError::ObjectFormatMismatch);
        }

        let ref_movement = ref_movement(previous, current);
        let head_moved =
            previous.head.is_some() && current.head.is_some() && previous.head != current.head;

        let replaced = previous.limitations.replace_refs != GitReplaceRefsState::None
            || current.limitations.replace_refs != GitReplaceRefsState::None;
        let shallow_changed =
            previous.limitations.shallow_history != current.limitations.shallow_history;

        let (ancestry, mut reason) = if !head_moved {
            (GitAncestryInference::NotApplicable, None)
        } else {
            match probe {
                GitAncestryProbe::PreviousIsAncestor if replaced => {
                    (GitAncestryInference::Unknown, Some(GitHistoryGapReason::ReplacedObjects))
                }
                GitAncestryProbe::PreviousIsAncestor => (GitAncestryInference::Descendant, None),
                GitAncestryProbe::PreviousNotAncestor if replaced => {
                    (GitAncestryInference::Unknown, Some(GitHistoryGapReason::ReplacedObjects))
                }
                GitAncestryProbe::PreviousNotAncestor => (
                    GitAncestryInference::NotDescendant,
                    Some(GitHistoryGapReason::HistoryRewritten),
                ),
                GitAncestryProbe::PreviousObjectMissing => {
                    (GitAncestryInference::Unknown, Some(GitHistoryGapReason::ObjectMissing))
                }
                GitAncestryProbe::ShallowBoundaryReached => {
                    (GitAncestryInference::Unknown, Some(GitHistoryGapReason::ShallowBoundary))
                }
                GitAncestryProbe::NotProbed => {
                    (GitAncestryInference::Unknown, Some(GitHistoryGapReason::AncestryNotProbed))
                }
            }
        };
        // A missing previous object is a gap even when HEAD did not move: the
        // earlier state can no longer be re-derived from the object database.
        if reason.is_none() && probe == GitAncestryProbe::PreviousObjectMissing {
            reason = Some(GitHistoryGapReason::ObjectMissing);
        }
        // A moved shallow boundary changes what history is visible, whatever
        // the ancestry answer was.
        if reason.is_none() && shallow_changed {
            reason = Some(GitHistoryGapReason::ShallowBoundary);
        }

        Ok(Self {
            schema_version: GIT_HISTORY_TRANSITION_SCHEMA_VERSION,
            repository_id: current.repository_id.clone(),
            ref_movement,
            ancestry,
            probe,
            gap: reason.map(|reason| GitHistoryGap {
                reason,
                last_known: GitHistoryState::of(previous),
                current: GitHistoryState::of(current),
            }),
        })
    }
}

fn ref_movement(previous: &GitSnapshotMetadata, current: &GitSnapshotMetadata) -> GitRefMovement {
    use GitBranchClass::{DetachedHead, Local, Unborn};
    match (previous.head.as_ref(), current.head.as_ref()) {
        (Some(_), None) => return GitRefMovement::BecameUnborn,
        (None, Some(_)) if previous.branch_class == Unborn => return GitRefMovement::FirstCommit,
        (None, Some(_)) | (None, None) => {
            return if previous.branch_class == current.branch_class {
                GitRefMovement::Unchanged
            } else {
                GitRefMovement::Unknown
            };
        }
        (Some(_), Some(_)) => {}
    }
    match (previous.branch_class, current.branch_class) {
        (Local, DetachedHead) => GitRefMovement::Detached,
        (DetachedHead, Local) => GitRefMovement::Attached,
        (before, after) if before == after => {
            if previous.head == current.head {
                GitRefMovement::Unchanged
            } else {
                GitRefMovement::HeadMoved
            }
        }
        _ => GitRefMovement::Unknown,
    }
}
