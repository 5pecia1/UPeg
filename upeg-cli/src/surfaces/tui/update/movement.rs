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
    let Some(current) = state
        .selected_board_pin()
        .filter(|pin| pin.tool_id == tool.id)
    else {
        return Effect::None;
    };
    let base = state.layouts.get(&board).cloned().unwrap_or_default();
    // Mutually exclusive with resize mode — see `resize::start_resize`.
    state.resize_mode = None;
    state.move_mode = Some(MoveMode {
        board,
        tool_id: tool.id,
        pin_id: current.pin_id,
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
            state.cancel_pin_preview();
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
    let Some(target) = move_mode
        .base
        .iter()
        .find(|pin| pin.pin_id == move_mode.pin_id)
    else {
        return;
    };
    let (w, _) = upeg_runtime::pegboard::effective_size(target);
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
        move_mode.pin_id.as_str(),
        move_mode.target_x,
        move_mode.target_y,
    );
    move_mode.preview = preview;
    let id = move_mode.pin_id.clone();
    state.focus_pin(&id);
}

fn commit_move(state: &mut State) -> Effect {
    let Some(move_mode) = state.move_mode.take() else {
        return Effect::None;
    };
    if move_mode.preview == move_mode.base {
        return Effect::None;
    }
    state.layouts.insert(move_mode.board, move_mode.preview);
    state.view = View::List;
    state.focus_pin(&move_mode.pin_id);
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
    let Some(pin) = state
        .selected_board_pin()
        .filter(|pin| pin.tool_id == tool.id)
    else {
        return Effect::None;
    };
    let mut snapshot = state.pegboard_snapshot();
    let moved = upeg_sources::pegboard::move_layout_entry(
        &mut snapshot,
        &board_key,
        pin.pin_id.as_str(),
        dir.into(),
    );
    if !moved {
        return Effect::None;
    }
    state.boards = snapshot.boards;
    state.layouts = snapshot.layouts;
    state.focus_pin(&pin.pin_id);
    Effect::SavePegboard
}

pub(super) fn toggle_pin_at_cursor(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
    let Some(board_key) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    let Some(pin) = state
        .selected_board_pin()
        .filter(|pin| pin.tool_id == tool.id)
    else {
        return Effect::None;
    };
    let Some(layout) = state.layouts.get_mut(&board_key) else {
        return Effect::None;
    };
    if !upeg_runtime::pegboard::remove_placement(layout, pin.pin_id.as_str()).changed() {
        return Effect::None;
    }
    state.cursor = state
        .cursor
        .min(state.visible_placements().len().saturating_sub(1));
    Effect::SavePegboard
}
