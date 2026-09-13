use ratatui::layout::Rect;
use upeg_core::ToolMeta;

use super::super::controls::{
    FilterBar, FilterBarClick, RIGHT_PANE_SCROLL_STEP, apply_filter_click, apply_filter_scroll,
};
use super::super::effects::Effect;
use super::super::grid::{
    BOARD_FILTER_PREFIX, TAG_FILTER_PREFIX, clamp_grid_h_scroll, clamp_grid_scroll,
    grid_h_scroll_step, grid_max_h_scroll, grid_max_scroll, grid_scroll_step, rect_contains,
    tool_index_at_grid_cell, tui_layout,
};
use super::super::model::{BodyPresentation, FocusArea, State, View};
use super::super::msg::{Key, Mouse, MouseKind, PointerPhase, ScrollAxis, ScrollDelta};
use super::super::scroll::ScrollOffset;
use super::{handle_key, run_selected_tool};

pub fn handle_mouse(
    state: &mut State,
    mouse: Mouse,
    tools: &[&'static ToolMeta],
    area: Rect,
) -> Effect {
    let layout = tui_layout(area);
    match mouse.kind {
        MouseKind::Scroll(delta) => {
            route_scroll(state, tools, &layout, mouse.column, mouse.row, delta);
            return Effect::None;
        }
        MouseKind::Pointer(PointerPhase::Down) => {}
        MouseKind::Pointer(PointerPhase::Drag | PointerPhase::Up) => return Effect::None,
    }

    // The keyboard router intercepts every key while `move_mode` or
    // `resize_mode` is set (see `handle_key_with_area`). The mouse path
    // used to bypass that intercept and silently change board/tag/
    // cursor/view, which left the keyboard listening for `Enter`
    // against a hidden preview on the *previous* board. Any meaningful
    // Down click ends both previews first so mouse and keyboard share
    // one modal-state.
    state.move_mode = None;
    state.resize_mode = None;

    if rect_contains(layout.boards, mouse.column, mouse.row) {
        state.focus = FocusArea::Boards;
        let board_options = state.board_filter_options();
        let board_scroll = state.board_scroll;
        if apply_filter_click(
            state,
            FilterBar::Board,
            FilterBarClick {
                area: layout.boards,
                prefix: BOARD_FILTER_PREFIX,
                options: &board_options,
                scroll: board_scroll,
                column: mouse.column,
                row: mouse.row,
            },
        ) {
            return Effect::SavePegboardSelection;
        }
        return Effect::None;
    }

    if rect_contains(layout.tags, mouse.column, mouse.row) {
        state.focus = FocusArea::Tags;
        let tag_options = state.tag_filter_options();
        let tag_scroll = state.tag_scroll;
        if apply_filter_click(
            state,
            FilterBar::Tag,
            FilterBarClick {
                area: layout.tags,
                prefix: TAG_FILTER_PREFIX,
                options: &tag_options,
                scroll: tag_scroll,
                column: mouse.column,
                row: mouse.row,
            },
        ) {
            return Effect::SavePegboardSelection;
        }
        return Effect::None;
    }

    // Body routing must match what `view.rs` actually rendered. The
    // BodyPresentation match owns the decision: Dialog views ignore
    // body clicks (their renderers handle keyboard input); body views
    // hit-test only the rectangles `body_layout` actually returns.
    // Adding a View variant forces `body_presentation()` to declare
    // its presentation, so a new variant cannot silently fall through
    // to grid hit-testing.
    let dom = match state.view.body_presentation() {
        BodyPresentation::Dialog => return Effect::None,
        BodyPresentation::Surfaces(dom) => dom,
    };
    let body = layout.body_layout(dom);

    if let Some(grid) = body.grid_rect()
        && rect_contains(grid, mouse.column, mouse.row)
    {
        state.focus = FocusArea::Grid;
        if let Some(idx) = tool_index_at_grid_cell(
            tools,
            grid,
            state.grid_scroll,
            state.grid_h_scroll,
            mouse.column,
            mouse.row,
        ) {
            state.cursor = idx.min(tools.len().saturating_sub(1));
            state.right_scroll = ScrollOffset::ZERO;
            state.view = View::Detail;
        }
        return Effect::None;
    }

    if let Some(right) = body.right_rect()
        && rect_contains(right, mouse.column, mouse.row)
    {
        state.focus = FocusArea::RightPane;
        let button_row = right.y.saturating_add(right.height.saturating_sub(2));
        let f1_start = right.x.saturating_add(2);
        let f1_end = f1_start.saturating_add(10);
        if mouse.row == button_row && mouse.column >= f1_start && mouse.column < f1_end {
            // Dialog views were filtered out at the BodyPresentation
            // match above so this match only handles body views; the
            // dialog arms are dead at runtime but kept explicit so a
            // new View variant must declare its F1-click behavior at
            // compile time.
            return match &state.view {
                View::Form { .. } => handle_key(state, Key::F(1), tools),
                // A run already owns the surface — the keyboard swallows
                // every non-cancel key while it is in flight, and the
                // F1 button must not be the one path that starts a
                // second dispatch on top of it.
                View::Running { .. } => Effect::None,
                View::List
                | View::Detail
                | View::Result { .. }
                | View::Settings { .. }
                | View::BoardEditor { .. }
                | View::ConfirmDeleteBoard { .. }
                | View::PinColorEditor(_)
                | View::ToolPicker { .. }
                | View::ConfirmQuit
                | View::ConfirmApproval { .. } => run_selected_tool(state, tools),
            };
        }
        if state.view == View::List {
            state.right_scroll = ScrollOffset::ZERO;
            state.view = View::Detail;
        }
    }

    Effect::None
}

/// Route a scroll wheel event to whichever pane sits under the cursor.
///
/// Filter bars are inherently horizontal containers, so both axes feed
/// the same `apply_filter_scroll` — the vertical wheel is the "natural"
/// gesture there. The grid responds per-axis. The right pane only knows
/// vertical scroll.
fn route_scroll(
    state: &mut State,
    tools: &[&'static ToolMeta],
    layout: &super::super::grid::TuiLayout,
    column: u16,
    row: u16,
    delta: ScrollDelta,
) {
    if rect_contains(layout.boards, column, row) {
        state.focus = FocusArea::Boards;
        let board_options = state.board_filter_options();
        apply_filter_scroll(
            state,
            FilterBar::Board,
            layout.boards,
            BOARD_FILTER_PREFIX,
            &board_options,
            delta,
        );
        return;
    }
    if rect_contains(layout.tags, column, row) {
        state.focus = FocusArea::Tags;
        let tag_options = state.tag_filter_options();
        apply_filter_scroll(
            state,
            FilterBar::Tag,
            layout.tags,
            TAG_FILTER_PREFIX,
            &tag_options,
            delta,
        );
        return;
    }
    // Body wheel routing mirrors click routing — single source for
    // which rectangles exist via `body_layout`. Dialog views ignore
    // the wheel; folded surfaces are absent from the returned shape.
    let dom = match state.view.body_presentation() {
        BodyPresentation::Dialog => return,
        BodyPresentation::Surfaces(dom) => dom,
    };
    let body = layout.body_layout(dom);
    if let Some(right) = body.right_rect()
        && rect_contains(right, column, row)
        && delta.axis == ScrollAxis::Vertical
    {
        state.focus = FocusArea::RightPane;
        state.right_scroll = if delta.forward {
            state
                .right_scroll
                .add_clamped(RIGHT_PANE_SCROLL_STEP, u16::MAX)
        } else {
            state.right_scroll.sub_saturating(RIGHT_PANE_SCROLL_STEP)
        };
        return;
    }
    if let Some(grid) = body.grid_rect()
        && rect_contains(grid, column, row)
    {
        state.focus = FocusArea::Grid;
        match delta.axis {
            ScrollAxis::Vertical => {
                state.grid_scroll = if delta.forward {
                    clamp_grid_scroll(
                        tools,
                        grid,
                        state.grid_scroll.add_clamped(grid_scroll_step(), u16::MAX),
                    )
                } else {
                    state
                        .grid_scroll
                        .sub_saturating(grid_scroll_step())
                        .clamp_to(grid_max_scroll(tools, grid))
                };
            }
            ScrollAxis::Horizontal => {
                state.grid_h_scroll = if delta.forward {
                    clamp_grid_h_scroll(
                        grid,
                        state
                            .grid_h_scroll
                            .add_clamped(grid_h_scroll_step(), u16::MAX),
                    )
                } else {
                    state
                        .grid_h_scroll
                        .sub_saturating(grid_h_scroll_step())
                        .clamp_to(grid_max_h_scroll(grid))
                };
            }
        }
    }
}
