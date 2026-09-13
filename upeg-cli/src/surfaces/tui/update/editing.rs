use upeg_core::{
    PinColorHex, Surface, ToolMeta,
    search::{SearchQuery, SearchResult, SearchSignals},
};

use super::super::effects::Effect;
use super::super::model::{
    BoardEditMode, BoardFilter, FocusArea, State, ToolPickerMode, ToolPickerReturn, View,
};
use super::super::scroll::ScrollOffset;

pub(super) fn commit_board_editor(state: &mut State) -> Effect {
    let View::BoardEditor { mode, buffer } = &state.view else {
        return Effect::None;
    };
    let title = buffer.trim().to_string();
    if title.is_empty() {
        return Effect::None;
    }
    let mode = mode.clone();
    let mut snapshot = state.pegboard_snapshot();
    let committed = match mode {
        BoardEditMode::AddBoard => {
            let new_key = upeg_sources::pegboard::add_board(&mut snapshot, &title);
            if let Some(key) = new_key {
                state.filters.board = BoardFilter::selected(key);
                true
            } else {
                false
            }
        }
        BoardEditMode::Rename { key, .. } => {
            upeg_sources::pegboard::rename_board(&mut snapshot, &key, &title)
        }
    };
    if !committed {
        return Effect::None;
    }
    state.boards = snapshot.boards;
    state.layouts = snapshot.layouts;
    state.view = View::List;
    state.right_scroll = ScrollOffset::ZERO;
    state.cursor = 0;
    state.sync_filter_cursors();
    Effect::SavePegboard
}

pub(super) fn commit_board_delete(state: &mut State, key: &str) -> Effect {
    let mut snapshot = state.pegboard_snapshot();
    if !upeg_sources::pegboard::remove_board(&mut snapshot, key) {
        state.view = View::List;
        return Effect::None;
    }
    state.boards = snapshot.boards;
    state.layouts = snapshot.layouts;
    if state.filters.board.as_deref() == Some(key) {
        state.filters.board = state
            .boards
            .first()
            .map_or(BoardFilter::All, |b| BoardFilter::selected(b.key.as_str()));
    }
    state.view = View::List;
    state.cursor = 0;
    state.right_scroll = ScrollOffset::ZERO;
    state.sync_filter_cursors();
    Effect::SavePegboard
}

pub(super) fn open_pin_color_editor(state: &mut State) -> Effect {
    let Some(board) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let Some((placement, _tool)) = state.visible_placements().get(state.cursor).cloned() else {
        return Effect::None;
    };
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::PinColorEditor(super::super::model::PinColorEditor::new(
        board,
        placement.tool_id,
        placement.color,
    ));
    Effect::None
}

pub(super) fn commit_pin_color_editor(state: &mut State) -> Effect {
    let editor = match &state.view {
        View::PinColorEditor(editor) => editor.clone(),
        _ => return Effect::None,
    };
    let color = match editor.color_to_apply() {
        Ok(color) => color,
        Err(err) => {
            if let View::PinColorEditor(editor) = &mut state.view {
                editor.error = Some(err.to_string());
            }
            return Effect::None;
        }
    };
    set_pin_color_and_close(state, &editor.board, &editor.tool_id, color)
}

pub(super) fn reset_pin_color_editor(state: &mut State) -> Effect {
    let editor = match &state.view {
        View::PinColorEditor(editor) => editor.clone(),
        _ => return Effect::None,
    };
    set_pin_color_and_close(state, &editor.board, &editor.tool_id, None)
}

fn set_pin_color_and_close(
    state: &mut State,
    board: &str,
    tool_id: &str,
    color: Option<PinColorHex>,
) -> Effect {
    let mut snapshot = state.pegboard_snapshot();
    let changed = upeg_sources::pegboard::set_pin_color(&mut snapshot, board, tool_id, color);
    state.boards = snapshot.boards;
    state.layouts = snapshot.layouts;
    state.view = View::List;
    state.right_scroll = ScrollOffset::ZERO;
    if changed {
        Effect::SavePegboard
    } else {
        Effect::None
    }
}

pub(super) fn commit_tool_picker(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
    let (mode, query, cursor) = match &state.view {
        View::ToolPicker {
            mode,
            query,
            cursor,
            ..
        } => (*mode, query.clone(), *cursor),
        _ => return Effect::None,
    };
    match mode {
        ToolPickerMode::Search => commit_tool_search(state, &query, cursor, tools),
        ToolPickerMode::Pin => commit_tool_pin(state, &query, cursor),
    }
}

fn commit_tool_search(
    state: &mut State,
    query: &str,
    cursor: usize,
    tools: &[&'static ToolMeta],
) -> Effect {
    let results = tool_picker_results(state, ToolPickerMode::Search, query, tools);
    let Some(tool) = results.get(cursor) else {
        return Effect::None;
    };
    let Some(index) = tools.iter().position(|candidate| candidate.id == tool.id) else {
        return Effect::None;
    };
    state.cursor = index;
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::Detail;
    Effect::None
}

fn commit_tool_pin(state: &mut State, query: &str, cursor: usize) -> Effect {
    let Some(board_key) = state.filters.board.as_deref().map(str::to_string) else {
        return Effect::None;
    };
    let results = tool_picker_results(state, ToolPickerMode::Pin, query, &[]);
    let Some(tool) = results.get(cursor) else {
        return Effect::None;
    };
    let tool_id = tool.id;
    let mut snapshot = state.pegboard_snapshot();
    use upeg_sources::pegboard::PinAction;
    match upeg_sources::pegboard::toggle_pin(&mut snapshot, &board_key, tool_id) {
        PinAction::Pinned | PinAction::Unpinned => {
            state.boards = snapshot.boards;
            state.layouts = snapshot.layouts;
            Effect::SavePegboard
        }
        PinAction::NoOp => Effect::None,
    }
}

pub(crate) fn tool_picker_results(
    state: &State,
    mode: ToolPickerMode,
    query: &str,
    visible_tools: &[&'static ToolMeta],
) -> Vec<&'static ToolMeta> {
    tool_picker_search_results_for_mode(
        mode,
        query,
        visible_tools,
        tool_picker_search_signals(state),
    )
    .into_iter()
    .map(|result| result.tool)
    .collect()
}

#[cfg(test)]
pub(crate) fn tool_picker_search_results(state: &State, query: &str) -> Vec<SearchResult<'static>> {
    search_runtime_tool_picker_results(query, tool_picker_search_signals(state))
}

pub(super) fn tool_picker_results_with_signals(
    mode: ToolPickerMode,
    query: &str,
    visible_tools: &[&'static ToolMeta],
    signals: SearchSignals,
) -> Vec<&'static ToolMeta> {
    tool_picker_search_results_for_mode(mode, query, visible_tools, signals)
        .into_iter()
        .map(|result| result.tool)
        .collect()
}

fn search_runtime_tool_picker_results(
    query: &str,
    signals: SearchSignals,
) -> Vec<SearchResult<'static>> {
    upeg_runtime::search_toolbox_tools(
        SearchQuery {
            needle: query,
            surface: Some(Surface::Tui),
            limit: None,
        },
        signals,
    )
}

fn tool_picker_search_results_for_mode(
    mode: ToolPickerMode,
    query: &str,
    visible_tools: &[&'static ToolMeta],
    signals: SearchSignals,
) -> Vec<SearchResult<'static>> {
    match mode {
        ToolPickerMode::Pin => search_runtime_tool_picker_results(query, signals),
        ToolPickerMode::Search => upeg_core::search::search_tools(
            visible_tools,
            SearchQuery {
                needle: query,
                surface: Some(Surface::Tui),
                limit: None,
            },
            signals,
        ),
    }
}

pub(super) fn tool_picker_search_signals(state: &State) -> SearchSignals {
    let mut signals = pinned_search_signals(state);
    signals.recent.clone_from(&state.tool_picker_recent);
    signals
}

fn pinned_search_signals(state: &State) -> SearchSignals {
    let snapshot = state.pegboard_snapshot();
    state
        .filters
        .board
        .as_deref()
        .and_then(|board| snapshot.layouts.get(board))
        .map_or_else(SearchSignals::default, |placements| {
            SearchSignals::from_pinned_placements(placements)
        })
}

pub(super) fn open_board_editor_add(state: &mut State) -> Effect {
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::BoardEditor {
        mode: BoardEditMode::AddBoard,
        buffer: String::new(),
    };
    Effect::None
}

pub(super) fn open_board_editor_rename(state: &mut State) -> Effect {
    let Some((key, title)) = state
        .current_board()
        .map(|b| (b.key.clone(), b.title.clone()))
    else {
        return Effect::None;
    };
    let mode = BoardEditMode::Rename {
        key,
        original_title: title.clone(),
    };
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::BoardEditor {
        mode,
        buffer: title,
    };
    Effect::None
}

pub(super) fn open_tool_picker(state: &mut State) -> Effect {
    if state.filters.board.is_none() {
        return Effect::None;
    }
    state.right_scroll = ScrollOffset::ZERO;
    let return_to = ToolPickerReturn::list(state.focus);
    state.focus = FocusArea::Grid;
    state.view = View::ToolPicker {
        mode: ToolPickerMode::Pin,
        return_to,
        query: String::new(),
        cursor: 0,
    };
    Effect::None
}

pub(super) fn open_tool_search(state: &mut State) -> Effect {
    state.right_scroll = ScrollOffset::ZERO;
    let return_to = if state.view == View::Detail {
        ToolPickerReturn::detail(state.focus)
    } else {
        ToolPickerReturn::list(state.focus)
    };
    state.focus = FocusArea::Grid;
    state.view = View::ToolPicker {
        mode: ToolPickerMode::Search,
        return_to,
        query: String::new(),
        cursor: 0,
    };
    Effect::None
}

pub(super) fn open_confirm_delete_board(state: &mut State) -> Effect {
    if state.boards.len() <= 1 {
        return Effect::None;
    }
    let Some((key, title)) = state
        .current_board()
        .map(|b| (b.key.clone(), b.title.clone()))
    else {
        return Effect::None;
    };
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::ConfirmDeleteBoard { key, title };
    Effect::None
}

/// Open the modeless quit-confirm overlay. `q` routes here instead of
/// quitting outright so a reflexive keypress can't drop the session.
pub(super) fn open_confirm_quit(state: &mut State) -> Effect {
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::ConfirmQuit;
    Effect::None
}
