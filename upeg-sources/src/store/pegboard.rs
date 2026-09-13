//! Pegboard persistence on the SQLite store: row-level CRUD plus the
//! whole-state load/save pair the current facade callers use.
//!
//! `load_state` filters tombstones (`deleted = 1`) and orders boards by
//! `position`; `save_state` diffs against the stored rows inside one write
//! transaction so `updated_at` only moves for rows that actually changed,
//! and removed rows become tombstones instead of being deleted. Row-level
//! APIs (`upsert_placement` / `tombstone_placement` / `save_selection`)
//! exist so callers can migrate off the load-all/save-all pattern later.
//!
//! Both whole-state calls take a [`BoardVisibility`], which translates
//! between the *store* key of a board row and the *visible* key a user
//! types. Every row this pass cannot see is skipped on read and left
//! strictly alone on write — including the tombstone sweep, which
//! compares in visible space. That is what keeps one project's board
//! pins from being wiped by a `save_state` performed inside another.

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension as _, Transaction, params};
use upeg_core::{ArgsPreset, BoardGuidance, ColSpan, PinColorHex, PinSpan, Placement, RowSpan};

use super::schema::{BOARDS_TABLE, PLACEMENTS_TABLE, SELECTION_ROW_ID, SELECTION_TABLE};
use super::{LIVE, Store, StoreError, TOMBSTONED, epoch_millis_now};
use crate::pegboard::scope::BoardVisibility;
use crate::pegboard::{BoardData, PegboardSelection, PegboardState};

impl Store {
    /// Update only guidance. A fresh store seeds the same default boards as
    /// the display facade, within the write transaction, before the edit.
    pub(crate) fn update_board_guidance(
        &mut self,
        key: &str,
        guidance: &BoardGuidance,
        visibility: &BoardVisibility,
    ) -> Result<bool, StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx_if_changed(|tx| {
            let sql = format!(
                "UPDATE {BOARDS_TABLE} SET description = ?1, instructions = ?2, \
                 updated_at = ?3, device_id = ?4 WHERE key = ?5 AND deleted = {LIVE}"
            );
            let values = params![
                guidance.description,
                guidance.instructions,
                now,
                device_id,
                visibility.store_key(key)
            ];
            if tx.execute(&sql, values)? > 0 {
                return Ok(true);
            }

            let mut rows = tx.prepare(&format!(
                "SELECT key FROM {BOARDS_TABLE} WHERE deleted = {LIVE}"
            ))?;
            let keys = rows.query_map([], |row| row.get::<_, String>(0))?;
            for stored_key in keys {
                if visibility.owns(&stored_key?).is_some() {
                    return Ok(false);
                }
            }
            let defaults = crate::pegboard::default_state();
            if !defaults.boards.iter().any(|board| board.key == key) {
                return Ok(false);
            }
            save_boards_tx(tx, &defaults.boards, visibility, now, &device_id)?;
            save_layouts_tx(tx, &defaults.layouts, visibility, now, &device_id)?;
            Ok(tx.execute(&sql, values)? > 0)
        })
    }

    /// Load the shared pegboard state: live boards in `position` order,
    /// live placements grouped per board (boards without placements get an
    /// explicit empty layout so "user unpinned everything" survives the
    /// round trip), and the single selection row.
    pub fn load_state(&self, visibility: &BoardVisibility) -> Result<PegboardState, StoreError> {
        // The board rows and their pins must come from one SQLite snapshot:
        // MCP binds a revision and dispatches from this same loaded state.
        let read = self.connection().unchecked_transaction()?;
        let boards = self.load_boards(visibility)?;
        let mut layouts: BTreeMap<String, Vec<Placement>> = boards
            .iter()
            .map(|board| (board.key.clone(), Vec::new()))
            .collect();
        for (store_key, placement) in self.load_live_placements()? {
            // Placements whose board was tombstoned, belongs to another
            // project, or sits on a shadowed row stay invisible.
            let Some(visible) = visibility.owns(&store_key) else {
                continue;
            };
            if let Some(entry) = layouts.get_mut(&visible) {
                entry.push(placement);
            }
        }
        let selection = self.load_selection()?;
        read.commit()?;
        Ok(PegboardState {
            boards,
            layouts,
            selection,
        })
    }

    /// Persist the full state in one write transaction: upsert changed
    /// rows, tombstone missing ones, replace the selection row.
    pub fn save_state(
        &mut self,
        state: &PegboardState,
        visibility: &BoardVisibility,
    ) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx(|tx| {
            save_boards_tx(tx, &state.boards, visibility, now, &device_id)?;
            save_layouts_tx(tx, &state.layouts, visibility, now, &device_id)?;
            save_selection_tx(tx, &state.selection, now, &device_id)
        })
    }

    /// Insert or update a single placement row. The board row must exist
    /// (foreign key) — callers create boards through `save_state` first.
    pub fn upsert_placement(
        &mut self,
        board_key: &str,
        placement: &Placement,
    ) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx(|tx| upsert_placement_tx(tx, board_key, placement, now, &device_id))
    }

    /// Tombstone a single placement row (no-op when the row is absent).
    pub fn tombstone_placement(
        &mut self,
        board_key: &str,
        tool_id: &str,
    ) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx(|tx| tombstone_placement_tx(tx, board_key, tool_id, now, &device_id))
    }

    /// [`Self::load_state`] with no project in scope. Test-only sugar:
    /// project-board visibility is exercised by the tests that care, and
    /// every other store test is about global rows.
    #[cfg(test)]
    pub(crate) fn load_state_global(&self) -> Result<PegboardState, StoreError> {
        self.load_state(&BoardVisibility::global_only())
    }

    /// [`Self::save_state`] with no project in scope. See
    /// [`Self::load_state_global`].
    #[cfg(test)]
    pub(crate) fn save_state_global(&mut self, state: &PegboardState) -> Result<(), StoreError> {
        self.save_state(state, &BoardVisibility::global_only())
    }

    /// Replace the single selection row.
    pub fn save_selection(&mut self, selection: &PegboardSelection) -> Result<(), StoreError> {
        let device_id = self.device_id()?;
        let now = epoch_millis_now();
        self.write_tx(|tx| save_selection_tx(tx, selection, now, &device_id))
    }

    fn load_boards(&self, visibility: &BoardVisibility) -> Result<Vec<BoardData>, StoreError> {
        let sql = format!(
            "SELECT key, title, description, instructions FROM {BOARDS_TABLE} \
             WHERE deleted = ?1 ORDER BY position, key"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([LIVE], |row| {
            let key: String = row.get(0)?;
            let title: String = row.get(1)?;
            let guidance = BoardGuidance {
                description: row.get(2)?,
                instructions: row.get(3)?,
            };
            Ok((key, title, guidance))
        })?;

        let mut boards: Vec<BoardData> = Vec::new();
        for row in rows {
            let (store_key, title, guidance) = row?;
            let Some(visible) = visibility.owns(&store_key) else {
                continue;
            };
            boards.push(BoardData {
                key: visible,
                title,
                guidance,
            });
        }
        Ok(boards)
    }

    fn load_live_placements(&self) -> Result<Vec<(String, Placement)>, StoreError> {
        let sql = format!(
            "SELECT board_key, tool_id, x, y, color, span_cols, span_rows, args_preset \
             FROM {PLACEMENTS_TABLE} WHERE deleted = ?1 ORDER BY board_key, y, x, tool_id"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let rows = stmt.query_map([LIVE], |row| {
            Ok(RawPlacementRow {
                board_key: row.get(0)?,
                tool_id: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                color: row.get(4)?,
                span_cols: row.get(5)?,
                span_rows: row.get(6)?,
                args_preset: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?.into_placement()?);
        }
        Ok(out)
    }

    fn load_selection(&self) -> Result<PegboardSelection, StoreError> {
        let sql =
            format!("SELECT board_key, tag FROM {SELECTION_TABLE} WHERE id = {SELECTION_ROW_ID}");
        let selection = self
            .connection()
            .query_row(&sql, [], |row| {
                Ok(PegboardSelection {
                    board_key: row.get(0)?,
                    tag: row.get(1)?,
                })
            })
            .optional()?;
        Ok(selection.unwrap_or_default())
    }
}

/// Raw SQLite row image of a placement; domain validation (color hex,
/// span newtypes, preset JSON) happens in [`Self::into_placement`] so a
/// corrupted database surfaces as [`StoreError::Corrupt`], never as a
/// silently-invalid [`Placement`].
struct RawPlacementRow {
    board_key: String,
    tool_id: String,
    x: u16,
    y: u16,
    color: Option<String>,
    span_cols: Option<u16>,
    span_rows: Option<u16>,
    args_preset: Option<String>,
}

impl RawPlacementRow {
    fn into_placement(self) -> Result<(String, Placement), StoreError> {
        let color = self
            .color
            .as_deref()
            .map(PinColorHex::parse)
            .transpose()
            .map_err(|err| corrupt("placements.color", &err))?;
        let span = match (self.span_cols, self.span_rows) {
            (None, None) => None,
            (Some(cols), Some(rows)) => Some(PinSpan::new(
                ColSpan::new(cols).map_err(|err| corrupt("placements.span_cols", &err))?,
                RowSpan::new(rows).map_err(|err| corrupt("placements.span_rows", &err))?,
            )),
            _ => {
                return Err(StoreError::Corrupt {
                    context: "placements.span",
                    message: "span_cols and span_rows must be set together".into(),
                });
            }
        };
        let args_preset = self
            .args_preset
            .as_deref()
            .map(ArgsPreset::parse)
            .transpose()
            .map_err(|err| corrupt("placements.args_preset", &err))?;
        let placement = Placement::new(self.tool_id, self.x, self.y)
            .with_color(color)
            .with_span(span)
            .with_args_preset(args_preset);
        Ok((self.board_key, placement))
    }
}

fn corrupt(context: &'static str, err: &impl std::fmt::Display) -> StoreError {
    StoreError::Corrupt {
        context,
        message: err.to_string(),
    }
}

fn save_boards_tx(
    tx: &Transaction<'_>,
    boards: &[BoardData],
    visibility: &BoardVisibility,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let select = format!(
        "SELECT key, title, position, deleted, description, instructions FROM {BOARDS_TABLE}"
    );
    let mut stmt = tx.prepare(&select)?;
    let existing: BTreeMap<String, (String, i64, i64, BoardGuidance)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                (
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    BoardGuidance {
                        description: row.get(4)?,
                        instructions: row.get(5)?,
                    },
                ),
            ))
        })?
        .collect::<Result<_, _>>()?;

    let upsert = format!(
        "INSERT INTO {BOARDS_TABLE} (key, title, position, updated_at, device_id, description, instructions, deleted) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, {LIVE}) \
         ON CONFLICT(key) DO UPDATE SET \
           title = excluded.title, position = excluded.position, \
           description = excluded.description, instructions = excluded.instructions, \
           updated_at = excluded.updated_at, device_id = excluded.device_id, \
           deleted = {LIVE}"
    );
    let empty_guidance = BoardGuidance::default();
    for (position, board) in boards.iter().enumerate() {
        let position = i64::try_from(position).unwrap_or(i64::MAX);
        let store_key = visibility.store_key(&board.key);
        // The project file owns guidance; a saved layout must not become
        // a second source or freeze a copied version of its instructions.
        let guidance = if visibility.is_project_board(&board.key) {
            &empty_guidance
        } else {
            &board.guidance
        };
        let unchanged =
            existing
                .get(&store_key)
                .is_some_and(|(title, pos, deleted, stored_guidance)| {
                    *title == board.title
                        && *pos == position
                        && *deleted == LIVE
                        && stored_guidance == guidance
                });
        if unchanged {
            continue;
        }
        tx.execute(
            &upsert,
            params![
                store_key,
                board.title,
                position,
                now,
                device_id,
                guidance.description,
                guidance.instructions
            ],
        )?;
    }

    let tombstone = format!(
        "UPDATE {BOARDS_TABLE} SET deleted = {TOMBSTONED}, updated_at = ?1, device_id = ?2 \
         WHERE key = ?3"
    );
    for (store_key, (_, _, deleted, _)) in &existing {
        if *deleted != LIVE {
            continue;
        }
        // A pass only sweeps rows it would itself write: another
        // project's namespace, and a global row shadowed by a same-id
        // project board, are both left exactly as they are.
        let Some(visible) = visibility.owns(store_key) else {
            continue;
        };
        if !boards.iter().any(|board| board.key == visible) {
            tx.execute(&tombstone, params![now, device_id, store_key])?;
        }
    }
    Ok(())
}

fn save_layouts_tx(
    tx: &Transaction<'_>,
    layouts: &BTreeMap<String, Vec<Placement>>,
    visibility: &BoardVisibility,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let select = format!(
        "SELECT board_key, tool_id, x, y, color, span_cols, span_rows, args_preset, deleted \
         FROM {PLACEMENTS_TABLE}"
    );
    let mut stmt = tx.prepare(&select)?;
    type PlacementImage = (
        u16,
        u16,
        Option<String>,
        Option<u16>,
        Option<u16>,
        Option<String>,
        i64,
    );
    let existing: BTreeMap<(String, String), PlacementImage> = stmt
        .query_map([], |row| {
            Ok((
                (row.get(0)?, row.get(1)?),
                (
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ),
            ))
        })?
        .collect::<Result<_, _>>()?;

    for (visible_key, placements) in layouts {
        let store_key = visibility.store_key(visible_key);
        for placement in placements {
            let image = placement_image(placement);
            let unchanged = existing
                .get(&(store_key.clone(), placement.tool_id.clone()))
                .is_some_and(|(x, y, color, cols, rows, preset, deleted)| {
                    (*x, *y, color, cols, rows, preset, *deleted)
                        == (
                            image.0, image.1, &image.2, &image.3, &image.4, &image.5, LIVE,
                        )
                });
            if unchanged {
                continue;
            }
            upsert_placement_tx(tx, &store_key, placement, now, device_id)?;
        }
    }

    let is_live_in = |visible_key: &str, tool_id: &str| {
        layouts.get(visible_key).is_some_and(|placements| {
            placements
                .iter()
                .any(|placement| placement.tool_id == tool_id)
        })
    };
    for ((store_key, tool_id), (.., deleted)) in &existing {
        if *deleted != LIVE {
            continue;
        }
        let Some(visible_key) = visibility.owns(store_key) else {
            continue;
        };
        if !is_live_in(&visible_key, tool_id) {
            tombstone_placement_tx(tx, store_key, tool_id, now, device_id)?;
        }
    }
    Ok(())
}

/// Column image of a [`Placement`] as stored: `(x, y, color, span_cols,
/// span_rows, args_preset)`.
fn placement_image(
    placement: &Placement,
) -> (
    u16,
    u16,
    Option<String>,
    Option<u16>,
    Option<u16>,
    Option<String>,
) {
    let (span_cols, span_rows) = placement.span.map_or((None, None), |span| {
        (Some(span.cols.get()), Some(span.rows.get()))
    });
    (
        placement.x,
        placement.y,
        placement
            .color
            .as_ref()
            .map(|color| color.as_str().to_string()),
        span_cols,
        span_rows,
        placement
            .args_preset
            .as_ref()
            .map(|preset| preset.as_str().to_string()),
    )
}

fn upsert_placement_tx(
    tx: &Transaction<'_>,
    board_key: &str,
    placement: &Placement,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let (x, y, color, span_cols, span_rows, args_preset) = placement_image(placement);
    let sql = format!(
        "INSERT INTO {PLACEMENTS_TABLE} \
           (board_key, tool_id, x, y, color, span_cols, span_rows, args_preset, \
            updated_at, device_id, deleted) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, {LIVE}) \
         ON CONFLICT(board_key, tool_id) DO UPDATE SET \
           x = excluded.x, y = excluded.y, color = excluded.color, \
           span_cols = excluded.span_cols, span_rows = excluded.span_rows, \
           args_preset = excluded.args_preset, updated_at = excluded.updated_at, \
           device_id = excluded.device_id, deleted = {LIVE}"
    );
    tx.execute(
        &sql,
        params![
            board_key,
            placement.tool_id,
            x,
            y,
            color,
            span_cols,
            span_rows,
            args_preset,
            now,
            device_id
        ],
    )?;
    Ok(())
}

fn tombstone_placement_tx(
    tx: &Transaction<'_>,
    board_key: &str,
    tool_id: &str,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let sql = format!(
        "UPDATE {PLACEMENTS_TABLE} SET deleted = {TOMBSTONED}, updated_at = ?1, device_id = ?2 \
         WHERE board_key = ?3 AND tool_id = ?4"
    );
    tx.execute(&sql, params![now, device_id, board_key, tool_id])?;
    // An unpinned tool's cached last outcome is stale UI state, not
    // user data — drop it in the same transaction so re-pinning never
    // resurrects a result from a previous pin lifetime.
    super::last_outcomes::clear_last_outcome_tx(tx, board_key, tool_id)?;
    Ok(())
}

fn save_selection_tx(
    tx: &Transaction<'_>,
    selection: &PegboardSelection,
    now: i64,
    device_id: &str,
) -> Result<(), StoreError> {
    let current = tx
        .query_row(
            &format!("SELECT board_key, tag FROM {SELECTION_TABLE} WHERE id = {SELECTION_ROW_ID}"),
            [],
            |row| {
                Ok(PegboardSelection {
                    board_key: row.get(0)?,
                    tag: row.get(1)?,
                })
            },
        )
        .optional()?;
    if current.as_ref() == Some(selection) {
        return Ok(());
    }
    let sql = format!(
        "INSERT INTO {SELECTION_TABLE} (id, board_key, tag, updated_at, device_id) \
         VALUES ({SELECTION_ROW_ID}, ?1, ?2, ?3, ?4) \
         ON CONFLICT(id) DO UPDATE SET \
           board_key = excluded.board_key, tag = excluded.tag, \
           updated_at = excluded.updated_at, device_id = excluded.device_id"
    );
    tx.execute(
        &sql,
        params![selection.board_key, selection.tag, now, device_id],
    )?;
    Ok(())
}
