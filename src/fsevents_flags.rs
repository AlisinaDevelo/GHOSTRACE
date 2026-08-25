//! Canonical interpretation of the flags delivered with an FSEvents callback.
//!
//! FSEvents is a lossy notification source.  This module keeps the raw 32-bit
//! value, maps every documented Apple bit to a stable enum, and makes loss,
//! boundaries, contradictions, and future bits visible before a caller can
//! treat the callback as a complete filesystem observation.

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

/// Version of the normalized FSEvents flag evidence contract.
pub const FSEVENTS_NORMALIZED_SCHEMA_VERSION: u32 = 1;

/// Documented `FSEventStreamEventFlags` values from Core Services.
pub const EVENT_FLAG_NONE: u32 = 0x0000_0000;
pub const EVENT_FLAG_MUST_SCAN_SUB_DIRS: u32 = 0x0000_0001;
pub const EVENT_FLAG_USER_DROPPED: u32 = 0x0000_0002;
pub const EVENT_FLAG_KERNEL_DROPPED: u32 = 0x0000_0004;
pub const EVENT_FLAG_EVENT_IDS_WRAPPED: u32 = 0x0000_0008;
pub const EVENT_FLAG_HISTORY_DONE: u32 = 0x0000_0010;
pub const EVENT_FLAG_ROOT_CHANGED: u32 = 0x0000_0020;
pub const EVENT_FLAG_MOUNT: u32 = 0x0000_0040;
pub const EVENT_FLAG_UNMOUNT: u32 = 0x0000_0080;
pub const EVENT_FLAG_ITEM_CHANGE_OWNER: u32 = 0x0000_4000;
pub const EVENT_FLAG_ITEM_CREATED: u32 = 0x0000_0100;
pub const EVENT_FLAG_ITEM_FINDER_INFO_MOD: u32 = 0x0000_2000;
pub const EVENT_FLAG_ITEM_INODE_META_MOD: u32 = 0x0000_0400;
pub const EVENT_FLAG_ITEM_IS_DIR: u32 = 0x0002_0000;
pub const EVENT_FLAG_ITEM_IS_FILE: u32 = 0x0001_0000;
pub const EVENT_FLAG_ITEM_IS_HARDLINK: u32 = 0x0010_0000;
pub const EVENT_FLAG_ITEM_IS_LAST_HARDLINK: u32 = 0x0020_0000;
pub const EVENT_FLAG_ITEM_IS_SYMLINK: u32 = 0x0004_0000;
pub const EVENT_FLAG_ITEM_MODIFIED: u32 = 0x0000_1000;
pub const EVENT_FLAG_ITEM_REMOVED: u32 = 0x0000_0200;
pub const EVENT_FLAG_ITEM_RENAMED: u32 = 0x0000_0800;
pub const EVENT_FLAG_ITEM_XATTR_MOD: u32 = 0x0000_8000;
pub const EVENT_FLAG_OWN_EVENT: u32 = 0x0008_0000;
pub const EVENT_FLAG_ITEM_CLONED: u32 = 0x0040_0000;

/// Every documented non-zero event flag, in canonical numeric order.
pub const DOCUMENTED_EVENT_FLAGS: &[FseventsEventFlag] = &[
    FseventsEventFlag::MustScanSubDirs,
    FseventsEventFlag::UserDropped,
    FseventsEventFlag::KernelDropped,
    FseventsEventFlag::EventIdsWrapped,
    FseventsEventFlag::HistoryDone,
    FseventsEventFlag::RootChanged,
    FseventsEventFlag::Mount,
    FseventsEventFlag::Unmount,
    FseventsEventFlag::ItemCreated,
    FseventsEventFlag::ItemRemoved,
    FseventsEventFlag::ItemInodeMetaMod,
    FseventsEventFlag::ItemRenamed,
    FseventsEventFlag::ItemModified,
    FseventsEventFlag::ItemFinderInfoMod,
    FseventsEventFlag::ItemChangeOwner,
    FseventsEventFlag::ItemXattrMod,
    FseventsEventFlag::ItemIsFile,
    FseventsEventFlag::ItemIsDir,
    FseventsEventFlag::ItemIsSymlink,
    FseventsEventFlag::OwnEvent,
    FseventsEventFlag::ItemIsHardlink,
    FseventsEventFlag::ItemIsLastHardlink,
    FseventsEventFlag::ItemCloned,
];

/// OR of all documented event bits.  Bits outside this mask are retained as
/// numeric evidence and never silently discarded.
pub const DOCUMENTED_EVENT_FLAG_MASK: u32 = EVENT_FLAG_MUST_SCAN_SUB_DIRS
    | EVENT_FLAG_USER_DROPPED
    | EVENT_FLAG_KERNEL_DROPPED
    | EVENT_FLAG_EVENT_IDS_WRAPPED
    | EVENT_FLAG_HISTORY_DONE
    | EVENT_FLAG_ROOT_CHANGED
    | EVENT_FLAG_MOUNT
    | EVENT_FLAG_UNMOUNT
    | EVENT_FLAG_ITEM_CREATED
    | EVENT_FLAG_ITEM_REMOVED
    | EVENT_FLAG_ITEM_INODE_META_MOD
    | EVENT_FLAG_ITEM_RENAMED
    | EVENT_FLAG_ITEM_MODIFIED
    | EVENT_FLAG_ITEM_FINDER_INFO_MOD
    | EVENT_FLAG_ITEM_CHANGE_OWNER
    | EVENT_FLAG_ITEM_XATTR_MOD
    | EVENT_FLAG_ITEM_IS_FILE
    | EVENT_FLAG_ITEM_IS_DIR
    | EVENT_FLAG_ITEM_IS_SYMLINK
    | EVENT_FLAG_OWN_EVENT
    | EVENT_FLAG_ITEM_IS_HARDLINK
    | EVENT_FLAG_ITEM_IS_LAST_HARDLINK
    | EVENT_FLAG_ITEM_CLONED;

/// A documented FSEvents event flag.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FseventsEventFlag {
    MustScanSubDirs,
    UserDropped,
    KernelDropped,
    EventIdsWrapped,
    HistoryDone,
    RootChanged,
    Mount,
    Unmount,
    ItemCreated,
    ItemRemoved,
    ItemInodeMetaMod,
    ItemRenamed,
    ItemModified,
    ItemFinderInfoMod,
    ItemChangeOwner,
    ItemXattrMod,
    ItemIsFile,
    ItemIsDir,
    ItemIsSymlink,
    OwnEvent,
    ItemIsHardlink,
    ItemIsLastHardlink,
    ItemCloned,
}

impl FseventsEventFlag {
    /// Returns the exact Core Services bit for this flag.
    pub const fn mask(self) -> u32 {
        match self {
            Self::MustScanSubDirs => EVENT_FLAG_MUST_SCAN_SUB_DIRS,
            Self::UserDropped => EVENT_FLAG_USER_DROPPED,
            Self::KernelDropped => EVENT_FLAG_KERNEL_DROPPED,
            Self::EventIdsWrapped => EVENT_FLAG_EVENT_IDS_WRAPPED,
            Self::HistoryDone => EVENT_FLAG_HISTORY_DONE,
            Self::RootChanged => EVENT_FLAG_ROOT_CHANGED,
            Self::Mount => EVENT_FLAG_MOUNT,
            Self::Unmount => EVENT_FLAG_UNMOUNT,
            Self::ItemCreated => EVENT_FLAG_ITEM_CREATED,
            Self::ItemRemoved => EVENT_FLAG_ITEM_REMOVED,
            Self::ItemInodeMetaMod => EVENT_FLAG_ITEM_INODE_META_MOD,
            Self::ItemRenamed => EVENT_FLAG_ITEM_RENAMED,
            Self::ItemModified => EVENT_FLAG_ITEM_MODIFIED,
            Self::ItemFinderInfoMod => EVENT_FLAG_ITEM_FINDER_INFO_MOD,
            Self::ItemChangeOwner => EVENT_FLAG_ITEM_CHANGE_OWNER,
            Self::ItemXattrMod => EVENT_FLAG_ITEM_XATTR_MOD,
            Self::ItemIsFile => EVENT_FLAG_ITEM_IS_FILE,
            Self::ItemIsDir => EVENT_FLAG_ITEM_IS_DIR,
            Self::ItemIsSymlink => EVENT_FLAG_ITEM_IS_SYMLINK,
            Self::OwnEvent => EVENT_FLAG_OWN_EVENT,
            Self::ItemIsHardlink => EVENT_FLAG_ITEM_IS_HARDLINK,
            Self::ItemIsLastHardlink => EVENT_FLAG_ITEM_IS_LAST_HARDLINK,
            Self::ItemCloned => EVENT_FLAG_ITEM_CLONED,
        }
    }
}

/// The canonical lossless representation of one raw FSEvents flag word.
///
/// `raw_flags` is retained for forward compatibility. `known_flags` is sorted
/// by numeric bit and `unknown_bits` is the exact bounded remainder. A caller
/// must not infer completeness from `known_flags` alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FseventsFlagSet {
    pub raw_flags: u32,
    pub known_flags: Vec<FseventsEventFlag>,
    pub unknown_bits: u32,
}

impl FseventsFlagSet {
    pub fn from_raw(raw_flags: u32) -> Self {
        let known_flags = DOCUMENTED_EVENT_FLAGS
            .iter()
            .copied()
            .filter(|flag| raw_flags & flag.mask() != 0)
            .collect();
        Self { raw_flags, known_flags, unknown_bits: raw_flags & !DOCUMENTED_EVENT_FLAG_MASK }
    }

    pub fn contains(&self, flag: FseventsEventFlag) -> bool {
        self.raw_flags & flag.mask() != 0
    }

    pub fn is_none(&self) -> bool {
        self.raw_flags == EVENT_FLAG_NONE
    }

    pub fn has_unknown_bits(&self) -> bool {
        self.unknown_bits != 0
    }

    pub fn unknown_bit_count(&self) -> u32 {
        self.unknown_bits.count_ones()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedFseventsFlagSet {
    raw_flags: u32,
    known_flags: Vec<FseventsEventFlag>,
    unknown_bits: u32,
}

impl<'de> Deserialize<'de> for FseventsFlagSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UncheckedFseventsFlagSet::deserialize(deserializer)?;
        let canonical = Self::from_raw(raw.raw_flags);
        if canonical.known_flags != raw.known_flags || canonical.unknown_bits != raw.unknown_bits {
            return Err(D::Error::custom("FSEvents flag set is not canonical"));
        }
        Ok(canonical)
    }
}

/// The reason an event requires a repair/rescan boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FseventsRescanReason {
    EventIdsWrapped,
    BothDropped,
    KernelDropped,
    UserDropped,
    MustScanSubDirs,
}

/// A source boundary that remains visible even when no event was dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FseventsBoundaryReason {
    RootChanged,
    Unmount,
    Mount,
    HistoryDone,
}

/// A documented combination that cannot be interpreted as one canonical item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FseventsContradictionReason {
    MultipleEntryKinds,
    MountAndUnmount,
}

/// Primary status assigned to a normalized callback event.
///
/// Unknown future bits use `Unsupported` as an explicit refusal to claim full
/// semantics. The raw and unknown numeric values remain in [`FseventsFlagSet`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FseventsEvidenceStatus {
    Observed,
    RescanRequired { reason: FseventsRescanReason },
    Boundary { reason: FseventsBoundaryReason },
    Unsupported { unknown_bits: u32 },
    Contradictory { reason: FseventsContradictionReason },
}

/// Whether the source can support a complete interpretation of this event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FseventsCompleteness {
    Complete,
    Lowered,
}

/// Normalized, path-free evidence from one FSEvents callback item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedFseventsEvent {
    pub schema_version: u32,
    pub event_id: u64,
    pub flags: FseventsFlagSet,
    pub status: FseventsEvidenceStatus,
    pub completeness: FseventsCompleteness,
}

impl NormalizedFseventsEvent {
    pub fn is_complete(&self) -> bool {
        self.completeness == FseventsCompleteness::Complete
    }

    pub fn status_code(&self) -> &'static str {
        match self.status {
            FseventsEvidenceStatus::Observed => "observed",
            FseventsEvidenceStatus::RescanRequired { .. } => "rescan_required",
            FseventsEvidenceStatus::Boundary { .. } => "boundary",
            FseventsEvidenceStatus::Unsupported { .. } => "unsupported",
            FseventsEvidenceStatus::Contradictory { .. } => "contradictory",
        }
    }

    /// Return the stable gap reason for a coverage-changing callback.
    ///
    /// The combined dropped case is intentionally distinct from each
    /// individual source flag; callers can still retain the raw flag set for
    /// the complete source evidence.
    pub fn gap_reason_code(&self) -> Option<&'static str> {
        match self.status {
            FseventsEvidenceStatus::RescanRequired { reason } => Some(match reason {
                FseventsRescanReason::EventIdsWrapped => "fsevents_event_ids_wrapped",
                FseventsRescanReason::BothDropped => "fsevents_both_dropped",
                FseventsRescanReason::KernelDropped => "fsevents_kernel_dropped",
                FseventsRescanReason::UserDropped => "fsevents_user_dropped",
                FseventsRescanReason::MustScanSubDirs => "fsevents_must_scan_sub_dirs",
            }),
            FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::RootChanged } => {
                Some("fsevents_root_changed")
            }
            FseventsEvidenceStatus::Observed
            | FseventsEvidenceStatus::Boundary { .. }
            | FseventsEvidenceStatus::Unsupported { .. }
            | FseventsEvidenceStatus::Contradictory { .. } => None,
        }
    }
}

/// Normalize one callback event ID and raw Core Services flag word.
pub fn normalize_fsevents_event(event_id: u64, raw_flags: u32) -> NormalizedFseventsEvent {
    let flags = FseventsFlagSet::from_raw(raw_flags);
    let status = status_for(&flags);
    let completeness = match status {
        FseventsEvidenceStatus::Observed
        | FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::HistoryDone } => {
            if flags.has_unknown_bits() {
                FseventsCompleteness::Lowered
            } else {
                FseventsCompleteness::Complete
            }
        }
        FseventsEvidenceStatus::Boundary { .. }
        | FseventsEvidenceStatus::RescanRequired { .. }
        | FseventsEvidenceStatus::Unsupported { .. }
        | FseventsEvidenceStatus::Contradictory { .. } => FseventsCompleteness::Lowered,
    };
    NormalizedFseventsEvent {
        schema_version: FSEVENTS_NORMALIZED_SCHEMA_VERSION,
        event_id,
        flags,
        status,
        completeness,
    }
}

fn status_for(flags: &FseventsFlagSet) -> FseventsEvidenceStatus {
    if flags.contains(FseventsEventFlag::ItemIsFile) && flags.contains(FseventsEventFlag::ItemIsDir)
    {
        return FseventsEvidenceStatus::Contradictory {
            reason: FseventsContradictionReason::MultipleEntryKinds,
        };
    }
    if flags.contains(FseventsEventFlag::Mount) && flags.contains(FseventsEventFlag::Unmount) {
        return FseventsEvidenceStatus::Contradictory {
            reason: FseventsContradictionReason::MountAndUnmount,
        };
    }
    if flags.has_unknown_bits() {
        return FseventsEvidenceStatus::Unsupported { unknown_bits: flags.unknown_bits };
    }
    if let Some(reason) = rescan_reason(flags) {
        return FseventsEvidenceStatus::RescanRequired { reason };
    }
    if flags.contains(FseventsEventFlag::RootChanged) {
        return FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::RootChanged };
    }
    if flags.contains(FseventsEventFlag::Unmount) {
        return FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::Unmount };
    }
    if flags.contains(FseventsEventFlag::Mount) {
        return FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::Mount };
    }
    if flags.contains(FseventsEventFlag::HistoryDone) {
        return FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::HistoryDone };
    }
    FseventsEvidenceStatus::Observed
}

fn rescan_reason(flags: &FseventsFlagSet) -> Option<FseventsRescanReason> {
    if flags.contains(FseventsEventFlag::EventIdsWrapped) {
        return Some(FseventsRescanReason::EventIdsWrapped);
    }
    let user_dropped = flags.contains(FseventsEventFlag::UserDropped);
    let kernel_dropped = flags.contains(FseventsEventFlag::KernelDropped);
    if user_dropped && kernel_dropped {
        return Some(FseventsRescanReason::BothDropped);
    }
    if kernel_dropped {
        return Some(FseventsRescanReason::KernelDropped);
    }
    if user_dropped {
        return Some(FseventsRescanReason::UserDropped);
    }
    if flags.contains(FseventsEventFlag::MustScanSubDirs) {
        return Some(FseventsRescanReason::MustScanSubDirs);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_mask_and_table_are_complete() {
        let mut mask = 0;
        for flag in DOCUMENTED_EVENT_FLAGS {
            assert_eq!(mask & flag.mask(), 0, "duplicate documented flag");
            mask |= flag.mask();
        }
        assert_eq!(mask, DOCUMENTED_EVENT_FLAG_MASK);
        assert_eq!(DOCUMENTED_EVENT_FLAGS.len(), 23);
        assert_eq!(FseventsFlagSet::from_raw(0).known_flags, Vec::new());
    }

    #[test]
    fn unknown_bits_are_retained_and_lower_completeness() {
        let event = normalize_fsevents_event(9, EVENT_FLAG_ITEM_MODIFIED | (1 << 31));
        assert_eq!(event.flags.raw_flags, EVENT_FLAG_ITEM_MODIFIED | (1 << 31));
        assert_eq!(event.flags.unknown_bits, 1 << 31);
        assert_eq!(event.flags.unknown_bit_count(), 1);
        assert_eq!(event.status, FseventsEvidenceStatus::Unsupported { unknown_bits: 1 << 31 });
        assert!(!event.is_complete());
    }

    #[test]
    fn loss_and_boundaries_have_deterministic_precedence() {
        let dropped = normalize_fsevents_event(
            10,
            EVENT_FLAG_MUST_SCAN_SUB_DIRS
                | EVENT_FLAG_USER_DROPPED
                | EVENT_FLAG_KERNEL_DROPPED
                | EVENT_FLAG_ITEM_IS_DIR,
        );
        assert_eq!(
            dropped.status,
            FseventsEvidenceStatus::RescanRequired { reason: FseventsRescanReason::BothDropped }
        );
        assert!(!dropped.is_complete());

        let history = normalize_fsevents_event(11, EVENT_FLAG_HISTORY_DONE);
        assert_eq!(
            history.status,
            FseventsEvidenceStatus::Boundary { reason: FseventsBoundaryReason::HistoryDone }
        );
        assert!(history.is_complete());
    }

    #[test]
    fn contradictory_types_are_refused_without_dropping_raw_flags() {
        let event = normalize_fsevents_event(
            12,
            EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_IS_DIR | EVENT_FLAG_ITEM_RENAMED,
        );
        assert_eq!(event.flags.known_flags.len(), 3);
        assert_eq!(
            event.status,
            FseventsEvidenceStatus::Contradictory {
                reason: FseventsContradictionReason::MultipleEntryKinds
            }
        );
        assert!(!event.is_complete());
    }

    #[test]
    fn canonical_flag_set_round_trip_rejects_drift() {
        let flags = FseventsFlagSet::from_raw(
            EVENT_FLAG_ITEM_CREATED | EVENT_FLAG_ITEM_IS_FILE | EVENT_FLAG_ITEM_CLONED,
        );
        let json = serde_json::to_string(&flags).expect("serialize flags");
        let round_trip: FseventsFlagSet = serde_json::from_str(&json).expect("round trip flags");
        assert_eq!(round_trip, flags);
        let drift = json.replace("item_created", "item_removed");
        assert!(serde_json::from_str::<FseventsFlagSet>(&drift).is_err());
    }
}
