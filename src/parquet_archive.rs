//! Opt-in Parquet cold archive derived from a validated JSONL export.
//!
//! The archive is a plaintext copy for long-term analysis. It never replaces
//! the encrypted journal or the JSONL export, is only created by an explicit
//! command, and is published only after every row has been read back from
//! the finished file, rebuilt into its JSONL record, and compared with the
//! source. The file carries the profile identity, the source export digests,
//! the row count, and a digest over the canonical rows in its footer, so a
//! later `verify_parquet_archive` detects conversion errors and tampering.
//!
//! Physical layout follows the v1 profile: flat columns in profile order,
//! zstd compression, no dictionary encoding, no statistics, no page index.

use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use parquet::{
    basic::{Compression, LogicalType, Repetition, TimeUnit, Type as PhysicalType, ZstdLevel},
    column::reader::get_typed_column_reader,
    data_type::{ByteArray, ByteArrayType, Int32Type, Int64Type},
    file::{
        metadata::KeyValue,
        properties::{EnabledStatistics, WriterProperties},
        reader::{FileReader, SerializedFileReader},
        writer::SerializedFileWriter,
    },
    schema::types::Type,
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    error::GhostraceError,
    export_schema::{read_bounded_line, validate_export, EXPORT_EVENT_SCHEMA_ID},
    model::EventEnvelope,
    parquet_profile::{
        checked_in_profile, ParquetArchiveProfile, PARQUET_ARCHIVE_PROFILE_JSON,
        PARQUET_ARCHIVE_PROFILE_SCHEMA_ID, PARQUET_ARCHIVE_PROFILE_VERSION,
    },
};

/// Shown before an archive is written and stored in its footer.
pub const PARQUET_ARCHIVE_PLAINTEXT_WARNING: &str = "A Parquet archive is an unencrypted copy of \
the exported events. Anyone who can read the file can read them, and deleting or expiring \
journal records does not delete this copy.";

const ROWS_PER_GROUP: usize = 16_384;
const BYTES_PER_GROUP: usize = 64 * 1024 * 1024;
const ZSTD_LEVEL: i32 = 3;
const KEY_PREFIX: &str = "ghostrace.archive.";

/// What an archive's footer declares, and what verification recomputed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ParquetArchiveReceipt {
    pub path: PathBuf,
    pub profile_schema_id: String,
    pub profile_version: u32,
    pub profile_sha256: String,
    pub source_manifest_sha256: String,
    pub source_event_body_sha256: String,
    pub row_count: u64,
    pub rows_sha256: String,
    pub file_sha256: String,
}

/// Build the profile row for one JSONL event record.
pub fn archive_row(record: &Value) -> Result<Map<String, Value>, GhostraceError> {
    let event = record.get("event").ok_or_else(|| invalid("record has no event"))?;
    let field = |name: &str| event.get(name).cloned().unwrap_or(Value::Null);
    let timestamp = |name: &str| -> Result<Value, GhostraceError> {
        let text = event.get(name).and_then(Value::as_str).ok_or_else(|| invalid("timestamp"))?;
        let nanos = DateTime::parse_from_rfc3339(text)
            .ok()
            .and_then(|time| time.timestamp_nanos_opt())
            .ok_or_else(|| invalid("timestamp is outside the nanosecond range"))?;
        Ok(Value::from(nanos))
    };
    let payload = event.get("payload").ok_or_else(|| invalid("event has no payload"))?;
    let mut row = Map::new();
    row.insert("event_id".into(), field("event_id"));
    row.insert("ingest_seq".into(), record.get("ingest_seq").cloned().unwrap_or(Value::Null));
    row.insert("schema_version".into(), field("schema_version"));
    row.insert("observed_at".into(), timestamp("observed_at")?);
    row.insert("ingested_at".into(), timestamp("ingested_at")?);
    for name in [
        "source",
        "kind",
        "collector_instance",
        "source_cursor",
        "provenance_version",
        "policy_profile_id",
        "policy_profile_version",
        "evidence",
        "parent_event_id",
    ] {
        row.insert(name.into(), field(name));
    }
    row.insert("payload_json".into(), Value::String(serde_json::to_string(payload)?));
    let gap = (field("kind") == "gap").then(|| payload.get("data")).flatten();
    let gap_field = |name: &str| gap.and_then(|data| data.get(name)).cloned();
    for (column, name) in [
        ("gap_source", "source"),
        ("gap_reason_code", "reason_code"),
        ("gap_dropped_count", "dropped_count"),
        ("gap_from_cursor", "from_cursor"),
        ("gap_to_cursor", "to_cursor"),
        ("gap_volume_digest", "volume_digest"),
    ] {
        row.insert(column.into(), gap_field(name).unwrap_or(Value::Null));
    }
    for (column, name) in
        [("gap_root_ids_json", "root_ids"), ("gap_remediation_json", "remediation")]
    {
        let value = match gap_field(name) {
            Some(Value::Null) | None => Value::Null,
            Some(value) => Value::String(serde_json::to_string(&value)?),
        };
        row.insert(column.into(), value);
    }
    Ok(row)
}

/// Rebuild the JSONL event record a profile row came from.
pub fn record_from_row(row: &Map<String, Value>) -> Result<Value, GhostraceError> {
    let get = |name: &str| row.get(name).cloned().unwrap_or(Value::Null);
    let timestamp = |name: &str| -> Result<Value, GhostraceError> {
        let nanos = row.get(name).and_then(Value::as_i64).ok_or_else(|| invalid("timestamp"))?;
        Ok(serde_json::to_value(DateTime::<Utc>::from_timestamp_nanos(nanos))?)
    };
    let payload_json =
        row.get("payload_json").and_then(Value::as_str).ok_or_else(|| invalid("payload_json"))?;
    let event = json!({
        "schema_version": get("schema_version"),
        "event_id": get("event_id"),
        "observed_at": timestamp("observed_at")?,
        "ingested_at": timestamp("ingested_at")?,
        "source": get("source"),
        "kind": get("kind"),
        "payload": serde_json::from_str::<Value>(payload_json)?,
        "collector_instance": get("collector_instance"),
        "source_cursor": get("source_cursor"),
        "provenance_version": get("provenance_version"),
        "policy_profile_id": get("policy_profile_id"),
        "policy_profile_version": get("policy_profile_version"),
        "evidence": get("evidence"),
        "parent_event_id": get("parent_event_id"),
    });
    Ok(json!({
        "record_type": "event",
        "schema_id": EXPORT_EVENT_SCHEMA_ID,
        "schema_version": get("schema_version"),
        "ingest_seq": get("ingest_seq"),
        "event": event,
    }))
}

/// Write `output` from the validated JSONL export at `export`. The export is
/// only read. The archive is written to a 0600 temporary file beside
/// `output`, read back and compared record by record with the export, then
/// renamed into place; an existing `output` is never replaced, and nothing
/// is left behind on failure.
pub fn write_parquet_archive(
    export: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<ParquetArchiveReceipt, GhostraceError> {
    let export = export.as_ref();
    let output = output.as_ref();
    let profile = checked_in_profile()?;
    let validation = validate_export(export)?;
    if output.exists() {
        return Err(GhostraceError::ArchiveExists(output.to_path_buf()));
    }
    let parent = match output.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let temporary = tempfile::Builder::new()
        .prefix(".ghostrace-archive-")
        .suffix(".parquet.tmp")
        .tempfile_in(parent)
        .map_err(|source| io_error(parent, source))?;

    let (manifest_sha256, mut rows) = open_rows(export)?;
    let footer = Footer {
        profile_sha256: sha256_hex(PARQUET_ARCHIVE_PROFILE_JSON.as_bytes()),
        source_manifest_sha256: manifest_sha256,
        source_event_body_sha256: validation.body_sha256.clone(),
    };
    let mut rows_digest = Sha256::new();
    let mut row_count = 0_u64;
    {
        let file = temporary.as_file().try_clone().map_err(|source| io_error(output, source))?;
        let properties = writer_properties(Vec::new());
        let mut writer = SerializedFileWriter::new(file, schema(&profile)?, properties)
            .map_err(parquet_error)?;
        let mut group = Vec::new();
        let mut group_bytes = 0;
        while let Some(row) = rows.next_row()? {
            profile.validate_row(&row)?;
            let line = serde_json::to_string(&row)?;
            rows_digest.update(line.as_bytes());
            rows_digest.update(b"\n");
            row_count += 1;
            profile.validate_row_count(row_count)?;
            group_bytes += line.len();
            group.push(row);
            if group.len() == ROWS_PER_GROUP || group_bytes >= BYTES_PER_GROUP {
                write_group(&mut writer, &profile, &group)?;
                group.clear();
                group_bytes = 0;
            }
        }
        if !group.is_empty() {
            write_group(&mut writer, &profile, &group)?;
        }
        let rows_sha256 = hex(&rows_digest.finalize_reset());
        for entry in footer.key_values(row_count, &rows_sha256) {
            writer.append_key_value_metadata(entry);
        }
        writer.close().map_err(parquet_error)?;
    }
    if row_count as usize != validation.event_count {
        return Err(invalid("archive row count differs from the export event count"));
    }
    temporary.as_file().sync_all().map_err(|source| io_error(output, source))?;

    let receipt = verify_parquet_archive(temporary.path(), export)?;
    temporary.persist_noclobber(output).map_err(|error| match error.error.kind() {
        std::io::ErrorKind::AlreadyExists => GhostraceError::ArchiveExists(output.to_path_buf()),
        _ => io_error(output, error.error),
    })?;
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(ParquetArchiveReceipt { path: output.to_path_buf(), ..receipt })
}

/// Check an archive against its footer and against the JSONL export it
/// claims to derive from: every row is rebuilt into its JSONL record and
/// compared, and the footer's counts and digests are recomputed.
pub fn verify_parquet_archive(
    archive: impl AsRef<Path>,
    export: impl AsRef<Path>,
) -> Result<ParquetArchiveReceipt, GhostraceError> {
    let archive = archive.as_ref();
    let export = export.as_ref();
    let profile = checked_in_profile()?;
    let validation = validate_export(export)?;
    let file = File::open(archive).map_err(|source| io_error(archive, source))?;
    let reader = SerializedFileReader::new(file).map_err(parquet_error)?;
    let metadata = reader.metadata().file_metadata();
    if metadata.schema_descr().root_schema() != schema(&profile)?.as_ref() {
        return Err(invalid("archive schema differs from the v1 profile"));
    }
    let footer = read_footer(metadata.key_value_metadata())?;
    let expected = |key: &str, value: &str| -> Result<(), GhostraceError> {
        if footer.get(key).map(String::as_str) == Some(value) {
            Ok(())
        } else {
            Err(invalid(format!("archive footer {key} does not match")))
        }
    };
    expected("profile_schema_id", PARQUET_ARCHIVE_PROFILE_SCHEMA_ID)?;
    expected("profile_version", &PARQUET_ARCHIVE_PROFILE_VERSION.to_string())?;
    expected("profile_sha256", &sha256_hex(PARQUET_ARCHIVE_PROFILE_JSON.as_bytes()))?;
    expected("source_event_body_sha256", &validation.body_sha256)?;

    let (manifest_sha256, mut source) = open_rows(export)?;
    expected("source_manifest_sha256", &manifest_sha256)?;
    let mut rows_digest = Sha256::new();
    let mut row_count = 0_u64;
    for index in 0..reader.num_row_groups() {
        let group = reader.get_row_group(index).map_err(parquet_error)?;
        for row in read_group(group.as_ref(), &profile)? {
            profile.validate_row(&row)?;
            let Some(source_row) = source.next_row()? else {
                return Err(invalid("archive has more rows than the export"));
            };
            let rebuilt = canonical_record(&record_from_row(&row)?)?;
            if row != source_row || rebuilt != canonical_record(&source.last_record)? {
                return Err(invalid(format!("archive row {row_count} differs from the export")));
            }
            rows_digest.update(serde_json::to_string(&row)?.as_bytes());
            rows_digest.update(b"\n");
            row_count += 1;
        }
    }
    if source.next_row()?.is_some() {
        return Err(invalid("archive has fewer rows than the export"));
    }
    let rows_sha256 = hex(&rows_digest.finalize());
    expected("row_count", &row_count.to_string())?;
    expected("rows_sha256", &rows_sha256)?;
    Ok(ParquetArchiveReceipt {
        path: archive.to_path_buf(),
        profile_schema_id: PARQUET_ARCHIVE_PROFILE_SCHEMA_ID.to_owned(),
        profile_version: PARQUET_ARCHIVE_PROFILE_VERSION,
        profile_sha256: sha256_hex(PARQUET_ARCHIVE_PROFILE_JSON.as_bytes()),
        source_manifest_sha256: manifest_sha256,
        source_event_body_sha256: validation.body_sha256,
        row_count,
        rows_sha256,
        file_sha256: file_sha256(archive)?,
    })
}

struct Footer {
    profile_sha256: String,
    source_manifest_sha256: String,
    source_event_body_sha256: String,
}

impl Footer {
    fn key_values(&self, row_count: u64, rows_sha256: &str) -> Vec<KeyValue> {
        [
            ("profile_schema_id", PARQUET_ARCHIVE_PROFILE_SCHEMA_ID.to_owned()),
            ("profile_version", PARQUET_ARCHIVE_PROFILE_VERSION.to_string()),
            ("profile_sha256", self.profile_sha256.clone()),
            ("source_manifest_sha256", self.source_manifest_sha256.clone()),
            ("source_event_body_sha256", self.source_event_body_sha256.clone()),
            ("row_count", row_count.to_string()),
            ("rows_sha256", rows_sha256.to_owned()),
            ("plaintext_warning", PARQUET_ARCHIVE_PLAINTEXT_WARNING.to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| KeyValue::new(format!("{KEY_PREFIX}{key}"), value))
        .collect()
    }
}

fn read_footer(
    entries: Option<&Vec<KeyValue>>,
) -> Result<std::collections::BTreeMap<String, String>, GhostraceError> {
    let mut footer = std::collections::BTreeMap::new();
    for entry in entries.into_iter().flatten() {
        if let Some(key) = entry.key.strip_prefix(KEY_PREFIX) {
            let value = entry.value.clone().ok_or_else(|| invalid("empty footer value"))?;
            if footer.insert(key.to_owned(), value).is_some() {
                return Err(invalid(format!("archive footer repeats {key}")));
            }
        }
    }
    Ok(footer)
}

/// Streams export event records as profile rows, keeping the last record.
struct RowStream {
    reader: BufReader<File>,
    path: PathBuf,
    last_record: Value,
}

impl RowStream {
    fn next_row(&mut self) -> Result<Option<Map<String, Value>>, GhostraceError> {
        let Some(line) = read_bounded_line(&mut self.reader, &self.path)? else {
            return Ok(None);
        };
        self.last_record = serde_json::from_slice(&line)?;
        archive_row(&self.last_record).map(Some)
    }
}

fn open_rows(export: &Path) -> Result<(String, RowStream), GhostraceError> {
    let file = File::open(export).map_err(|source| io_error(export, source))?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let manifest =
        read_bounded_line(&mut reader, export)?.ok_or_else(|| invalid("export is empty"))?;
    Ok((
        sha256_hex(&manifest),
        RowStream { reader, path: export.to_path_buf(), last_record: Value::Null },
    ))
}

/// A record in the form the journal would serialize it, so equal events
/// compare equal whatever their source spelling.
fn canonical_record(record: &Value) -> Result<Value, GhostraceError> {
    let mut record = record.clone();
    let event: EventEnvelope = serde_json::from_value(record["event"].take())?;
    record["event"] = serde_json::to_value(&event)?;
    Ok(record)
}

fn schema(profile: &ParquetArchiveProfile) -> Result<Arc<Type>, GhostraceError> {
    let mut fields = Vec::with_capacity(profile.columns.len());
    for column in &profile.columns {
        let (physical, logical) = match column.physical_type.as_str() {
            "utf8" => (PhysicalType::BYTE_ARRAY, LogicalType::String),
            "uint32" => (PhysicalType::INT32, LogicalType::integer(32, false)),
            "uint64" => (PhysicalType::INT64, LogicalType::integer(64, false)),
            "int64" => (PhysicalType::INT64, LogicalType::integer(64, true)),
            "timestamp_nanos_utc" => {
                (PhysicalType::INT64, LogicalType::timestamp(true, TimeUnit::NANOS))
            }
            other => return Err(invalid(format!("unsupported physical type {other}"))),
        };
        let repetition = if column.nullable { Repetition::OPTIONAL } else { Repetition::REQUIRED };
        let field = Type::primitive_type_builder(&column.name, physical)
            .with_logical_type(Some(logical))
            .with_repetition(repetition)
            .build()
            .map_err(parquet_error)?;
        fields.push(Arc::new(field));
    }
    let root = Type::group_type_builder("ghostrace_event")
        .with_fields(fields)
        .build()
        .map_err(parquet_error)?;
    Ok(Arc::new(root))
}

fn writer_properties(metadata: Vec<KeyValue>) -> Arc<WriterProperties> {
    let level = ZstdLevel::try_new(ZSTD_LEVEL).expect("zstd level 3 is valid");
    Arc::new(
        WriterProperties::builder()
            .set_compression(Compression::ZSTD(level))
            .set_dictionary_enabled(false)
            .set_statistics_enabled(EnabledStatistics::None)
            .set_offset_index_disabled(true)
            .set_created_by(format!("ghostrace {}", env!("CARGO_PKG_VERSION")))
            .set_key_value_metadata((!metadata.is_empty()).then_some(metadata))
            .build(),
    )
}

fn write_group<W: Write + Send>(
    writer: &mut SerializedFileWriter<W>,
    profile: &ParquetArchiveProfile,
    rows: &[Map<String, Value>],
) -> Result<(), GhostraceError> {
    let mut group = writer.next_row_group().map_err(parquet_error)?;
    for column in &profile.columns {
        let values = rows.iter().map(|row| row.get(&column.name).unwrap_or(&Value::Null));
        let definitions =
            values.clone().map(|value| i16::from(!value.is_null())).collect::<Vec<_>>();
        let levels = column.nullable.then_some(definitions.as_slice());
        let present = values.filter(|value| !value.is_null());
        let mut writer = group
            .next_column()
            .map_err(parquet_error)?
            .ok_or_else(|| invalid("archive schema has fewer columns than the profile"))?;
        let mismatch = || invalid(format!("column {} value type", column.name));
        match column.physical_type.as_str() {
            "utf8" => {
                let data = present
                    .map(|value| {
                        value.as_str().map(|text| ByteArray::from(text.as_bytes().to_vec()))
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(mismatch)?;
                writer.typed::<ByteArrayType>().write_batch(&data, levels, None)
            }
            "uint32" => {
                let data = present
                    .map(|value| value.as_u64().and_then(|n| u32::try_from(n).ok()))
                    .map(|n| n.map(|n| n as i32))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(mismatch)?;
                writer.typed::<Int32Type>().write_batch(&data, levels, None)
            }
            "uint64" => {
                let data = present
                    .map(|value| value.as_u64().map(|n| n as i64))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(mismatch)?;
                writer.typed::<Int64Type>().write_batch(&data, levels, None)
            }
            _ => {
                let data =
                    present.map(Value::as_i64).collect::<Option<Vec<_>>>().ok_or_else(mismatch)?;
                writer.typed::<Int64Type>().write_batch(&data, levels, None)
            }
        }
        .map_err(parquet_error)?;
        writer.close().map_err(parquet_error)?;
    }
    group.close().map_err(parquet_error)?;
    Ok(())
}

fn read_group(
    group: &dyn parquet::file::reader::RowGroupReader,
    profile: &ParquetArchiveProfile,
) -> Result<Vec<Map<String, Value>>, GhostraceError> {
    let rows = usize::try_from(group.metadata().num_rows())
        .map_err(|_| invalid("row group has a negative row count"))?;
    if group.num_columns() != profile.columns.len() {
        return Err(invalid("row group column count differs from the profile"));
    }
    let mut table = vec![Map::new(); rows];
    for (index, column) in profile.columns.iter().enumerate() {
        let reader = group.get_column_reader(index).map_err(parquet_error)?;
        let mut definitions = Vec::with_capacity(rows);
        let values: Vec<Value> = match column.physical_type.as_str() {
            "utf8" => {
                let mut data = Vec::new();
                read_all(
                    get_typed_column_reader::<ByteArrayType>(reader),
                    rows,
                    &mut definitions,
                    &mut data,
                )?;
                data.into_iter()
                    .map(|bytes| {
                        String::from_utf8(bytes.data().to_vec())
                            .map(Value::String)
                            .map_err(|_| invalid(format!("column {} is not UTF-8", column.name)))
                    })
                    .collect::<Result<_, _>>()?
            }
            "uint32" => {
                let mut data = Vec::new();
                read_all(
                    get_typed_column_reader::<Int32Type>(reader),
                    rows,
                    &mut definitions,
                    &mut data,
                )?;
                data.into_iter().map(|n| Value::from(n as u32)).collect()
            }
            "uint64" => {
                let mut data = Vec::new();
                read_all(
                    get_typed_column_reader::<Int64Type>(reader),
                    rows,
                    &mut definitions,
                    &mut data,
                )?;
                data.into_iter().map(|n| Value::from(n as u64)).collect()
            }
            _ => {
                let mut data = Vec::new();
                read_all(
                    get_typed_column_reader::<Int64Type>(reader),
                    rows,
                    &mut definitions,
                    &mut data,
                )?;
                data.into_iter().map(Value::from).collect()
            }
        };
        let mut values = values.into_iter();
        for (position, row) in table.iter_mut().enumerate() {
            let defined = !column.nullable || definitions.get(position) == Some(&1);
            let value = if defined {
                values.next().ok_or_else(|| invalid(format!("column {} is short", column.name)))?
            } else {
                Value::Null
            };
            row.insert(column.name.clone(), value);
        }
        if values.next().is_some() {
            return Err(invalid(format!("column {} has extra values", column.name)));
        }
    }
    Ok(table)
}

fn read_all<T: parquet::data_type::DataType>(
    mut reader: parquet::column::reader::ColumnReaderImpl<T>,
    rows: usize,
    definitions: &mut Vec<i16>,
    values: &mut Vec<T::T>,
) -> Result<(), GhostraceError> {
    let mut read = 0;
    while read < rows {
        let (records, _, _) = reader
            .read_records(rows - read, Some(definitions), None, values)
            .map_err(parquet_error)?;
        if records == 0 {
            return Err(invalid("column ended before its row group"));
        }
        read += records;
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, GhostraceError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|source| io_error(path, source))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex(&digest.finalize()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn invalid(message: impl Into<String>) -> GhostraceError {
    GhostraceError::ArchiveInvalid(message.into())
}

fn parquet_error(error: parquet::errors::ParquetError) -> GhostraceError {
    GhostraceError::ArchiveInvalid(format!("Parquet: {error}"))
}

fn io_error(path: &Path, source: std::io::Error) -> GhostraceError {
    GhostraceError::Io { path: path.to_path_buf(), source }
}
