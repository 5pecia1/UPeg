//! Pegboard board + layout snapshots exposed to the Flutter UI.
//!
//! The API exposes both the read side of
//! `upeg-pegboard-ui::features::boards` / `::features::layouts` and the
//! Flutter-owned mutation seams: tag filtering, pin lifecycle, board CRUD,
//! drag/drop commits, push previews, and empty-board suggestions.
//!
//! All DTOs are `#[frb(non_opaque)]` with owned `String`/numeric fields so
//! Dart sees plain classes — no opaque handles.

use std::collections::BTreeSet;

use upeg_core::{
    ArgsPreset, ArgsPresetError, ColSpan, PinColorError, PinColorHex, PinSpan, PinSpanError,
    Placement, RowSpan, ToolMeta,
};
use upeg_pegboard_ui::features::boards::{
    Board, add_board as add_board_inner, default_boards, load_boards, remove_board,
    rename_board as rename_board_inner, save_boards,
};
use upeg_pegboard_ui::features::layouts::{
    ALL_TAG, BoardLayouts, count_for_tag as count_for_tag_inner, default_layouts, load_layouts,
    save_layouts, tag_options as tag_options_inner, tool_is_pinned_in_layout,
};
use upeg_pegboard_ui::platform::storage::{self, PegboardSelectionValue};
use upeg_runtime::pegboard::{
    Direction as PlacementDirection, effective_size, move_placement, pin_placement, placement_size,
    set_pin_color as set_pin_color_in_layout, unpin_placement,
};
use upeg_runtime::{ToolMetaRuntimeExt, toolbox_tool, toolbox_tools};

use super::boot::FrbError;
use super::keyboard::OrderDirectionDto;
use super::tools::ToolDto;

/// Cap on returned empty-board suggestions so the UI never paints more
/// than three onboarding tiles.
const EMPTY_BOARD_SUGGESTION_LIMIT: usize = 3;

/// Dart-mirrored view of [`upeg_pegboard_ui::features::boards::Board`].
///
/// `Board` carries `&'static str` fields backed by `Box::leak`ed strings;
/// crossing the FFI boundary requires owned values.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BoardDto {
    pub key: String,
    pub title: String,
}

impl From<&Board> for BoardDto {
    fn from(b: &Board) -> Self {
        Self {
            key: b.key.to_string(),
            title: b.title.to_string(),
        }
    }
}

/// Single pin placement on a board.
///
/// Mirrors [`upeg_core::Placement`] (with the effective size widened to
/// `(w, h)` for Dart layout convenience). `x`/`y` carry the raw
/// pegboard coordinates the Dart canvas uses for absolute-position layout
/// (drag & drop).
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PlacementDto {
    pub tool_id: String,
    /// Pegboard column.
    pub x: u32,
    /// Pegboard row.
    pub y: u32,
    /// Effective width in pegboard cells: the user span override when
    /// set, else the manifest footprint (U1=1, U2=2, U2T=1).
    pub w: u32,
    /// Effective height in pegboard cells: the user span override when
    /// set, else the manifest footprint (U1=1, U2=1, U2T=2).
    pub h: u32,
    /// User-specified border color for the pin (`#RRGGBB`), or `None` to use defaults.
    pub color: Option<String>,
    /// User span override width in cells; `None` when the manifest
    /// footprint applies. Set together with `span_rows`.
    pub span_cols: Option<u32>,
    /// User span override height in cells; `None` when the manifest
    /// footprint applies. Set together with `span_cols`.
    pub span_rows: Option<u32>,
    /// Canonical JSON object text of the pin's saved argument preset,
    /// or `None` when the pin has no preset.
    pub args_preset_json: Option<String>,
}

/// Build a `PlacementDto` from a stored [`Placement`], deriving the
/// `(w, h)` span through [`upeg_runtime::pegboard::effective_size`]: the
/// user span override wins, else the tool manifest's `pegboard_units`.
/// Unknown tool ids fall back to `(1, 1)`.
///
/// Free function (not an `impl PlacementDto` method) because
/// flutter_rust_bridge would otherwise pick the `&Placement` borrow up
/// as an opaque type. Lives behind a non-`pub` doorway so codegen also
/// skips re-exporting it.
fn placement_dto_from(p: &Placement) -> PlacementDto {
    let (w, h) = effective_size(p);
    let (span_cols, span_rows) = p.span.map_or((None, None), |span| {
        (
            Some(u32::from(span.cols.get())),
            Some(u32::from(span.rows.get())),
        )
    });
    PlacementDto {
        tool_id: p.tool_id.clone(),
        x: u32::from(p.x),
        y: u32::from(p.y),
        w: u32::from(w),
        h: u32::from(h),
        color: p.color.as_ref().map(|c| c.as_str().to_string()),
        span_cols,
        span_rows,
        args_preset_json: p
            .args_preset
            .as_ref()
            .map(|preset| preset.as_str().to_string()),
    }
}

/// Snapshot of one board's placements, returned by [`load_layout_snapshot`].
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct LayoutSnapshotDto {
    pub board_key: String,
    /// Fixed canonical pegboard column count from `upeg_core::BOARD_COLS`.
    /// Flutter uses this six-column width for grid/drop targets; rows grow
    /// downward instead of expanding the board horizontally.
    pub board_cols: u32,
    pub placements: Vec<PlacementDto>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct PegboardSelectionDto {
    pub board_key: Option<String>,
    pub tag: String,
}

impl From<PegboardSelectionValue> for PegboardSelectionDto {
    fn from(selection: PegboardSelectionValue) -> Self {
        Self {
            board_key: selection.board_key,
            tag: selection.tag,
        }
    }
}

impl From<PegboardSelectionDto> for PegboardSelectionValue {
    fn from(dto: PegboardSelectionDto) -> Self {
        Self {
            board_key: dto.board_key,
            tag: dto.tag,
        }
    }
}

/// Load the persisted boards list and its layouts map together, each
/// falling back to defaults when no saved state exists.
///
/// The layouts default is derived from the boards (`default_layouts(&boards)`),
/// so the two reads share one ordering invariant: boards first, then layouts
/// keyed off them. Every read/mutate entry point that needs both pins goes
/// through here instead of re-spelling the `unwrap_or_else` pair.
fn load_boards_and_layouts() -> (Vec<Board>, BoardLayouts) {
    let boards = load_boards().unwrap_or_else(default_boards);
    let layouts = load_layouts(&boards).unwrap_or_else(|| default_layouts(&boards));
    (boards, layouts)
}

/// Best-effort board list: persisted layout if present, else defaults.
///
/// Mirrors the boot-time `load_boards().unwrap_or_else(default_boards)`
/// pattern from `upeg-pegboard-ui`.
#[flutter_rust_bridge::frb(sync)]
pub fn list_boards() -> Vec<BoardDto> {
    let boards = load_boards().unwrap_or_else(default_boards);
    boards.iter().map(BoardDto::from).collect()
}

/// Fetch the persisted placement list for `board_key`, or the default
/// layout for that board if no saved state exists.
///
/// Unknown `board_key` returns an empty placement list (Dart side decides
/// whether to render a "no such board" view or just an empty canvas).
#[flutter_rust_bridge::frb(sync)]
pub fn load_layout_snapshot(board_key: String) -> LayoutSnapshotDto {
    load_layout_snapshot_for_filter(board_key, None)
}

/// Fetch the placement list for `board_key`, optionally narrowed by an
/// effective runtime tag.
///
/// Flutter routes board rendering through this API so Rust remains the single
/// source of truth for tag matching. `None` and `"all"` both mean no tag
/// filter.
#[flutter_rust_bridge::frb(sync)]
pub fn load_layout_snapshot_for_filter(
    board_key: String,
    tag: Option<String>,
) -> LayoutSnapshotDto {
    let tag = tag
        .as_deref()
        .map(str::trim)
        .filter(|tag| !tag.is_empty() && *tag != ALL_TAG);
    let (_boards, layouts) = load_boards_and_layouts();
    let mut raw_placements = layouts.get(board_key.as_str()).cloned().unwrap_or_default();
    raw_placements.sort_by_key(|placement| (placement.y, placement.x));
    let placements = raw_placements
        .iter()
        .filter(|placement| placement_matches_tag(placement, tag))
        .map(placement_dto_from)
        .collect();
    LayoutSnapshotDto {
        board_key,
        board_cols: u32::from(upeg_core::BOARD_COLS),
        placements,
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn load_pegboard_selection() -> PegboardSelectionDto {
    storage::load_pegboard_selection().into()
}

#[flutter_rust_bridge::frb(sync)]
pub fn save_pegboard_selection(selection: PegboardSelectionDto) -> Result<(), FrbError> {
    let selection = PegboardSelectionValue::from(selection);
    storage::save_pegboard_selection(&selection).map_err(|()| FrbError::Io {
        message: "save pegboard selection".to_string(),
    })
}

fn placement_matches_tag(placement: &Placement, tag: Option<&str>) -> bool {
    toolbox_tool(&placement.tool_id).is_some_and(|tool| tag.is_none_or(|tag| tool.has_tag(tag)))
}

// ─── Tag filter (read-only) ────────────────────────────────────────────

/// Returns the available tag set for the palette / canvas tag chip row.
///
/// The list begins with the `"all"` sentinel followed by every distinct
/// tag carried by any registered tool, sorted lexically. Delegates to
/// `upeg_pegboard_ui::features::layouts::tag_options`, the shared
/// pure-Rust source of truth.
#[flutter_rust_bridge::frb(sync)]
pub fn tag_options() -> Vec<String> {
    let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
    tag_options_inner(&tools)
}

/// Count of tools that carry `tag`. `tag = "all"` returns the total
/// number of registered tools. Mirrors
/// [`upeg_pegboard_ui::features::layouts::count_for_tag`] so the
/// Dart tag chip row can render `"<tag> <n>"` without re-iterating
/// over the toolbox on every rebuild. Returned as `u32` because the
/// FRB wire vocabulary (Dart `int`) is happiest with concrete
/// fixed-size integers and the toolbox count never approaches `u32::MAX`.
#[flutter_rust_bridge::frb(sync)]
pub fn count_for_tag(tag: String) -> u32 {
    let tools: Vec<&'static ToolMeta> = toolbox_tools().collect();
    let n = count_for_tag_inner(&tools, &tag);
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Returns the available tag set for the pins placed on `board_key`.
///
/// `None` means the union of pinned tools across all boards. The list begins
/// with the `"all"` sentinel and then includes tags from the pinned tools only.
#[flutter_rust_bridge::frb(sync)]
pub fn tag_options_for_board(board_key: Option<String>) -> Vec<String> {
    let (boards, layouts) = load_boards_and_layouts();
    let tools = pinned_tools_for_board_in(&boards, &layouts, board_key.as_deref());
    tag_options_inner(&tools)
}

/// Count pins on `board_key` whose tool carries `tag`.
///
/// `tag = "all"` counts every pinned tool for that board. `None` board means
/// the union of pinned tools across all boards.
#[flutter_rust_bridge::frb(sync)]
pub fn count_pinned_for_tag(board_key: Option<String>, tag: String) -> u32 {
    let (boards, layouts) = load_boards_and_layouts();
    let tools = pinned_tools_for_board_in(&boards, &layouts, board_key.as_deref());
    let n = count_for_tag_inner(&tools, &tag);
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn pinned_tools_for_board_in(
    boards: &[Board],
    layouts: &BoardLayouts,
    board_key: Option<&str>,
) -> Vec<&'static ToolMeta> {
    placements_for_board_and_tag_in_layouts(boards, layouts, board_key, None)
        .into_iter()
        .filter_map(|placement| toolbox_tool(&placement.tool_id))
        .collect()
}

fn placements_for_board_and_tag_in_layouts(
    boards: &[Board],
    layouts: &BoardLayouts,
    board_key: Option<&str>,
    tag: Option<&str>,
) -> Vec<Placement> {
    let board_key = board_key.map(str::trim).filter(|key| !key.is_empty());
    let mut placements = if let Some(key) = board_key {
        layouts.get(key).cloned().unwrap_or_default()
    } else {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for board in boards {
            let Some(layout) = layouts.get(board.key) else {
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
    placements.sort_by_key(|placement| (placement.y, placement.x));
    placements
        .into_iter()
        .filter(|placement| {
            let Some(tool) = toolbox_tool(&placement.tool_id) else {
                return false;
            };
            tag.is_none_or(|needle| tool.has_tag(needle))
        })
        .collect()
}

/// Returns the tools that carry the requested `tag`, sorted by id.
///
/// `tag = "all"` matches every tool. Unknown tags return an empty list.
#[flutter_rust_bridge::frb(sync)]
pub fn tools_for_tag(tag: String) -> Vec<ToolDto> {
    let needle = tag.trim().to_string();
    let mut out: Vec<ToolDto> = toolbox_tools()
        .filter(|meta| needle == ALL_TAG || meta.has_tag(&needle))
        .map(ToolDto::from)
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

// ─── Pin lifecycle (mutating) ──────────────────────────────────────────

/// `true` iff `tool_id` already has a placement on `board_key`.
#[flutter_rust_bridge::frb(sync)]
pub fn is_tool_pinned(board_key: String, tool_id: String) -> bool {
    let (_boards, layouts) = load_boards_and_layouts();
    tool_is_pinned_in_layout(&layouts, &board_key, &tool_id)
}

/// List of board keys whose persisted layout already contains `tool_id`.
///
/// Drives the palette's per-board pin chip cluster — each chip's
/// filled/outlined state derives from whether the board key appears in
/// this list. The set is recomputed from the persisted layouts on every
/// call so the Flutter side never holds stale snapshots.
#[flutter_rust_bridge::frb(sync)]
pub fn pinned_boards_for_tool(tool_id: String) -> Vec<String> {
    let (boards, layouts) = load_boards_and_layouts();
    boards
        .iter()
        .filter(|board| {
            layouts
                .get(board.key)
                .is_some_and(|v| v.iter().any(|p| p.tool_id == tool_id))
        })
        .map(|board| board.key.to_string())
        .collect()
}

/// Append a placement for `tool_id` at the next free slot on `board_key`,
/// then persist the layouts map. No-op (still `Ok`) if the tool is already
/// pinned. Returns [`FrbError::Validation`] when the board is unknown.
#[flutter_rust_bridge::frb(sync)]
pub fn pin_tool(board_key: String, tool_id: String) -> Result<(), FrbError> {
    mutate_layout(&board_key, |layout| {
        pin_placement(layout, &tool_id);
    })
}

/// Remove every placement carrying `tool_id` from `board_key` and
/// persist. No-op (still `Ok`) if the tool wasn't pinned. Returns
/// [`FrbError::Validation`] when the board is unknown.
#[flutter_rust_bridge::frb(sync)]
pub fn unpin_tool(board_key: String, tool_id: String) -> Result<(), FrbError> {
    mutate_layout(&board_key, |layout| {
        unpin_placement(layout, &tool_id);
    })
}

/// Swap the selected pin with its row-major neighbour in one persisted layout
/// mutation. This uses the runtime placement swap path rather than composing
/// two push-placement moves, so unrelated pins keep their relative slots.
#[flutter_rust_bridge::frb(sync)]
pub fn reorder_pin(
    board_key: String,
    tool_id: String,
    direction: OrderDirectionDto,
) -> Result<(), FrbError> {
    if toolbox_tool(&tool_id).is_none() {
        return Err(FrbError::Validation {
            field: "tool_id".to_string(),
            reason: format!("tool `{tool_id}` is not registered"),
        });
    }
    mutate_layout(&board_key, |layout| {
        let _ = move_placement(layout, &tool_id, placement_direction(direction));
    })
}

const fn placement_direction(direction: OrderDirectionDto) -> PlacementDirection {
    match direction {
        OrderDirectionDto::Previous => PlacementDirection::Prev,
        OrderDirectionDto::Next => PlacementDirection::Next,
    }
}

fn pin_color_error_to_frb(err: PinColorError) -> FrbError {
    FrbError::Validation {
        field: "color".to_string(),
        reason: err.to_string(),
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn set_pin_color(
    board_key: String,
    tool_id: String,
    color: Option<String>,
) -> Result<(), FrbError> {
    let color = color
        .as_deref()
        .map(PinColorHex::parse)
        .transpose()
        .map_err(pin_color_error_to_frb)?;
    mutate_layout(&board_key, |layout| {
        let _ = set_pin_color_in_layout(layout, &tool_id, color.clone());
    })
}

fn pin_span_error_to_frb(field: &str, err: PinSpanError) -> FrbError {
    FrbError::Validation {
        field: field.to_string(),
        reason: err.to_string(),
    }
}

fn args_preset_error_to_frb(err: ArgsPresetError) -> FrbError {
    FrbError::Validation {
        field: "args_preset".to_string(),
        reason: err.to_string(),
    }
}

/// Set the user span override for `tool_id` on `board_key` and persist.
///
/// `cols` / `rows` are validated through the [`ColSpan`] / [`RowSpan`]
/// newtypes (1..=`BOARD_COLS` columns, ≥1 rows) before any state is
/// touched; failures surface as [`FrbError::Validation`], mirroring
/// [`set_pin_color`]. A tool that isn't pinned on the board is a silent
/// no-op, same as the color path.
///
/// After the span is applied the layout is re-anchored through
/// [`upeg_runtime::pegboard::place_tool_with_push`] at the pin's current
/// cell: growing a pin over a neighbour pushes that neighbour forward in
/// row-major order (exactly what [`preview_resize`] projected) instead
/// of persisting an overlapping layout.
#[flutter_rust_bridge::frb(sync)]
pub fn set_pin_span(
    board_key: String,
    tool_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), FrbError> {
    let span = validated_pin_span(cols, rows)?;
    mutate_layout(&board_key, |layout| {
        let anchor = layout.iter_mut().find(|p| p.tool_id == tool_id).map(|p| {
            p.span = Some(span);
            (p.x, p.y)
        });
        if let Some((x, y)) = anchor {
            let _ = upeg_runtime::pegboard::place_tool_with_push(layout, &tool_id, x, y);
        }
    })
}

/// Validate raw `(cols, rows)` from Dart into a [`PinSpan`], mapping
/// range violations to [`FrbError::Validation`]. Shared by
/// [`set_pin_span`] and [`preview_resize`]'s stricter callers.
fn validated_pin_span(cols: u32, rows: u32) -> Result<PinSpan, FrbError> {
    let cols = u16::try_from(cols)
        .map_err(|_| FrbError::Validation {
            field: "cols".to_string(),
            reason: format!("cols `{cols}` exceeds u16 range"),
        })
        .and_then(|cols| ColSpan::new(cols).map_err(|err| pin_span_error_to_frb("cols", err)))?;
    let rows = u16::try_from(rows)
        .map_err(|_| FrbError::Validation {
            field: "rows".to_string(),
            reason: format!("rows `{rows}` exceeds u16 range"),
        })
        .and_then(|rows| RowSpan::new(rows).map_err(|err| pin_span_error_to_frb("rows", err)))?;
    Ok(PinSpan::new(cols, rows))
}

/// Drop the user span override for `tool_id` on `board_key` so the
/// manifest footprint applies again.
#[flutter_rust_bridge::frb(sync)]
pub fn clear_pin_span(board_key: String, tool_id: String) -> Result<(), FrbError> {
    mutate_layout(&board_key, |layout| {
        if let Some(placement) = layout.iter_mut().find(|p| p.tool_id == tool_id) {
            placement.span = None;
        }
    })
}

/// Save an argument preset on `tool_id`'s placement on `board_key`.
///
/// `preset_json` must be a JSON object without the reserved
/// execution-context key ([`upeg_core::ArgsPreset`]'s contract);
/// violations surface as [`FrbError::Validation`] before any state is
/// touched.
#[flutter_rust_bridge::frb(sync)]
pub fn set_pin_args_preset(
    board_key: String,
    tool_id: String,
    preset_json: String,
) -> Result<(), FrbError> {
    let preset = ArgsPreset::parse(&preset_json).map_err(args_preset_error_to_frb)?;
    mutate_layout(&board_key, |layout| {
        if let Some(placement) = layout.iter_mut().find(|p| p.tool_id == tool_id) {
            placement.args_preset = Some(preset.clone());
        }
    })
}

/// Remove the saved argument preset from `tool_id`'s placement on
/// `board_key`.
#[flutter_rust_bridge::frb(sync)]
pub fn clear_pin_args_preset(board_key: String, tool_id: String) -> Result<(), FrbError> {
    mutate_layout(&board_key, |layout| {
        if let Some(placement) = layout.iter_mut().find(|p| p.tool_id == tool_id) {
            placement.args_preset = None;
        }
    })
}

/// Helper: load the persisted boards + layouts, mutate the placement
/// vector for one board, then `save_layouts` the result. Validation
/// errors short-circuit before any persistence happens.
fn mutate_layout<F>(board_key: &str, mut mutate: F) -> Result<(), FrbError>
where
    F: FnMut(&mut Vec<Placement>),
{
    let (boards, mut layouts) = load_boards_and_layouts();
    let static_key: &'static str = boards
        .iter()
        .find(|b| b.key == board_key)
        .map(|b| b.key)
        .ok_or_else(|| FrbError::Validation {
            field: "board_key".to_string(),
            reason: format!("unknown board `{board_key}`"),
        })?;
    let layout = layouts.entry(static_key).or_default();
    mutate(layout);
    save_layouts(&layouts);
    Ok(())
}

// ─── Board CRUD ────────────────────────────────────────────────────────

/// Create a new board with the given user-supplied `title`. Returns the
/// generated stable `key` (slugified, deduped) so Dart can immediately
/// switch the current-board selection without re-listing.
///
/// Returns [`FrbError::Validation`] when the title is blank or all
/// whitespace, mirroring `add_board`'s `None` contract.
#[flutter_rust_bridge::frb(sync)]
pub fn create_board(title: String) -> Result<String, FrbError> {
    let mut boards = load_boards().unwrap_or_else(default_boards);
    let key = add_board_inner(&mut boards, &title).ok_or_else(|| FrbError::Validation {
        field: "title".to_string(),
        reason: "board title must be non-empty".to_string(),
    })?;
    save_boards(&boards);
    Ok(key.to_string())
}

/// Rename `board_key` to `new_title`. Returns [`FrbError::Validation`]
/// when the title is blank or the board doesn't exist.
#[flutter_rust_bridge::frb(sync)]
pub fn rename_board(board_key: String, new_title: String) -> Result<(), FrbError> {
    let mut boards = load_boards().unwrap_or_else(default_boards);
    if !rename_board_inner(&mut boards, &board_key, &new_title) {
        return Err(FrbError::Validation {
            field: "new_title".to_string(),
            reason: "board not found or title was blank".to_string(),
        });
    }
    save_boards(&boards);
    Ok(())
}

/// Whether the Project Manifest this process detected declares
/// `board_key`.
///
/// The store-side counterpart is
/// `upeg_sources::pegboard::BoardVisibility::is_project_board`; both
/// read the one process-global registry the loader fills
/// ([`upeg_runtime::pegboard_project`]). This one is spelled on the
/// runtime primitive because `upeg-sources` is a native-only dependency
/// here, and the PWA must apply the same rule (with no manifest in
/// play, the answer there is always `false`).
pub(crate) fn is_project_declared_board(board_key: &str) -> bool {
    upeg_runtime::pegboard_project::project_board_scope()
        .is_some_and(|scope| scope.declaration(board_key).is_some())
}

/// Delete `board_key` from the persisted boards list and drop its
/// layout entry. Returns [`FrbError::Validation`] when the board
/// doesn't exist, or when the active Project Manifest declares it.
///
/// The project-board refusal is the same rule
/// `upeg_sources::pegboard::remove_board` applies to the TUI: the
/// manifest is a project board's source of truth, so the next load
/// merges it straight back — but the delete would already have swept
/// the board's row, tombstoning every pin on it. Refusing is the honest
/// answer; the delete cannot succeed, only destroy.
#[flutter_rust_bridge::frb(sync)]
pub fn delete_board(board_key: String) -> Result<(), FrbError> {
    if is_project_declared_board(&board_key) {
        return Err(FrbError::Validation {
            field: "board_key".to_string(),
            reason: format!(
                "`{board_key}` is declared by the project manifest; \
                 remove it from `upeg.toml` instead"
            ),
        });
    }
    let mut boards = load_boards().unwrap_or_else(default_boards);
    if !remove_board(&mut boards, &board_key) {
        return Err(FrbError::Validation {
            field: "board_key".to_string(),
            reason: format!("unknown board `{board_key}`"),
        });
    }
    save_boards(&boards);
    // Drop the orphan layout entry to keep storage tidy.
    let mut layouts: BoardLayouts =
        load_layouts(&boards).unwrap_or_else(|| default_layouts(&boards));
    layouts.retain(|k, _| *k != board_key);
    save_layouts(&layouts);
    Ok(())
}

// ─── Drag-and-drop / move ──────────────────────────────────────────────

/// Commit a pin drag-drop on `board_key`: anchor the dragged tool at
/// `(anchor_x, anchor_y)`, clamp the horizontal anchor inside the fixed
/// six-column grid, and push colliding tools forward in row-major order until
/// the layout is non-overlapping again. Rows grow downward; the board never
/// expands horizontally beyond [`upeg_core::BOARD_COLS`].
///
/// The Dart side computes the drop anchor cell from
/// `(localPosition / cellSize)`, clamping it to the widget's own span,
/// and hands it to this function as `(anchor_x, anchor_y)`; the
/// row-major push/reflow step then runs through
/// `upeg_runtime::pegboard::place_tool_with_push` so it stays canonical
/// across surfaces.
///
/// Returns [`FrbError::Validation`] when the board is unknown, when the anchor
/// is outside the `u16` grid range, when the tool is not registered, or when
/// the manifest span cannot fit within the fixed board width. Positive
/// horizontal anchors beyond [`upeg_core::BOARD_COLS`] are clamped to the last
/// valid start column before the runtime reflow runs.
#[flutter_rust_bridge::frb(sync)]
pub fn move_pin(
    board_key: String,
    tool_id: String,
    anchor_x: u32,
    anchor_y: u32,
) -> Result<(), FrbError> {
    let x: u16 = u16::try_from(anchor_x).map_err(|_| FrbError::Validation {
        field: "anchor_x".to_string(),
        reason: format!("anchor_x `{anchor_x}` exceeds u16 range"),
    })?;
    let y: u16 = u16::try_from(anchor_y).map_err(|_| FrbError::Validation {
        field: "anchor_y".to_string(),
        reason: format!("anchor_y `{anchor_y}` exceeds u16 range"),
    })?;
    if toolbox_tool(&tool_id).is_none() {
        return Err(FrbError::Validation {
            field: "tool_id".to_string(),
            reason: format!("tool `{tool_id}` is not registered"),
        });
    }
    let (_boards, layouts) = load_boards_and_layouts();
    let span_w = effective_span_width(&layouts, &board_key, &tool_id);
    let board_cols = upeg_runtime::pegboard::fixed_board_cols();
    if span_w == 0 || u32::from(span_w) > board_cols {
        return Err(FrbError::Validation {
            field: "tool_id".to_string(),
            reason: format!(
                "tool `{tool_id}` span width {span_w} does not fit fixed board width {board_cols}"
            ),
        });
    }
    let max_start_x = board_cols - u32::from(span_w);
    let max_start_x = u16::try_from(max_start_x).unwrap_or(u16::MAX);
    let x = x.min(max_start_x);
    mutate_layout(&board_key, |layout| {
        let _ = upeg_runtime::pegboard::place_tool_with_push(layout, &tool_id, x, y);
    })
}

/// Effective span width used for horizontal clamping: the pinned
/// placement's user span override when present, else the manifest
/// footprint ([`placement_size`]) for tools not (yet) on the board.
fn effective_span_width(layouts: &BoardLayouts, board_key: &str, tool_id: &str) -> u16 {
    layouts
        .get(board_key)
        .and_then(|layout| layout.iter().find(|p| p.tool_id == tool_id))
        .map_or_else(|| placement_size(tool_id).0, |p| effective_size(p).0)
}

// ─── Push-placement preview (F16) ──────────────────────────────────────

/// Preview the layout that WOULD result if `tool_id` were dropped at
/// `(anchor_x, anchor_y)` on `board_key`, WITHOUT mutating the
/// persisted state. Mirrors [`move_pin`]'s commit semantics through
/// [`upeg_runtime::pegboard::place_tool_with_push`] but runs against a CLONE
/// of the current layout so the read is side-effect-free. Horizontal anchors
/// are clamped/reflowed inside the fixed six columns; rows grow downward.
///
/// Returns an empty `Vec` when the board is unknown, the tool is not
/// pinned on the board, the anchor is outside the `u16` grid range, or
/// the tool is not registered or cannot fit within the fixed board width. The
/// empty result lets the Dart canvas hide the preview overlay without needing
/// a separate "should display" signal.
///
/// Drives F16: the Dart BoardCanvas overlays the returned placements
/// in a ghost layer while move-mode is active, so the user sees the
/// projected push before committing.
#[flutter_rust_bridge::frb(sync)]
pub fn preview_push(
    board_key: String,
    tool_id: String,
    anchor_x: u32,
    anchor_y: u32,
) -> Vec<PlacementDto> {
    let Ok(x) = u16::try_from(anchor_x) else {
        return Vec::new();
    };
    let Ok(y) = u16::try_from(anchor_y) else {
        return Vec::new();
    };
    if toolbox_tool(&tool_id).is_none() {
        return Vec::new();
    }
    let (_boards, layouts) = load_boards_and_layouts();
    let span_w = effective_span_width(&layouts, &board_key, &tool_id);
    let board_cols = upeg_runtime::pegboard::fixed_board_cols();
    if span_w == 0 || u32::from(span_w) > board_cols {
        return Vec::new();
    }
    let max_start_x = board_cols - u32::from(span_w);
    let max_start_x = u16::try_from(max_start_x).unwrap_or(u16::MAX);
    let x = x.min(max_start_x);

    let Some((_, layout)) = layouts.iter().find(|(k, _)| **k == board_key) else {
        return Vec::new();
    };
    if !layout.iter().any(|p| p.tool_id == tool_id) {
        return Vec::new();
    }
    let mut projected = layout.clone();
    let _ = upeg_runtime::pegboard::place_tool_with_push(&mut projected, &tool_id, x, y);
    projected.iter().map(placement_dto_from).collect()
}

/// Preview the layout that WOULD result if `tool_id`'s span were set to
/// `(cols, rows)` on `board_key`, WITHOUT mutating persisted state.
/// Mirrors [`set_pin_span`]'s commit semantics — apply the span to a
/// CLONE of the current layout, then re-anchor through
/// [`upeg_runtime::pegboard::place_tool_with_push`] at the pin's current
/// cell — so the resize preview and the eventual commit can never
/// disagree.
///
/// Returns an empty `Vec` when the board is unknown, the tool is not
/// pinned on the board, or `(cols, rows)` is outside the legal span
/// range (`ColSpan`/`RowSpan`), matching [`preview_push`]'s "empty means
/// hide the overlay" contract. The Dart canvas compares the returned
/// coordinates against the current layout to decide whether the resize
/// would push neighbours (warn tint) or drop cleanly.
#[flutter_rust_bridge::frb(sync)]
pub fn preview_resize(
    board_key: String,
    tool_id: String,
    cols: u32,
    rows: u32,
) -> Vec<PlacementDto> {
    let Ok(span) = validated_pin_span(cols, rows) else {
        return Vec::new();
    };
    if toolbox_tool(&tool_id).is_none() {
        return Vec::new();
    }
    let (_boards, layouts) = load_boards_and_layouts();
    let Some((_, layout)) = layouts.iter().find(|(k, _)| **k == board_key) else {
        return Vec::new();
    };
    let Some(current) = layout.iter().find(|p| p.tool_id == tool_id) else {
        return Vec::new();
    };
    let (anchor_x, anchor_y) = (current.x, current.y);
    let mut projected = layout.clone();
    if let Some(placement) = projected.iter_mut().find(|p| p.tool_id == tool_id) {
        placement.span = Some(span);
    }
    let _ =
        upeg_runtime::pegboard::place_tool_with_push(&mut projected, &tool_id, anchor_x, anchor_y);
    projected.iter().map(placement_dto_from).collect()
}

// ─── Empty-board suggestions ───────────────────────────────────────────

/// Suggest up to three tools to pin on an otherwise empty `board_key`.
///
/// Suggestion policy: tools whose tag list contains the board's slug
/// (case-insensitive) win; if none match we fall back to the first three
/// tools by id. The board key is treated as a tag rather than a keyword
/// search so the Dart side gets stable, deterministic picks.
#[flutter_rust_bridge::frb(sync)]
pub fn suggestions_for_empty_board(board_key: String) -> Vec<ToolDto> {
    let board_tag = board_key.to_lowercase();
    let matched: Vec<ToolDto> = toolbox_tools()
        .filter(|m| m.tags.iter().any(|t| t.eq_ignore_ascii_case(&board_tag)))
        .take(EMPTY_BOARD_SUGGESTION_LIMIT)
        .map(ToolDto::from)
        .collect();
    if matched.is_empty() {
        toolbox_tools()
            .take(EMPTY_BOARD_SUGGESTION_LIMIT)
            .map(ToolDto::from)
            .collect()
    } else {
        matched
    }
}

/// Process-global project-board scope, for tests on both this module
/// and `super::backup` (both apply the same project-board rule, so they
/// share one fixture).
///
/// Native-only: `upeg-sources`, whose store the assertions read back
/// through, is a native-only dependency of this crate.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod project_scope_test_support {
    use upeg_core::BoardKey;
    use upeg_runtime::pegboard_project::{
        ProjectBoardDecl, ProjectBoardScope, clear_project_board_scope, set_project_board_scope,
    };

    /// A manifest path that exists only for tests — the namespace is
    /// derived from it, so it just has to be distinct from any real one.
    const TEST_MANIFEST_PATH: &str = "/frb-scratch-project/upeg.toml";

    /// RAII holder: declares one project board for the lifetime of the
    /// guard and clears the scope on drop, including on a panicking
    /// assertion, so no test can leak project boards into the next one.
    pub(crate) struct ScopedProjectBoard;

    impl ScopedProjectBoard {
        pub(crate) fn declare(id: &str, label: &str) -> Self {
            set_project_board_scope(ProjectBoardScope::for_manifest(
                std::path::Path::new(TEST_MANIFEST_PATH),
                vec![ProjectBoardDecl::new(
                    BoardKey::parse(id).expect("test board id parses"),
                    label.to_string(),
                )],
            ));
            Self
        }
    }

    impl Drop for ScopedProjectBoard {
        fn drop(&mut self) {
            clear_project_board_scope();
        }
    }
}

#[cfg(test)]
mod tests;
