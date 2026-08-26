//! Explicit retention-residue guarantees and a read-only artifact inventory.
//!
//! A residue report never deletes or rewrites anything. It names the four
//! distinct operations a future retention command must keep separate:
//! logical row deletion, SQLite compaction, cryptographic key erasure, and
//! responsibility for copies outside the live journal. File paths are never
//! serialized; the report contains only typed counts and byte totals.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{error::GhostraceError, journal::Journal};

/// Version of the residue-report wire contract.
pub const RESIDUE_REPORT_SCHEMA_VERSION: u32 = 1;

const SQLITE_SHADOW_SUFFIXES: &[&str] = &["_data", "_idx", "_content", "_docsize", "_config"];

/// A retention operation with a distinct guarantee and failure boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionMode {
    Logical,
    Compaction,
    CryptographicErasure,
    ExternalCopy,
}

/// Public explanation of one deletion mode. These strings are contract text,
/// not a claim that the fixture headstart performs the operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionModeDescription {
    pub mode: DeletionMode,
    pub guarantee: String,
    pub cost: String,
    pub sqlite_behavior: String,
    pub unsupported_media: Vec<String>,
    pub external_copy_responsibility: String,
}

/// Path-free inventory of one artifact class. `observed_count` includes
/// unsafe/non-regular entries; `regular_file_count` counts only files whose
/// bytes were safe to measure without following a link.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidueArtifactSummary {
    pub kind: ResidueArtifactKind,
    pub observed_count: u64,
    pub regular_file_count: u64,
    pub bytes: u64,
}

/// Artifact classes that can retain data after a logical row deletion.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidueArtifactKind {
    Database,
    Wal,
    Shm,
    RollbackJournal,
    Temporary,
    Backup,
    FtsShadow,
    ArchiveShadow,
}

/// Read-only, privacy-safe retention residue report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidueReport {
    pub schema_version: u32,
    pub modes: Vec<DeletionModeDescription>,
    pub artifacts: Vec<ResidueArtifactSummary>,
    pub external_backup_count: u64,
    pub sqlite_secure_delete_enabled: bool,
    pub fts_shadow_table_count: u64,
    pub archive_shadow_table_count: u64,
    pub notes: Vec<String>,
}

impl ResidueReport {
    /// Build a report without changing the journal or any supplied backup.
    /// Backup paths are aggregated and never returned to the caller.
    pub fn for_journal(
        journal: &Journal,
        external_backups: &[PathBuf],
    ) -> Result<Self, GhostraceError> {
        let mut artifacts = artifact_summaries();
        let mut external_backup_count = 0_u64;

        if let Some(path) = journal.path() {
            add_file_summary(&mut artifacts, ResidueArtifactKind::Database, path)?;
            for (kind, suffix) in [
                (ResidueArtifactKind::Wal, "-wal"),
                (ResidueArtifactKind::Shm, "-shm"),
                (ResidueArtifactKind::RollbackJournal, "-journal"),
                (ResidueArtifactKind::Temporary, "-tmp"),
                (ResidueArtifactKind::Backup, "-backup"),
            ] {
                add_file_summary(&mut artifacts, kind, &sidecar_path(path, suffix)?)?;
            }
        }
        for backup in external_backups {
            external_backup_count = external_backup_count.saturating_add(1);
            add_file_summary(&mut artifacts, ResidueArtifactKind::Backup, backup)?;
        }

        let (sqlite_secure_delete_enabled, fts_shadow_table_count, archive_shadow_table_count) =
            journal.with_read_snapshot(read_sqlite_residue_metadata)?;
        set_count(&mut artifacts, ResidueArtifactKind::FtsShadow, fts_shadow_table_count);
        set_count(&mut artifacts, ResidueArtifactKind::ArchiveShadow, archive_shadow_table_count);

        let report = Self {
            schema_version: RESIDUE_REPORT_SCHEMA_VERSION,
            modes: mode_descriptions(),
            artifacts,
            external_backup_count,
            sqlite_secure_delete_enabled,
            fts_shadow_table_count,
            archive_shadow_table_count,
            notes: vec![
                "logical deletion changes selected SQLite rows but does not promise byte erasure"
                    .to_owned(),
                "compaction may rewrite the live database but does not rewrite external copies"
                    .to_owned(),
                "cryptographic erasure destroys key material; ciphertext copies may remain"
                    .to_owned(),
                "exports, backups, snapshots, crash files, and filesystem media require separate handling"
                    .to_owned(),
                "SQLite secure_delete does not guarantee virtual-table shadow or filesystem erasure"
                    .to_owned(),
            ],
        };
        report.validate()?;
        Ok(report)
    }

    pub fn mode_descriptions() -> Vec<DeletionModeDescription> {
        mode_descriptions()
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.schema_version != RESIDUE_REPORT_SCHEMA_VERSION {
            return Err(GhostraceError::ResidueReportInvalid(
                "residue report schema version is unsupported".to_owned(),
            ));
        }
        let expected_modes = [
            DeletionMode::Logical,
            DeletionMode::Compaction,
            DeletionMode::CryptographicErasure,
            DeletionMode::ExternalCopy,
        ];
        if self.modes.len() != expected_modes.len()
            || self.modes.iter().map(|mode| mode.mode).collect::<Vec<_>>() != expected_modes
        {
            return Err(GhostraceError::ResidueReportInvalid(
                "residue modes are missing or out of order".to_owned(),
            ));
        }
        if self.artifacts.len() != 8
            || self.artifacts.iter().map(|artifact| artifact.kind).collect::<Vec<_>>()
                != [
                    ResidueArtifactKind::Database,
                    ResidueArtifactKind::Wal,
                    ResidueArtifactKind::Shm,
                    ResidueArtifactKind::RollbackJournal,
                    ResidueArtifactKind::Temporary,
                    ResidueArtifactKind::Backup,
                    ResidueArtifactKind::FtsShadow,
                    ResidueArtifactKind::ArchiveShadow,
                ]
        {
            return Err(GhostraceError::ResidueReportInvalid(
                "residue artifact classes are missing or out of order".to_owned(),
            ));
        }
        if self.fts_shadow_table_count
            != self
                .artifacts
                .iter()
                .find(|artifact| artifact.kind == ResidueArtifactKind::FtsShadow)
                .map(|artifact| artifact.observed_count)
                .unwrap_or_default()
            || self.archive_shadow_table_count
                != self
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.kind == ResidueArtifactKind::ArchiveShadow)
                    .map(|artifact| artifact.observed_count)
                    .unwrap_or_default()
        {
            return Err(GhostraceError::ResidueReportInvalid(
                "shadow-table counts do not match the artifact inventory".to_owned(),
            ));
        }
        Ok(())
    }
}

fn mode_descriptions() -> Vec<DeletionModeDescription> {
    let unsupported = vec![
        "filesystem snapshots and backup copies".to_owned(),
        "SSD wear levelling and privileged recovery media".to_owned(),
    ];
    vec![
        DeletionModeDescription {
            mode: DeletionMode::Logical,
            guarantee: "selected rows are absent from the live SQLite view".to_owned(),
            cost: "lowest I/O cost; free pages, WAL frames, and copies may remain".to_owned(),
            sqlite_behavior: "secure_delete may clear some cells but does not cover all virtual-table shadow storage".to_owned(),
            unsupported_media: unsupported.clone(),
            external_copy_responsibility: "exports, backups, and snapshots are not changed".to_owned(),
        },
        DeletionModeDescription {
            mode: DeletionMode::Compaction,
            guarantee: "a successful checkpoint/VACUUM rewrites the live database and can reclaim free pages".to_owned(),
            cost: "additional I/O and temporary space; readers or open transactions can cause refusal".to_owned(),
            sqlite_behavior: "VACUUM is not a secure-erasure primitive and does not rewrite WAL, SHM, backups, or snapshots".to_owned(),
            unsupported_media: unsupported.clone(),
            external_copy_responsibility: "every external copy remains the owner's separate responsibility".to_owned(),
        },
        DeletionModeDescription {
            mode: DeletionMode::CryptographicErasure,
            guarantee: "destroyed journal key material makes authenticated ciphertext unusable to a keyless reader".to_owned(),
            cost: "irreversible for the affected key generation and all ciphertext using it".to_owned(),
            sqlite_behavior: "database, WAL, shadow, and backup bytes can remain but are not decryptable without the key".to_owned(),
            unsupported_media: unsupported.clone(),
            external_copy_responsibility: "plaintext exports and independently encrypted copies are not covered".to_owned(),
        },
        DeletionModeDescription {
            mode: DeletionMode::ExternalCopy,
            guarantee: "known exports and backups can be removed only when their owner and media permit it".to_owned(),
            cost: "requires an explicit inventory, permissions, and a separate recovery decision".to_owned(),
            sqlite_behavior: "has no effect on the live SQLite journal".to_owned(),
            unsupported_media: vec![
                "filesystem snapshots, Time Machine, cloud sync, and offline media may be outside the inventory".to_owned(),
            ],
            external_copy_responsibility: "the user or administrator must handle copies outside this process".to_owned(),
        },
    ]
}

fn artifact_summaries() -> Vec<ResidueArtifactSummary> {
    [
        ResidueArtifactKind::Database,
        ResidueArtifactKind::Wal,
        ResidueArtifactKind::Shm,
        ResidueArtifactKind::RollbackJournal,
        ResidueArtifactKind::Temporary,
        ResidueArtifactKind::Backup,
        ResidueArtifactKind::FtsShadow,
        ResidueArtifactKind::ArchiveShadow,
    ]
    .into_iter()
    .map(|kind| ResidueArtifactSummary { kind, observed_count: 0, regular_file_count: 0, bytes: 0 })
    .collect()
}

fn add_file_summary(
    artifacts: &mut [ResidueArtifactSummary],
    kind: ResidueArtifactKind,
    path: &Path,
) -> Result<(), GhostraceError> {
    let Some(summary) = artifacts.iter_mut().find(|artifact| artifact.kind == kind) else {
        return Err(GhostraceError::ResidueReportInvalid("artifact class is missing".to_owned()));
    };
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(GhostraceError::Io { path: path.to_path_buf(), source });
        }
    };
    summary.observed_count = summary.observed_count.saturating_add(1);
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(());
    }
    summary.regular_file_count = summary.regular_file_count.saturating_add(1);
    summary.bytes = summary.bytes.saturating_add(metadata.len());
    Ok(())
}

fn set_count(artifacts: &mut [ResidueArtifactSummary], kind: ResidueArtifactKind, count: u64) {
    if let Some(summary) = artifacts.iter_mut().find(|artifact| artifact.kind == kind) {
        summary.observed_count = count;
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> Result<PathBuf, GhostraceError> {
    let name = path.file_name().ok_or(GhostraceError::UnsafePath)?.to_string_lossy();
    Ok(path.with_file_name(format!("{name}{suffix}")))
}

fn read_sqlite_residue_metadata(
    connection: &Connection,
) -> Result<(bool, u64, u64), GhostraceError> {
    let secure_delete =
        connection.query_row("PRAGMA secure_delete", [], |row| row.get::<_, i64>(0))?;
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type IN ('table', 'shadow') ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut fts_shadow_table_count = 0_u64;
    let mut archive_shadow_table_count = 0_u64;
    while let Some(row) = rows.next()? {
        let name = row.get::<_, String>(0)?;
        if name.starts_with("archive_") {
            archive_shadow_table_count = archive_shadow_table_count.saturating_add(1);
        } else if SQLITE_SHADOW_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
            fts_shadow_table_count = fts_shadow_table_count.saturating_add(1);
        }
    }
    Ok((secure_delete != 0, fts_shadow_table_count, archive_shadow_table_count))
}
