use upeg_core::{BOARD_COLS, ToolMeta};
use upeg_core::{KeyboardCommand, NavDirection};

use super::super::effects::Effect;
use super::super::model::{FocusArea, MoveMode, State, View};

pub(super) fn start_move(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
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
    // Mutually exclusive with resize mode — see `resize::start_resize`.
    state.resize_mode = None;
    state.move_mode = Some(MoveMode {
        board,
        tool_id: tool.id,
        target_x: current.x,
        target_y: current.y,
        base: base.clone(),
        preview: base,
    });
    state.view = View::List;
    state.focus = FocusArea::Grid;
    Effect::None
}

pub(super) fn handle_move_command(state: &mut State, command: KeyboardCommand) -> Effect {
    match command {
        KeyboardCommand::Cancel => {
            state.move_mode = None;
            Effect::None
        }
        KeyboardCommand::Commit => commit_move(state),
        KeyboardCommand::Move(NavDirection::Left) => {
            move_target_by(state, MoveTargetDelta::Left);
            Effect::None
        }
        KeyboardCommand::Move(NavDirection::Right) => {
            move_target_by(state, MoveTargetDelta::Right);
            Effect::None
        }
        KeyboardCommand::Move(NavDirection::Up) => {
            move_target_by(state, MoveTargetDelta::Up);
            Effect::None
        }
        KeyboardCommand::Move(NavDirection::Down) => {
            move_target_by(state, MoveTargetDelta::Down);
            Effect::None
        }
        _ => Effect::None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveTargetDelta {
    Left,
    Right,
    Up,
    Down,
}

fn move_target_by(state: &mut State, delta: MoveTargetDelta) {
    let Some(move_mode) = state.move_mode.as_mut() else {
        return;
    };
    let (w, _) = upeg_runtime::pegboard::placement_size(move_mode.tool_id);
    let max_x = BOARD_COLS.saturating_sub(w.clamp(1, BOARD_COLS));
    match delta {
        MoveTargetDelta::Left => move_mode.target_x = move_mode.target_x.saturating_sub(1),
        MoveTargetDelta::Right => {
            move_mode.target_x = move_mode.target_x.saturating_add(1).min(max_x);
        }
        MoveTargetDelta::Up => move_mode.target_y = move_mode.target_y.saturating_sub(1),
        MoveTargetDelta::Down => move_mode.target_y = move_mode.target_y.saturating_add(1),
    }
    let mut preview = move_mode.base.clone();
    let _ = upeg_runtime::pegboard::place_tool_with_push(
        &mut preview,
        move_mode.tool_id,
        move_mode.target_x,
        move_mode.target_y,
    );
    move_mode.preview = preview;
}

fn commit_move(state: &mut State) -> Effect {
    let Some(move_mode) = state.move_mode.take() else {
        return Effect::None;
    };
    if move_mode.preview == move_mode.base {
        return Effect::None;
    }
    let moved_id = move_mode.tool_id;
    state.layouts.insert(move_mode.board, move_mode.preview);
    state.view = View::List;
    // Selection-after-commit: the grid order resorts by (y, x) once the
    // pushed preview lands, so the moved tool's index almost always
    // changes. Walk the new placement list and re-anchor the cursor on
    // the moved tool so the next `p` / `m` / `[` / `]` acts on the
    // same tool the user just placed.
    if let Some(idx) = state
        .visible_placements()
        .iter()
        .position(|(_, tool)| tool.id == moved_id)
    {
        state.cursor = idx;
    }
    Effect::SavePegboard
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MoveDir {
    Prev,
    Next,
}

impl From<MoveDir> for upeg_runtime::pegboard::Direction {
    fn from(dir: MoveDir) -> Self {
        match dir {
            MoveDir::Prev => Self::Prev,
            MoveDir::Next => Self::Next,
        }
    }
}

pub(super) fn move_cursor_layout(
    state: &mut State,
    tools: &[&'static ToolMeta],
    dir: MoveDir,
) -> Effect {
    let Some(board_key) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    let tool_id = tool.id;
    let mut snapshot = state.pegboard_snapshot();
    let moved =
        upeg_sources::pegboard::move_layout_entry(&mut snapshot, &board_key, tool_id, dir.into());
    if !moved {
        return Effect::None;
    }
    state.boards = snapshot.boards;
    state.layouts = snapshot.layouts;
    state.cursor = match dir {
        MoveDir::Prev => state.cursor.saturating_sub(1),
        MoveDir::Next => state.cursor.saturating_add(1),
    };
    Effect::SavePegboard
}

pub(super) fn toggle_pin_at_cursor(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
    let Some(board_key) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    let tool_id = tool.id;
    let mut snapshot = state.pegboard_snapshot();
    let action = upeg_sources::pegboard::toggle_pin(&mut snapshot, &board_key, tool_id);
    use upeg_sources::pegboard::PinAction;
    match action {
        PinAction::Pinned | PinAction::Unpinned => {
            state.boards = snapshot.boards;
            state.layouts = snapshot.layouts;
            if matches!(action, PinAction::Unpinned)
                && let Some(ids) = state.layouts.get(&board_key)
                && state.cursor >= ids.len()
            {
                state.cursor = ids.len().saturating_sub(1);
            }
            Effect::SavePegboard
        }
        PinAction::NoOp => Effect::None,
    }
}
