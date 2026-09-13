use ratatui::layout::Rect;
use upeg_core::{
    KeyStroke, KeyboardCommand, KeyboardContext, KeyboardScope, NavDirection, PageDirection,
    resolve_key,
};

use super::grid::{
    BOARD_FILTER_PREFIX, BodyLayout, TAG_FILTER_PREFIX, TuiLayout, clamp_filter_bar_scroll,
    clamp_grid_h_scroll, clamp_grid_scroll, filter_bar_scroll_step, filter_option_at_bar_cell,
    scroll_filter_option_into_view, tui_layout,
};
use super::model::{
    BoardFilter, BodyPresentation, FocusArea, State, View, cycle_board_filter, cycle_tag_filter,
    filter_option_index, normalize_tag_filter_for_board_in,
};
#[cfg(test)]
use super::msg::Key;
use super::msg::ScrollDelta;
use super::scroll::{Horizontal, ScrollOffset};
use upeg_core::ToolMeta;

pub(crate) const RIGHT_PANE_SCROLL_STEP: u16 = 3;
const RIGHT_PANE_PAGE_SCROLL_STEP: u16 = 10;

/// Which body-row surfaces are actually rendered for the current
/// `(layout, view)` pair. Enumerating the four legal shapes — rather
/// than carrying two booleans — makes the dialog case (`Neither`)
/// impossible to mistake for a body case, and rules out the illegal
/// `(grid=false, right=false)` outside of dialog.
///
/// Single source of truth so render (view.rs), mouse routing
/// (update/mouse.rs), and focus reconciliation (clamp_state_to_area +
/// next_focus) can never drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisiblePanes {
    /// Wide non-dialog: both grid and right pane render.
    Both,
    /// Narrow + Board-dominant view: only the grid renders in the body.
    BoardOnly,
    /// Narrow + RightPane-dominant view: only the right pane renders.
    RightPaneOnly,
    /// Dialog view: neither body sub-surface renders. Filter bars stay
    /// visible regardless.
    Neither,
}

impl VisiblePanes {
    /// Filter bars always render; body sub-surfaces follow the variant.
    pub(crate) const fn focus_visible(self, focus: FocusArea) -> bool {
        use FocusArea::{Boards, Grid, RightPane, Tags};
        matches!(
            (self, focus),
            (_, Boards | Tags)
                | (Self::Both | Self::BoardOnly, Grid)
                | (Self::Both | Self::RightPaneOnly, RightPane)
        )
    }

    /// Repair target when current focus has folded out, or `None` if
    /// no body surface is currently rendered (dialog). The clamp pass
    /// uses `None` to leave focus alone — the next non-dialog frame
    /// will sort it without an arbitrary fallback.
    pub(crate) const fn preferred_body_focus(self) -> Option<FocusArea> {
        match self {
            Self::Both | Self::RightPaneOnly => Some(FocusArea::RightPane),
            Self::BoardOnly => Some(FocusArea::Grid),
            Self::Neither => None,
        }
    }
}

pub(crate) fn visible_panes(layout: &TuiLayout, view: &View) -> VisiblePanes {
    match view.body_presentation() {
        BodyPresentation::Dialog => VisiblePanes::Neither,
        BodyPresentation::Surfaces(dom) => match layout.body_layout(dom) {
            BodyLayout::Dual { .. } => VisiblePanes::Both,
            BodyLayout::BoardOnly(_) => VisiblePanes::BoardOnly,
            BodyLayout::RightPaneOnly(_) => VisiblePanes::RightPaneOnly,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilterBar {
    Board,
    Tag,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FilterBarClick<'a> {
    pub(crate) area: Rect,
    pub(crate) prefix: &'a str,
    pub(crate) options: &'a [String],
    pub(crate) scroll: ScrollOffset<Horizontal>,
    pub(crate) column: u16,
    pub(crate) row: u16,
}

#[cfg(test)]
pub(crate) fn apply_filter_key(state: &mut State, key: Key) -> bool {
    let context = KeyboardContext {
        scope: KeyboardScope::Board,
        has_tool_focus: true,
    };
    let Some(command) = resolve_key(context, key.into()) else {
        return false;
    };
    apply_filter_command(state, command)
}

pub(crate) fn apply_filter_command(state: &mut State, command: KeyboardCommand) -> bool {
    match command {
        KeyboardCommand::CycleBoardFilter => {
            let board_opts = state.board_filter_options();
            state.filters.board = cycle_board_filter(&state.filters.board, &board_opts);
            normalize_tag_filter_for_board_in(&mut state.filters, &state.boards, &state.layouts);
            state.board_cursor = filter_option_index(state.filters.board.as_deref(), &board_opts);
        }
        KeyboardCommand::CycleTagFilter => {
            let tag_options = state.tag_filter_options();
            state.filters.tag = cycle_tag_filter(&state.filters.tag, &tag_options);
            state.tag_cursor = filter_option_index(state.filters.tag.as_deref(), &tag_options);
        }
        KeyboardCommand::ClearBoardFilter => {
            state.filters.board = BoardFilter::All;
            normalize_tag_filter_for_board_in(&mut state.filters, &state.boards, &state.layouts);
            state.board_cursor = 0;
        }
        KeyboardCommand::SwitchBoard(slot) => {
            let digit = char::from(b'0' + slot.as_u8());
            let Some(board_filter) = state.board_for_digit(digit) else {
                return false;
            };
            state.filters.board = board_filter;
            normalize_tag_filter_for_board_in(&mut state.filters, &state.boards, &state.layouts);
            let board_opts = state.board_filter_options();
            state.board_cursor = filter_option_index(state.filters.board.as_deref(), &board_opts);
        }
        _ => return false,
    }

    reset_after_filter_change(state);
    true
}

pub(crate) fn apply_filter_option(state: &mut State, bar: FilterBar, option: &str) {
    match bar {
        FilterBar::Board => {
            state.filters.select_board(option);
            normalize_tag_filter_for_board_in(&mut state.filters, &state.boards, &state.layouts);
            let board_opts = state.board_filter_options();
            state.board_cursor = filter_option_index(state.filters.board.as_deref(), &board_opts);
        }
        FilterBar::Tag => {
            state.filters.select_tag(option);
            let tag_options = state.tag_filter_options();
            state.tag_cursor = filter_option_index(state.filters.tag.as_deref(), &tag_options);
        }
    }
    reset_after_filter_change(state);
}

pub(crate) fn apply_filter_click(
    state: &mut State,
    bar: FilterBar,
    click: FilterBarClick<'_>,
) -> bool {
    let Some(option) = filter_option_at_bar_cell(
        click.area,
        click.prefix,
        click.options,
        click.scroll,
        click.column,
        click.row,
    ) else {
        return false;
    };
    state.focus = bar.focus_area();
    apply_filter_option(state, bar, option);
    true
}

pub(crate) fn apply_filter_scroll(
    state: &mut State,
    bar: FilterBar,
    area: Rect,
    prefix: &str,
    options: &[String],
    delta: ScrollDelta,
) -> bool {
    let current = match bar {
        FilterBar::Board => &mut state.board_scroll,
        FilterBar::Tag => &mut state.tag_scroll,
    };
    let step = filter_bar_scroll_step();
    // The filter bar is a horizontal scroll container, so both axes
    // route to the same scroll: vertical wheel moves the bar's content,
    // horizontal wheel does too. The split exists so a horizontal-wheel
    // terminal feels native; semantically it's one axis.
    let next = if delta.forward {
        current.add_clamped(step, u16::MAX)
    } else {
        current.sub_saturating(step)
    };
    *current = clamp_filter_bar_scroll(area, prefix, options, next);
    true
}

pub(crate) fn handle_focus_command(
    state: &mut State,
    command: KeyboardCommand,
    area: Option<Rect>,
) -> bool {
    // Focus movement consults the same VisiblePanes truth used by
    // render and clamp_state_to_area, so the cursor skips folded
    // surfaces in narrow mode. Without `area` we preserve wide-mode
    // behavior.
    let panes = area.map_or(VisiblePanes::Both, |a| {
        visible_panes(&tui_layout(a), &state.view)
    });
    match command {
        KeyboardCommand::FocusNext => {
            state.focus = next_focus(state.focus, panes);
            ensure_focused_filter_visible(state, area);
            true
        }
        KeyboardCommand::FocusPrevious => {
            state.focus = previous_focus(state.focus, panes);
            ensure_focused_filter_visible(state, area);
            true
        }
        KeyboardCommand::Move(NavDirection::Down) if state.focus == FocusArea::Boards => {
            state.focus = FocusArea::Tags;
            ensure_focused_filter_visible(state, area);
            true
        }
        KeyboardCommand::Move(NavDirection::Up) if state.focus == FocusArea::Tags => {
            state.focus = FocusArea::Boards;
            ensure_focused_filter_visible(state, area);
            true
        }
        KeyboardCommand::Move(NavDirection::Down) if state.focus == FocusArea::Tags => {
            if panes.focus_visible(FocusArea::Grid) {
                state.focus = FocusArea::Grid;
                true
            } else if let Some(focus) = panes.preferred_body_focus() {
                state.focus = focus;
                true
            } else {
                false
            }
        }
        KeyboardCommand::Move(NavDirection::Left) if state.focus == FocusArea::RightPane => {
            if panes.focus_visible(FocusArea::Grid) {
                state.focus = FocusArea::Grid;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

pub(crate) fn handle_focused_control_key(
    state: &mut State,
    stroke: KeyStroke,
    area: Option<Rect>,
) -> bool {
    if let Some(bar) = state.focus.as_filter_bar() {
        let context = KeyboardContext::new(KeyboardScope::FilterBar);
        let Some(command) = resolve_key(context, stroke) else {
            return false;
        };
        return handle_filter_focus_command(state, command, area, bar);
    }
    if state.focus == FocusArea::RightPane {
        let context = KeyboardContext::new(KeyboardScope::RightPane);
        let Some(command) = resolve_key(context, stroke) else {
            return false;
        };
        return handle_right_pane_command(state, command);
    }
    false
}

/// Clamp every scroll axis in [`State`] against the current viewport.
///
/// The TUI render loop calls this once per frame so a terminal resize
/// (or any post-load state that carries scroll values from a wider
/// viewport) can't leave scrolls past their new maxima. Exposing the
/// per-frame setup as one function gives [`super::effects`] a single
/// call and lets unit tests pin the resize-regression behavior without
/// spinning a real terminal.
pub(crate) fn clamp_state_to_area(state: &mut State, tools: &[&'static ToolMeta], area: Rect) {
    let layout = tui_layout(area);
    state.grid_scroll = clamp_grid_scroll(tools, layout.left, state.grid_scroll);
    state.grid_h_scroll = clamp_grid_h_scroll(layout.left, state.grid_h_scroll);

    let board_options = state.board_filter_options();
    state.board_scroll = clamp_filter_bar_scroll(
        layout.boards,
        BOARD_FILTER_PREFIX,
        &board_options,
        state.board_scroll,
    );
    let tag_options = state.tag_filter_options();
    state.tag_scroll = clamp_filter_bar_scroll(
        layout.tags,
        TAG_FILTER_PREFIX,
        &tag_options,
        state.tag_scroll,
    );
    state.clamp_filter_cursors(board_options.len(), tag_options.len());

    // Narrow-mode focus repair: a wide-to-narrow resize (or a view
    // transition that flips which surface dominates) can leave focus
    // on a folded pane, where it silently swallows arrow keys against
    // nothing rendered. Reconcile to the visible body surface. In a
    // dialog `preferred_body_focus` is `None` — focus stays put so the
    // user's previous body focus is restored verbatim on Esc.
    let panes = visible_panes(&layout, &state.view);
    if !panes.focus_visible(state.focus)
        && let Some(repair) = panes.preferred_body_focus()
    {
        state.focus = repair;
    }
}

pub(crate) fn ensure_focused_filter_visible(state: &mut State, area: Option<Rect>) {
    if let Some(bar) = state.focus.as_filter_bar() {
        let options = bar.options(state);
        ensure_filter_cursor_visible_in_bar(state, area, bar, &options);
    }
}

fn reset_after_filter_change(state: &mut State) {
    state.cursor = 0;
    state.grid_scroll = ScrollOffset::ZERO;
    state.right_scroll = ScrollOffset::ZERO;
    state.view = View::List;
}

// FilterBar inherent methods. Each `match self` lives in exactly one
// place so adding a third filter bar means touching this impl block
// alone instead of nine free functions.
impl FilterBar {
    pub(crate) const fn focus_area(self) -> FocusArea {
        match self {
            Self::Board => FocusArea::Boards,
            Self::Tag => FocusArea::Tags,
        }
    }

    pub(crate) const fn prefix(self) -> &'static str {
        match self {
            Self::Board => BOARD_FILTER_PREFIX,
            Self::Tag => TAG_FILTER_PREFIX,
        }
    }

    pub(crate) fn options(self, state: &State) -> Vec<String> {
        match self {
            Self::Board => state.board_filter_options(),
            Self::Tag => state.tag_filter_options(),
        }
    }

    pub(crate) fn area_in(self, full: Rect) -> Rect {
        let layout = tui_layout(full);
        match self {
            Self::Board => layout.boards,
            Self::Tag => layout.tags,
        }
    }

    pub(crate) const fn cursor(self, state: &State) -> usize {
        match self {
            Self::Board => state.board_cursor,
            Self::Tag => state.tag_cursor,
        }
    }

    pub(crate) const fn set_cursor(self, state: &mut State, cursor: usize) {
        match self {
            Self::Board => state.board_cursor = cursor,
            Self::Tag => state.tag_cursor = cursor,
        }
    }

    pub(crate) const fn scroll(self, state: &State) -> ScrollOffset<Horizontal> {
        match self {
            Self::Board => state.board_scroll,
            Self::Tag => state.tag_scroll,
        }
    }

    pub(crate) const fn set_scroll(self, state: &mut State, scroll: ScrollOffset<Horizontal>) {
        match self {
            Self::Board => state.board_scroll = scroll,
            Self::Tag => state.tag_scroll = scroll,
        }
    }
}

impl FocusArea {
    pub(crate) const fn as_filter_bar(self) -> Option<FilterBar> {
        match self {
            Self::Boards => Some(FilterBar::Board),
            Self::Tags => Some(FilterBar::Tag),
            Self::Grid | Self::RightPane => None,
        }
    }
}

const fn focus_step_forward(focus: FocusArea) -> FocusArea {
    match focus {
        FocusArea::Grid => FocusArea::Boards,
        FocusArea::Boards => FocusArea::Tags,
        FocusArea::Tags => FocusArea::RightPane,
        FocusArea::RightPane => FocusArea::Grid,
    }
}

const fn focus_step_backward(focus: FocusArea) -> FocusArea {
    match focus {
        FocusArea::Grid => FocusArea::RightPane,
        FocusArea::RightPane => FocusArea::Tags,
        FocusArea::Tags => FocusArea::Boards,
        FocusArea::Boards => FocusArea::Grid,
    }
}

/// Walk the focus cycle one direction (forward or backward) until a
/// visible focus is found, or return the input `focus` after one full
/// cycle if nothing else is visible. Termination is bounded by the
/// cycle returning to `focus`; no magic count, no hard-coded variant
/// count to drift if `FocusArea` grows.
fn step_until_visible(
    focus: FocusArea,
    panes: VisiblePanes,
    step: fn(FocusArea) -> FocusArea,
) -> FocusArea {
    let mut cursor = step(focus);
    while cursor != focus {
        if panes.focus_visible(cursor) {
            return cursor;
        }
        cursor = step(cursor);
    }
    focus
}

fn next_focus(focus: FocusArea, panes: VisiblePanes) -> FocusArea {
    step_until_visible(focus, panes, focus_step_forward)
}

fn previous_focus(focus: FocusArea, panes: VisiblePanes) -> FocusArea {
    step_until_visible(focus, panes, focus_step_backward)
}

fn handle_filter_focus_command(
    state: &mut State,
    command: KeyboardCommand,
    area: Option<Rect>,
    bar: FilterBar,
) -> bool {
    let options = bar.options(state);
    if options.is_empty() {
        return false;
    }

    let current = bar.cursor(state).min(options.len().saturating_sub(1));
    let next = match command {
        KeyboardCommand::Move(NavDirection::Left) => current.saturating_sub(1),
        KeyboardCommand::Move(NavDirection::Right) => current
            .saturating_add(1)
            .min(options.len().saturating_sub(1)),
        KeyboardCommand::Home => 0,
        KeyboardCommand::End => options.len().saturating_sub(1),
        KeyboardCommand::Open => {
            if let Some(option) = options.get(current) {
                apply_filter_option(state, bar, option);
                ensure_focused_filter_visible(state, area);
                return true;
            }
            return false;
        }
        _ => return false,
    };

    bar.set_cursor(state, next);
    ensure_filter_cursor_visible_in_bar(state, area, bar, &options);
    true
}

const fn handle_right_pane_command(state: &mut State, command: KeyboardCommand) -> bool {
    match command {
        KeyboardCommand::Move(NavDirection::Down) => {
            state.right_scroll = state
                .right_scroll
                .add_clamped(RIGHT_PANE_SCROLL_STEP, u16::MAX);
            true
        }
        KeyboardCommand::Move(NavDirection::Up) => {
            state.right_scroll = state.right_scroll.sub_saturating(RIGHT_PANE_SCROLL_STEP);
            true
        }
        KeyboardCommand::Page(PageDirection::Down) => {
            state.right_scroll = state
                .right_scroll
                .add_clamped(RIGHT_PANE_PAGE_SCROLL_STEP, u16::MAX);
            true
        }
        KeyboardCommand::Page(PageDirection::Up) => {
            state.right_scroll = state
                .right_scroll
                .sub_saturating(RIGHT_PANE_PAGE_SCROLL_STEP);
            true
        }
        KeyboardCommand::Home => {
            state.right_scroll = ScrollOffset::ZERO;
            true
        }
        KeyboardCommand::End => {
            state.right_scroll = ScrollOffset::new(u16::MAX);
            true
        }
        _ => false,
    }
}

fn ensure_filter_cursor_visible_in_bar(
    state: &mut State,
    area: Option<Rect>,
    bar: FilterBar,
    options: &[String],
) {
    let Some(area) = area else {
        return;
    };
    let next = scroll_filter_option_into_view(
        bar.area_in(area),
        bar.prefix(),
        options,
        bar.cursor(state),
        bar.scroll(state),
    );
    bar.set_scroll(state, next);
}
