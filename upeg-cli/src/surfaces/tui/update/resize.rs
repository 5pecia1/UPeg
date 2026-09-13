//! Pin-resize mode — the size twin of [`super::movement`]'s coordinate
//! move. Same modal anatomy: `start_resize` snapshots the committed
//! layout as `base`, every accepted span change recomputes `preview`
//! through the shared push-placement primitive, and only Commit turns
//! the preview into the committed layout.

use upeg_core::{ColSpan, KeyboardCommand, PinSpan, RowSpan, ToolMeta};

use super::super::effects::Effect;
use super::super::model::{FocusArea, ResizeMode, State, View};

pub(super) fn start_resize(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
    let Some(board) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    let base = state.layouts.get(&board).cloned().unwrap_or_default();
    let Some(current) = base.iter().find(|p| p.tool_id == tool.id) else {
        return Effect::None;
    };
    let (cols, rows) = upeg_runtime::pegboard::effective_size(current);
    // Mutually exclusive with move mode. The modal key intercepts
    // already prevent entering one mode from inside the other, but
    // clearing here keeps the invariant local and future-proof.
    state.move_mode = None;
    state.resize_mode = Some(ResizeMode {
        board,
        tool_id: tool.id,
        cols,
        rows,
        base: base.clone(),
        preview: base,
    });
    state.view = View::List;
    state.focus = FocusArea::Grid;
    Effect::None
}

pub(super) fn handle_resize_command(state: &mut State, command: KeyboardCommand) -> Effect {
    match command {
        KeyboardCommand::Cancel => {
            state.resize_mode = None;
            Effect::None
        }
        KeyboardCommand::Commit => commit_resize(state),
        KeyboardCommand::ResizeBy { cols, rows } => {
            resize_span_by(state, cols, rows);
            Effect::None
        }
        KeyboardCommand::ResetSpan => {
            reset_span_to_manifest(state);
            Effect::None
        }
        _ => Effect::None,
    }
}

/// `u16` span axis plus a signed keyboard delta. `None` means the raw
/// arithmetic already left the representable range — the keypress is
/// ignored before the span newtypes even get to validate.
fn checked_apply_delta(value: u16, delta: i8) -> Option<u16> {
    u16::try_from(i32::from(value) + i32::from(delta)).ok()
}

fn resize_span_by(state: &mut State, cols_delta: i8, rows_delta: i8) {
    let Some(resize) = state.resize_mode.as_mut() else {
        return;
    };
    let Some(next_cols) = checked_apply_delta(resize.cols, cols_delta) else {
        return;
    };
    let Some(next_rows) = checked_apply_delta(resize.rows, rows_delta) else {
        return;
    };
    // Validation lives in the span newtypes: cols outside
    // 1..=BOARD_COLS or rows below 1 reject, and the keypress is
    // ignored without touching the current preview.
    let Ok(col_span) = ColSpan::new(next_cols) else {
        return;
    };
    let Ok(row_span) = RowSpan::new(next_rows) else {
        return;
    };
    resize.cols = col_span.get();
    resize.rows = row_span.get();
    recompute_preview(resize);
}

/// `0` — drop the user override and preview the manifest footprint
/// again. Committing afterwards persists `span = None` (see
/// [`span_override`]), not a redundant override equal to the manifest.
fn reset_span_to_manifest(state: &mut State) {
    let Some(resize) = state.resize_mode.as_mut() else {
        return;
    };
    let (cols, rows) = upeg_runtime::pegboard::placement_size(resize.tool_id);
    resize.cols = cols;
    resize.rows = rows;
    recompute_preview(resize);
}

/// The span the previewed placement should carry: `None` when the
/// current size equals the manifest footprint (no override needed),
/// otherwise the validated `PinSpan`.
fn span_override(tool_id: &str, cols: u16, rows: u16) -> Option<PinSpan> {
    if (cols, rows) == upeg_runtime::pegboard::placement_size(tool_id) {
        return None;
    }
    let col_span = ColSpan::new(cols).ok()?;
    let row_span = RowSpan::new(rows).ok()?;
    Some(PinSpan::new(col_span, row_span))
}

/// Rebuild `preview` from `base`: stamp the current span onto the
/// target placement, then re-place it at its committed anchor so growth
/// pushes colliding neighbours instead of overlapping them.
fn recompute_preview(resize: &mut ResizeMode) {
    let Some(anchor) = resize.base.iter().find(|p| p.tool_id == resize.tool_id) else {
        return;
    };
    let (anchor_x, anchor_y) = (anchor.x, anchor.y);
    let mut preview = resize.base.clone();
    if let Some(target) = preview.iter_mut().find(|p| p.tool_id == resize.tool_id) {
        target.span = span_override(resize.tool_id, resize.cols, resize.rows);
    }
    let _ = upeg_runtime::pegboard::place_tool_with_push(
        &mut preview,
        resize.tool_id,
        anchor_x,
        anchor_y,
    );
    resize.preview = preview;
}

fn commit_resize(state: &mut State) -> Effect {
    let Some(resize) = state.resize_mode.take() else {
        return Effect::None;
    };
    // `preview` carries the span override on the target placement, so
    // `preview == base` already covers both "no geometry change" and
    // "span override unchanged" — nothing to persist.
    if resize.preview == resize.base {
        return Effect::None;
    }
    let resized_id = resize.tool_id;
    state.layouts.insert(resize.board, resize.preview);
    state.view = View::List;
    // Selection-after-commit: growth can push neighbours and re-sort
    // the (y, x) grid order, so re-anchor the cursor on the resized
    // tool — same contract as `commit_move`.
    if let Some(idx) = state
        .visible_placements()
        .iter()
        .position(|(_, tool)| tool.id == resized_id)
    {
        state.cursor = idx;
    }
    Effect::SavePegboard
}
