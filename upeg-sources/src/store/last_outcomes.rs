//! Per-pin last dispatch outcome persistence on the SQLite store.
//!
//! Keyed per placement (`board_key`, `tool_id`): the same tool pinned on
//! two boards keeps two independent outcomes. Rows are a device-local
//! cache — an unpin deletes the row outright (see
//! `store::pegboard::tombstone_placement_tx`), and `record` simply
//! overwrites; there is no tombstone/merge story.
//!
//! `outputs_json` carries the canonical output entries as opaque JSON
//! (the FRB layer owns the entry shape). Before insert the payload is
//! pushed through [`cap_outputs_json`] so one enormous tool output can
//! never bloat the store: past [`OUTPUTS_JSON_MAX_BYTES`] only the
//! primary output entry survives (or nothing, when even that is too
//! big) and `truncated` records that the cap fired.

use rusqlite::params;

use super::schema::LAST_OUTCOMES_TABLE;
use super::{Store, StoreError, epoch_millis_now};

/// Upper bound on the stored `outputs_json` payload, in bytes.
pub const OUTPUTS_JSON_MAX_BYTES: usize = 64 * 1024;

/// A freshly-dispatched outcome to persist. `truncated` / `updated_at` /
/// `device_id` are store-owned and never supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLastOutcome {
    pub tool_id: String,
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
    pub tool_id: String,
    pub ok: bool,
    pub primary_output_id: Option<String>,
    pub outputs_json: String,
    /// `true` when [`cap_outputs_json`] dropped outputs on insert.
    pub truncated: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub updated_at_ms: i64,
}

/// Result of applying the size cap to an `outputs_json` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CappedOutputsJson {
    pub json: String,
    pub truncated: bool,
}

/// Pure size-cap policy for `outputs_json`, applied before every insert.
///
/// - Within [`OUTPUTS_JSON_MAX_BYTES`]: stored verbatim, `truncated: false`.
/// - Over the cap: only the primary output entry (matched by `"id" ==
///   primary_output_id` inside the JSON array; first entry when no
///   primary id is set) is kept as a one-element preview array,
///   `truncated: true`.
/// - When even that preview exceeds the cap — or the payload is not a
///   JSON array — everything is dropped (`[]`, `truncated: true`).
pub fn cap_outputs_json(outputs_json: &str, primary_output_id: Option<&str>) -> CappedOutputsJson {
    if outputs_json.len() <= OUTPUTS_JSON_MAX_BYTES {
        return CappedOutputsJson {
            json: outputs_json.to_string(),
            truncated: false,
        };
    }
    let preview = primary_entry_json(outputs_json, primary_output_id)
        .filter(|entry| entry.len() <= OUTPUTS_JSON_MAX_BYTES);
    CappedOutputsJson {
        json: preview.unwrap_or_else(empty_outputs_json),
        truncated: true,
    }
}

/// JSON text of an empty outputs array — the "nothing survived the cap"
/// fallback.
fn empty_outputs_json() -> String {
    "[]".to_string()
}

/// Extract the primary entry of an outputs array as a one-element JSON
/// array. Entry matching is shape-agnostic: any array of objects whose
/// `id` field equals `primary_output_id` works; without a primary id the
/// first entry wins.
fn primary_entry_json(outputs_json: &str, primary_output_id: Option<&str>) -> Option<String> {
    let serde_json::Value::Array(entries) = serde_json::from_str(outputs_json).ok()? else {
        return None;
    };
    let primary = match primary_output_id {
        Some(id) => entries
            .iter()
            .find(|entry| entry.get("id").and_then(serde_json::Value::as_str) == Some(id)),
        None => entries.first(),
    }?;
    serde_json::to_string(&vec![primary]).ok()
}

impl Store {
    /// Upsert the last outcome for one placement, applying the size cap
    /// and stamping `updated_at` / `device_id`.
    pub fn record_last_outcome(
        &mut self,
        board_key: &str,
        outcome: &NewLastOutcome,
    ) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        let capped = cap_outputs_json(&outcome.outputs_json, outcome.primary_output_id.as_deref());
        let sql = format!(
            "INSERT INTO {LAST_OUTCOMES_TABLE} \
               (board_key, tool_id, ok, primary_output_id, outputs_json, truncated, \
                error_code, error_message, updated_at, device_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(board_key, tool_id) DO UPDATE SET \
               ok = excluded.ok, primary_output_id = excluded.primary_output_id, \
               outputs_json = excluded.outputs_json, truncated = excluded.truncated, \
               error_code = excluded.error_code, error_message = excluded.error_message, \
               updated_at = excluded.updated_at, device_id = excluded.device_id"
        );
        self.write_tx(|tx| {
            tx.execute(
                &sql,
                params![
                    board_key,
                    outcome.tool_id,
                    outcome.ok,
                    outcome.primary_output_id,
                    capped.json,
                    capped.truncated,
                    outcome.error_code,
                    outcome.error_message,
                    now,
                    device_id,
                ],
            )?;
            Ok(())
        })
    }

    /// All persisted outcomes for one board, sorted by `tool_id`.
    pub fn load_last_outcomes(
        &self,
        board_key: &str,
    ) -> Result<Vec<LastOutcomeRecord>, StoreError> {
        let sql = format!(
            "SELECT tool_id, ok, primary_output_id, outputs_json, truncated, \
                    error_code, error_message, updated_at \
             FROM {LAST_OUTCOMES_TABLE} WHERE board_key = ?1 ORDER BY tool_id"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([board_key], |row| {
            Ok(LastOutcomeRecord {
                tool_id: row.get(0)?,
                ok: row.get(1)?,
                primary_output_id: row.get(2)?,
                outputs_json: row.get(3)?,
                truncated: row.get(4)?,
                error_code: row.get(5)?,
                error_message: row.get(6)?,
                updated_at_ms: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Drop one placement's persisted outcome (no-op when absent). The
    /// pegboard tombstone path calls the transaction-level twin
    /// automatically; this is the direct seam for callers that clear an
    /// outcome without unpinning.
    pub fn clear_last_outcome(&mut self, board_key: &str, tool_id: &str) -> Result<(), StoreError> {
        self.write_tx(|tx| clear_last_outcome_tx(tx, board_key, tool_id))
    }
}

/// Transaction-level delete shared with the pegboard tombstone path so
/// unpinning a tool always clears its persisted outcome in the same
/// write transaction.
pub(crate) fn clear_last_outcome_tx(
    tx: &rusqlite::Transaction<'_>,
    board_key: &str,
    tool_id: &str,
) -> Result<(), StoreError> {
    let sql = format!("DELETE FROM {LAST_OUTCOMES_TABLE} WHERE board_key = ?1 AND tool_id = ?2");
    tx.execute(&sql, params![board_key, tool_id])?;
    Ok(())
}
