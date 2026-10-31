//! Per-pin last dispatch outcome persistence on the SQLite store.
//!
//! Keyed per placement (`board_key`, `pin_id`): repeated pins of the same
//! tool keep independent outcomes. Rows are a device-local
//! cache — an unpin deletes the row outright (see
//! `store::pegboard::tombstone_placement_tx`), and `record` simply
//! overwrites; there is no tombstone/merge story.
//!
//! `outputs_json` carries the complete canonical output entries as opaque
//! JSON (the FRB layer owns the entry shape). The `truncated` field remains
//! for legacy rows produced by older releases; new writes never truncate.

use rusqlite::params;

use super::schema::LAST_OUTCOMES_TABLE;
use super::{Store, StoreError, epoch_millis_now};

/// A freshly-dispatched outcome to persist. `truncated` / `updated_at` /
/// `device_id` are store-owned and never supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLastOutcome {
    pub pin_id: String,
    pub tool_id: String,
    /// Exact user input snapshot supplied to the dispatch, when available.
    pub args_json: Option<String>,
    pub ok: bool,
    pub primary_output_id: Option<String>,
    /// JSON array of canonical output entries (opaque to the store).
    pub outputs_json: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// One persisted outcome row as read back from the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastOutcomeRecord {
    pub pin_id: String,
    pub tool_id: String,
    pub args_json: Option<String>,
    pub ok: bool,
    pub primary_output_id: Option<String>,
    pub outputs_json: String,
    /// `true` for a legacy row whose outputs were truncated on insert.
    pub truncated: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub updated_at_ms: i64,
}

impl Store {
    /// Upsert the complete last outcome for one placement and stamp its
    /// recording time and device id.
    pub fn record_last_outcome(
        &mut self,
        board_key: &str,
        outcome: &NewLastOutcome,
    ) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        let sql = format!(
            "INSERT INTO {LAST_OUTCOMES_TABLE} \
               (board_key, pin_id, tool_id, ok, primary_output_id, outputs_json, truncated, \
                error_code, error_message, updated_at, device_id, args_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
             ON CONFLICT(board_key, pin_id) DO UPDATE SET \
               tool_id = excluded.tool_id, ok = excluded.ok, primary_output_id = excluded.primary_output_id, \
               outputs_json = excluded.outputs_json, truncated = excluded.truncated, \
               error_code = excluded.error_code, error_message = excluded.error_message, \
               updated_at = excluded.updated_at, device_id = excluded.device_id, \
               args_json = excluded.args_json"
        );
        self.write_local_cache_tx(|tx| {
            tx.execute(
                &sql,
                params![
                    board_key,
                    outcome.pin_id,
                    outcome.tool_id,
                    outcome.ok,
                    outcome.primary_output_id,
                    outcome.outputs_json,
                    false,
                    outcome.error_code,
                    outcome.error_message,
                    now,
                    device_id,
                    outcome.args_json,
                ],
            )?;
            Ok(())
        })
    }

    /// All persisted outcomes for one board, sorted by `pin_id`.
    pub fn load_last_outcomes(
        &self,
        board_key: &str,
    ) -> Result<Vec<LastOutcomeRecord>, StoreError> {
        let sql = format!(
            "SELECT pin_id, tool_id, ok, primary_output_id, outputs_json, truncated, \
                    error_code, error_message, updated_at, args_json \
             FROM {LAST_OUTCOMES_TABLE} WHERE board_key = ?1 ORDER BY pin_id"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([board_key], |row| {
            Ok(LastOutcomeRecord {
                pin_id: row.get(0)?,
                tool_id: row.get(1)?,
                ok: row.get(2)?,
                primary_output_id: row.get(3)?,
                outputs_json: row.get(4)?,
                truncated: row.get(5)?,
                error_code: row.get(6)?,
                error_message: row.get(7)?,
                updated_at_ms: row.get(8)?,
                args_json: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Drop one placement's persisted outcome (no-op when absent). The
    /// pegboard tombstone path calls the transaction-level twin
    /// automatically; this is the direct seam for callers that clear an
    /// outcome without unpinning.
    pub fn clear_last_outcome(&mut self, board_key: &str, pin_id: &str) -> Result<(), StoreError> {
        self.write_local_cache_tx(|tx| clear_last_outcome_tx(tx, board_key, pin_id))
    }
}

/// Transaction-level delete shared with the pegboard tombstone path so
/// unpinning a placement always clears its persisted outcome in the same
/// write transaction.
pub(crate) fn clear_last_outcome_tx(
    tx: &rusqlite::Transaction<'_>,
    board_key: &str,
    pin_id: &str,
) -> Result<(), StoreError> {
    let sql = format!("DELETE FROM {LAST_OUTCOMES_TABLE} WHERE board_key = ?1 AND pin_id = ?2");
    tx.execute(&sql, params![board_key, pin_id])?;
    Ok(())
}

pub(crate) fn clear_board_last_outcomes_tx(
    tx: &rusqlite::Transaction<'_>,
    board_key: &str,
) -> Result<(), StoreError> {
    tx.execute(
        &format!("DELETE FROM {LAST_OUTCOMES_TABLE} WHERE board_key = ?1"),
        [board_key],
    )?;
    Ok(())
}
