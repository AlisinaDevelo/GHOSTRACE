//! Bounded local SQLite integrity checks and recovery guidance.
//!
//! The report is diagnostic only. It never repairs, rewrites, or deletes a
//! journal. A failing check is a stop signal: preserve the original, work on a
//! verified copy, and keep the failure receipt with the recovery decision.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::GhostraceError;

/// Version of the integrity-check wire contract.
pub const INTEGRITY_REPORT_SCHEMA_VERSION: u32 = 1;
const MAX_INTEGRITY_MESSAGES: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;

/// One SQLite foreign-key violation, without a filesystem path or payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityForeignKeyViolation {
    pub table: String,
    pub rowid: Option<i64>,
    pub parent: String,
    pub foreign_key: u64,
}

/// Path-free, bounded result of SQLite integrity checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityReport {
    pub schema_version: u32,
    pub integrity_ok: bool,
    pub integrity_messages: Vec<String>,
    pub foreign_key_violations: Vec<IntegrityForeignKeyViolation>,
    pub user_version: u32,
    pub migration_count: u64,
    pub recovery_guidance: Vec<String>,
}

impl IntegrityReport {
    pub(crate) fn from_connection(connection: &Connection) -> Result<Self, GhostraceError> {
        let mut integrity_messages = Vec::new();
        let mut statement = connection.prepare("PRAGMA integrity_check")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if integrity_messages.len() >= MAX_INTEGRITY_MESSAGES {
                break;
            }
            integrity_messages.push(bounded_text("integrity result", row.get::<_, String>(0)?)?);
        }
        if integrity_messages.is_empty() {
            return Err(GhostraceError::IntegrityReportInvalid(
                "SQLite returned no integrity result".to_owned(),
            ));
        }

        let mut foreign_key_violations = Vec::new();
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if foreign_key_violations.len() >= MAX_INTEGRITY_MESSAGES {
                break;
            }
            let foreign_key = row.get::<_, i64>(3)?;
            foreign_key_violations.push(IntegrityForeignKeyViolation {
                table: bounded_text("foreign-key table", row.get::<_, String>(0)?)?,
                rowid: row.get::<_, Option<i64>>(1)?,
                parent: bounded_text("foreign-key parent", row.get::<_, String>(2)?)?,
                foreign_key: u64::try_from(foreign_key).map_err(|_| {
                    GhostraceError::IntegrityReportInvalid(
                        "foreign-key identifier is out of range".to_owned(),
                    )
                })?,
            });
        }

        let user_version =
            connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        let user_version = u32::try_from(user_version).map_err(|_| {
            GhostraceError::IntegrityReportInvalid("SQLite user_version is out of range".to_owned())
        })?;
        let migration_count =
            connection.query_row("SELECT COUNT(*) FROM migration_records", [], |row| {
                row.get::<_, i64>(0)
            })?;
        let migration_count = u64::try_from(migration_count).map_err(|_| {
            GhostraceError::IntegrityReportInvalid("migration count is out of range".to_owned())
        })?;

        let report = Self {
            schema_version: INTEGRITY_REPORT_SCHEMA_VERSION,
            integrity_ok: integrity_messages.len() == 1
                && integrity_messages[0] == "ok"
                && foreign_key_violations.is_empty(),
            integrity_messages,
            foreign_key_violations,
            user_version,
            migration_count,
            recovery_guidance: vec![
                "stop ingestion when integrity_ok is false; do not continue on an unverified journal".to_owned(),
                "preserve the original database and sidecars before attempting recovery".to_owned(),
                "work on a private verified copy and retain before-and-after integrity receipts".to_owned(),
                "recovery does not prove deletion or erase external snapshots and backups".to_owned(),
            ],
        };
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != INTEGRITY_REPORT_SCHEMA_VERSION
            || self.integrity_messages.is_empty()
            || self.integrity_messages.len() > MAX_INTEGRITY_MESSAGES
            || self.foreign_key_violations.len() > MAX_INTEGRITY_MESSAGES
            || self.recovery_guidance.len() != 4
            || self.recovery_guidance.iter().any(|value| value.is_empty())
        {
            return Err(GhostraceError::IntegrityReportInvalid(
                "integrity report shape is invalid".to_owned(),
            ));
        }
        let computed_ok = self.integrity_messages.len() == 1
            && self.integrity_messages[0] == "ok"
            && self.foreign_key_violations.is_empty();
        if self.integrity_ok != computed_ok {
            return Err(GhostraceError::IntegrityReportInvalid(
                "integrity status does not match check results".to_owned(),
            ));
        }
        Ok(())
    }
}

fn bounded_text(label: &str, value: String) -> Result<String, GhostraceError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES || value.chars().any(char::is_control)
    {
        return Err(GhostraceError::IntegrityReportInvalid(format!(
            "{label} exceeds the bounded diagnostic contract"
        )));
    }
    Ok(value)
}
