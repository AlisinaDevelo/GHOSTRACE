//! SQLite journal and migration runner.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    crypto::{decrypt_payload, encrypt_payload, KeyProvider, SharedKeyProvider},
    error::GhostraceError,
    model::{
        CollectorInstanceId, EventEnvelope, EventSource, Evidence, IngestionOrigin, OriginBinding,
        PolicyProfileId, ProvenanceVersion, SourceCursor, EVENT_SCHEMA_VERSION,
    },
    policy::PolicyProfile,
};

const MIGRATION: &str = include_str!("../migrations/0001_init.sql");

#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub ingest_seq: u64,
    pub event: EventEnvelope,
}

#[derive(Clone)]
pub struct Journal {
    conn: Arc<Mutex<Connection>>,
    key_provider: SharedKeyProvider,
    path: Option<PathBuf>,
}

impl Journal {
    pub fn new_in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory(provider)
    }

    pub fn open_in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::in_memory(provider)
    }

    /// Opens a file-backed journal for synthetic fixture and migration tests.
    /// This is not a production persistence API; live storage remains disabled
    /// until a Keychain-backed provider is available.
    pub fn open_fixture<P, K>(path: P, provider: K) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
        K: KeyProvider + 'static,
    {
        Self::open_with_provider(path, Arc::new(provider))
    }

    fn open_with_provider<P>(path: P, provider: SharedKeyProvider) -> Result<Self, GhostraceError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            let parent_preexisted = parent.exists();
            fs::create_dir_all(parent)
                .map_err(|source| GhostraceError::Io { path: parent.to_path_buf(), source })?;
            if !parent_preexisted {
                set_directory_permissions(parent)?;
            }
            verify_directory_permissions(parent)?;
        }
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        set_file_permissions(&path)?;
        Self::from_connection(connection, provider, Some(path))
    }

    pub fn in_memory<K>(provider: K) -> Result<Self, GhostraceError>
    where
        K: KeyProvider + 'static,
    {
        Self::from_connection(Connection::open_in_memory()?, Arc::new(provider), None)
    }

    fn from_connection(
        connection: Connection,
        provider: SharedKeyProvider,
        path: Option<PathBuf>,
    ) -> Result<Self, GhostraceError> {
        configure_connection(&connection, path.is_some())?;
        connection.execute_batch(MIGRATION)?;
        Ok(Self { conn: Arc::new(Mutex::new(connection)), key_provider: provider, path })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the SQLite mode.  File-backed journals should report `wal`; an
    /// in-memory SQLite connection reports `memory` because SQLite has no WAL
    /// file to coordinate there.
    pub fn journal_mode(&self) -> Result<String, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    pub fn synchronous_mode(&self) -> Result<String, GhostraceError> {
        let connection = self.lock_connection()?;
        let value: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        Ok(match value {
            0 => "OFF",
            1 => "NORMAL",
            2 => "FULL",
            3 => "EXTRA",
            _ => "UNKNOWN",
        }
        .to_owned())
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, GhostraceError> {
        let connection = self.lock_connection()?;
        let value: i64 = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        Ok(value == 1)
    }

    pub fn schema_version_count(&self) -> Result<u64, GhostraceError> {
        let connection = self.lock_connection()?;
        Ok(connection.query_row("SELECT COUNT(*) FROM schema_versions", [], |row| row.get(0))?)
    }

    pub fn ingest(
        &self,
        origin: &IngestionOrigin,
        event: &EventEnvelope,
        policy: &PolicyProfile,
    ) -> Result<u64, GhostraceError> {
        Ok(self.ingest_batch(origin, std::slice::from_ref(event), policy)?[0])
    }

    /// Inserts accepted events and cursor progress in one SQLite transaction.
    /// Parent references must already exist or point to an earlier event in this
    /// batch; a missing parent is rejected before commit.
    pub fn ingest_batch(
        &self,
        origin: &IngestionOrigin,
        events: &[EventEnvelope],
        policy: &PolicyProfile,
    ) -> Result<Vec<u64>, GhostraceError> {
        let mut ids = HashSet::with_capacity(events.len());
        for event in events {
            origin.validate_event(event)?;
            event.validate()?;
            policy.authorize(event)?;
            if !ids.insert(event.event_id) {
                return Err(GhostraceError::InvalidEvent(format!(
                    "duplicate event_id in batch: {}",
                    event.event_id
                )));
            }
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        record_policy_profile(&transaction, policy)?;
        let sequences = insert_events(&transaction, events, self.key_provider.as_ref())?;
        transaction.commit()?;
        Ok(sequences)
    }

    pub fn event(&self, event_id: Uuid) -> Result<EventEnvelope, GhostraceError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                    kind, collector_instance, source_cursor, provenance_version,
                    policy_profile_id, policy_profile_version, evidence, parent_event_id,
                    payload_ciphertext
             FROM events WHERE event_id = ?1",
        )?;
        let row = statement.query_row(params![event_id.to_string()], row_to_stored).optional()?;
        match row {
            Some(raw) => decode_stored(raw, self.key_provider.as_ref()).map(|stored| stored.event),
            None => Err(GhostraceError::EventNotFound(event_id)),
        }
    }

    pub fn events(&self) -> Result<Vec<StoredEvent>, GhostraceError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT ingest_seq, event_id, schema_version, observed_at, ingested_at, source,
                    kind, collector_instance, source_cursor, provenance_version,
                    policy_profile_id, policy_profile_version, evidence, parent_event_id,
                    payload_ciphertext
             FROM events ORDER BY ingest_seq ASC",
        )?;
        let mut rows = statement.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(decode_stored(row_to_stored(row)?, self.key_provider.as_ref())?);
        }
        Ok(result)
    }

    pub fn raw_payload_ciphertext(&self, event_id: Uuid) -> Result<Vec<u8>, GhostraceError> {
        let connection = self.lock_connection()?;
        let value = connection
            .query_row(
                "SELECT payload_ciphertext FROM events WHERE event_id = ?1",
                params![event_id.to_string()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?;
        value.ok_or(GhostraceError::EventNotFound(event_id))
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, GhostraceError> {
        self.conn.lock().map_err(|_| GhostraceError::Migration("journal mutex poisoned".to_owned()))
    }
}

fn record_policy_profile(
    transaction: &Transaction<'_>,
    profile: &PolicyProfile,
) -> Result<(), GhostraceError> {
    let json = serde_json::to_string(profile)?;
    let existing = transaction
        .query_row(
            "SELECT profile_json FROM policy_metadata
             WHERE profile_id = ?1 AND profile_version = ?2",
            params![profile.id, profile.version],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if existing != json {
            return Err(GhostraceError::InvalidEvent(
                "a policy ID and version cannot be reused with different rules".to_owned(),
            ));
        }
        return Ok(());
    }
    transaction.execute(
        "INSERT INTO policy_metadata(profile_id, profile_version, profile_json, recorded_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![profile.id, profile.version, json, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

fn insert_events(
    transaction: &Transaction<'_>,
    events: &[EventEnvelope],
    provider: &dyn KeyProvider,
) -> Result<Vec<u64>, GhostraceError> {
    let mut sequences = Vec::with_capacity(events.len());
    for event in events {
        if let Some(parent_id) = event.parent_event_id {
            let exists: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM events WHERE event_id = ?1",
                    params![parent_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(GhostraceError::InvalidEvent(format!(
                    "parent event does not exist: {parent_id}"
                )));
            }
        }
        let payload = serde_json::to_vec(&event.payload)?;
        let aad = associated_data(event)?;
        let ciphertext = encrypt_payload(provider, &aad, &payload)?;
        transaction.execute(
            "INSERT INTO events(
                event_id, schema_version, observed_at, ingested_at, source, kind,
                collector_instance, source_cursor, provenance_version, policy_profile_id,
                policy_profile_version, evidence, parent_event_id, payload_ciphertext
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                event.event_id.to_string(),
                event.schema_version,
                event.observed_at.to_rfc3339(),
                event.ingested_at.to_rfc3339(),
                event.source.to_string(),
                event.kind.to_string(),
                event.collector_instance(),
                event.source_cursor(),
                event.provenance_version(),
                event.policy_profile_id.as_str(),
                event.policy_profile_version,
                serde_json::to_string(&event.evidence)?,
                event.parent_event_id.map(|id| id.to_string()),
                ciphertext,
            ],
        )?;
        let sequence = transaction.last_insert_rowid();
        if let Some(cursor) = event.source_cursor() {
            transaction.execute(
                "INSERT INTO cursors(source, collector_instance, source_cursor, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(source, collector_instance) DO UPDATE SET source_cursor=excluded.source_cursor,
                     updated_at=excluded.updated_at",
                params![
                    event.source.to_string(),
                    event.collector_instance(),
                    cursor,
                    event.ingested_at.to_rfc3339()
                ],
            )?;
        }
        sequences.push(sequence as u64);
    }
    Ok(sequences)
}

#[derive(Debug)]
struct RawStored {
    ingest_seq: u64,
    event_id: String,
    schema_version: u32,
    observed_at: String,
    ingested_at: String,
    source: String,
    kind: String,
    collector_instance: String,
    source_cursor: Option<String>,
    provenance_version: String,
    policy_profile_id: String,
    policy_profile_version: u32,
    evidence: String,
    parent_event_id: Option<String>,
    payload_ciphertext: Vec<u8>,
}

fn row_to_stored(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawStored> {
    Ok(RawStored {
        ingest_seq: row.get(0)?,
        event_id: row.get(1)?,
        schema_version: row.get(2)?,
        observed_at: row.get(3)?,
        ingested_at: row.get(4)?,
        source: row.get(5)?,
        kind: row.get(6)?,
        collector_instance: row.get(7)?,
        source_cursor: row.get(8)?,
        provenance_version: row.get(9)?,
        policy_profile_id: row.get(10)?,
        policy_profile_version: row.get(11)?,
        evidence: row.get(12)?,
        parent_event_id: row.get(13)?,
        payload_ciphertext: row.get(14)?,
    })
}

fn decode_stored(
    raw: RawStored,
    provider: &dyn KeyProvider,
) -> Result<StoredEvent, GhostraceError> {
    let aad = associated_data_raw(&raw)?;
    let event_id = Uuid::parse_str(&raw.event_id)
        .map_err(|_| GhostraceError::InvalidEvent("stored event_id is invalid".to_owned()))?;
    let schema_version = raw.schema_version;
    if schema_version != EVENT_SCHEMA_VERSION {
        return Err(GhostraceError::UnsupportedSchema(schema_version));
    }
    let observed_at = parse_timestamp(&raw.observed_at)?;
    let ingested_at = parse_timestamp(&raw.ingested_at)?;
    let source = parse_source(&raw.source)?;
    let kind = parse_kind(&raw.kind)?;
    let evidence: Evidence = serde_json::from_str(&raw.evidence)?;
    let payload: crate::model::EventPayload =
        serde_json::from_slice(&decrypt_payload(provider, &aad, &raw.payload_ciphertext)?)?;
    let parent_event_id =
        raw.parent_event_id.map(|id| Uuid::parse_str(&id)).transpose().map_err(|_| {
            GhostraceError::InvalidEvent("stored parent_event_id is invalid".to_owned())
        })?;
    let collector_instance = CollectorInstanceId::try_from(raw.collector_instance)?;
    let source_cursor = raw.source_cursor.map(SourceCursor::try_from).transpose()?;
    let provenance_version = ProvenanceVersion::try_from(raw.provenance_version)?;
    let policy_profile_id = PolicyProfileId::try_from(raw.policy_profile_id)?;
    let event = EventEnvelope::from_parts(
        EVENT_SCHEMA_VERSION,
        event_id,
        observed_at,
        ingested_at,
        source,
        kind,
        payload,
        collector_instance,
        source_cursor,
        provenance_version,
        policy_profile_id,
        raw.policy_profile_version,
        evidence,
        parent_event_id,
        OriginBinding::Stored,
    );
    event.validate()?;
    Ok(StoredEvent { ingest_seq: raw.ingest_seq, event })
}

#[derive(Serialize)]
struct MetadataAad<'a> {
    domain: &'static str,
    event_id: &'a str,
    schema_version: u32,
    observed_at: &'a str,
    ingested_at: &'a str,
    source: &'a str,
    kind: &'a str,
    collector_instance: &'a str,
    source_cursor: Option<&'a str>,
    provenance_version: &'a str,
    policy_profile_id: &'a str,
    policy_profile_version: u32,
    evidence: &'a str,
    parent_event_id: Option<&'a str>,
}

fn associated_data(event: &EventEnvelope) -> Result<Vec<u8>, GhostraceError> {
    let event_id = event.event_id.to_string();
    let observed_at = event.observed_at.to_rfc3339();
    let ingested_at = event.ingested_at.to_rfc3339();
    let source = event.source.to_string();
    let kind = event.kind.to_string();
    let evidence = serde_json::to_string(&event.evidence)?;
    let parent_event_id = event.parent_event_id.map(|id| id.to_string());
    serialize_associated_data(MetadataAad {
        domain: "ghostrace:event-metadata:v1",
        event_id: &event_id,
        schema_version: event.schema_version,
        observed_at: &observed_at,
        ingested_at: &ingested_at,
        source: &source,
        kind: &kind,
        collector_instance: event.collector_instance(),
        source_cursor: event.source_cursor(),
        provenance_version: event.provenance_version(),
        policy_profile_id: event.policy_profile_id.as_str(),
        policy_profile_version: event.policy_profile_version,
        evidence: &evidence,
        parent_event_id: parent_event_id.as_deref(),
    })
}

fn associated_data_raw(raw: &RawStored) -> Result<Vec<u8>, GhostraceError> {
    serialize_associated_data(MetadataAad {
        domain: "ghostrace:event-metadata:v1",
        event_id: &raw.event_id,
        schema_version: raw.schema_version,
        observed_at: &raw.observed_at,
        ingested_at: &raw.ingested_at,
        source: &raw.source,
        kind: &raw.kind,
        collector_instance: &raw.collector_instance,
        source_cursor: raw.source_cursor.as_deref(),
        provenance_version: &raw.provenance_version,
        policy_profile_id: &raw.policy_profile_id,
        policy_profile_version: raw.policy_profile_version,
        evidence: &raw.evidence,
        parent_event_id: raw.parent_event_id.as_deref(),
    })
}

fn serialize_associated_data(metadata: MetadataAad<'_>) -> Result<Vec<u8>, GhostraceError> {
    Ok(serde_json::to_vec(&metadata)?)
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, GhostraceError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| GhostraceError::InvalidEvent("stored timestamp is invalid".to_owned()))
}

fn parse_source(value: &str) -> Result<EventSource, GhostraceError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| GhostraceError::InvalidEvent("stored source is invalid".to_owned()))
}

fn parse_kind(value: &str) -> Result<crate::model::EventKind, GhostraceError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| GhostraceError::InvalidEvent("stored event kind is invalid".to_owned()))
}

fn configure_connection(connection: &Connection, file_backed: bool) -> Result<(), GhostraceError> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    if file_backed {
        connection.pragma_update(None, "journal_mode", "WAL")?;
    }
    connection.pragma_update(None, "synchronous", "FULL")?;
    Ok(())
}

fn set_directory_permissions(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    }
    Ok(())
}

fn verify_directory_permissions(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::metadata(path)
            .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?
            .permissions()
            .mode();
        if permissions & 0o077 != 0 {
            return Err(GhostraceError::InsecurePermissions(path.to_path_buf()));
        }
    }
    Ok(())
}

fn set_file_permissions(path: &Path) -> Result<(), GhostraceError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|source| GhostraceError::Io { path: path.to_path_buf(), source })?;
    }
    Ok(())
}
