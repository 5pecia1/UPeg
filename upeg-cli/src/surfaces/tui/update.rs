//! TEA (The Elm Architecture) update step for the TUI surface.
//!
//! **Purity invariant** — no function in this module performs I/O:
//! no filesystem, no network, no `dispatch_tool` calls, no terminal
//! writes, no `eprintln`. State mutates in place via the `&mut State`
//! parameter; the only externally-visible effect is the returned
//! [`Effect`] value. The event loop in [`super::effects`] interprets
//! the `Effect` and feeds results back as fresh [`Msg`] values.
//!
//! If a new feature needs I/O, add an [`Effect`] variant — never a
//! direct call from this module. This keeps `update` trivially unit-
//! testable: feed a `Msg`, assert on `State` and `Effect`.

use ratatui::layout::Rect;
use serde_json::{Value, json};
use upeg_core::{
    KeyStroke, KeyboardCommand, KeyboardContext, KeyboardScope, NavDirection, OrderDirection,
    OutputEntry, Surface, ToolMeta, resolve_key,
};
use upeg_runtime::{ProgressEvent, ToolApprovalPolicy};

use crate::app::args::APPROVE_RESERVED_INPUT_NAME;
use crate::domain::execution::dispatch::Outcome;

use super::controls::{
    apply_filter_command, ensure_focused_filter_visible, handle_focus_command,
    handle_focused_control_key,
};
use super::effects::Effect;
use super::grid::{
    GridDirection, ensure_grid_cursor_visible, ensure_grid_cursor_visible_h, move_cursor_in_grid,
    tui_layout,
};
use super::model::{
    ActiveRun, FocusArea, LiveTail, RunToken, State, ToolPickerReturnView, TuiFormState, View,
};
use super::msg::{Key, Msg};
use super::scroll::ScrollOffset;

/// Sync both grid scroll axes so the focused cell stays in view after a
/// cursor move. The canvas is fixed at BOARD_COLS columns, so narrow
/// terminals rely on horizontal scroll to reach right-edge cells; doing
/// both together keeps the v/h pair coherent at every transition.
fn sync_grid_scroll_to_cursor(state: &mut State, tools: &[&'static ToolMeta], area: Rect) {
    state.grid_scroll = ensure_grid_cursor_visible(tools, area, state.cursor, state.grid_scroll);
    state.grid_h_scroll =
        ensure_grid_cursor_visible_h(tools, area, state.cursor, state.grid_h_scroll);
}

mod editing;
mod mouse;
mod movement;
mod presentation;
mod resize;
mod settings;

use editing::commit_pin_color_editor;
pub(crate) use editing::tool_picker_results;
#[cfg(test)]
pub(crate) use editing::tool_picker_search_results;
use editing::{
    commit_board_delete, commit_board_editor, commit_tool_picker, open_board_editor_add,
    open_board_editor_rename, open_confirm_delete_board, open_confirm_quit, open_pin_color_editor,
    open_tool_picker, open_tool_search, reset_pin_color_editor, tool_picker_results_with_signals,
    tool_picker_search_signals,
};
pub use mouse::handle_mouse;
use movement::{
    MoveDir, handle_move_command, move_cursor_layout, start_move, toggle_pin_at_cursor,
};
use presentation::{apply_outcome_with_host, open_result_action, result_navigation_bounds};
use resize::{handle_resize_command, start_resize};
use settings::{cycle_focused_value, open_settings};

/// Pure TEA update function. Runtime code feeds terminal/network/tool results
/// in as [`Msg`] values; the return value describes the only side effect the
/// event loop may perform.
pub fn update(state: &mut State, msg: Msg<'_>) -> Effect {
    match msg {
        Msg::KeyPress {
            stroke,
            tools,
            area,
        } => handle_key_stroke_with_area(state, stroke, tools, area),
        Msg::Mouse { mouse, tools, area } => handle_mouse(state, mouse, tools, area),
        Msg::ToolDone {
            run,
            host,
            tool_id,
            outcome,
        } => {
            if state.is_active_run(run) {
                apply_outcome_with_host(state, tool_id, outcome, host)
            } else {
                Effect::None
            }
        }
        Msg::ToolProgress { run, event } => {
            apply_progress(state, run, event);
            Effect::None
        }
    }
}

/// The surface a TUI dispatch is stamped with on both routes
/// (standalone and host-attached — see `dispatch_through_host_or_local`
/// in [`super::effects`]), and therefore the only surface whose approval
/// this TUI can offer.
const TUI_APPROVAL_SURFACE: Surface = Surface::Tui;

/// Separator between surface labels in the "who may approve this"
/// explanation. Matches `upeg_loader`'s chain-approval error text, whose
/// own `surface_label_list` is crate-private, so the user sees one
/// spelling wherever the answer surfaces.
const APPROVAL_SURFACE_LABEL_SEPARATOR: &str = "/";

/// The single approval barrier in front of every TUI dispatch.
///
/// Both entry points (a zero-input Tool's Run and the Form's Run) route
/// through here, so no keyboard or mouse path can start a gated run
/// without a human having said yes first.
pub(super) fn dispatch_or_confirm(state: &mut State, tool_id: &'static str, args: Value) -> Effect {
    // Before the approval barrier, not after: collecting a "yes" for a
    // run that cannot start would be asking a question whose answer is
    // then thrown away.
    if let Some(active) = state.active_run {
        return refuse_second_run(state, active);
    }
    let policy = upeg_runtime::tool_approval_policy(tool_id);
    if !policy.requires_approval() {
        return start_run(state, tool_id, args);
    }
    if policy.honors(TUI_APPROVAL_SURFACE) {
        state.view = View::ConfirmApproval { tool_id, args };
        return Effect::None;
    }
    // The Tool has a barrier this surface may not lift. Prompting anyway
    // would collect a "yes" the chain would then refuse, so the pane
    // says who *can* approve instead.
    let surfaces = approval_surface_labels(&policy);
    state.right_scroll = ScrollOffset::ZERO;
    state.focus = FocusArea::RightPane;
    state.view = View::Result {
        tool_id,
        outputs: Vec::new(),
        text: crate::i18n::t_args(
            state.tweaks.locale,
            "tui.approval.denied_for_surface",
            &[("surfaces", surfaces.as_str())],
        ),
        is_error: true,
    };
    Effect::None
}

fn approval_surface_labels(policy: &ToolApprovalPolicy) -> String {
    policy
        .surfaces()
        .iter()
        .map(|surface| surface.label())
        .collect::<Vec<_>>()
        .join(APPROVAL_SURFACE_LABEL_SEPARATOR)
}

/// Hand the run to the event loop and switch the right pane to its live
/// view. Every dispatch the TUI starts goes through here, so
/// `View::Running` and the in-flight worker are created together.
pub(super) fn start_run(state: &mut State, tool_id: &'static str, args: Value) -> Effect {
    state.result_inputs = args.clone();
    state.result_row = 0;
    state.result_action = 0;
    state.restore_result_row_key = None;
    state.result_run = None;
    state.result_host = None;
    let run = state.start_active_run(tool_id);
    open_running_pane(state, tool_id);
    Effect::Dispatch { run, tool_id, args }
}

/// Put the right pane on `tool_id`'s live view. Split out so both a
/// fresh run and a return to an already-running one produce exactly the
/// same pane.
fn open_running_pane(state: &mut State, tool_id: &'static str) {
    state.right_scroll = ScrollOffset::ZERO;
    state.focus = FocusArea::RightPane;
    state.view = View::Running {
        tool_id,
        tail: LiveTail::default(),
        cancelling: false,
    };
}

/// A second Run while the first is still in flight.
///
/// The event loop runs exactly one dispatch, so a second
/// `Effect::Dispatch` would be dropped on the floor while the new pane
/// filled with the *old* run's output and ended on the old run's result.
/// Instead the surface names what is running and puts the user back on
/// that run's pane — where `Esc` still cancels it. The tail restarts
/// empty: leaving the pane dropped it, and inventing the missing lines
/// would be worse than admitting they are gone.
fn refuse_second_run(state: &mut State, active: ActiveRun) -> Effect {
    open_running_pane(state, active.tool_id);
    state.status_message = Some(crate::i18n::t_args(
        state.tweaks.locale,
        "tui.run.already_running",
        &[("tool_id", &crate::display_id(active.tool_id))],
    ));
    Effect::None
}

/// Stamp the reserved chain-approval key onto the args the user just
/// approved. Non-object args are returned untouched: a reserved *key*
/// has nowhere to live in a scalar or an array envelope.
fn with_chain_approval(args: Value) -> Value {
    match args {
        Value::Object(mut map) => {
            map.insert(APPROVE_RESERVED_INPUT_NAME.to_string(), Value::Bool(true));
            Value::Object(map)
        }
        other => other,
    }
}

/// Apply one incremental output chunk from the dispatch named by `run`.
///
/// Two guards, and both are load-bearing. The run has to be the one the
/// model still considers active, so a straggler from a finished run
/// cannot fold into the next run's pane; and the view has to still be
/// `Running`, so a chunk that arrives after the user navigated away has
/// nowhere to resurrect a run that is already over.
pub fn apply_progress(state: &mut State, run: RunToken, event: ProgressEvent) {
    if !state.is_active_run(run) {
        return;
    }
    if let View::Running { tail, .. } = &mut state.view {
        tail.push_chunk(&event.chunk);
    }
}

pub(super) fn run_selected_tool(state: &mut State, tools: &[&'static ToolMeta]) -> Effect {
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    if !tool.is_on_surface(upeg_core::Surface::Tui) {
        state.right_scroll = ScrollOffset::ZERO;
        state.focus = FocusArea::RightPane;
        let surfaces_label = tool.surfaces_label();
        state.view = View::Result {
            tool_id: tool.id,
            outputs: Vec::new(),
            text: crate::i18n::t_args(
                state.tweaks.locale,
                "tui.error.not_on_surface",
                &[("surfaces", surfaces_label.as_str())],
            ),
            is_error: true,
        };
        return Effect::None;
    }
    if tool.input_spec.fields.is_empty() {
        return dispatch_or_confirm(state, tool.id, json!({}));
    }
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::Form {
        tool_id: tool.id,
        form: TuiFormState::new(tool.input_spec.clone()),
    };
    Effect::None
}

/// Pure state transition for one key event. `num_tools` is needed to clamp
/// the cursor at the bottom of the list. `tools` lets us read the current
/// tool's typed inputs when entering Form view.
pub fn handle_key(state: &mut State, key: Key, tools: &[&'static ToolMeta]) -> Effect {
    handle_key_stroke_with_area(state, key.into(), tools, None)
}

fn handle_key_stroke_with_area(
    state: &mut State,
    stroke: KeyStroke,
    tools: &[&'static ToolMeta],
    area: Option<Rect>,
) -> Effect {
    // Any ephemeral toast (e.g. F2 clipboard feedback) is a one-shot
    // notice — the next keystroke, whatever it is, dismisses it.
    state.status_message = None;

    if matches!(state.view, View::Result { .. }) && !stroke.modifiers.has_command_modifier() {
        let (row_count, action_count) = result_navigation_bounds(state);
        match stroke.key {
            Key::Char('a') => return open_result_action(state),
            Key::Up => {
                state.result_row = state.result_row.saturating_sub(1);
                state.result_action = 0;
                return Effect::None;
            }
            Key::Down => {
                state.result_row = state
                    .result_row
                    .saturating_add(1)
                    .min(row_count.saturating_sub(1));
                state.result_action = 0;
                return Effect::None;
            }
            Key::Left => {
                state.result_action = state.result_action.saturating_sub(1);
                return Effect::None;
            }
            Key::Right => {
                state.result_action = state
                    .result_action
                    .saturating_add(1)
                    .min(action_count.saturating_sub(1));
                return Effect::None;
            }
            _ => {}
        }
    }

    if state.move_mode.is_some() {
        let context = KeyboardContext::new(KeyboardScope::Moving);
        let Some(command) = resolve_key(context, stroke) else {
            return Effect::None;
        };
        return handle_move_command(state, command);
    }

    if state.resize_mode.is_some() {
        let context = KeyboardContext::new(KeyboardScope::Resize);
        let Some(command) = resolve_key(context, stroke) else {
            return Effect::None;
        };
        return handle_resize_command(state, command);
    }

    if matches!(
        state.view,
        View::List | View::Detail | View::Result { .. } | View::Running { .. }
    ) {
        let board_context = KeyboardContext {
            scope: KeyboardScope::Board,
            has_tool_focus: !tools.is_empty(),
        };
        if let Some(command) = resolve_key(board_context, stroke) {
            if handle_focus_command(state, command, area) {
                return Effect::None;
            }
            if matches!(state.view, View::List | View::Detail)
                && apply_filter_command(state, command)
            {
                ensure_focused_filter_visible(state, area);
                return Effect::SavePegboardSelection;
            }
            if matches!(state.view, View::Result { .. })
                && let Some(effect) = handle_result_view_command(state, command, tools)
            {
                return effect;
            }
            if matches!(state.view, View::Running { .. })
                && let Some(effect) = handle_running_view_command(state, command)
            {
                return effect;
            }
        }
        if handle_focused_control_key(state, stroke, area) {
            return Effect::None;
        }
    }

    // Result semantics: Enter/F1/Space re-run the same tool, Esc/q
    // dismiss to List (both handled above via `handle_result_view_command`).
    // Everything else that reaches here is an unconsumed key — unlike
    // the old "any key dismisses" behavior, the result now stays
    // visible until explicitly replaced or closed.
    if matches!(state.view, View::Result { .. }) {
        return Effect::None;
    }

    // A run owns the surface while it is in flight: the only gesture
    // `View::Running` answers is Esc/q (cancel, handled above), so every
    // other key is swallowed rather than allowed to open a form, a
    // dialog, or a second dispatch on top of the live one.
    if matches!(state.view, View::Running { .. }) {
        return Effect::None;
    }

    let context = KeyboardContext {
        scope: keyboard_scope_for_view(&state.view),
        has_tool_focus: !tools.is_empty(),
    };
    let Some(command) = resolve_key(context, stroke) else {
        return Effect::None;
    };

    let picker_signals = if matches!(state.view, View::ToolPicker { .. }) {
        Some(tool_picker_search_signals(state))
    } else {
        None
    };

    match &mut state.view {
        View::List => handle_list_command(state, command, tools, area),
        View::Detail => handle_detail_command(state, command, tools),

        View::Form { tool_id, form } => {
            let n = form.len();
            match command {
                KeyboardCommand::FocusNext => {
                    if n > 0 {
                        form.focused = (form.focused + 1) % n;
                    }
                    Effect::None
                }
                KeyboardCommand::FocusPrevious => {
                    if n > 0 {
                        form.focused = if form.focused == 0 {
                            n - 1
                        } else {
                            form.focused - 1
                        };
                    }
                    Effect::None
                }
                KeyboardCommand::Move(NavDirection::Left) => {
                    form.cycle_focused_typed_value(true);
                    Effect::None
                }
                KeyboardCommand::Move(NavDirection::Right) => {
                    form.cycle_focused_typed_value(false);
                    Effect::None
                }
                KeyboardCommand::Backspace => {
                    form.backspace_focused_text();
                    Effect::None
                }
                KeyboardCommand::ClearInput => {
                    form.clear_focused_text();
                    Effect::None
                }
                KeyboardCommand::Text(c) => {
                    if c == ' ' && form.cycle_focused_typed_value(false) {
                        return Effect::None;
                    }
                    form.push_char_to_focused_text(c);
                    Effect::None
                }
                KeyboardCommand::Run => {
                    let current_tool_id = *tool_id;
                    match form.args() {
                        Ok(args) => dispatch_or_confirm(state, current_tool_id, args),
                        Err(text) => {
                            state.right_scroll = ScrollOffset::ZERO;
                            state.focus = FocusArea::RightPane;
                            state.view = View::Result {
                                tool_id: current_tool_id,
                                outputs: Vec::new(),
                                text,
                                is_error: true,
                            };
                            Effect::None
                        }
                    }
                }
                KeyboardCommand::Close => back_one_level(state),
                _ => Effect::None,
            }
        }

        View::Result { .. } => Effect::None,

        // Both are unreachable at runtime — Result and Running return
        // above — but the exhaustive match keeps a future variant from
        // silently inheriting "do nothing".
        View::Running { .. } => Effect::None,

        // Approval barrier: Confirm stamps the reserved approval key
        // onto the args the user just saw and starts the run; Cancel
        // drops both the args and the prompt.
        View::ConfirmApproval { tool_id, args } => match command {
            KeyboardCommand::Confirm => {
                let tool_id = *tool_id;
                let args = with_chain_approval(std::mem::take(args));
                start_run(state, tool_id, args)
            }
            KeyboardCommand::Cancel => back_one_level(state),
            _ => Effect::None,
        },

        // Stage E-1/E-2: View::Settings entry, navigation, and value
        // cycling. Field cursor walks Locale → Theme → Accent → Locale
        // (next/prev are exhaustive); ←/→ cycles the focused field's
        // value. Persistence ships in Stage E-3 (Effect::SaveTweaks).
        View::Settings { focused_field } => match command {
            KeyboardCommand::FocusNext => {
                *focused_field = focused_field.next();
                Effect::None
            }
            KeyboardCommand::FocusPrevious => {
                *focused_field = focused_field.prev();
                Effect::None
            }
            KeyboardCommand::Move(NavDirection::Right) => {
                cycle_focused_value(state, /* forward = */ true);
                // Persist on every edit so a TUI session's preferences
                // survive cold restart and surface in the desktop UI.
                Effect::SaveTweaks
            }
            KeyboardCommand::Move(NavDirection::Left) => {
                cycle_focused_value(state, /* forward = */ false);
                Effect::SaveTweaks
            }
            KeyboardCommand::Close => back_one_level(state),
            _ => Effect::None,
        },

        // Phase 6: single-line text editor for board add / rename.
        // typing accumulates into `buffer`; Enter commits through
        // sources; ESC discards. The shared `back_one_level` already
        // handles ESC fall-through to List, so we only branch on the
        // commit / typing-style keys here.
        View::BoardEditor { mode, buffer } => match command {
            KeyboardCommand::Commit => commit_board_editor(state),
            KeyboardCommand::Cancel => back_one_level(state),
            KeyboardCommand::Backspace => {
                buffer.pop();
                Effect::None
            }
            KeyboardCommand::ClearInput => {
                buffer.clear();
                Effect::None
            }
            KeyboardCommand::Text(c) => {
                buffer.push(c);
                // Silence the unused `mode` warning while leaving the
                // pattern destructured so a future variant addition
                // forces this match arm to update.
                let _ = mode;
                Effect::None
            }
            _ => Effect::None,
        },

        // Phase 6 (delete confirm) / Phase 8 wiring stub: only y / n /
        // ESC are meaningful here. ESC is caught above.
        View::ConfirmDeleteBoard {
            key: target_key, ..
        } => match command {
            KeyboardCommand::Confirm => {
                let target = target_key.clone();
                commit_board_delete(state, &target)
            }
            KeyboardCommand::Cancel => back_one_level(state),
            _ => Effect::None,
        },

        // Modeless accidental-quit guard: `q` opens this confirm overlay
        // instead of dropping the session outright. Confirm quits; Cancel
        // (n / q / Esc) returns to the board.
        View::ConfirmQuit => match command {
            KeyboardCommand::Confirm => Effect::Quit,
            KeyboardCommand::Cancel => back_one_level(state),
            _ => Effect::None,
        },

        View::PinColorEditor(editor) => match command {
            KeyboardCommand::Commit => commit_pin_color_editor(state),
            KeyboardCommand::Cancel => back_one_level(state),
            KeyboardCommand::Backspace => {
                editor.backspace();
                Effect::None
            }
            KeyboardCommand::ClearInput => {
                editor.clear();
                Effect::None
            }
            KeyboardCommand::Text('q' | 'Q') => back_one_level(state),
            KeyboardCommand::Text('r' | 'R') => reset_pin_color_editor(state),
            KeyboardCommand::Text(c @ '1'..='9')
                if !matches!(editor.draft, super::model::PinColorEditorDraft::Custom(_)) =>
            {
                editor.select_palette_digit(c);
                Effect::None
            }
            KeyboardCommand::Text(c) => {
                editor.push_char(c);
                Effect::None
            }
            _ => Effect::None,
        },

        // ToolPicker: live-filter through shared search. Pin mode targets the
        // runtime toolbox; search mode targets the current visible tool slice.
        // Both modes combine current pinned placements with recent signals
        // loaded by the effect layer into State.
        View::ToolPicker {
            mode,
            query,
            cursor,
            ..
        } => match command {
            KeyboardCommand::Cancel => back_one_level(state),
            KeyboardCommand::Backspace => {
                query.pop();
                *cursor = 0;
                Effect::None
            }
            KeyboardCommand::ClearInput => {
                query.clear();
                *cursor = 0;
                Effect::None
            }
            KeyboardCommand::Text(c) => {
                query.push(c);
                *cursor = 0;
                Effect::None
            }
            KeyboardCommand::Move(NavDirection::Up) => {
                *cursor = cursor.saturating_sub(1);
                Effect::None
            }
            KeyboardCommand::Move(NavDirection::Down) => {
                let Some(signals) = picker_signals else {
                    return Effect::None;
                };
                let len = tool_picker_results_with_signals(*mode, query, tools, signals).len();
                let next = cursor.saturating_add(1);
                *cursor = next.min(len.saturating_sub(1));
                Effect::None
            }
            KeyboardCommand::Commit => commit_tool_picker(state, tools),
            _ => Effect::None,
        },
    }
}

const fn keyboard_scope_for_view(view: &View) -> KeyboardScope {
    match view {
        // A run is a state of the board, not a modal — same choice
        // `View::Result` makes.
        View::List | View::Result { .. } | View::Running { .. } => KeyboardScope::Board,
        View::Detail => KeyboardScope::Detail,
        View::Form { .. } => KeyboardScope::Form,
        View::Settings { .. } => KeyboardScope::Settings,
        View::BoardEditor { .. } => KeyboardScope::BoardEditor,
        View::ConfirmDeleteBoard { .. } => KeyboardScope::ConfirmDelete,
        View::PinColorEditor(_) => KeyboardScope::BoardEditor,
        View::ToolPicker { .. } => KeyboardScope::ToolPicker,
        // The quit-confirm and approval overlays reuse the generic
        // yes/no scope (Enter/F1/y = Confirm, Esc/n/q = Cancel).
        View::ConfirmQuit | View::ConfirmApproval { .. } => KeyboardScope::ConfirmDelete,
    }
}

fn handle_list_command(
    state: &mut State,
    command: KeyboardCommand,
    tools: &[&'static ToolMeta],
    area: Option<Rect>,
) -> Effect {
    match command {
        KeyboardCommand::Move(direction) => {
            move_list_cursor(state, direction, tools, area);
            Effect::None
        }
        KeyboardCommand::Run => run_selected_tool(state, tools),
        KeyboardCommand::Open => {
            state.right_scroll = ScrollOffset::ZERO;
            state.view = View::Detail;
            Effect::None
        }
        // `q` (Quit) opens the confirm-quit overlay; Esc (Close) at the
        // board root is a no-op now — it no longer drops the session, so
        // a reflexive Escape can't quit the app.
        KeyboardCommand::Quit => open_confirm_quit(state),
        KeyboardCommand::Close => Effect::None,
        KeyboardCommand::OpenSettings => open_settings(state),
        KeyboardCommand::TogglePin => toggle_pin_at_cursor(state, tools),
        KeyboardCommand::StartMove => start_move(state, tools),
        KeyboardCommand::StartResize => start_resize(state, tools),
        KeyboardCommand::Reorder(OrderDirection::Previous) => {
            move_cursor_layout(state, tools, MoveDir::Prev)
        }
        KeyboardCommand::Reorder(OrderDirection::Next) => {
            move_cursor_layout(state, tools, MoveDir::Next)
        }
        KeyboardCommand::NewBoard => open_board_editor_add(state),
        KeyboardCommand::RenameBoard => open_board_editor_rename(state),
        KeyboardCommand::DeleteBoard => open_confirm_delete_board(state),
        KeyboardCommand::EditPinColor => open_pin_color_editor(state),
        KeyboardCommand::OpenToolPicker => open_tool_picker(state),
        KeyboardCommand::Search => open_tool_search(state),
        _ => Effect::None,
    }
}

fn handle_detail_command(
    state: &mut State,
    command: KeyboardCommand,
    tools: &[&'static ToolMeta],
) -> Effect {
    match command {
        KeyboardCommand::Run => run_selected_tool(state, tools),
        KeyboardCommand::Close => back_one_level(state),
        KeyboardCommand::OpenSettings => open_settings(state),
        KeyboardCommand::StartMove => start_move(state, tools),
        // Detail scope does not bind `e`, so this arm is currently
        // unreachable via keys — kept for symmetry with StartMove.
        KeyboardCommand::StartResize => start_resize(state, tools),
        KeyboardCommand::Search => open_tool_search(state),
        KeyboardCommand::Copy => copy_detail_tool_id(state, tools),
        _ => Effect::None,
    }
}

/// `View::Result`'s own command handling, layered on top of the shared
/// Board-scope resolution (List and Result share a scope so Move/
/// Search/filter keys keep working uniformly). Returns `None` for
/// commands Result doesn't specially handle, so the caller falls
/// through to "stays visible, unconsumed".
fn handle_result_view_command(
    state: &mut State,
    command: KeyboardCommand,
    tools: &[&'static ToolMeta],
) -> Option<Effect> {
    match command {
        // Enter/F1/Space all resolve to Run in Board scope; re-running
        // the same tool goes through the same path a fresh Run would
        // (dispatch immediately, or reopen the Form for a tool with
        // required inputs since no prior args are cached on `View::Result`).
        KeyboardCommand::Run => Some(run_selected_tool(state, tools)),
        KeyboardCommand::Close | KeyboardCommand::Quit => Some(back_one_level(state)),
        KeyboardCommand::Copy => Some(copy_result_output(state)),
        _ => None,
    }
}

/// `View::Running`'s own command handling, layered on the shared Board
/// scope exactly like [`handle_result_view_command`]. Esc / q are the
/// whole contract: ask the in-flight dispatch to stop. Returns `None`
/// for everything else so the caller's swallow guard takes over.
///
/// A *second* Esc / q opens the confirm-quit overlay. Cancellation is
/// only a request, and an invoker that never polls the token would
/// otherwise leave the surface with no exit at all — every other key is
/// swallowed while a run is in flight. The escalation goes *through*
/// the modeless quit guard rather than past it: `q`/Esc at the board
/// root already opens `View::ConfirmQuit` precisely so a reflexive
/// double-tap cannot drop the session, and Esc-during-a-slow-run is
/// exactly that reflex. The escape hatch costs one deliberate `y`.
///
/// The escalation still answers `Effect::CancelRun`, so the stop request
/// is re-sent (cancelling is idempotent) and survives even when the user
/// then backs out of the quit overlay and `Effect::Quit` never fires.
fn handle_running_view_command(state: &mut State, command: KeyboardCommand) -> Option<Effect> {
    match command {
        KeyboardCommand::Close | KeyboardCommand::Quit => {
            let View::Running { cancelling, .. } = &mut state.view else {
                return None;
            };
            if *cancelling {
                // Honesty gap, accepted: `back_one_level` takes
                // ConfirmQuit to `View::List`, so backing out of the
                // overlay does not return to the running pane. The run
                // itself is untouched — it keeps going, `apply_progress`
                // simply stops accumulating (the tail is dropped), and
                // its `Msg::ToolDone` still lands as `View::Result`.
                let _ = open_confirm_quit(state);
                return Some(Effect::CancelRun);
            }
            // Cancellation is a request, not a guarantee: the pane says
            // so until the tool's final envelope actually arrives.
            *cancelling = true;
            Some(Effect::CancelRun)
        }
        _ => None,
    }
}

/// F2 in Detail copies the tool id — the one piece of manifest text a
/// user is likely to want on the clipboard (e.g. to paste into a CLI
/// invocation or a bug report) before ever running the tool. There is
/// no "output" to copy yet at this point, unlike Result.
fn copy_detail_tool_id(state: &State, tools: &[&'static ToolMeta]) -> Effect {
    let Some(tool) = tools.get(state.cursor) else {
        return Effect::None;
    };
    Effect::CopyToClipboard(tool.id.to_string())
}

/// F2 in Result copies the same precedence GUI's copy affordance uses
/// (`flutter_app/lib/src/pages/expanded_modal_page.dart::_copyTextForOutcome`):
/// the error message if the run failed, else the primary output's
/// display text, else a canonical JSON dump of every output.
fn copy_result_output(state: &State) -> Effect {
    let View::Result {
        tool_id,
        outputs,
        text,
        is_error,
    } = &state.view
    else {
        return Effect::None;
    };
    let candidate = if *is_error {
        non_empty_owned(text)
    } else {
        non_empty_owned(text)
            .or_else(|| primary_output_text(tool_id, outputs))
            .or_else(|| canonical_outputs_json(outputs))
    };
    candidate.map_or(Effect::None, Effect::CopyToClipboard)
}

fn non_empty_owned(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_string())
}

/// The tool's declared `primary_output_id` wins; absent that (or when
/// it doesn't match any entry), the first output stands in — same
/// fallback as the GUI's `CanonicalToolResultView.primaryOutput`.
fn primary_output_text(tool_id: &str, outputs: &[OutputEntry]) -> Option<String> {
    if outputs.is_empty() {
        return None;
    }
    let primary_id = upeg_runtime::toolbox_tool(tool_id).and_then(|meta| meta.primary_output_id);
    let entry = primary_id
        .and_then(|id| outputs.iter().find(|entry| entry.id == id))
        .or_else(|| outputs.first())?;
    non_empty_owned(&upeg_runtime::output_value_text(&entry.value))
}

fn canonical_outputs_json(outputs: &[OutputEntry]) -> Option<String> {
    if outputs.is_empty() {
        return None;
    }
    serde_json::to_string(&json!({ "outputs": outputs })).ok()
}

fn move_list_cursor(
    state: &mut State,
    direction: NavDirection,
    tools: &[&'static ToolMeta],
    area: Option<Rect>,
) {
    let num_tools = tools.len();
    match direction {
        NavDirection::Down => {
            if let Some(area) = area {
                let layout = tui_layout(area);
                state.cursor =
                    move_cursor_in_grid(tools, layout.left, state.cursor, GridDirection::Down);
                sync_grid_scroll_to_cursor(state, tools, layout.left);
            } else if num_tools > 0 {
                state.cursor = (state.cursor + 1).min(num_tools - 1);
            }
        }
        NavDirection::Up => {
            if let Some(area) = area {
                let layout = tui_layout(area);
                state.cursor =
                    move_cursor_in_grid(tools, layout.left, state.cursor, GridDirection::Up);
                sync_grid_scroll_to_cursor(state, tools, layout.left);
            } else {
                state.cursor = state.cursor.saturating_sub(1);
            }
        }
        NavDirection::Left => {
            if let Some(area) = area {
                let layout = tui_layout(area);
                state.cursor =
                    move_cursor_in_grid(tools, layout.left, state.cursor, GridDirection::Left);
                sync_grid_scroll_to_cursor(state, tools, layout.left);
            } else {
                state.cursor = state.cursor.saturating_sub(1);
            }
        }
        NavDirection::Right => {
            if let Some(area) = area {
                let layout = tui_layout(area);
                state.cursor =
                    move_cursor_in_grid(tools, layout.left, state.cursor, GridDirection::Right);
                sync_grid_scroll_to_cursor(state, tools, layout.left);
            } else if num_tools > 0 {
                state.cursor = (state.cursor + 1).min(num_tools - 1);
            }
        }
    }
}

fn back_one_level(state: &mut State) -> Effect {
    if matches!(state.view, View::Form { .. } | View::Result { .. })
        && let Some(frame) = state.presentation_frames.pop()
    {
        state.view = frame.view;
        state.result_inputs = frame.inputs;
        state.result_row = frame.selected_row;
        state.result_action = frame.selected_action;
        state.result_run = frame.result_run;
        state.refresh_after_tool = None;
        state.right_scroll = ScrollOffset::ZERO;
        return Effect::None;
    }
    state.view = match &state.view {
        View::List => return Effect::Quit,
        View::Detail => View::List,
        View::Form { .. } => View::List,
        View::Result { .. } => View::List,
        View::Settings { .. } => View::List,
        // Phase 6+ destructive overlays — ESC discards and returns to
        // the grid; the buffer / target gets dropped along with the
        // view variant.
        View::BoardEditor { .. }
        | View::ConfirmDeleteBoard { .. }
        | View::PinColorEditor(_)
        | View::ConfirmQuit
        | View::ConfirmApproval { .. }
        | View::Running { .. } => View::List,
        View::ToolPicker { return_to, .. } => {
            let return_to = *return_to;
            state.focus = return_to.focus;
            match return_to.view {
                ToolPickerReturnView::List => View::List,
                ToolPickerReturnView::Detail => View::Detail,
            }
        }
    };
    state.right_scroll = ScrollOffset::ZERO;
    Effect::None
}

/// Apply a dispatch `Outcome` (computed by the event loop) to the state.
/// Pure — separated from the loop so tests can simulate dispatch effects.
///
/// The final envelope is the end of the run by definition, so this is
/// also where [`State::active_run`] is released. Unconditionally: the
/// TUI runs one dispatch at a time (see [`refuse_second_run`]), and
/// leaving the flag set on an envelope that somehow did not match would
/// wedge the surface into "already running" for the rest of the session.
pub fn apply_outcome(state: &mut State, tool_id: &'static str, outcome: Outcome) -> Effect {
    apply_outcome_with_host(
        state,
        tool_id,
        outcome,
        super::model::PresentationHost::LocalTui,
    )
}
