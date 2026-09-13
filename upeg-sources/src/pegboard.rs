//! Shared native Pegboard state.
//!
//! TUI and native Desktop must not invent separate board/layout/tag state.
//! This module is the facade over the shared SQLite store
//! ([`crate::store::Store`]); individual surfaces still own rendering and
//! platform-specific storage glue.
//!
//! Storage is coordinate-based: each placement carries an `(x, y)` chosen
//! by the user; the tool's `(w, h)` is derived from its manifest's
//! `pegboard_units`. The canonical column count is [`upeg_core::BOARD_COLS`].
//!
//! Cross-surface change detection uses [`state_change_rev`] (the store's
//! write counter), not file mtime: under WAL the main database file's
//! mtime does not reliably move on commit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use upeg_core::{
    BUILTIN_BOARDS, PinColorError, PinColorHex, PinSpan, Placement, Surface, ToolMeta,
};

use crate::store::{Store, StoreError, export};
use upeg_runtime::pegboard::{
    move_placement as move_placement_in_layout, pin_placement, place_tool_with_push,
    placements_from_ordered_ids, reconcile_placements, set_pin_color as set_pin_color_in_layout,
    swap_placement_pair as swap_placement_pair_in_layout, unpin_placement,
};
use upeg_runtime::{ToolMetaRuntimeExt, toolbox_tool, toolbox_tools};

mod guidance;
pub mod scope;

pub use guidance::{
    BoardGuidanceEditError, board_guidance_in, set_board_guidance, set_board_guidance_in,
    set_board_guidance_to_path_in,
};
pub use scope::BoardVisibility;
pub use upeg_core::BoardGuidance;
pub use upeg_runtime::pegboard::{Direction, slugify};

pub use upeg_core::paths::STORE_FILE;
pub const ALL_TAG: &str = "all";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardData {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub guidance: BoardGuidance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PegboardState {
    pub boards: Vec<BoardData>,
    pub layouts: BTreeMap<String, Vec<Placement>>,
    #[serde(default)]
    pub selection: PegboardSelection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PegboardSelection {
    pub board_key: Option<String>,
    pub tag: String,
}

impl Default for PegboardSelection {
    fn default() -> Self {
        Self {
            board_key: None,
            tag: ALL_TAG.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PegboardStateError {
    #[error("config root unavailable; set UPEG_HOME, HOME, or APPDATA")]
    ConfigRootUnavailable,
    #[error("pegboard state JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pegboard store: {0}")]
    Store(#[from] StoreError),
}

pub fn state_path_from_root(root: &Path) -> PathBuf {
    upeg_core::paths::store_path_in(root)
}

pub fn state_path_from_env() -> Option<PathBuf> {
    upeg_core::paths::store_path()
}

/// Change revision of the store at `path`: `None` while no store exists
/// or nothing was ever written, then a counter that grows with every
/// write. Successor of the old file-mtime probe — surfaces poll this to
/// pick up another surface's edits.
pub fn state_change_rev_from_path(path: &Path) -> Option<u64> {
    // Probing must not create the database as a side effect.
    if !path.exists() {
        return None;
    }
    Store::open_at(path).ok()?.change_rev().ok().flatten()
}

pub fn state_change_rev() -> Option<u64> {
    state_path_from_env().and_then(|path| state_change_rev_from_path(&path))
}

/// The boards a fresh state starts with, read from the shared const
/// table ([`upeg_core::BUILTIN_BOARDS`]) rather than spelled out here —
/// a Project Manifest extends the same list at runtime through
/// [`merge_project_boards`], so "which boards exist" has to be data.
pub fn default_boards() -> Vec<BoardData> {
    BUILTIN_BOARDS
        .iter()
        .map(|board| BoardData {
            key: board.key.into(),
            title: board.title.into(),
            guidance: BoardGuidance::default(),
        })
        .collect()
}

/// Add the active Project Manifest's declared boards to `state`, in
/// declaration order, after the global ones.
///
/// A declared board that is already present keeps its saved title and
/// placements, while its guidance always comes from the current manifest.
/// A declared board with no row yet is seeded with the tools that
/// name it in their manifest `boards` array, exactly like a built-in.
///
/// This runs on every load, which is what makes a project board exist
/// only while its manifest is detected: nothing persists the merge.
pub fn merge_project_boards(state: &mut PegboardState, visibility: &BoardVisibility) {
    let Some(scope) = visibility.project() else {
        return;
    };
    for declaration in scope.boards() {
        let key = declaration.id.as_str();
        if let Some(board) = state.boards.iter_mut().find(|board| board.key == key) {
            board.guidance.clone_from(&declaration.guidance);
            continue;
        }
        state.boards.push(BoardData {
            key: key.to_string(),
            title: declaration.label.clone(),
            guidance: declaration.guidance.clone(),
        });
        state
            .layouts
            .entry(key.to_string())
            .or_insert_with(|| seeded_layout(key));
    }
}

/// Placements a board starts with: every Desktop-surface tool whose
/// manifest declares this board, in id order.
fn seeded_layout(board_key: &str) -> Vec<Placement> {
    let mut ids: Vec<String> = toolbox_tools()
        .filter(|tool| tool.is_on_surface(Surface::Desktop))
        .filter(|tool| tool.is_on_board(board_key))
        .map(|tool| tool.id.to_string())
        .collect();
    ids.sort();
    placements_from_ordered_ids(ids.iter().map(String::as_str))
}

pub fn default_layouts(boards: &[BoardData]) -> BTreeMap<String, Vec<Placement>> {
    boards
        .iter()
        .map(|board| (board.key.clone(), seeded_layout(&board.key)))
        .collect()
}

pub fn default_state() -> PegboardState {
    let boards = default_boards();
    PegboardState {
        layouts: default_layouts(&boards),
        boards,
        selection: PegboardSelection::default(),
    }
}

/// Load the shared state as seen through `visibility`: the store's
/// global boards, this project's boards (merged in even when they have
/// never been saved), and nothing belonging to another project.
///
/// The explicit-visibility variant is the primary one — it is pure with
/// respect to process state, so a test can exercise "inside project A"
/// and "outside any project" in the same run.
pub fn load_state_from_path_in(
    path: &Path,
    visibility: &BoardVisibility,
) -> Result<PegboardState, PegboardStateError> {
    let store = Store::open_at(path)?;
    // Sanitize FIRST: that pass owns the "an empty store means the
    // built-in boards" fallback, and merging before it would make a
    // project board count as content and suppress the built-ins.
    let mut state = sanitize_state(store.load_state(visibility)?);
    merge_project_boards(&mut state, visibility);
    Ok(state)
}

pub fn load_state_from_path(path: &Path) -> Result<PegboardState, PegboardStateError> {
    load_state_from_path_in(path, &BoardVisibility::from_process())
}

/// Persist `state` through `visibility`: project boards land in their
/// namespace, and rows this visibility cannot see are left untouched.
pub fn save_state_to_path_in(
    path: &Path,
    state: &PegboardState,
    visibility: &BoardVisibility,
) -> Result<(), PegboardStateError> {
    let mut store = Store::open_at(path)?;
    store.save_state(&sanitize_state(state.clone()), visibility)?;
    Ok(())
}

pub fn save_state_to_path(path: &Path, state: &PegboardState) -> Result<(), PegboardStateError> {
    save_state_to_path_in(path, state, &BoardVisibility::from_process())
}

pub fn load_state() -> PegboardState {
    let visibility = BoardVisibility::from_process();
    let Some(path) = state_path_from_env() else {
        return default_state_in(&visibility);
    };
    if let Ok(state) = load_state_from_path_in(&path, &visibility) {
        return state;
    }
    default_state_in(&visibility)
}

/// Fallback state when no store is reachable: the built-in boards plus
/// the active project's declared boards, so a project board is listed
/// even before anything has ever been written.
fn default_state_in(visibility: &BoardVisibility) -> PegboardState {
    let mut state = default_state();
    merge_project_boards(&mut state, visibility);
    state
}

pub fn save_state(state: &PegboardState) -> Result<(), PegboardStateError> {
    let path = state_path_from_env().ok_or(PegboardStateError::ConfigRootUnavailable)?;
    save_state_to_path(&path, state)
}

pub fn boards_json() -> Option<String> {
    export::boards_to_json(&load_state().boards).ok()
}

pub fn layouts_json() -> Option<String> {
    export::layouts_to_json(&load_state().layouts).ok()
}

pub fn selected_tag_value() -> Option<String> {
    Some(load_state().selection.tag)
}

pub fn selection_value() -> PegboardSelection {
    load_state().selection
}

pub fn save_selection_value(selection: PegboardSelection) -> Result<(), PegboardStateError> {
    let mut state = load_state();
    state.selection = selection;
    state = sanitize_state(state);
    save_state(&state)
}

pub fn save_boards_json(json: &str) -> Result<(), PegboardStateError> {
    let boards = export::boards_from_json(json)?;
    let mut state = load_state();
    state.boards = boards;
    state = sanitize_state(state);
    save_state(&state)
}

pub fn save_layouts_json(json: &str) -> Result<(), PegboardStateError> {
    let mut state = load_state();
    state.layouts = export::layouts_from_json(json)?;
    state = sanitize_state(state);
    save_state(&state)
}

pub fn save_selected_tag_value(tag: &str) -> Result<(), PegboardStateError> {
    let mut state = load_state();
    state.selection.tag = normalize_tag(tag);
    state = sanitize_state(state);
    save_state(&state)
}

pub fn board_keys() -> Vec<String> {
    load_state()
        .boards
        .into_iter()
        .map(|board| board.key)
        .collect()
}

/// Outcome of [`toggle_pin`] / [`pin_tool`] / [`unpin_tool`]. Naming the
/// three states explicitly lets callers tell "successfully unpinned" from
/// "did nothing because the tool was unknown" without relying on a
/// `bool` overload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinAction {
    /// The tool was just added to the board's layout.
    Pinned,
    /// The tool was just removed from the board's layout.
    Unpinned,
    /// Nothing changed — the tool is unknown to the registry, the board
    /// doesn't exist, or `pin_tool` was called on an already-pinned id.
    NoOp,
}

/// Append `-2`, `-3`, … until the slug is unique among `boards`.
fn dedupe_board_key(base: &str, boards: &[BoardData]) -> String {
    let exists = |k: &str| boards.iter().any(|b| b.key == k);
    if !exists(base) {
        return base.to_string();
    }
    let max_attempt = boards.len() + 2;
    for n in 2..=max_attempt {
        let candidate = format!("{base}-{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", max_attempt + 1)
}

/// Add a new board with the given title to the shared pegboard state.
/// Returns the new board's key on success, or `None` if the title is
/// blank/all-whitespace. The key is a slug derived from the title,
/// deduped against existing boards. An empty layout entry is inserted
/// so subsequent `pin_tool` calls land cleanly.
pub fn add_board(state: &mut PegboardState, title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }
    let base_key = slugify(trimmed);
    let key = dedupe_board_key(&base_key, &state.boards);
    state.boards.push(BoardData {
        key: key.clone(),
        title: trimmed.to_string(),
        guidance: BoardGuidance::default(),
    });
    state.layouts.insert(key.clone(), Vec::new());
    Some(key)
}

/// Remove a board by key, dropping its layout entry as well. Returns
/// `true` if a board was actually removed. Caller is responsible for
/// any selected-board fallback (move the cursor / filter onto a
/// remaining board).
///
/// A board the active Project Manifest declares cannot be removed: the
/// manifest is its source of truth, so the next load would merge it
/// straight back. Refusing here is the honest answer instead of a
/// delete that silently undoes itself.
pub fn remove_board(state: &mut PegboardState, key: &str) -> bool {
    if BoardVisibility::from_process().is_project_board(key) {
        return false;
    }
    let before = state.boards.len();
    state.boards.retain(|b| b.key != key);
    if state.boards.len() == before {
        return false;
    }
    state.layouts.remove(key);
    true
}

/// Rename a board: replace its `title` while keeping its `key` (so
/// existing layouts stay attached). Returns `true` if the board exists
/// and the new title is non-blank.
pub fn rename_board(state: &mut PegboardState, key: &str, new_title: &str) -> bool {
    let trimmed = new_title.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(board) = state.boards.iter_mut().find(|b| b.key == key) else {
        return false;
    };
    board.title = trimmed.to_string();
    true
}

/// Pin a tool to the board, appending to the end of its layout. Returns
/// [`PinAction::Pinned`] on first pin, [`PinAction::NoOp`] if the tool
/// id is unknown to the registry or already pinned. Idempotent — re-
/// pinning produces `NoOp`, not a duplicate entry.
///
/// Uses `entry().or_default()` so a freshly-added board (no layout
/// entry yet) initialises cleanly. This matches Desktop's iter-183 fix:
/// the previous `get_mut` path silently dropped pins to new boards.
pub fn pin_tool(state: &mut PegboardState, board: &str, id: &str) -> PinAction {
    if !state.boards.iter().any(|b| b.key == board) {
        return PinAction::NoOp;
    }
    let entry = state.layouts.entry(board.to_string()).or_default();
    if pin_placement(entry, id).changed() {
        PinAction::Pinned
    } else {
        PinAction::NoOp
    }
}

/// Unpin a tool from the board. Returns [`PinAction::Unpinned`] on
/// success, [`PinAction::NoOp`] if the board or tool isn't present.
pub fn unpin_tool(state: &mut PegboardState, board: &str, id: &str) -> PinAction {
    let Some(entry) = state.layouts.get_mut(board) else {
        return PinAction::NoOp;
    };
    if unpin_placement(entry, id).changed() {
        PinAction::Unpinned
    } else {
        PinAction::NoOp
    }
}

/// Toggle a tool's pin state on the board: pin it if absent, unpin it
/// if present. Returns the resulting transition. Unknown tools / boards
/// produce `NoOp`. This is the single primitive the surfaces wire to
/// their `p` (Desktop + TUI) toggle key and the ToolPicker Enter.
pub fn toggle_pin(state: &mut PegboardState, board: &str, id: &str) -> PinAction {
    if toolbox_tool(id).is_none() {
        return PinAction::NoOp;
    }
    if !state.boards.iter().any(|b| b.key == board) {
        return PinAction::NoOp;
    }
    let entry = state.layouts.entry(board.to_string()).or_default();
    if let Some(pos) = entry.iter().position(|p| p.tool_id == id) {
        entry.remove(pos);
        PinAction::Unpinned
    } else if pin_placement(entry, id).changed() {
        PinAction::Pinned
    } else {
        PinAction::NoOp
    }
}

/// Move a pinned tool one slot toward `direction` within its board's
/// layout. Returns `true` if a swap happened, `false` if the move would
/// fall off either end, or if the tool isn't pinned on this board.
///
/// Desktop drag-and-drop and TUI `[` / `]` both route through this so
/// the two surfaces share one ordering invariant. Drag-and-drop's
/// "drop A onto B" gesture maps to repeated calls or to the lower-level
/// [`swap_layout_pair`].
pub fn move_layout_entry(
    state: &mut PegboardState,
    board: &str,
    id: &str,
    direction: Direction,
) -> bool {
    let Some(entry) = state.layouts.get_mut(board) else {
        return false;
    };
    move_placement_in_layout(entry, id, direction).changed()
}

/// Swap two pinned ids inside one board's layout. Drag-and-drop's
/// "drop A onto B" maps here directly. Returns `true` if both ids are
/// present and distinct; otherwise leaves state untouched.
pub fn swap_layout_pair(state: &mut PegboardState, board: &str, src: &str, tgt: &str) -> bool {
    if src == tgt {
        return false;
    }
    let Some(entry) = state.layouts.get_mut(board) else {
        return false;
    };
    swap_placement_pair_in_layout(entry, src, tgt).changed()
}

pub fn set_pin_color(
    state: &mut PegboardState,
    board: &str,
    id: &str,
    color: Option<PinColorHex>,
) -> bool {
    let Some(entry) = state.layouts.get_mut(board) else {
        return false;
    };
    set_pin_color_in_layout(entry, id, color).changed()
}

/// Set (or clear, with `None`) the per-pin size override for `id` on
/// `board`, then re-anchor the layout at the pin's current cell so a
/// grown pin pushes its neighbours forward instead of overlapping them.
///
/// Same contract the Desktop resize gesture goes through
/// (`upeg_frb::api::pegboard::set_pin_span`), so a CLI `--units` pin and
/// a dragged resize leave identical state.
pub fn set_pin_span(
    state: &mut PegboardState,
    board: &str,
    id: &str,
    span: Option<PinSpan>,
) -> bool {
    let Some(entry) = state.layouts.get_mut(board) else {
        return false;
    };
    let Some(anchor) = entry.iter_mut().find(|p| p.tool_id == id).map(|p| {
        p.span = span;
        (p.x, p.y)
    }) else {
        return false;
    };
    let (x, y) = anchor;
    place_tool_with_push(entry, id, x, y);
    true
}

pub fn set_pin_color_hex(
    state: &mut PegboardState,
    board: &str,
    id: &str,
    color: Option<&str>,
) -> Result<bool, PinColorError> {
    let color = color.map(PinColorHex::parse).transpose()?;
    Ok(set_pin_color(state, board, id, color))
}

pub fn tools_for_board_and_tag(board: Option<&str>, tag: Option<&str>) -> Vec<&'static ToolMeta> {
    tools_for_board_and_tag_in(&load_state(), board, tag)
}

/// State-borrowing sibling of [`tools_for_board_and_tag`]. Surfaces
/// holding a cached [`PegboardState`] (e.g. the TUI's in-memory copy)
/// route through here so a freshly-pinned tool is visible without
/// waiting for the disk to round-trip.
pub fn tools_for_board_and_tag_in(
    state: &PegboardState,
    board: Option<&str>,
    tag: Option<&str>,
) -> Vec<&'static ToolMeta> {
    placements_for_board_and_tag_in(state, board, tag)
        .into_iter()
        .map(|(_, tool)| tool)
        .collect()
}

/// Variant of [`tools_for_board_and_tag`] that returns each tool paired
/// with its stored `(x, y)` placement. Order matches `tools_for_board_and_tag`
/// (sorted by `(y, x)`), so the two functions stay in lockstep.
/// Surfaces that honor exact coordinates (Desktop, and TUI when it can
/// fit `BOARD_COLS`) read positions from here.
pub fn placements_for_board_and_tag(
    board: Option<&str>,
    tag: Option<&str>,
) -> Vec<(Placement, &'static ToolMeta)> {
    placements_for_board_and_tag_in(&load_state(), board, tag)
}

/// Pure variant for unit tests: takes the state directly instead of
/// reading from disk, so collisions between parallel tests over
/// `UPEG_HOME` don't matter.
pub fn placements_for_board_and_tag_in(
    state: &PegboardState,
    board: Option<&str>,
    tag: Option<&str>,
) -> Vec<(Placement, &'static ToolMeta)> {
    let board_key = board.map(str::trim).filter(|b| !b.is_empty());
    let mut placements: Vec<Placement> = if let Some(key) = board_key {
        state.layouts.get(key).cloned().unwrap_or_default()
    } else {
        // Union across boards, dedup by tool_id, preserve placement of
        // first occurrence so a tool's `(x, y)` is stable.
        let mut seen = BTreeSet::new();
        let mut out: Vec<Placement> = Vec::new();
        for board_data in &state.boards {
            let Some(layout) = state.layouts.get(&board_data.key) else {
                continue;
            };
            for placement in layout {
                if seen.insert(placement.tool_id.clone()) {
                    out.push(placement.clone());
                }
            }
        }
        out
    };
    placements.sort_by_key(|p| (p.y, p.x));
    let tag = tag
        .map(str::trim)
        .filter(|tag| !tag.is_empty() && *tag != ALL_TAG);
    placements
        .into_iter()
        .filter_map(|placement| {
            let tool = toolbox_tool(&placement.tool_id)?;
            if tag.is_some_and(|t| !tool.has_tag(t)) {
                return None;
            }
            Some((placement, tool))
        })
        .collect()
}

/// Board-and-tag placements visible on `surface`: the on-surface subset
/// of [`placements_for_board_and_tag_in`] for a single board.
///
/// This is the single board-enumeration entry point. Before this
/// function existed, CLI (`upeg board <b> list`), HTTP (`/v1/boards*`),
/// and MCP (board-scoped `tools/list`) each re-implemented "this
/// board's pins, filtered to my surface" independently, and the CLI
/// copy silently dropped the `is_on_surface` filter — `upeg board dev
/// list --json` listed desktop-only pins (e.g. `memo.scratch`) that
/// `upeg board dev call memo.scratch` could never dispatch. Every
/// surface now calls this (or its single-tool sibling
/// [`board_placement_on_surface_in`]) so the *set of listed ids* can't
/// diverge by surface again — only the JSON shape each surface wraps
/// it in does.
pub fn board_entries_on_surface_in(
    state: &PegboardState,
    board: &str,
    tag: Option<&str>,
    surface: Surface,
) -> Vec<(Placement, &'static ToolMeta)> {
    placements_for_board_and_tag_in(state, Some(board), tag)
        .into_iter()
        .filter(|(_, tool)| tool.is_on_surface(surface))
        .collect()
}

/// Whether the user's state declares a board with this key.
pub fn board_exists_in(state: &PegboardState, board: &str) -> bool {
    state.boards.iter().any(|b| b.key == board)
}

/// The user's stored placement for `tool_id` on `board`, if the tool is
/// pinned there — surface-blind.
///
/// Board-scoped call GATES must not read this directly: CLI (`upeg board
/// <b> call`), HTTP (`/v1/boards/{b}/tools/{id}`) and MCP (`--board`) all
/// route through [`board_placement_on_surface_in`], which additionally
/// requires the tool to be registered on the calling surface — so a pin
/// the board's listing hides can never still be dispatchable through it.
///
/// What remains here is the surface-independent question — "did the user
/// pin this, and with what preset?" — which is what the global
/// `upeg --board <b> call` preset resolution asks (that flag carries
/// board context onto an otherwise ordinary call; it is not a pin gate).
pub fn placement_in<'a>(
    state: &'a PegboardState,
    board: &str,
    tool_id: &str,
) -> Option<&'a Placement> {
    state
        .layouts
        .get(board)?
        .iter()
        .find(|placement| placement.tool_id == tool_id)
}

/// Single-tool sibling of [`board_entries_on_surface_in`]: `Some` iff
/// `tool_id` is pinned on `board` in `state` AND its tool is registered
/// on `surface`. Board-scoped call gates (HTTP
/// `/v1/boards/{b}/tools/{id}`, MCP board-scoped `tools/call`) route
/// through this so a tool that's excluded from the board's listing can
/// never still be dispatchable through it (or vice versa).
pub fn board_placement_on_surface_in<'a>(
    state: &'a PegboardState,
    board: &str,
    tool_id: &str,
    surface: Surface,
) -> Option<&'a Placement> {
    let placement = placement_in(state, board, tool_id)?;
    toolbox_tool(tool_id)
        .filter(|tool| tool.is_on_surface(surface))
        .map(|_| placement)
}

pub fn tag_options_for_board(board: Option<&str>) -> Vec<String> {
    tag_options_for_board_in(&load_state(), board)
}

/// State-borrowing sibling of [`tag_options_for_board`].
pub fn tag_options_for_board_in(state: &PegboardState, board: Option<&str>) -> Vec<String> {
    let mut tags: Vec<String> = placements_for_board_and_tag_in(state, board, None)
        .into_iter()
        .map(|(_, tool)| tool)
        .flat_map(ToolMetaRuntimeExt::tag_labels)
        .collect();
    tags.sort();
    tags.dedup();
    let mut out = Vec::with_capacity(tags.len() + 1);
    out.push(ALL_TAG.into());
    out.extend(tags);
    out
}

/// Keys of all boards in the given state, in declared order. Companion
/// to [`board_keys`] for callers with a cached state in hand.
#[must_use]
pub fn board_keys_in(state: &PegboardState) -> Vec<String> {
    state.boards.iter().map(|b| b.key.clone()).collect()
}

// ─── Sanitization ──────────────────────────────────────────────

fn sanitize_state(mut state: PegboardState) -> PegboardState {
    state.boards = sanitize_boards(state.boards);
    let board_keys: BTreeSet<String> = state.boards.iter().map(|board| board.key.clone()).collect();
    let mut layouts = BTreeMap::new();
    for board in &state.boards {
        let raw = state.layouts.remove(&board.key).unwrap_or_else(|| {
            default_layouts(std::slice::from_ref(board))
                .remove(&board.key)
                .unwrap_or_default()
        });
        layouts.insert(board.key.clone(), reconcile_placements(raw));
    }
    state.layouts = layouts
        .into_iter()
        .filter(|(board, _)| board_keys.contains(board))
        .collect();
    state.selection = sanitize_selection(state.selection, &state.boards, &state.layouts);
    state
}

fn sanitize_selection(
    selection: PegboardSelection,
    boards: &[BoardData],
    layouts: &BTreeMap<String, Vec<Placement>>,
) -> PegboardSelection {
    let board_key = selection.board_key.and_then(|key| {
        let trimmed = key.trim();
        if boards.iter().any(|board| board.key == trimmed) {
            Some(trimmed.to_string())
        } else {
            None
        }
    });
    let tag = normalize_tag(&selection.tag);
    let options = tag_options_for_board_in(
        &PegboardState {
            boards: boards.to_vec(),
            layouts: layouts.clone(),
            selection: PegboardSelection::default(),
        },
        board_key.as_deref(),
    );
    let tag = if options.iter().any(|option| option == &tag) {
        tag
    } else {
        ALL_TAG.to_string()
    };
    PegboardSelection { board_key, tag }
}

fn sanitize_boards(boards: Vec<BoardData>) -> Vec<BoardData> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for board in boards {
        let key = board.key.trim();
        let title = board.title.trim();
        if key.is_empty() || title.is_empty() || !seen.insert(key.to_string()) {
            continue;
        }
        out.push(BoardData {
            key: key.to_string(),
            title: title.to_string(),
            guidance: board.guidance,
        });
    }
    if out.is_empty() {
        default_boards()
    } else {
        out
    }
}

fn normalize_tag(tag: &str) -> String {
    let tag = tag.trim();
    if tag.is_empty() {
        ALL_TAG.into()
    } else {
        tag.to_string()
    }
}

/// Remove a tool from every board (e.g., after uninstall).
pub fn remove_tool_everywhere(state: &mut PegboardState, tool_id: &str) -> bool {
    let mut changed = false;
    for placements in state.layouts.values_mut() {
        if unpin_placement(placements, tool_id).changed() {
            changed = true;
        }
    }
    changed
}

/// Try to move `tool_id` on `board` to `(x, y)`. Returns `true` if the
/// move was applied. Colliding tools are pushed forward in row-major order
/// so the final layout still fits within `BOARD_COLS` and has no overlaps.
pub fn move_placement(
    state: &mut PegboardState,
    board_key: &str,
    tool_id: &str,
    x: u16,
    y: u16,
) -> bool {
    let Some(placements) = state.layouts.get_mut(board_key) else {
        return false;
    };
    place_tool_with_push(placements, tool_id, x, y).changed()
}

#[cfg(test)]
mod guidance_tests;
#[cfg(test)]
mod pegboard_tests;
#[cfg(test)]
mod project_board_tests;
