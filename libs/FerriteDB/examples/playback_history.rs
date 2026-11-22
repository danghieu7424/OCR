use ferrite_btree::{RecordRef, Value, ValueRef};
use ferrite_vfs::OsVfs;
use ferritedb_core::{Database, DatabaseError};
use std::error::Error;
use std::path::PathBuf;
use thiserror::Error as ThisError;

const HISTORY_TABLE: &str = "ferritedb_playback_history";
const HISTORY_META_TABLE: &str = "playback_history_meta";
const HISTORY_SCHEMA_VERSION: u32 = 1;
const HISTORY_COLUMNS: [&str; 4] = ["media_path", "position_ms", "duration_ms", "updated_at_ms"];

#[derive(Debug, ThisError)]
enum HistoryError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("media path is empty or contains a NUL byte")]
    InvalidMediaPath,
    #[error("playback history record is malformed")]
    CorruptRecord,
    #[error("playback history schema conflicts with existing metadata")]
    SchemaConflict,
    #[error("playback history schema version {0} is newer than supported")]
    UnsupportedSchemaVersion(u32),
    #[error("playback history value exceeds the supported integer range")]
    IntegerRange,
}

#[derive(Debug, Eq, PartialEq)]
struct HistoryEntry {
    media_path: String,
    position_ms: u64,
    duration_ms: u64,
    updated_at_ms: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let database_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("playback-history.fdb"));
    let media_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "media/example.mp4".to_owned());
    let vfs = OsVfs;
    let mut database = if database_path.exists() {
        Database::open(&vfs, &database_path, 64)?
    } else {
        Database::create(&vfs, &database_path, 64)?
    };

    ensure_schema(&mut database)?;
    upsert(
        &mut database,
        &media_path,
        30_000,
        120_000,
        1_800_000_000_000,
    )?;
    println!("{:?}", get(&mut database, &media_path)?);
    Ok(())
}

fn ensure_schema(database: &mut Database) -> Result<(), HistoryError> {
    let meta_columns = optional_table_columns(database, HISTORY_META_TABLE)?;
    if let Some(columns) = &meta_columns
        && (columns.len() != 1 || columns[0] != "version")
    {
        return Err(HistoryError::SchemaConflict);
    }

    let history_columns = optional_table_columns(database, HISTORY_TABLE)?;
    if let Some(columns) = &history_columns
        && (columns.len() != HISTORY_COLUMNS.len()
            || columns
                .iter()
                .zip(HISTORY_COLUMNS)
                .any(|(actual, expected)| actual.as_str() != expected))
    {
        return Err(HistoryError::SchemaConflict);
    }

    let meta_row = if meta_columns.is_some() {
        database.get_from(HISTORY_META_TABLE, 1)?
    } else {
        None
    };
    let version = meta_row
        .as_deref()
        .map(decode_schema_version)
        .transpose()?
        .unwrap_or(0);
    if version > HISTORY_SCHEMA_VERSION {
        return Err(HistoryError::UnsupportedSchemaVersion(version));
    }
    if version == HISTORY_SCHEMA_VERSION {
        if history_columns.is_none() {
            return Err(HistoryError::SchemaConflict);
        }
        return Ok(());
    }

    let mut transaction = database.begin_transaction();
    if meta_columns.is_none() {
        transaction.create_table(HISTORY_META_TABLE, &["version"])?;
    }
    if history_columns.is_none() {
        transaction.create_table(HISTORY_TABLE, &HISTORY_COLUMNS)?;
    }
    if meta_row.is_some() {
        transaction.update_in(HISTORY_META_TABLE, 1, &[Value::Integer(1)])?;
    } else {
        transaction.insert_into(HISTORY_META_TABLE, 1, &[Value::Integer(1)])?;
    }
    transaction.commit()?;
    Ok(())
}

fn optional_table_columns(
    database: &mut Database,
    table: &str,
) -> Result<Option<Vec<String>>, HistoryError> {
    match database.table_columns(table) {
        Ok(columns) => Ok(Some(columns)),
        Err(DatabaseError::TableNotFound(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn decode_schema_version(bytes: &[u8]) -> Result<u32, HistoryError> {
    let record = RecordRef::new(bytes).map_err(|_| HistoryError::CorruptRecord)?;
    if record.field_count() != 1 {
        return Err(HistoryError::CorruptRecord);
    }
    match record.get(0).map_err(|_| HistoryError::CorruptRecord)? {
        Some(ValueRef::Integer(version)) => {
            u32::try_from(version).map_err(|_| HistoryError::CorruptRecord)
        }
        _ => Err(HistoryError::CorruptRecord),
    }
}

fn upsert(
    database: &mut Database,
    media_path: &str,
    position_ms: u64,
    duration_ms: u64,
    updated_at_ms: u64,
) -> Result<i64, HistoryError> {
    let media_path = normalize_media_path(media_path)?;
    let values = [
        Value::Text(&media_path),
        Value::Integer(i64::try_from(position_ms).map_err(|_| HistoryError::IntegerRange)?),
        Value::Integer(i64::try_from(duration_ms).map_err(|_| HistoryError::IntegerRange)?),
        Value::Integer(i64::try_from(updated_at_ms).map_err(|_| HistoryError::IntegerRange)?),
    ];

    let mut transaction = database.begin_transaction();
    let rows = transaction.scan_table(HISTORY_TABLE)?;
    let mut matching_rowids = Vec::new();
    for (rowid, bytes) in rows {
        if decode_entry(&bytes)?.media_path == media_path {
            matching_rowids.push(rowid);
        }
    }

    let rowid = if let Some((&rowid, duplicates)) = matching_rowids.split_first() {
        transaction.update_in(HISTORY_TABLE, rowid, &values)?;
        for duplicate in duplicates {
            transaction.delete_from(HISTORY_TABLE, *duplicate)?;
        }
        rowid
    } else {
        let rowid = transaction.next_rowid(HISTORY_TABLE)?;
        transaction.insert_into(HISTORY_TABLE, rowid, &values)?;
        rowid
    };
    transaction.commit()?;
    Ok(rowid)
}

fn get(database: &mut Database, media_path: &str) -> Result<Option<HistoryEntry>, HistoryError> {
    let media_path = normalize_media_path(media_path)?;
    for (_, bytes) in database.scan_table(HISTORY_TABLE)? {
        let entry = decode_entry(&bytes)?;
        if entry.media_path == media_path {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn decode_entry(bytes: &[u8]) -> Result<HistoryEntry, HistoryError> {
    let record = RecordRef::new(bytes).map_err(|_| HistoryError::CorruptRecord)?;
    if record.field_count() != HISTORY_COLUMNS.len() {
        return Err(HistoryError::CorruptRecord);
    }
    let media_path = match record.get(0).map_err(|_| HistoryError::CorruptRecord)? {
        Some(ValueRef::Text(path)) => normalize_media_path(path)?,
        _ => return Err(HistoryError::CorruptRecord),
    };
    let integer = |column| -> Result<u64, HistoryError> {
        match record
            .get(column)
            .map_err(|_| HistoryError::CorruptRecord)?
        {
            Some(ValueRef::Integer(value)) => {
                u64::try_from(value).map_err(|_| HistoryError::CorruptRecord)
            }
            _ => Err(HistoryError::CorruptRecord),
        }
    };
    Ok(HistoryEntry {
        media_path,
        position_ms: integer(1)?,
        duration_ms: integer(2)?,
        updated_at_ms: integer(3)?,
    })
}

fn normalize_media_path(path: &str) -> Result<String, HistoryError> {
    if path.is_empty() || path.contains('\0') {
        return Err(HistoryError::InvalidMediaPath);
    }

    let path = path.replace('\\', "/");
    let (prefix, rest, absolute) = if path.starts_with('/') {
        ("/", path.trim_start_matches('/'), true)
    } else if path.len() >= 3
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path.as_bytes()[1] == b':'
        && path.as_bytes()[2] == b'/'
    {
        (&path[..3], &path[3..], true)
    } else {
        ("", path.as_str(), false)
    };

    let mut components = Vec::new();
    for component in rest.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.last().is_some_and(|previous| *previous != "..") {
                    components.pop();
                } else if !absolute {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }

    let joined = components.join("/");
    let normalized = if prefix == "/" {
        format!("/{joined}")
    } else if !prefix.is_empty() {
        format!("{prefix}{joined}")
    } else {
        joined
    };
    if normalized.is_empty() || normalized == "/" || normalized.ends_with(":/") {
        return Err(HistoryError::InvalidMediaPath);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_vfs::SimVfs;
    use std::path::Path;

    #[test]
    fn migration_and_normalized_upsert_preserve_unique_history() {
        let vfs = SimVfs::new();
        let path = Path::new("example-playback-history.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");

        ensure_schema(&mut database).expect("migrate playback schema");
        let rowid = upsert(
            &mut database,
            "/storage//Movies/../movie.mp4",
            1_500,
            7_000,
            100,
        )
        .expect("insert history");
        assert_eq!(
            upsert(&mut database, "/storage/movie.mp4", 2_500, 7_000, 200)
                .expect("update existing history"),
            rowid
        );
        assert_eq!(
            database
                .scan_table(HISTORY_TABLE)
                .expect("scan history table")
                .len(),
            1
        );
        assert_eq!(
            get(&mut database, "/storage/./movie.mp4").expect("read history"),
            Some(HistoryEntry {
                media_path: "/storage/movie.mp4".to_owned(),
                position_ms: 2_500,
                duration_ms: 7_000,
                updated_at_ms: 200,
            })
        );
        drop(database);

        let mut reopened = Database::open(&vfs, path, 4).expect("reopen database");
        ensure_schema(&mut reopened).expect("verify schema version");
        assert_eq!(
            get(&mut reopened, "/storage/movie.mp4")
                .expect("read reopened history")
                .expect("history exists")
                .position_ms,
            2_500
        );
    }

    #[test]
    fn migration_keeps_history_from_the_previous_core_schema() {
        let vfs = SimVfs::new();
        let path = Path::new("example-legacy-playback-history.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        database
            .create_table(HISTORY_TABLE, &HISTORY_COLUMNS)
            .expect("create legacy history table");
        database
            .insert_into(
                HISTORY_TABLE,
                1,
                &[
                    Value::Text("/storage//Movies/../old.mp4"),
                    Value::Integer(300),
                    Value::Integer(900),
                    Value::Integer(50),
                ],
            )
            .expect("insert legacy history");
        drop(database);

        let mut reopened = Database::open(&vfs, path, 4).expect("open legacy database");
        ensure_schema(&mut reopened).expect("migrate legacy history metadata");
        assert_eq!(
            get(&mut reopened, "/storage/old.mp4").expect("read legacy history"),
            Some(HistoryEntry {
                media_path: "/storage/old.mp4".to_owned(),
                position_ms: 300,
                duration_ms: 900,
                updated_at_ms: 50,
            })
        );
    }

    #[test]
    fn normalization_rejects_empty_root_and_nul_paths() {
        assert_eq!(
            normalize_media_path(r"C:\Movies\.\Movie.mp4").expect("normalize Windows path"),
            "C:/Movies/Movie.mp4"
        );
        assert_eq!(
            normalize_media_path("../media/./movie.mp4").expect("normalize relative path"),
            "../media/movie.mp4"
        );
        assert!(matches!(
            normalize_media_path("a/.."),
            Err(HistoryError::InvalidMediaPath)
        ));
        assert!(matches!(
            normalize_media_path("/media/\0movie.mp4"),
            Err(HistoryError::InvalidMediaPath)
        ));
    }
}
