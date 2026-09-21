//! Direct unit tests for the geometry helpers. Update-path tests
//! cover the orchestration; these pin the math so an off-by-one in
//! clipping or scroll-clamping surfaces here, not three layers up.

use super::*;

/// Single-cell placement hint. For existing tests that only care about
/// `(x, y)`.
fn hint_1x1(x: u16, y: u16) -> Option<PlacementHint> {
    hint(x, y, 1, 1)
}

fn hint(x: u16, y: u16, w: u16, h: u16) -> Option<PlacementHint> {
    Some(PlacementHint { x, y, w, h })
}

fn cell(col: usize, row: usize, col_span: usize, row_span: usize) -> GridCell {
    let content_x = 1_u16;
    let content_y = 1_u16;
    let x = content_x.saturating_add(col as u16 * (GRID_CELL_WIDTH + GRID_GAP));
    let y = content_y.saturating_add(row as u16 * (GRID_CELL_HEIGHT + GRID_GAP));
    let width = (col_span as u16 * GRID_CELL_WIDTH)
        .saturating_add((col_span.saturating_sub(1) as u16) * GRID_GAP);
    let height = (row_span as u16 * GRID_CELL_HEIGHT)
        .saturating_add((row_span.saturating_sub(1) as u16) * GRID_GAP);
    GridCell {
        index: 0,
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
    }
}

#[test]
fn grid_canvas_width_is_six_cells_plus_five_gaps() {
    let expected = (BOARD_COLS * GRID_CELL_WIDTH) + (BOARD_COLS - 1) * GRID_GAP;
    assert_eq!(grid_canvas_width(), expected);
}

#[test]
fn grid_h_scroll_step_is_cell_width_plus_gap() {
    assert_eq!(grid_h_scroll_step(), GRID_CELL_WIDTH + GRID_GAP);
}

#[test]
fn grid_max_h_scroll_is_zero_when_canvas_fits_viewport() {
    let area = Rect::new(0, 0, grid_canvas_width().saturating_add(4), 30);
    assert_eq!(grid_max_h_scroll(area), 0);
}

#[test]
fn grid_max_h_scroll_is_zero_at_exact_canvas_width() {
    let area = Rect::new(0, 0, grid_canvas_width().saturating_add(2), 30);
    assert_eq!(grid_max_h_scroll(area), 0);
}

#[test]
fn grid_max_h_scroll_grows_positive_as_viewport_narrows() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    assert_eq!(
        grid_max_h_scroll(area),
        grid_canvas_width().saturating_sub(content.width)
    );
}

#[test]
fn clamp_grid_h_scroll_never_exceeds_max() {
    let area = Rect::new(0, 0, 60, 20);
    let max = grid_max_h_scroll(area);
    assert_eq!(clamp_grid_h_scroll(area, ScrollOffset::new(0)).get(), 0);
    assert_eq!(
        clamp_grid_h_scroll(area, ScrollOffset::new(max / 2)).get(),
        max / 2
    );
    assert_eq!(clamp_grid_h_scroll(area, ScrollOffset::new(max)).get(), max);
    assert_eq!(
        clamp_grid_h_scroll(area, ScrollOffset::new(max.saturating_add(50))).get(),
        max
    );
}

#[test]
fn grid_h_scrollbar_area_is_single_bottom_row() {
    let area = Rect::new(0, 0, 60, 20);
    let bar = grid_h_scrollbar_area(area).expect("valid area");
    assert_eq!(bar.height, 1);
    assert_eq!(bar.width, area.width.saturating_sub(2));
    assert_eq!(bar.y, area.y + area.height - 1);
}

#[test]
fn grid_h_scrollbar_area_is_none_when_too_narrow() {
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 2, 5)).is_none());
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 1, 5)).is_none());
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 30, 0)).is_none());
}

#[test]
fn ensure_grid_cursor_visible_h_scrolls_when_cell_right_of_viewport() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(5, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    assert!(
        scroll > 0,
        "a cell at the right edge must produce positive scroll"
    );
    let cells = pegboard_cells_inner(&tools, area);
    let right = cells[0].rect.x - content.x + cells[0].rect.width;
    assert_eq!(scroll.get(), right.saturating_sub(content.width));
}

#[test]
fn ensure_grid_cursor_visible_h_scrolls_back_left_when_cell_left_of_viewport() {
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(0, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(50));
    assert_eq!(
        scroll.get(),
        0,
        "a cell at the left edge must return scroll to 0"
    );
}

#[test]
fn ensure_grid_cursor_visible_h_leaves_visible_cell_alone() {
    let area = Rect::new(0, 0, 200, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(2, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    assert_eq!(
        scroll.get(),
        0,
        "an already-visible cell must not change scroll"
    );
}

#[test]
fn ensure_grid_cursor_visible_h_shows_left_edge_when_cell_wider_than_viewport() {
    // Cell width 18 + gap 1 = 19, so a span-2 cell is 37 wide. Making
    // content.width narrower (area.width=20, content.width=18) puts the
    // cell wider than the viewport.
    let area = Rect::new(0, 0, 20, 20);
    let _guard = PlacementHintsGuard::set(vec![hint(2, 0, 2, 1)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U2_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let content = grid_content_area(area);
    let left = cells[0].rect.x.saturating_sub(content.x);
    let scroll =
        ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(left.saturating_add(100)));
    assert_eq!(
        scroll.get(),
        left,
        "a cell wider than the viewport must show its left edge"
    );
}

#[test]
fn grid_visible_rect_is_none_when_fully_left() {
    let area = Rect::new(0, 0, 60, 20);
    let c = cell(0, 0, 1, 1);
    let scroll = (c.rect.x - grid_content_area(area).x) + c.rect.width + 5;
    assert!(grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::new(scroll)).is_none());
}

#[test]
fn grid_visible_rect_is_none_when_fully_right() {
    let area = Rect::new(0, 0, 60, 20);
    let c = cell(5, 0, 1, 1);
    assert!(grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::ZERO).is_none());
}

#[test]
fn grid_visible_rect_aligns_to_left_edge_when_left_clipped() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(0, 0, 1, 1);
    let h_scroll = 5_u16;
    let r = grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::new(h_scroll))
        .expect("must be partially visible");
    assert_eq!(r.x, content.x);
    assert_eq!(r.width, c.rect.width.saturating_sub(h_scroll));
}

#[test]
fn grid_visible_rect_aligns_to_content_boundary_when_right_clipped() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(1, 0, 1, 1);
    let r = grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::ZERO)
        .expect("must be at least partially visible");
    let visible_right = content.x.saturating_add(content.width);
    assert!(r.x + r.width <= visible_right);
    assert!(r.x >= c.rect.x);
}

#[test]
fn ensure_grid_cursor_visible_h_leaves_scroll_when_right_edge_equals_viewport() {
    // The boundary where the cell's right_edge equals viewport_right
    // exactly. If the comparison flipped from `>` to `>=` it would
    // scroll needlessly.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(0, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let content = grid_content_area(area);
    let cell_right = cells[0].rect.x.saturating_sub(content.x) + cells[0].rect.width;
    // Set scroll so viewport_right == cell_right.
    let scroll = cell_right.saturating_sub(content.width);
    let next = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(scroll));
    assert_eq!(next.get(), scroll, "must not change scroll at the boundary");
}

#[test]
fn ensure_grid_cursor_visible_h_leaves_scroll_when_left_edge_equals_h_scroll() {
    // The boundary where the cell's left_edge equals h_scroll exactly.
    // If the comparison flipped from `<` to `<=` it would scroll one
    // cell further, to 0.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(2, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let content = grid_content_area(area);
    let cell_left = cells[0].rect.x.saturating_sub(content.x);
    let next = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(cell_left));
    assert_eq!(
        next.get(),
        cell_left,
        "must not change scroll at the boundary"
    );
}

#[test]
fn grid_max_h_scroll_returns_defined_value_without_panic_at_extreme_areas() {
    // At width 0/1/2, content.width == 0 (subtracting the 2-cell border
    // goes negative → 0). The saturating_sub guard must return
    // canvas_width unchanged with no panic. In a real render the
    // scrollbar stays hidden because grid_h_scrollbar_area is None.
    for width in [0_u16, 1, 2] {
        assert_eq!(
            grid_max_h_scroll(Rect::new(0, 0, width, 5)),
            grid_canvas_width(),
            "width={width} must return canvas_width without panicking"
        );
    }
}

#[test]
fn placement_hints_guard_restores_hints_on_panic() {
    use std::panic::AssertUnwindSafe;
    // Raise a panic while holding `PlacementHintsGuard::set` and
    // intercept it with catch_unwind — Drop must have restored hints to
    // vec![].
    let _outer_guard = PlacementHintsGuard::set(vec![]);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = PlacementHintsGuard::set(vec![hint_1x1(3, 3)]);
        assert!(
            placement_hint(0).is_some(),
            "hints must be alive right before the panic"
        );
        panic!("intentional panic");
    }));
    assert!(result.is_err(), "the panic must be caught by catch_unwind");
    assert!(
        placement_hint(0).is_none(),
        "the guard's Drop must have emptied hints (panic-safe)"
    );
}

#[test]
fn tool_index_at_grid_cell_maps_horizontally_scrolled_cell() {
    // With h_scroll > 0, check the click coordinate maps to the cell at
    // its post-scroll visible position. Pins the hit-rect math directly,
    // separate from the integration test.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(5, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let h_scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    let cells = pegboard_cells_inner(&tools, area);
    let visible = grid_visible_rect(cells[0], area, ScrollOffset::ZERO, h_scroll)
        .expect("the cell must be at least partially visible after scrolling");

    let hit = tool_index_at_grid_cell(
        &tools,
        area,
        ScrollOffset::ZERO,
        h_scroll,
        visible.x + 1,
        visible.y + 1,
    );
    assert_eq!(hit, Some(0));

    // Clicking the (0,0) region, now scrolled out of view, misses.
    let miss = tool_index_at_grid_cell(
        &tools,
        area,
        ScrollOffset::ZERO,
        h_scroll,
        area.x + 1,
        area.y + 1,
    );
    assert_eq!(miss, None);
}

#[test]
fn grid_visible_rect_clips_both_axes() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(1, 1, 1, 1);
    let r = grid_visible_rect(c, area, ScrollOffset::new(2), ScrollOffset::new(5))
        .expect("must be partially visible");
    assert!(r.x >= content.x);
    assert!(r.y >= content.y);
    assert!(r.x + r.width <= content.x.saturating_add(content.width));
    assert!(r.y + r.height <= content.y.saturating_add(content.height));
}

#[test]
fn tui_layout_places_two_panels_on_wide_terminal() {
    let layout = tui_layout(Rect::new(0, 0, 120, 30));
    assert!(layout.left.width > 0, "must have a board area");
    assert!(layout.right.width > 0, "must have a right pane area");
    assert!(!layout.is_narrow(), "a wide terminal is not narrow");
}

#[test]
fn tui_layout_keeps_right_pane_just_above_threshold() {
    let layout = tui_layout(Rect::new(0, 0, MIN_DUAL_PANE_WIDTH, 30));
    assert!(
        layout.right.width > 0,
        "must be dual-pane at MIN_DUAL_PANE_WIDTH({MIN_DUAL_PANE_WIDTH})",
    );
    assert!(!layout.is_narrow());
}

#[test]
fn tui_layout_folds_right_pane_just_below_threshold() {
    let narrow = MIN_DUAL_PANE_WIDTH.saturating_sub(1);
    let layout = tui_layout(Rect::new(0, 0, narrow, 30));
    assert_eq!(
        layout.right.width, 0,
        "the right side must fold when narrow"
    );
    assert_eq!(
        layout.left.width, narrow,
        "boards must occupy the full body width",
    );
    assert!(layout.is_narrow());
}

#[test]
fn tui_layout_body_equals_left_plus_right_width() {
    // body() is used by both dialog full-body rendering and the narrow
    // swap, so it must always equal the sum of left+right widths, narrow
    // or dual.
    for width in [40_u16, MIN_DUAL_PANE_WIDTH, 120] {
        let layout = tui_layout(Rect::new(0, 0, width, 30));
        assert_eq!(
            layout.body().width,
            layout.left.width.saturating_add(layout.right.width),
            "width={width}: body must equal the left+right sum",
        );
        assert_eq!(layout.body().x, layout.left.x);
    }
}

// Static stubs used by ensure_grid_cursor_visible_h tests. Kept tiny
// (no input spec, no surfaces beyond TUI) since the tests only need
// pegboard_units → grid_span.
use upeg_core::{InputSpec, Invoker, PegboardUnits, PinKind, Surface, ToolMeta};
static STUB_U1_TOOL: ToolMeta = ToolMeta {
    id: "stub.u1",
    toolkit: "stub",
    local_id: "u1",
    tags: &[],
    display_label: "Stub",
    description: "",
    input_spec: InputSpec::empty(),
    output_spec: upeg_core::OutputSpec::empty(),
    primary_output_id: None,
    effect: upeg_core::ToolEffect::Unknown,
    presentation: None,
    source: upeg_core::Source::UserInput,
    pin: PinKind::Inline,
    pegboard_units: PegboardUnits::U1,
    invoker: Invoker::Function,
    surfaces: &[Surface::Tui],
    boards: &[],
};
static STUB_U2_TOOL: ToolMeta = ToolMeta {
    id: "stub.u2",
    toolkit: "stub",
    local_id: "u2",
    tags: &[],
    display_label: "Stub",
    description: "",
    input_spec: InputSpec::empty(),
    output_spec: upeg_core::OutputSpec::empty(),
    primary_output_id: None,
    effect: upeg_core::ToolEffect::Unknown,
    presentation: None,
    source: upeg_core::Source::UserInput,
    pin: PinKind::Inline,
    pegboard_units: PegboardUnits::U2,
    invoker: Invoker::Function,
    surfaces: &[Surface::Tui],
    boards: &[],
};

#[test]
fn cell_span_follows_hint_w_h_over_manifest_when_hint_present() {
    // A placement with a span override arrives as a hint carrying
    // effective_size. Even a U1 manifest (1x1) tool renders 2x2 when the
    // hint says 2x2 — the basis for drawing the resize preview through
    // the same path.
    let area = Rect::new(0, 0, 200, 40);
    let _guard = PlacementHintsGuard::set(vec![hint(0, 0, 2, 2)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    assert_eq!(
        (cells[0].col_span, cells[0].row_span),
        (2, 2),
        "hint w/h must win over the manifest grid_span"
    );
}

#[test]
fn cell_span_falls_back_to_manifest_grid_span_without_hint() {
    // A tool with no placement (hint None) uses pegboard_units as before.
    let area = Rect::new(0, 0, 200, 40);
    let _guard = PlacementHintsGuard::set(vec![None]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U2_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let (w, h) = STUB_U2_TOOL.pegboard_units.grid_span();
    assert_eq!(
        (cells[0].col_span, cells[0].row_span),
        (w as usize, h as usize),
        "hint None must fall back to the manifest size"
    );
}

#[test]
fn hint_w_over_board_cols_is_clamped_to_cols() {
    // effective_size guarantees a ColSpan of at most 6, but the render
    // clamp stays defensive against corrupted hints.
    let area = Rect::new(0, 0, 200, 40);
    let oversized = GRID_COLS as u16 + 2;
    let _guard = PlacementHintsGuard::set(vec![hint(0, 0, oversized, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    assert_eq!(cells[0].col_span, GRID_COLS, "w clamps to GRID_COLS");
    assert_eq!(cells[0].row_span, 1, "h 0 clamps to a minimum of 1");
}
