//! Offline, key-free health inspection. Only fixed enums and aggregate counts
//! cross this boundary: never format a database value, path, or underlying error.
//! Availability is not integrity, authenticity, permission, or runtime health.

use std::{
    fmt::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rusqlite::{limits::Limit, Connection};
use serde::Serialize;

use crate::{error::GhostraceError, journal, storage};

pub const HEALTH_SCHEMA_VERSION: u32 = 1;
pub const HEALTH_QUERY_TIMEOUT: Duration = Duration::from_secs(2);
pub const HEALTH_MAX_VM_STEPS: usize = 2_000_000;
pub const HEALTH_MAX_SIDECAR_BYTES: u64 = 64 * 1024 * 1024;
pub const HEALTH_MAX_SQL_VALUE_BYTES: i32 = 64 * 1024;
const PROGRESS_INTERVAL: usize = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    AvailableUnverified,
    NeedsAttention,
    NotChecked,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthReason {
    MetadataReadable,
    MissingStorage,
    UnsafeStorage,
    UnreadableStorage,
    UnsupportedFormat,
    StorageBusy,
    StorageBudgetExceeded,
    QueryBudgetExceeded,
    CoverageGaps,
    InactiveOrUnknownCursor,
    NoPolicyRecords,
    NoEventsObserved,
    NoCursorRecords,
    KeyNotRead,
    RuntimeNotProbed,
    PermissionNotProbed,
    OfflineNoUpdateCheck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthRemediation {
    VerifyWithConfiguredKey,
    SelectExistingJournal,
    ReviewPrivateStorage,
    PreserveAndInspectRecovery,
    UseCompatibleVersion,
    RetryAtQuietTime,
    ReviewCoverageLocally,
    ReviewCursorLocally,
    ReviewPolicyLocally,
    ReviewKeyCustody,
    InspectRuntimeLocally,
    ReviewPermissionsLocally,
    ReviewReleaseLocally,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct HealthComponent {
    pub status: HealthStatus,
    pub reason: HealthReason,
    pub remediation: HealthRemediation,
}

impl HealthComponent {
    fn new(status: HealthStatus, reason: HealthReason, remediation: HealthRemediation) -> Self {
        Self { status, reason, remediation }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HealthCounts {
    pub events: u64,
    pub gaps: u64,
    pub policy_versions: u64,
    pub cursors: u64,
    pub inactive_or_unknown_cursors: u64,
    pub diagnostics: u64,
}

/// Serialize-only output. Versions come from this binary, never the input.
#[derive(Clone, Debug, Serialize)]
pub struct HealthReport {
    pub schema_version: u32,
    pub software_version: &'static str,
    /// Present only after the compiled migration and column contract matches.
    pub journal_schema_version: Option<u32>,
    pub journal: HealthComponent,
    pub key: HealthComponent,
    pub collectors: HealthComponent,
    pub policy: HealthComponent,
    pub cursors: HealthComponent,
    pub coverage: HealthComponent,
    pub service: HealthComponent,
    pub permissions: HealthComponent,
    pub updates: HealthComponent,
    /// Null on every failed read: an unavailable count must not become zero.
    pub counts: Option<HealthCounts>,
}

impl HealthReport {
    /// Does not create a home, migrate, checkpoint, decrypt, consult Keychain,
    /// contact a service/browser, inspect system permissions, or use a network.
    pub fn inspect(path: &Path) -> Self {
        let mut report = Self::unknown();
        match inspect_counts(path) {
            Ok((counts, schema_version)) => {
                report.journal = available();
                report.journal_schema_version = Some(schema_version);
                report.policy = if counts.policy_versions == 0 {
                    HealthComponent::new(
                        HealthStatus::NeedsAttention,
                        HealthReason::NoPolicyRecords,
                        HealthRemediation::ReviewPolicyLocally,
                    )
                } else {
                    available()
                };
                report.cursors = if counts.cursors == 0 {
                    HealthComponent::new(
                        HealthStatus::NotChecked,
                        HealthReason::NoCursorRecords,
                        HealthRemediation::ReviewCursorLocally,
                    )
                } else if counts.inactive_or_unknown_cursors > 0 {
                    HealthComponent::new(
                        HealthStatus::NeedsAttention,
                        HealthReason::InactiveOrUnknownCursor,
                        HealthRemediation::ReviewCursorLocally,
                    )
                } else {
                    available()
                };
                report.coverage = if counts.gaps > 0 {
                    HealthComponent::new(
                        HealthStatus::NeedsAttention,
                        HealthReason::CoverageGaps,
                        HealthRemediation::ReviewCoverageLocally,
                    )
                } else if counts.events == 0 {
                    HealthComponent::new(
                        HealthStatus::NotChecked,
                        HealthReason::NoEventsObserved,
                        HealthRemediation::ReviewCoverageLocally,
                    )
                } else {
                    available()
                };
                report.counts = Some(counts);
            }
            Err((reason, remediation)) => {
                report.journal =
                    HealthComponent::new(HealthStatus::Unavailable, reason, remediation);
            }
        }
        report
    }

    pub fn readable(&self) -> bool {
        self.counts.is_some()
    }

    /// Human text uses only constant labels, enum codes and aggregate counts.
    pub fn human(&self) -> String {
        let mut output = format!("GHOSTRACE {} offline health\nAvailability is not authenticated integrity. No key was read.\n", self.software_version);
        for (name, component) in [
            ("journal", &self.journal),
            ("key", &self.key),
            ("collectors", &self.collectors),
            ("policy", &self.policy),
            ("cursors", &self.cursors),
            ("coverage", &self.coverage),
            ("service", &self.service),
            ("permissions", &self.permissions),
            ("updates", &self.updates),
        ] {
            // Enum serializers cannot contain database or caller-controlled text.
            let status = serde_json::to_string(&component.status).expect("fixed status");
            let reason = serde_json::to_string(&component.reason).expect("fixed reason");
            writeln!(
                &mut output,
                "{name}: {} ({})",
                status.trim_matches('"'),
                reason.trim_matches('"')
            )
            .unwrap();
            writeln!(&mut output, "  {}", component.remediation.text()).unwrap();
        }
        if let Some(counts) = &self.counts {
            writeln!(&mut output, "Counts: {} events, {} gaps, {} policy versions, {} cursors ({} inactive/unknown), {} diagnostics.",
                counts.events, counts.gaps, counts.policy_versions, counts.cursors,
                counts.inactive_or_unknown_cursors, counts.diagnostics).unwrap();
        } else {
            output.push_str("Counts: unknown.\n");
        }
        output
    }

    fn unknown() -> Self {
        let runtime = HealthComponent::new(
            HealthStatus::NotChecked,
            HealthReason::RuntimeNotProbed,
            HealthRemediation::InspectRuntimeLocally,
        );
        Self {
            schema_version: HEALTH_SCHEMA_VERSION,
            software_version: env!("CARGO_PKG_VERSION"),
            journal_schema_version: None,
            journal: runtime,
            key: HealthComponent::new(
                HealthStatus::NotChecked,
                HealthReason::KeyNotRead,
                HealthRemediation::ReviewKeyCustody,
            ),
            collectors: runtime,
            policy: runtime,
            cursors: runtime,
            coverage: runtime,
            service: runtime,
            permissions: HealthComponent::new(
                HealthStatus::NotChecked,
                HealthReason::PermissionNotProbed,
                HealthRemediation::ReviewPermissionsLocally,
            ),
            updates: HealthComponent::new(
                HealthStatus::NotChecked,
                HealthReason::OfflineNoUpdateCheck,
                HealthRemediation::ReviewReleaseLocally,
            ),
            counts: None,
        }
    }
}

impl HealthRemediation {
    fn text(self) -> &'static str {
        match self {
            Self::VerifyWithConfiguredKey => "Verify authenticated state separately using the journal's configured key; metadata alone is not trusted.",
            Self::SelectExistingJournal => "Select an existing journal. Health inspection never creates one.",
            Self::ReviewPrivateStorage => "Review ownership, private file/directory modes and link safety locally; do not loosen permissions.",
            Self::PreserveAndInspectRecovery => "Preserve the original journal and inspect the documented recovery procedure before repair.",
            Self::UseCompatibleVersion => "Use a version compatible with this journal. This command performs no migration.",
            Self::RetryAtQuietTime => "Retry when other journal activity is quiet; large reads are refused within a fixed query budget.",
            Self::ReviewCoverageLocally => "Inspect coverage gaps locally. Gaps are not evidence of observed activity.",
            Self::ReviewCursorLocally => "Inspect cursor state locally before resuming collection; do not reset blindly.",
            Self::ReviewPolicyLocally => "Inspect source consent and policy locally before recording; no authorization is inferred here.",
            Self::ReviewKeyCustody => "Review key custody locally. This report never reads a key or requests keychain approval.",
            Self::InspectRuntimeLocally => "Inspect the explicit collector/service process locally; this offline report cannot establish whether it is running.",
            Self::ReviewPermissionsLocally => "Review operating-system permissions locally. No permission was queried or requested.",
            Self::ReviewReleaseLocally => "Compare the reported version with trusted release information locally. No update server was contacted.",
        }
    }
}

fn available() -> HealthComponent {
    HealthComponent::new(
        HealthStatus::AvailableUnverified,
        HealthReason::MetadataReadable,
        HealthRemediation::VerifyWithConfiguredKey,
    )
}

type Refusal = (HealthReason, HealthRemediation);

fn inspect_counts(path: &Path) -> Result<(HealthCounts, u32), Refusal> {
    // Refuse absent paths before the secure storage helper can prepare a parent.
    std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            (HealthReason::MissingStorage, HealthRemediation::SelectExistingJournal)
        } else {
            (HealthReason::UnreadableStorage, HealthRemediation::PreserveAndInspectRecovery)
        }
    })?;
    let connection = storage::open_bounded_read_only_database(path, HEALTH_MAX_SIDECAR_BYTES)
        .map_err(classify_error)?;
    install_sql_limits(&connection).map_err(|error| classify_error(error.into()))?;
    let exceeded =
        install_budget(&connection, HEALTH_MAX_VM_STEPS, HEALTH_QUERY_TIMEOUT).map_err(|_| {
            (HealthReason::UnreadableStorage, HealthRemediation::PreserveAndInspectRecovery)
        })?;
    let result = read_counts(&connection);
    result.map_err(|error| {
        if exceeded.load(Ordering::Relaxed) {
            (HealthReason::QueryBudgetExceeded, HealthRemediation::RetryAtQuietTime)
        } else {
            classify_error(error)
        }
    })
}

fn classify_error(error: GhostraceError) -> Refusal {
    match error {
        GhostraceError::UnsafePath
        | GhostraceError::PathRace
        | GhostraceError::UnexpectedOwner
        | GhostraceError::UnexpectedHardLinks
        | GhostraceError::InsecurePermissions(_) => {
            (HealthReason::UnsafeStorage, HealthRemediation::ReviewPrivateStorage)
        }
        GhostraceError::ReadOnlyResourceLimit => {
            (HealthReason::StorageBudgetExceeded, HealthRemediation::PreserveAndInspectRecovery)
        }
        GhostraceError::FutureMigration { .. } | GhostraceError::UnsupportedDowngrade { .. } => {
            (HealthReason::UnsupportedFormat, HealthRemediation::UseCompatibleVersion)
        }
        GhostraceError::Database(rusqlite::Error::SqliteFailure(code, _))
            if matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            (HealthReason::StorageBusy, HealthRemediation::RetryAtQuietTime)
        }
        GhostraceError::Database(rusqlite::Error::SqliteFailure(code, _))
            if code.code == rusqlite::ErrorCode::TooBig =>
        {
            (HealthReason::StorageBudgetExceeded, HealthRemediation::PreserveAndInspectRecovery)
        }
        _ => (HealthReason::UnreadableStorage, HealthRemediation::PreserveAndInspectRecovery),
    }
}

fn install_sql_limits(connection: &Connection) -> rusqlite::Result<()> {
    // SQLite opcode budgets alone cannot bound a single string/BLOB operation
    // or schema parse. Set connection-local limits before the first query.
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, HEALTH_MAX_SQL_VALUE_BYTES)?;
    connection.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, HEALTH_MAX_SQL_VALUE_BYTES)?;
    connection.set_limit(Limit::SQLITE_LIMIT_COLUMN, 32)?;
    connection.set_limit(Limit::SQLITE_LIMIT_EXPR_DEPTH, 64)?;
    Ok(())
}

fn install_budget(
    connection: &Connection,
    max_steps: usize,
    timeout: Duration,
) -> rusqlite::Result<Arc<AtomicBool>> {
    let exceeded = Arc::new(AtomicBool::new(false));
    let flag = exceeded.clone();
    let steps = AtomicUsize::new(0);
    let started = Instant::now();
    connection.progress_handler(
        PROGRESS_INTERVAL as i32,
        Some(move || {
            let stop = steps
                .fetch_add(PROGRESS_INTERVAL, Ordering::Relaxed)
                .saturating_add(PROGRESS_INTERVAL)
                >= max_steps
                || started.elapsed() >= timeout;
            if stop {
                flag.store(true, Ordering::Relaxed);
            }
            stop
        }),
    )?;
    Ok(exceeded)
}

fn read_counts(connection: &Connection) -> Result<(HealthCounts, u32), GhostraceError> {
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.execute_batch(
        "PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-2048; BEGIN DEFERRED;",
    )?;
    let schema_version = journal::validate_read_only_schema(connection)?;
    let compared_fields_bounded: bool = connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM events WHERE typeof(kind)!='text' OR length(CAST(kind AS BLOB))>32)
            AND NOT EXISTS(SELECT 1 FROM cursors WHERE typeof(state)!='text' OR length(CAST(state AS BLOB))>32)",
        [], |row| row.get(0),
    )?;
    if !compared_fields_bounded {
        return Err(GhostraceError::MigrationLedger(
            "counted enum fields exceed read-only bounds".to_owned(),
        ));
    }
    let counts = connection.query_row(
        // COUNT(1) deliberately emits VM work per row. COUNT(*) can use a
        // single B-tree Count opcode and evade an instruction-based budget.
        "SELECT (SELECT COUNT(1) FROM events), (SELECT COUNT(1) FROM events WHERE kind='gap'),
            (SELECT COUNT(1) FROM policy_metadata), (SELECT COUNT(1) FROM cursors),
            (SELECT COUNT(1) FROM cursors WHERE state != 'active'), (SELECT COUNT(1) FROM diagnostics)",
        [], |row| Ok(HealthCounts { events: count(row, 0)?, gaps: count(row, 1)?, policy_versions: count(row, 2)?,
            cursors: count(row, 3)?, inactive_or_unknown_cursors: count(row, 4)?, diagnostics: count(row, 5)? }))?;
    connection.execute_batch("ROLLBACK")?;
    Ok((counts, schema_version))
}

fn count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_and_elapsed_budgets_interrupt_work() {
        for (max_steps, timeout) in [(1000, Duration::from_secs(2)), (usize::MAX, Duration::ZERO)] {
            let connection = Connection::open_in_memory().unwrap();
            let exceeded = install_budget(&connection, max_steps, timeout).unwrap();
            let result = connection.query_row(
                "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000) SELECT SUM(x) FROM n", [], |row| row.get::<_, i64>(0));
            assert!(result.is_err(), "resource limit must stop execution");
            assert!(exceeded.load(Ordering::Relaxed));
        }
    }

    #[test]
    fn a_single_large_sqlite_value_is_limited_before_progress_callbacks() {
        let connection = Connection::open_in_memory().unwrap();
        install_sql_limits(&connection).unwrap();
        let result = connection
            .query_row("SELECT printf('%1000000s','x')", [], |row| row.get::<_, String>(0));
        assert!(matches!(result, Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == rusqlite::ErrorCode::TooBig));
    }
}
