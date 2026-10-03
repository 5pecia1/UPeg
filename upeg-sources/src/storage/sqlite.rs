use std::path::Path;
use std::time::Duration;

use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
    types::ValueRef,
};
use serde_json::{Value, json};

use super::{Result, canonical, digest, exists, refuse};

fn read_connection(path: &Path) -> Result<Connection> {
    upeg_core::paths::check_storage_path(path)?;
    let wal = path.with_file_name(format!(
        "{}-wal",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let shm = path.with_file_name(format!(
        "{}-shm",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let has_wal = exists(&wal)? && std::fs::metadata(&wal)?.len() > 0;
    if has_wal && !exists(&shm)? {
        return Err(refuse(
            "WAL exists without its shared-memory index; quiesce/checkpoint with the old application before planning",
        ));
    }
    let raw = path
        .to_str()
        .ok_or_else(|| refuse("SQLite path is not UTF-8"))?;
    let escaped: String = raw
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/:._-".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    let uri = format!(
        "file:{escaped}?mode=ro{}",
        if has_wal { "" } else { "&immutable=1" }
    );
    let connection = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.execute_batch("BEGIN")?;
    Ok(connection)
}

fn integrity(connection: &Connection) -> Result<()> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(refuse("SQLite integrity_check failed"));
    }
    if connection
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(refuse("SQLite foreign_key_check failed"));
    }
    Ok(())
}

pub(super) fn fingerprint(path: &Path) -> Result<(String, u64)> {
    let connection = read_connection(path)?;
    integrity(&connection)?;
    let user_version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application_id: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    let mut schema = connection.prepare(
        "SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name,tbl_name,sql",
    )?;
    let definitions = schema
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut tables = Vec::new();
    for (kind, name, _, _) in &definitions {
        if kind != "table" {
            continue;
        }
        let quoted = name.replace('"', "\"\"");
        let mut statement = connection.prepare(&format!("SELECT * FROM \"{quoted}\""))?;
        let count = statement.column_count();
        let mut rows = statement.query([])?;
        let mut encoded = Vec::new();
        while let Some(row) = rows.next()? {
            let mut values: Vec<Value> = Vec::new();
            for index in 0..count {
                values.push(match row.get_ref(index)? {
                    ValueRef::Null => json!(["null"]),
                    ValueRef::Integer(value) => json!(["integer", value]),
                    ValueRef::Real(value) => json!(["real", value.to_bits()]),
                    ValueRef::Text(value) => json!(["text", value]),
                    ValueRef::Blob(value) => json!(["blob", value]),
                });
            }
            encoded.push(canonical(&values)?);
        }
        encoded.sort();
        tables.push((name, encoded));
    }
    let bytes = canonical(
        &json!({"user_version":user_version,"application_id":application_id,"schema":definitions,"tables":tables}),
    )?;
    Ok((digest(&bytes), bytes.len() as u64))
}

pub(super) fn writer_exclusion(path: &Path) -> Result<Option<Connection>> {
    if !exists(path)? {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.execute_batch("BEGIN IMMEDIATE")?;
    Ok(Some(connection))
}

pub(super) fn backup(source: &Path, destination: &Path) -> Result<()> {
    if exists(destination)? {
        return Err(refuse("backup destination already exists"));
    }
    // `destination` is always our own staging/publishing temp path, never a
    // user-facing file. Clear any stale -journal/-wal/-shm siblings left by a
    // prior interrupted attempt at this same temp name before opening a new
    // connection here: a leftover WAL from an earlier, unrelated backup could
    // otherwise be replayed onto the fresh main file.
    for suffix in ["-journal", "-wal", "-shm"] {
        let sidecar = destination.with_file_name(format!(
            "{}{suffix}",
            destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));
        upeg_core::paths::check_storage_path(&sidecar)?;
        if exists(&sidecar)? {
            std::fs::remove_file(&sidecar)?;
        }
    }
    let source = read_connection(source)?;
    let mut destination = Connection::open(destination)?;
    destination.busy_timeout(Duration::from_millis(250))?;
    {
        let backup = Backup::new(&source, &mut destination)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            match backup.step(256)? {
                StepResult::Done => break,
                StepResult::More if std::time::Instant::now() < deadline => {}
                _ => {
                    return Err(refuse(
                        "SQLite backup busy, locked, or timed out; stop all writers and retry",
                    ));
                }
            }
        }
    }
    integrity(&destination)?;
    destination.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE")?;
    Ok(())
}
