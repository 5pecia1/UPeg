use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
};
use upeg_core::{BOARD_COLS, ToolMeta};

use super::model::BodyDominant;
use super::scroll::{Horizontal, ScrollOffset, Vertical};

const GRID_CELL_WIDTH: u16 = 18;
const GRID_CELL_HEIGHT: u16 = 5;
const GRID_GAP: u16 = 1;
const FILTER_BAR_SCROLL_STEP: u16 = 8;

pub(crate) const BOARD_FILTER_PREFIX: &str = " boards ";
pub(crate) const TAG_FILTER_PREFIX: &str = " tags ";

// Below this body width the right pane crowds the board out of its
// own surface — at 50 columns Min(28) would leave the board with 22.
// Mirror the GUI's overflow:auto + width:max-content invariant: the
// pegboard is the primary surface, so once we run out of room for both
// panes the right pane folds and the board (or, for Form/Result views,
// the right-pane content) takes the full body. See [[feature/layout]]
// and `view::render_with_context` for the swap logic.
pub(crate) const MIN_DUAL_PANE_WIDTH: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GridCell {
    pub(crate) index: usize,
    pub(crate) rect: Rect,
    pub(crate) col: usize,
    pub(crate) row: usize,
    pub(crate) col_span: usize,
    pub(crate) row_span: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TuiLayout {
    pub(crate) header: Rect,
    pub(crate) boards: Rect,
    pub(crate) tags: Rect,
    pub(crate) left: Rect,
    pub(crate) right: Rect,
    pub(crate) footer: Rect,
}

/// How the body row is split for non-dialog views, after applying
/// the narrow-mode swap. Variants enumerate the three legal shapes —
/// width-0 sentinels are not used. Render, mouse routing, and focus
/// reconciliation match exhaustively so a new shape (e.g., a 3-column
/// canvas) would force every consumer to opt in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyLayout {
    /// Wide terminal: both sub-surfaces render side by side.
    Dual { grid: Rect, right: Rect },
    /// Narrow + Board-dominant (List/Detail): pegboard takes the body,
    /// right pane is folded out.
    BoardOnly(Rect),
    /// Narrow + RightPane-dominant (Form/Result): right pane takes the
    /// body, pegboard is folded out.
    RightPaneOnly(Rect),
}

impl BodyLayout {
    /// Pegboard grid rectangle if it renders, `None` if folded out.
    /// Lets mouse routing hit-test without re-destructuring the enum
    /// at every call site.
    pub(crate) const fn grid_rect(self) -> Option<Rect> {
        match self {
            Self::Dual { grid, .. } | Self::BoardOnly(grid) => Some(grid),
            Self::RightPaneOnly(_) => None,
        }
    }

    /// Right-pane rectangle if it renders, `None` if folded out.
    pub(crate) const fn right_rect(self) -> Option<Rect> {
        match self {
            Self::Dual { right, .. } | Self::RightPaneOnly(right) => Some(right),
            Self::BoardOnly(_) => None,
        }
    }
}

impl TuiLayout {
    /// Body row is too narrow to host both sub-surfaces. Used by tests
    /// and as a quick predicate by callers that don't care about which
    /// surface wins — [`Self::body_layout`] returns the rectangle.
    pub(crate) const fn is_narrow(&self) -> bool {
        self.right.width == 0
    }

    /// Full body rect (left + right). Dialog renderers paint into this
    /// directly; non-dialog renderers go through [`Self::body_layout`].
    pub(crate) const fn body(&self) -> Rect {
        Rect {
            x: self.left.x,
            y: self.left.y,
            width: self.left.width.saturating_add(self.right.width),
            height: self.left.height,
        }
    }

    /// Resolve the body split for a non-dialog view. The caller
    /// supplies the View's dominant surface; this function picks
    /// `Dual` in wide mode and the matching `*Only` variant in narrow.
    /// Folded surfaces are simply absent from the returned shape —
    /// callers never see a zero-width rect.
    pub(crate) const fn body_layout(&self, dom: BodyDominant) -> BodyLayout {
        if self.is_narrow() {
            match dom {
                BodyDominant::Board => BodyLayout::BoardOnly(self.body()),
                BodyDominant::RightPane => BodyLayout::RightPaneOnly(self.body()),
            }
        } else {
            BodyLayout::Dual {
                grid: self.left,
                right: self.right,
            }
        }
    }
}

pub(crate) fn tui_layout(area: Rect) -> TuiLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let body_row = vertical[3];
    let (left, right) = if body_row.width < MIN_DUAL_PANE_WIDTH {
        let collapsed_right = Rect {
            x: body_row.x.saturating_add(body_row.width),
            y: body_row.y,
            width: 0,
            height: body_row.height,
        };
        (body_row, collapsed_right)
    } else {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Min(28)])
            .split(body_row);
        (body[0], body[1])
    };
    TuiLayout {
        header: vertical[0],
        boards: vertical[1],
        tags: vertical[2],
        left,
        right,
        footer: vertical[4],
    }
}

pub(crate) const fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub(crate) const GRID_COLS: usize = BOARD_COLS as usize;

pub(crate) const fn grid_canvas_width() -> u16 {
    // Canonical canvas width: BOARD_COLS cells separated by GRID_GAP.
    // The pegboard always renders at this width regardless of terminal
    // size — narrow terminals reveal only a slice and rely on
    // horizontal scroll (mirrors the GUI's overflow-x:auto wrapper).
    let cols = BOARD_COLS;
    cols * GRID_CELL_WIDTH + cols.saturating_sub(1) * GRID_GAP
}

pub(crate) const fn grid_content_area(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

pub(crate) fn filter_option_at_bar_cell<'a>(
    area: Rect,
    prefix: &str,
    options: &'a [String],
    scroll: ScrollOffset<Horizontal>,
    column: u16,
    row: u16,
) -> Option<&'a str> {
    let content = grid_content_area(area);
    let viewport_width = filter_bar_viewport_width(area, prefix, options);
    if !rect_contains(content, column, row) || row != content.y {
        return None;
    }
    let visible_column = column.saturating_sub(content.x);
    if visible_column >= viewport_width {
        return None;
    }

    let relative_column = visible_column.saturating_add(scroll.get());
    let mut option_start = text_width(prefix);
    for option in options {
        let label = format!(" {option} ");
        let option_end = option_start.saturating_add(text_width(&label));
        if relative_column >= option_start && relative_column < option_end {
            return Some(option.as_str());
        }
        option_start = option_end;
    }
    None
}

pub(crate) const fn filter_bar_scroll_step() -> u16 {
    FILTER_BAR_SCROLL_STEP
}

pub(crate) fn filter_bar_content_width(prefix: &str, options: &[String]) -> u16 {
    options.iter().fold(text_width(prefix), |width, option| {
        width.saturating_add(text_width(&format!(" {option} ")))
    })
}

pub(crate) const fn filter_bar_viewport_width(
    area: Rect,
    _prefix: &str,
    _options: &[String],
) -> u16 {
    let content = grid_content_area(area);
    if content.width == 0 {
        return 0;
    }
    content.width
}

pub(crate) fn filter_option_range(
    prefix: &str,
    options: &[String],
    index: usize,
) -> Option<(u16, u16)> {
    let mut option_start = text_width(prefix);
    for (option_index, option) in options.iter().enumerate() {
        let label = format!(" {option} ");
        let option_end = option_start.saturating_add(text_width(&label));
        if option_index == index {
            return Some((option_start, option_end));
        }
        option_start = option_end;
    }
    None
}

pub(crate) fn scroll_filter_option_into_view(
    area: Rect,
    prefix: &str,
    options: &[String],
    index: usize,
    scroll: ScrollOffset<Horizontal>,
) -> ScrollOffset<Horizontal> {
    let Some((start, end)) = filter_option_range(prefix, options, index) else {
        return clamp_filter_bar_scroll(area, prefix, options, scroll);
    };
    let viewport_width = filter_bar_viewport_width(area, prefix, options);
    if viewport_width == 0 {
        return ScrollOffset::ZERO;
    }
    let scroll_raw = scroll.get();
    let next_raw = if start < scroll_raw {
        start
    } else if end > scroll_raw.saturating_add(viewport_width) {
        end.saturating_sub(viewport_width)
    } else {
        scroll_raw
    };
    clamp_filter_bar_scroll(area, prefix, options, ScrollOffset::new(next_raw))
}

pub(crate) fn filter_bar_max_scroll(area: Rect, prefix: &str, options: &[String]) -> u16 {
    filter_bar_content_width(prefix, options)
        .saturating_sub(filter_bar_viewport_width(area, prefix, options))
}

pub(crate) fn clamp_filter_bar_scroll(
    area: Rect,
    prefix: &str,
    options: &[String],
    scroll: ScrollOffset<Horizontal>,
) -> ScrollOffset<Horizontal> {
    scroll.clamp_to(filter_bar_max_scroll(area, prefix, options))
}

pub(crate) const fn filter_bar_scrollbar_area(area: Rect) -> Option<Rect> {
    let content = grid_content_area(area);
    if content.width == 0 || content.height < 2 {
        return None;
    }
    Some(Rect {
        x: content.x,
        y: content.y.saturating_add(1),
        width: content.width,
        height: 1,
    })
}

pub(crate) const fn grid_scrollbar_area(area: Rect) -> Option<Rect> {
    if area.width == 0 || area.height <= 2 {
        return None;
    }
    Some(Rect {
        x: area.x.saturating_add(area.width.saturating_sub(1)),
        y: area.y.saturating_add(1),
        width: 1,
        height: area.height.saturating_sub(2),
    })
}

pub(crate) const fn grid_h_scrollbar_area(area: Rect) -> Option<Rect> {
    if area.height == 0 || area.width <= 2 {
        return None;
    }
    Some(Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width.saturating_sub(2),
        height: 1,
    })
}

fn text_width(text: &str) -> u16 {
    Span::raw(text).width().min(u16::MAX as usize) as u16
}

pub(crate) const fn grid_scroll_step() -> u16 {
    GRID_CELL_HEIGHT + GRID_GAP
}

pub(crate) const fn grid_h_scroll_step() -> u16 {
    GRID_CELL_WIDTH + GRID_GAP
}

/// Effective geometry of one visible placement, produced once per frame
/// from the canonical layout. `(x, y)` is the user-owned anchor; `(w, h)`
/// is `upeg_runtime::pegboard::effective_size` — span override wins,
/// manifest footprint otherwise — so runtime stays the single geometry
/// source and the grid never re-derives sizes from `pegboard_units`
/// when a placement exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlacementHint {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[cfg(test)]
impl PlacementHint {
    /// 1x1 hint for tests that only exercise the anchor coordinate.
    pub(crate) const fn unit(x: u16, y: u16) -> Self {
        Self { x, y, w: 1, h: 1 }
    }
}

// Thread-local cache for the current frame's placement hints. The TUI
// render loop refreshes it once per frame via [`set_placement_hints`];
// geometry helpers consult it without every caller having to thread an
// extra parameter through the entire pure-update path. Safe because
// the TUI is single-threaded.
thread_local! {
    static PLACEMENT_HINTS: std::cell::RefCell<Vec<Option<PlacementHint>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn set_placement_hints(hints: Vec<Option<PlacementHint>>) {
    PLACEMENT_HINTS.with(|cell| *cell.borrow_mut() = hints);
}

/// RAII guard for [`PLACEMENT_HINTS`]. Set hints on construction, clear
/// on drop — including panic unwind. Tests use this to avoid leaking
/// thread-local state into siblings.
#[cfg(test)]
pub(crate) struct PlacementHintsGuard;

#[cfg(test)]
impl PlacementHintsGuard {
    pub(crate) fn set(hints: Vec<Option<PlacementHint>>) -> Self {
        set_placement_hints(hints);
        Self
    }
}

#[cfg(test)]
impl Drop for PlacementHintsGuard {
    fn drop(&mut self) {
        set_placement_hints(vec![]);
    }
}

fn placement_hint(index: usize) -> Option<PlacementHint> {
    PLACEMENT_HINTS.with(|cell| cell.borrow().get(index).copied().flatten())
}

pub(crate) fn pegboard_cells(tools: &[&'static ToolMeta], area: Rect) -> Vec<GridCell> {
    pegboard_cells_inner(tools, area)
}

fn pegboard_cells_inner(tools: &[&'static ToolMeta], area: Rect) -> Vec<GridCell> {
    if tools.is_empty() {
        return Vec::new();
    }
    let content = grid_content_area(area);
    let cols = GRID_COLS;
    // Canonical pegboard is always BOARD_COLS wide; stored (x, y) hints
    // are honored verbatim so every surface places the same tool in the
    // same column. Narrow terminals slice the canvas via grid_h_scroll.
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut cells = Vec::with_capacity(tools.len());

    for (index, tool) in tools.iter().enumerate() {
        let hint = placement_hint(index);
        // Hinted placements carry their effective size (span override
        // included); the manifest `pegboard_units` footprint only backs
        // tools rendered without a placement.
        let (declared_cols, declared_rows) = match hint {
            Some(hint) => (hint.w, hint.h),
            None => tool.pegboard_units.grid_span(),
        };
        let col_span = (declared_cols as usize).clamp(1, cols);
        let row_span = (declared_rows as usize).max(1);
        let (col, row) = match hint {
            Some(hint) => {
                let max_col = cols.saturating_sub(col_span);
                let target_col = (hint.x as usize).min(max_col);
                let target_row = hint.y as usize;
                if slot_is_open(&occupied, target_col, target_row, col_span, row_span, cols) {
                    (target_col, target_row)
                } else {
                    first_open_slot(&occupied, cols, col_span, row_span)
                }
            }
            None => first_open_slot(&occupied, cols, col_span, row_span),
        };
        mark_occupied(&mut occupied, col, row, col_span, row_span, cols);

        let x = content
            .x
            .saturating_add((col as u16) * (GRID_CELL_WIDTH + GRID_GAP));
        let y = content
            .y
            .saturating_add((row as u16) * (GRID_CELL_HEIGHT + GRID_GAP));
        let width = (col_span as u16)
            .saturating_mul(GRID_CELL_WIDTH)
            .saturating_add((col_span.saturating_sub(1) as u16).saturating_mul(GRID_GAP));
        let height = (row_span as u16)
            .saturating_mul(GRID_CELL_HEIGHT)
            .saturating_add((row_span.saturating_sub(1) as u16).saturating_mul(GRID_GAP));

        cells.push(GridCell {
            index,
            rect: Rect {
                x,
                y,
                width,
                height,
            },
            col,
            row,
            col_span,
            row_span,
        });
    }

    cells
}

pub(crate) fn grid_max_scroll(tools: &[&'static ToolMeta], area: Rect) -> u16 {
    let content = grid_content_area(area);
    if content.height == 0 {
        return 0;
    }
    pegboard_cells_inner(tools, area)
        .iter()
        .map(|cell| {
            cell.rect
                .y
                .saturating_sub(content.y)
                .saturating_add(cell.rect.height)
        })
        .max()
        .unwrap_or(0)
        .saturating_sub(content.height)
}

pub(crate) fn clamp_grid_scroll(
    tools: &[&'static ToolMeta],
    area: Rect,
    scroll: ScrollOffset<Vertical>,
) -> ScrollOffset<Vertical> {
    scroll.clamp_to(grid_max_scroll(tools, area))
}

pub(crate) fn grid_max_h_scroll(area: Rect) -> u16 {
    let content = grid_content_area(area);
    grid_canvas_width().saturating_sub(content.width)
}

pub(crate) fn clamp_grid_h_scroll(
    area: Rect,
    scroll: ScrollOffset<Horizontal>,
) -> ScrollOffset<Horizontal> {
    scroll.clamp_to(grid_max_h_scroll(area))
}

pub(crate) fn ensure_grid_cursor_visible(
    tools: &[&'static ToolMeta],
    area: Rect,
    cursor: usize,
    scroll: ScrollOffset<Vertical>,
) -> ScrollOffset<Vertical> {
    let content = grid_content_area(area);
    if content.height == 0 {
        return ScrollOffset::ZERO;
    }
    let cells = pegboard_cells_inner(tools, area);
    let Some(cell) = cells.iter().find(|cell| cell.index == cursor) else {
        return clamp_grid_scroll(tools, area, scroll);
    };

    let scroll_raw = scroll.get();
    let top = cell.rect.y.saturating_sub(content.y);
    let bottom = top.saturating_add(cell.rect.height);
    let viewport_bottom = scroll_raw.saturating_add(content.height);
    let next_raw = if top < scroll_raw {
        top
    } else if bottom > viewport_bottom {
        if cell.rect.height >= content.height {
            top
        } else {
            bottom.saturating_sub(content.height)
        }
    } else {
        scroll_raw
    };
    clamp_grid_scroll(tools, area, ScrollOffset::new(next_raw))
}

pub(crate) fn ensure_grid_cursor_visible_h(
    tools: &[&'static ToolMeta],
    area: Rect,
    cursor: usize,
    scroll: ScrollOffset<Horizontal>,
) -> ScrollOffset<Horizontal> {
    let content = grid_content_area(area);
    if content.width == 0 {
        return ScrollOffset::ZERO;
    }
    let cells = pegboard_cells_inner(tools, area);
    let Some(cell) = cells.iter().find(|cell| cell.index == cursor) else {
        return clamp_grid_h_scroll(area, scroll);
    };

    let scroll_raw = scroll.get();
    let left = cell.rect.x.saturating_sub(content.x);
    let right = left.saturating_add(cell.rect.width);
    let viewport_right = scroll_raw.saturating_add(content.width);
    let next_raw = if left < scroll_raw {
        left
    } else if right > viewport_right {
        if cell.rect.width >= content.width {
            left
        } else {
            right.saturating_sub(content.width)
        }
    } else {
        scroll_raw
    };
    clamp_grid_h_scroll(area, ScrollOffset::new(next_raw))
}

pub(crate) fn grid_visible_rect(
    cell: GridCell,
    area: Rect,
    v_scroll: ScrollOffset<Vertical>,
    h_scroll: ScrollOffset<Horizontal>,
) -> Option<Rect> {
    let content = grid_content_area(area);
    if content.width == 0 || content.height == 0 {
        return None;
    }

    let v_scroll = v_scroll.get();
    let h_scroll = h_scroll.get();

    let top = cell.rect.y.saturating_sub(content.y);
    let bottom = top.saturating_add(cell.rect.height);
    let viewport_bottom = v_scroll.saturating_add(content.height);
    if bottom <= v_scroll || top >= viewport_bottom {
        return None;
    }

    let left = cell.rect.x.saturating_sub(content.x);
    let right = left.saturating_add(cell.rect.width);
    let viewport_right = h_scroll.saturating_add(content.width);
    if right <= h_scroll || left >= viewport_right {
        return None;
    }

    let hidden_above = v_scroll.saturating_sub(top);
    let visible_top = top.saturating_sub(v_scroll);
    let y = content.y.saturating_add(visible_top);
    let height = cell
        .rect
        .height
        .saturating_sub(hidden_above)
        .min(content.y.saturating_add(content.height).saturating_sub(y));
    if height == 0 {
        return None;
    }

    let hidden_left = h_scroll.saturating_sub(left);
    let visible_left = left.saturating_sub(h_scroll);
    let x = content.x.saturating_add(visible_left);
    let width = cell
        .rect
        .width
        .saturating_sub(hidden_left)
        .min(content.x.saturating_add(content.width).saturating_sub(x));
    if width == 0 {
        return None;
    }

    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

pub(crate) fn tool_index_at_grid_cell(
    tools: &[&'static ToolMeta],
    area: Rect,
    v_scroll: ScrollOffset<Vertical>,
    h_scroll: ScrollOffset<Horizontal>,
    column: u16,
    row: u16,
) -> Option<usize> {
    if !rect_contains(area, column, row) || tools.is_empty() {
        return None;
    }
    pegboard_cells_inner(tools, area)
        .into_iter()
        .find(|cell| {
            grid_visible_rect(*cell, area, v_scroll, h_scroll)
                .is_some_and(|rect| rect_contains(rect, column, row))
        })
        .map(|cell| cell.index)
}

pub(crate) fn move_cursor_in_grid(
    tools: &[&'static ToolMeta],
    area: Rect,
    cursor: usize,
    direction: GridDirection,
) -> usize {
    let cells = pegboard_cells_inner(tools, area);
    let Some(current) = cells.iter().find(|cell| cell.index == cursor) else {
        return cursor.min(tools.len().saturating_sub(1));
    };

    let candidate = primary_grid_candidate(&cells, *current, direction)
        .or_else(|| fallback_grid_candidate(&cells, *current, direction));

    candidate.map_or(cursor, |cell| cell.index)
}

fn primary_grid_candidate(
    cells: &[GridCell],
    current: GridCell,
    direction: GridDirection,
) -> Option<GridCell> {
    match direction {
        GridDirection::Left => cells
            .iter()
            .filter(|cell| row_ranges_overlap(**cell, current))
            .filter(|cell| cell.col + cell.col_span <= current.col)
            .max_by_key(|cell| (cell.col + cell.col_span, cell.row))
            .copied(),
        GridDirection::Right => cells
            .iter()
            .filter(|cell| row_ranges_overlap(**cell, current))
            .filter(|cell| cell.col >= current.col + current.col_span)
            .min_by_key(|cell| (cell.col, cell.row))
            .copied(),
        GridDirection::Up => cells
            .iter()
            .filter(|cell| col_ranges_overlap(**cell, current))
            .filter(|cell| cell.row + cell.row_span <= current.row)
            .max_by_key(|cell| (cell.row + cell.row_span, cell.col))
            .copied(),
        GridDirection::Down => cells
            .iter()
            .filter(|cell| col_ranges_overlap(**cell, current))
            .filter(|cell| cell.row >= current.row + current.row_span)
            .min_by_key(|cell| (cell.row, cell.col))
            .copied(),
    }
}

fn fallback_grid_candidate(
    cells: &[GridCell],
    current: GridCell,
    direction: GridDirection,
) -> Option<GridCell> {
    cells
        .iter()
        .filter(|cell| cell.index != current.index)
        .filter(|cell| cell_is_in_direction(**cell, current, direction))
        .min_by_key(|cell| fallback_grid_score(**cell, current, direction))
        .copied()
}

const fn cell_is_in_direction(cell: GridCell, current: GridCell, direction: GridDirection) -> bool {
    match direction {
        GridDirection::Left => cell.col + cell.col_span <= current.col,
        GridDirection::Right => cell.col >= current.col + current.col_span,
        GridDirection::Up => cell.row + cell.row_span <= current.row,
        GridDirection::Down => cell.row >= current.row + current.row_span,
    }
}

const fn fallback_grid_score(
    cell: GridCell,
    current: GridCell,
    direction: GridDirection,
) -> (usize, usize, usize, usize, usize) {
    match direction {
        GridDirection::Left => (
            current.col.saturating_sub(cell.col + cell.col_span),
            center_distance(cell_center_row2(cell), cell_center_row2(current)),
            current.col.saturating_sub(cell.col),
            cell.row,
            cell.index,
        ),
        GridDirection::Right => (
            cell.col.saturating_sub(current.col + current.col_span),
            center_distance(cell_center_row2(cell), cell_center_row2(current)),
            cell.col,
            cell.row,
            cell.index,
        ),
        GridDirection::Up => (
            current.row.saturating_sub(cell.row + cell.row_span),
            center_distance(cell_center_col2(cell), cell_center_col2(current)),
            current.row.saturating_sub(cell.row),
            cell.col,
            cell.index,
        ),
        GridDirection::Down => (
            cell.row.saturating_sub(current.row + current.row_span),
            center_distance(cell_center_col2(cell), cell_center_col2(current)),
            cell.row,
            cell.col,
            cell.index,
        ),
    }
}

const fn cell_center_col2(cell: GridCell) -> usize {
    cell.col.saturating_mul(2).saturating_add(cell.col_span)
}

const fn cell_center_row2(cell: GridCell) -> usize {
    cell.row.saturating_mul(2).saturating_add(cell.row_span)
}

const fn center_distance(a: usize, b: usize) -> usize {
    a.abs_diff(b)
}

fn first_open_slot(
    occupied: &[Vec<bool>],
    cols: usize,
    col_span: usize,
    row_span: usize,
) -> (usize, usize) {
    let mut row = 0;
    loop {
        for col in 0..=cols.saturating_sub(col_span) {
            if slot_is_open(occupied, col, row, col_span, row_span, cols) {
                return (col, row);
            }
        }
        row += 1;
    }
}

fn slot_is_open(
    occupied: &[Vec<bool>],
    col: usize,
    row: usize,
    col_span: usize,
    row_span: usize,
    cols: usize,
) -> bool {
    if col + col_span > cols {
        return false;
    }
    for r in row..row + row_span {
        if let Some(existing_row) = occupied.get(r) {
            for c in col..col + col_span {
                if existing_row.get(c).copied().unwrap_or(false) {
                    return false;
                }
            }
        }
    }
    true
}

fn mark_occupied(
    occupied: &mut Vec<Vec<bool>>,
    col: usize,
    row: usize,
    col_span: usize,
    row_span: usize,
    cols: usize,
) {
    while occupied.len() < row + row_span {
        occupied.push(vec![false; cols]);
    }
    for occupied_row in occupied.iter_mut().skip(row).take(row_span) {
        for cell in occupied_row.iter_mut().skip(col).take(col_span) {
            *cell = true;
        }
    }
}

const fn ranges_overlap(a_start: usize, a_span: usize, b_start: usize, b_span: usize) -> bool {
    a_start < b_start + b_span && b_start < a_start + a_span
}

const fn row_ranges_overlap(a: GridCell, b: GridCell) -> bool {
    ranges_overlap(a.row, a.row_span, b.row, b.row_span)
}

const fn col_ranges_overlap(a: GridCell, b: GridCell) -> bool {
    ranges_overlap(a.col, a.col_span, b.col, b.col_span)
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
