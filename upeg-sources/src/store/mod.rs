//! SQLite-backed shared native state store.
//!
//! Native surfaces (TUI, Flutter desktop via FRB) persist shared state in a
//! single SQLite database under the config root
//! ([`upeg_core::paths::STORE_FILE`]). v1 carries the pegboard tables
//! (boards / placements / selection); v2 adds memos and the metadata-only
//! execution log; v3 adds the per-pin last-outcome cache.
//!
//! Concurrency model: `journal_mode=WAL` lets one writer coexist with
//! readers across processes; writers open `BEGIN IMMEDIATE` transactions and
//! wait up to [`BUSY_TIMEOUT`] for a competing writer. Cross-surface change
//! detection is driven by the [`Store::change_rev`] counter (a `meta` row
//! bumped by every write transaction) — under WAL the main database file's
//! mtime does not reliably change on commit, so file-mtime polling would be
//! a silent regression.

mod execution_log;
pub mod export;
mod last_outcomes;
mod memos;
mod pegboard;
mod schema;

#[cfg(test)]
mod tests;

pub use execution_log::{ExecutionLogFilter, ExecutionLogRecord};
pub use last_outcomes::{
    CappedOutputsJson, LastOutcomeRecord, NewLastOutcome, OUTPUTS_JSON_MAX_BYTES, cap_outputs_json,
};

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

/// `meta` key holding the monotonically increasing change revision.
/// Absent until the first write; every write transaction adds 1.
const META_REV_KEY: &str = "rev";
/// `meta` key holding this installation's stable device id (UUIDv7,
/// generated on first open). Reserved for later sync/merge steps.
const META_DEVICE_ID_KEY: &str = "device_id";
/// How long a writer waits for a competing write transaction before
/// giving up with `SQLITE_BUSY`.
const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

/// SQLite value of the `deleted` flag for live rows.
pub(crate) const LIVE: i64 = 0;
/// SQLite value of the `deleted` flag for tombstoned rows.
pub(crate) const TOMBSTONED: i64 = 1;

const PRAGMA_JOURNAL_MODE: &str = "journal_mode";
/// WAL: readers never block the (single) writer and vice versa.
const JOURNAL_MODE_WAL: &str = "WAL";
const PRAGMA_SYNCHRONOUS: &str = "synchronous";
/// NORMAL is durable-enough under WAL (fsync on checkpoint, not per-commit).
const SYNCHRONOUS_NORMAL: &str = "NORMAL";
const PRAGMA_FOREIGN_KEYS: &str = "foreign_keys";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("config root unavailable; set UPEG_HOME, HOME, or APPDATA")]
    ConfigRootUnavailable,
    #[error("store I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("store SQL: {0}")]
    Sql(#[from] rusqlite::Error),
    /// A persisted value failed domain validation on read (e.g. a color
    /// or span that no longer passes its newtype constructor).
    #[error("store row corrupt ({context}): {message}")]
    Corrupt {
        context: &'static str,
        message: String,
    },
}

/// One open connection to the shared state database. Cheap to open;
/// surface code opens a fresh `Store` per operation rather than holding a
/// long-lived connection.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open the store at the default per-user path
    /// ([`upeg_core::paths::store_path`]).
    pub fn open() -> Result<Self, StoreError> {
        let path = upeg_core::paths::store_path().ok_or(StoreError::ConfigRootUnavailable)?;
        Self::open_at(&path)
    }

    /// Open (creating if needed) the store at an explicit database path.
    /// First-class API for tests and for callers that already resolved a
    /// config root.
    pub fn open_at(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(Connection::open(path)?)
    }

    /// In-memory store for unit tests only — nothing is shared across
    /// connections, so WAL/mtime/cross-process semantics do not apply.
    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self, StoreError> {
        // `PRAGMA journal_mode` answers with the resulting mode (e.g.
        // "memory" for in-memory databases), so it goes through the
        // check variant instead of the row-less `pragma_update`.
        conn.pragma_update_and_check(None, PRAGMA_JOURNAL_MODE, JOURNAL_MODE_WAL, |_row| Ok(()))?;
        conn.pragma_update(None, PRAGMA_SYNCHRONOUS, SYNCHRONOUS_NORMAL)?;
        conn.pragma_update(None, PRAGMA_FOREIGN_KEYS, true)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        schema::migrate(&mut conn)?;
        ensure_device_id(&conn)?;
        Ok(Self { conn })
    }

    /// Change revision of this database: `None` before the first write,
    /// then a counter that grows by 1 per write transaction. Surfaces
    /// poll this instead of file mtime (see module docs).
    pub fn change_rev(&self) -> Result<Option<u64>, StoreError> {
        read_meta_u64(&self.conn, META_REV_KEY)
    }

    /// Stable per-installation device id, generated (UUIDv7) on first open.
    pub fn device_id(&self) -> Result<String, StoreError> {
        read_meta(&self.conn, META_DEVICE_ID_KEY)?.ok_or_else(|| StoreError::Corrupt {
            context: "meta.device_id",
            message: "missing device_id row".into(),
        })
    }

    /// Run `f` inside a `BEGIN IMMEDIATE` write transaction and bump the
    /// change revision on commit. Every mutation of the store routes
    /// through here so rev-based polling can never miss a write.
    pub(crate) fn write_tx<T>(
        &mut self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let out = f(&tx)?;
        bump_rev(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// A rejected conditional edit rolls back without publishing a revision.
    pub(crate) fn write_tx_if_changed(
        &mut self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<bool, StoreError>,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !f(&tx)? {
            return Ok(false);
        }
        bump_rev(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn ensure_device_id(conn: &Connection) -> Result<(), StoreError> {
    let sql = format!(
        "INSERT OR IGNORE INTO {meta} (key, value) VALUES (?1, ?2)",
        meta = schema::META_TABLE,
    );
    conn.execute(
        &sql,
        rusqlite::params![META_DEVICE_ID_KEY, uuid::Uuid::now_v7().to_string()],
    )?;
    Ok(())
}

fn read_meta(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    use rusqlite::OptionalExtension as _;
    let sql = format!(
        "SELECT value FROM {meta} WHERE key = ?1",
        meta = schema::META_TABLE,
    );
    Ok(conn
        .query_row(&sql, [key], |row| row.get::<_, String>(0))
        .optional()?)
}

fn read_meta_u64(conn: &Connection, key: &str) -> Result<Option<u64>, StoreError> {
    let Some(raw) = read_meta(conn, key)? else {
        return Ok(None);
    };
    raw.parse::<u64>()
        .map(Some)
        .map_err(|err| StoreError::Corrupt {
            context: "meta counter",
            message: format!("{key}={raw}: {err}"),
        })
}

fn write_meta(conn: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    let sql = format!(
        "INSERT INTO {meta} (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        meta = schema::META_TABLE,
    );
    conn.execute(&sql, rusqlite::params![key, value])?;
    Ok(())
}

fn bump_rev(conn: &Connection) -> Result<(), StoreError> {
    let next = read_meta_u64(conn, META_REV_KEY)?
        .unwrap_or(0)
        .saturating_add(1);
    write_meta(conn, META_REV_KEY, &next.to_string())
}

/// Milliseconds since the Unix epoch, clamped into `i64` (SQLite INTEGER).
pub(crate) fn epoch_millis_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
