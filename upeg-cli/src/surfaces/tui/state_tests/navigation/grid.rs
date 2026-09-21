use super::*;

#[test]
fn pegboard_cells_follow_placement_hints() {
    // The canvas is always BOARD_COLS-wide (narrow terminals slice it
    // via grid_h_scroll), so stored (col, row) hints are honored at
    // every viewport width.
    let tools = wrapped_grid_fixture_tools();
    let area = Rect::new(0, 0, 200, 40);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(2, 0)),
        Some(PlacementHint::unit(4, 3)),
        None,
        None,
        None,
    ]);
    let cells = pegboard_cells(&tools, layout.left);
    assert_eq!(cells[0].col, 2, "must follow hint (2, 0)");
    assert_eq!(cells[0].row, 0);
    assert_eq!(cells[1].col, 4, "must follow hint (4, 3)");
    assert_eq!(cells[1].row, 3);
}

#[test]
fn colliding_placement_hints_fall_back_to_first_free_cell() {
    // Two hints colliding on the same cell: first wins (honored),
    // second auto-packs around it so the user still sees both tools
    // even when state has stale/conflicting positions.
    let tools = wrapped_grid_fixture_tools();
    let area = Rect::new(0, 0, 200, 40);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(0, 0)),
        None,
        None,
        None,
    ]);
    let cells = pegboard_cells(&tools, layout.left);
    assert_eq!((cells[0].col, cells[0].row), (0, 0));
    assert_ne!(
        (cells[1].col, cells[1].row),
        (0, 0),
        "the second placement must fall back to the first free cell on hint collision"
    );
}

#[test]
fn pegboard_cells_follow_declared_units_and_hit_testing() {
    // Canonical 6-col canvas: WIDE(2×1) at (0,0), TALL(1×2) at (2,0),
    // ONE auto-packs to the next free row-0 slot at col 3.
    let tools = span_fixture_tools();
    let area = Rect::new(0, 0, 200, 30);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![]);
    let cells = pegboard_cells(&tools, layout.left);

    assert_eq!(cells[0].col_span, 2);
    assert_eq!(cells[0].row_span, 1);
    assert_eq!(cells[1].col_span, 1);
    assert_eq!(cells[1].row_span, 2);
    assert_eq!(cells[2].col, 3);
    assert_eq!(cells[2].row, 0);

    let hit_wide = tool_index_at_grid_cell(
        &tools,
        layout.left,
        ScrollOffset::ZERO,
        ScrollOffset::ZERO,
        cells[0].rect.x + cells[0].rect.width - 2,
        cells[0].rect.y + 1,
    );
    assert_eq!(hit_wide, Some(0));

    let hit_tall_lower_row = tool_index_at_grid_cell(
        &tools,
        layout.left,
        ScrollOffset::ZERO,
        ScrollOffset::ZERO,
        cells[1].rect.x + 1,
        cells[1].rect.y + cells[1].rect.height - 2,
    );
    assert_eq!(hit_tall_lower_row, Some(1));
}

#[test]
fn grid_movement_uses_card_size_info() {
    // Pin the multi-row layout via hints so the test exercises Down/Up
    // navigation across rows. Without hints the 6-col canonical canvas
    // would auto-pack everything onto row 0 and Down/Up would no-op.
    let tools = span_fixture_tools();
    let area = Rect::new(0, 0, 200, 30);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(2, 0)),
        Some(PlacementHint::unit(0, 1)),
        Some(PlacementHint::unit(1, 1)),
    ]);

    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 0, GridDirection::Right),
        1
    );
    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 0, GridDirection::Down),
        2
    );
    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 2, GridDirection::Up),
        0
    );
    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 2, GridDirection::Right),
        3
    );
}

#[test]
fn grid_movement_falls_back_to_lower_left_card_without_column_overlap() {
    // Force the 3-then-2 wrap shape via hints so we can verify the
    // fallback-candidate path picks the nearest below-row neighbor
    // even when no column overlap exists.
    let tools = wrapped_grid_fixture_tools();
    let area = Rect::new(0, 0, 200, 30);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(1, 0)),
        Some(PlacementHint::unit(2, 0)),
        Some(PlacementHint::unit(0, 1)),
        Some(PlacementHint::unit(1, 1)),
    ]);
    let cells = pegboard_cells(&tools, layout.left);

    assert_eq!((cells[2].row, cells[2].col), (0, 2));
    assert_eq!((cells[3].row, cells[3].col), (1, 0));
    assert_eq!((cells[4].row, cells[4].col), (1, 1));

    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 2, GridDirection::Down),
        4,
        "moving down from the right edge must reach the nearest lower-row card even without column overlap"
    );
    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 4, GridDirection::Left),
        3,
        "after reaching the lower row, the lower-left card must still be reachable"
    );
}

#[test]
fn keyboard_movement_scrolls_focused_grid_card_into_view() {
    // Canvas is fixed at BOARD_COLS=6, so on a 60-wide area the right
    // edge of the canvas is off-screen and Right-key navigation triggers
    // horizontal auto-scroll. The hints anchor cells[1] far enough right
    // that the move is guaranteed to leave the viewport.
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(5, 0)),
        Some(PlacementHint::unit(0, 1)),
    ]);

    let outcome = update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Right),
            tools: t,
            area: Some(area),
        },
    );
    let selected = pegboard_cells(t, layout.left)
        .into_iter()
        .find(|cell| cell.index == s.cursor);
    let visible = selected
        .and_then(|cell| grid_visible_rect(cell, layout.left, s.grid_scroll, s.grid_h_scroll));

    assert_eq!(outcome, Effect::None);
    assert_eq!(s.cursor, 1);
    assert!(
        s.grid_h_scroll > 0,
        "focusing beyond the viewport's right edge must move the horizontal scroll"
    );
    let visible = visible.expect("the selected cell must be visible after the move");
    assert!(
        visible.width >= 3,
        "there must be room to draw the selected card"
    );
}

#[test]
fn render_shows_keyboard_focused_card_after_auto_scroll() {
    use ratatui::backend::TestBackend;

    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(5, 0)),
        Some(PlacementHint::unit(0, 1)),
    ]);

    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Right),
            tools: t,
            area: Some(area),
        },
    );

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| render(f, &s, t)).unwrap();
    let grid_text = buffer_region_text(terminal.backend().buffer(), tui_layout(area).left);

    assert!(
        grid_text.contains("test.with_input"),
        "the focused card must render inside the grid after auto-scroll. grid: {grid_text}"
    );
    assert!(
        !grid_text.contains("test.simple"),
        "the first card must be scrolled out of the grid viewport. grid: {grid_text}"
    );
}

#[test]
fn render_shows_horizontal_scrollbar_when_grid_overflows() {
    // Canvas is fixed at BOARD_COLS=6 cells wide (≈113 chars), so on a
    // 60-wide area the horizontal scrollbar is the always-present axis.
    use ratatui::backend::TestBackend;

    let s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);
    let scrollbar_area =
        grid_h_scrollbar_area(layout.left).expect("must have a horizontal scrollbar area");

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| render(f, &s, t)).unwrap();
    let scrollbar_text = buffer_region_text(terminal.backend().buffer(), scrollbar_area);

    assert!(
        scrollbar_text.contains('#'),
        "a horizontally overflowing pegboard must render a horizontal scrollbar. scrollbar: {scrollbar_text}"
    );
}

#[test]
fn horizontal_scrollbar_not_drawn_when_grid_fits_viewport() {
    // Wide viewport: the whole canvas is visible so there must be no
    // horizontal scrollbar — negative complement of
    // `render_shows_horizontal_scrollbar_when_grid_overflows`.
    use ratatui::backend::TestBackend;

    let s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 200, 30);
    let layout = tui_layout(area);
    let scrollbar_area =
        grid_h_scrollbar_area(layout.left).expect("the single bottom row always exists");

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| render(f, &s, t)).unwrap();
    let scrollbar_text = buffer_region_text(terminal.backend().buffer(), scrollbar_area);

    assert!(
        !scrollbar_text.contains('#'),
        "when the viewport holds the whole canvas the horizontal scrollbar must not be drawn. scrollbar: {scrollbar_text}"
    );
}

#[test]
fn mouse_click_uses_scrolled_grid_coordinates() {
    let mut s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);
    let _guard = PlacementHintsGuard::set(vec![
        Some(PlacementHint::unit(0, 0)),
        Some(PlacementHint::unit(5, 0)),
        Some(PlacementHint::unit(0, 1)),
    ]);

    update(
        &mut s,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Right),
            tools: t,
            area: Some(area),
        },
    );
    assert!(s.grid_h_scroll > 0);
    s.cursor = 0;

    let visible_second = pegboard_cells(t, layout.left)
        .into_iter()
        .find(|cell| cell.index == 1)
        .and_then(|cell| grid_visible_rect(cell, layout.left, s.grid_scroll, s.grid_h_scroll))
        .expect("the second card must be visible after scrolling");

    let action = handle_mouse(
        &mut s,
        Mouse {
            column: visible_second.x + 1,
            row: visible_second.y + 1,
            kind: MouseKind::Pointer(PointerPhase::Down),
        },
        t,
        area,
    );

    assert_eq!(action, Action::None);
    assert_eq!(s.cursor, 1);
    assert_eq!(s.view, View::Detail);
}
