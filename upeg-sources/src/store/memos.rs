//! Memo persistence on the SQLite store.
//!
//! Memos are a flat `String → String` map (key → body). `load_memos`
//! filters tombstones (`deleted = 1`); `save_memos` diffs against the
//! stored rows inside one write transaction so `updated_at` only moves
//! for rows that actually changed, and removed keys become tombstones
//! instead of being deleted — mirroring the pegboard save path so a
//! future sync step can merge instead of clobber.

use std::collections::BTreeMap;

use rusqlite::{Transaction, params};

use super::schema::MEMOS_TABLE;
use super::{LIVE, Store, StoreError, TOMBSTONED, epoch_millis_now};

impl Store {
    /// Load all live memos, sorted by key (BTreeMap order).
    pub fn load_memos(&self) -> Result<BTreeMap<String, String>, StoreError> {
        let sql = format!("SELECT key, body FROM {MEMOS_TABLE} WHERE deleted = ?1 ORDER BY key");
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([LIVE], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(StoreError::from)
    }

    /// Replace the persisted memo map in one write transaction: upsert
    /// changed rows, tombstone keys missing from `memos`.
    pub fn save_memos(&mut self, memos: &BTreeMap<String, String>) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx(|tx| save_memos_tx(tx, memos, now, &device_id))
    }
}

fn save_memos_tx(
    tx: &Transaction<'_>,
    memos: &BTreeMap<String, String>,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let select = format!("SELECT key, body, deleted FROM {MEMOS_TABLE}");
    let mut stmt = tx.prepare(&select)?;
    let existing: BTreeMap<String, (String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
        .collect::<Result<_, _>>()?;

    let upsert = format!(
        "INSERT INTO {MEMOS_TABLE} (key, body, updated_at, device_id, deleted) \
         VALUES (?1, ?2, ?3, ?4, {LIVE}) \
         ON CONFLICT(key) DO UPDATE SET \
           body = excluded.body, updated_at = excluded.updated_at, \
           device_id = excluded.device_id, deleted = {LIVE}"
    );
    for (key, body) in memos {
        let unchanged = existing
            .get(key)
            .is_some_and(|(stored, deleted)| stored == body && *deleted == LIVE);
        if unchanged {
            continue;
        }
        tx.execute(&upsert, params![key, body, now, device_id])?;
    }

    let tombstone = format!(
        "UPDATE {MEMOS_TABLE} SET deleted = {TOMBSTONED}, updated_at = ?1, device_id = ?2 \
         WHERE key = ?3"
    );
    for (key, (_, deleted)) in &existing {
        if *deleted == LIVE && !memos.contains_key(key) {
            tx.execute(&tombstone, params![now, device_id, key])?;
        }
    }
    Ok(())
}
