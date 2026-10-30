//! Current form input for each pin, kept separate from dispatch presets and
//! from the input that produced the last recorded outcome.

use rusqlite::{Transaction, params};

use super::schema::PIN_DRAFTS_TABLE;
use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinDraftRecord {
    pub pin_id: String,
    pub tool_id: String,
    pub args_json: String,
}

impl Store {
    pub fn save_pin_draft(
        &mut self,
        board_key: &str,
        draft: &PinDraftRecord,
    ) -> Result<(), StoreError> {
        self.write_local_cache_tx(|tx| {
            tx.execute(
                &format!(
                    "INSERT INTO {PIN_DRAFTS_TABLE} (board_key, pin_id, tool_id, args_json) \
                     VALUES (?1, ?2, ?3, ?4) ON CONFLICT(board_key, pin_id) DO UPDATE SET \
                     tool_id = excluded.tool_id, args_json = excluded.args_json"
                ),
                params![board_key, draft.pin_id, draft.tool_id, draft.args_json],
            )?;
            Ok(())
        })
    }

    pub fn load_pin_drafts(&self, board_key: &str) -> Result<Vec<PinDraftRecord>, StoreError> {
        let mut stmt = self.connection().prepare(&format!(
            "SELECT pin_id, tool_id, args_json FROM {PIN_DRAFTS_TABLE} \
             WHERE board_key = ?1 ORDER BY pin_id"
        ))?;
        let rows = stmt.query_map([board_key], |row| {
            Ok(PinDraftRecord {
                pin_id: row.get(0)?,
                tool_id: row.get(1)?,
                args_json: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn clear_pin_draft(&mut self, board_key: &str, pin_id: &str) -> Result<(), StoreError> {
        self.write_local_cache_tx(|tx| clear_pin_draft_tx(tx, board_key, pin_id))
    }
}

pub(crate) fn clear_pin_draft_tx(
    tx: &Transaction<'_>,
    board_key: &str,
    pin_id: &str,
) -> Result<(), StoreError> {
    tx.execute(
        &format!("DELETE FROM {PIN_DRAFTS_TABLE} WHERE board_key = ?1 AND pin_id = ?2"),
        params![board_key, pin_id],
    )?;
    Ok(())
}

pub(crate) fn clear_board_pin_drafts_tx(
    tx: &Transaction<'_>,
    board_key: &str,
) -> Result<(), StoreError> {
    tx.execute(
        &format!("DELETE FROM {PIN_DRAFTS_TABLE} WHERE board_key = ?1"),
        [board_key],
    )?;
    Ok(())
}
