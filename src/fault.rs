//! Deterministic, opt-in fault injection for the storage recovery matrix.
//!
//! A plan is inert unless a caller explicitly supplies it to a journal
//! constructor.  It is intended for fixture tests and recovery drills, not for
//! live collection.  Schedules are bounded, named, and occurrence-based so a
//! failing case can be reduced to one stable JSON record.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::error::GhostraceError;

const MAX_SCHEDULES: usize = 128;
const MAX_OCCURRENCE: u32 = 1024;

/// Durable boundaries at which a test may fail or terminate the process.
///
/// The points intentionally name the operation and phase.  A schedule's
/// `occurrence` disambiguates repeated migrations or events in one operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultPoint {
    StorageBeforeOpen,
    StorageAfterOpen,
    StorageBeforeVerify,
    StorageAfterVerify,
    MigrationBeforeTransaction,
    MigrationAfterSql,
    MigrationBeforeCommit,
    MigrationAfterCommit,
    IngestBeforeTransaction,
    IngestAfterTransaction,
    KeyBeforeAccess,
    KeyAfterAccess,
    EventBeforeInsert,
    EventAfterInsert,
    CursorBeforeUpdate,
    CursorAfterUpdate,
    DiagnosticBeforeInsert,
    DiagnosticAfterInsert,
    IngestBeforeCommit,
    IngestAfterCommit,
    ControlBeforeTransaction,
    ControlAfterTransaction,
    ControlBeforeCommit,
    ControlAfterCommit,
    CheckpointBefore,
    CheckpointAfter,
    BackupBeforeCopy,
    BackupAfterCopy,
}

impl FaultPoint {
    pub const ALL: [Self; 28] = [
        Self::StorageBeforeOpen,
        Self::StorageAfterOpen,
        Self::StorageBeforeVerify,
        Self::StorageAfterVerify,
        Self::MigrationBeforeTransaction,
        Self::MigrationAfterSql,
        Self::MigrationBeforeCommit,
        Self::MigrationAfterCommit,
        Self::IngestBeforeTransaction,
        Self::IngestAfterTransaction,
        Self::KeyBeforeAccess,
        Self::KeyAfterAccess,
        Self::EventBeforeInsert,
        Self::EventAfterInsert,
        Self::CursorBeforeUpdate,
        Self::CursorAfterUpdate,
        Self::DiagnosticBeforeInsert,
        Self::DiagnosticAfterInsert,
        Self::IngestBeforeCommit,
        Self::IngestAfterCommit,
        Self::ControlBeforeTransaction,
        Self::ControlAfterTransaction,
        Self::ControlBeforeCommit,
        Self::ControlAfterCommit,
        Self::CheckpointBefore,
        Self::CheckpointAfter,
        Self::BackupBeforeCopy,
        Self::BackupAfterCopy,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StorageBeforeOpen => "storage_before_open",
            Self::StorageAfterOpen => "storage_after_open",
            Self::StorageBeforeVerify => "storage_before_verify",
            Self::StorageAfterVerify => "storage_after_verify",
            Self::MigrationBeforeTransaction => "migration_before_transaction",
            Self::MigrationAfterSql => "migration_after_sql",
            Self::MigrationBeforeCommit => "migration_before_commit",
            Self::MigrationAfterCommit => "migration_after_commit",
            Self::IngestBeforeTransaction => "ingest_before_transaction",
            Self::IngestAfterTransaction => "ingest_after_transaction",
            Self::KeyBeforeAccess => "key_before_access",
            Self::KeyAfterAccess => "key_after_access",
            Self::EventBeforeInsert => "event_before_insert",
            Self::EventAfterInsert => "event_after_insert",
            Self::CursorBeforeUpdate => "cursor_before_update",
            Self::CursorAfterUpdate => "cursor_after_update",
            Self::DiagnosticBeforeInsert => "diagnostic_before_insert",
            Self::DiagnosticAfterInsert => "diagnostic_after_insert",
            Self::IngestBeforeCommit => "ingest_before_commit",
            Self::IngestAfterCommit => "ingest_after_commit",
            Self::ControlBeforeTransaction => "control_before_transaction",
            Self::ControlAfterTransaction => "control_after_transaction",
            Self::ControlBeforeCommit => "control_before_commit",
            Self::ControlAfterCommit => "control_after_commit",
            Self::CheckpointBefore => "checkpoint_before",
            Self::CheckpointAfter => "checkpoint_after",
            Self::BackupBeforeCopy => "backup_before_copy",
            Self::BackupAfterCopy => "backup_after_copy",
        }
    }
}

impl fmt::Display for FaultPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Action taken when a schedule reaches its occurrence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultAction {
    /// Return a bounded error so the caller can assert transaction rollback.
    Return,
    /// Abort the current process, emulating power loss or SIGKILL at a named
    /// boundary.  This action must be exercised in a child process.
    Abort,
}

/// One minimized, reproducible fault schedule.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FaultSchedule {
    pub point: FaultPoint,
    pub occurrence: u32,
    pub action: FaultAction,
}

#[derive(Debug, Default)]
struct FaultState {
    counts: BTreeMap<FaultPoint, u32>,
    fired: Vec<FaultSchedule>,
}

/// An explicit fault schedule shared by the journal's operations.
#[derive(Clone, Debug)]
pub struct FaultPlan {
    seed: u64,
    schedules: Arc<Vec<FaultSchedule>>,
    state: Arc<Mutex<FaultState>>,
}

impl FaultPlan {
    /// Construct an inert plan.  This is the default for every public journal
    /// constructor.
    pub fn none() -> Self {
        Self::from_schedules(0, Vec::new()).expect("empty fault schedule is valid")
    }

    /// Construct a bounded plan from a retained schedule fixture.
    pub fn from_schedules(
        seed: u64,
        schedules: Vec<FaultSchedule>,
    ) -> Result<Self, GhostraceError> {
        if schedules.len() > MAX_SCHEDULES {
            return Err(GhostraceError::InvalidFaultPlan(
                "fault schedule exceeds the 128-entry bound".to_owned(),
            ));
        }
        if schedules
            .iter()
            .any(|schedule| schedule.occurrence == 0 || schedule.occurrence > MAX_OCCURRENCE)
        {
            return Err(GhostraceError::InvalidFaultPlan(
                "fault occurrence must be between 1 and 1024".to_owned(),
            ));
        }
        Ok(Self {
            seed,
            schedules: Arc::new(schedules),
            state: Arc::new(Mutex::new(FaultState::default())),
        })
    }

    pub fn fail_once(point: FaultPoint) -> Self {
        Self::from_schedules(
            0,
            vec![FaultSchedule { point, occurrence: 1, action: FaultAction::Return }],
        )
        .expect("single fault schedule is valid")
    }

    pub fn abort_once(point: FaultPoint) -> Self {
        Self::from_schedules(
            0,
            vec![FaultSchedule { point, occurrence: 1, action: FaultAction::Abort }],
        )
        .expect("single fault schedule is valid")
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn schedules(&self) -> &[FaultSchedule] {
        self.schedules.as_slice()
    }

    /// Observe one named point.  The plan counts every occurrence, then fires
    /// only the matching schedule.  A poisoned test mutex is a bounded fault,
    /// never a reason to continue without the matrix's safety gate.
    pub(crate) fn hit(&self, point: FaultPoint) -> Result<(), GhostraceError> {
        let action = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| GhostraceError::InjectedFault { point: point.to_string() })?;
            let count = state.counts.entry(point).or_default();
            *count = count.saturating_add(1);
            self.schedules
                .iter()
                .find(|schedule| schedule.point == point && schedule.occurrence == *count)
                .copied()
                .inspect(|schedule| state.fired.push(*schedule))
                .map(|schedule| schedule.action)
        };
        match action {
            Some(FaultAction::Return) => {
                Err(GhostraceError::InjectedFault { point: point.to_string() })
            }
            Some(FaultAction::Abort) => std::process::abort(),
            None => Ok(()),
        }
    }

    pub fn fired(&self) -> Vec<FaultSchedule> {
        self.state.lock().map(|state| state.fired.clone()).unwrap_or_default()
    }

    pub fn counts(&self) -> BTreeMap<String, u32> {
        self.state
            .lock()
            .map(|state| {
                state.counts.iter().map(|(point, count)| (point.to_string(), *count)).collect()
            })
            .unwrap_or_default()
    }
}

impl Default for FaultPlan {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_bounded_and_occurrence_based() {
        let plan = FaultPlan::from_schedules(
            42,
            vec![FaultSchedule {
                point: FaultPoint::EventBeforeInsert,
                occurrence: 2,
                action: FaultAction::Return,
            }],
        )
        .expect("plan");
        assert_eq!(plan.seed(), 42);
        assert!(plan.hit(FaultPoint::EventBeforeInsert).is_ok());
        assert!(matches!(
            plan.hit(FaultPoint::EventBeforeInsert),
            Err(GhostraceError::InjectedFault { .. })
        ));
        assert_eq!(plan.fired().len(), 1);
        assert_eq!(plan.counts()[FaultPoint::EventBeforeInsert.as_str()], 2);
    }
}
