use super::*;

#[test]
fn 페그보드_셀은_배치_힌트를_따른다() {
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
    assert_eq!(cells[0].col, 2, "힌트 (2, 0)을 따라야 한다");
    assert_eq!(cells[0].row, 0);
    assert_eq!(cells[1].col, 4, "힌트 (4, 3)을 따라야 한다");
    assert_eq!(cells[1].row, 3);
}

#[test]
fn 배치_힌트가_충돌하면_첫_빈칸으로_대체된다() {
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
        "두 번째 배치는 힌트가 충돌하면 첫 번째 빈칸으로 대체되어야 한다"
    );
}

#[test]
fn 페그보드_셀은_선언된_단위와_히트_테스트를_따른다() {
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
fn 그리드_이동은_카드_크기_정보를_사용한다() {
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
fn 열이_겹치지_않으면_그리드_이동은_왼쪽_아래_카드로_대체된다() {
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
        "오른쪽 끝에서 아래로 이동하면 열이 겹치지 않아도 가장 가까운 아래 행 카드로 가야 한다"
    );
    assert_eq!(
        move_cursor_in_grid(&tools, layout.left, 4, GridDirection::Left),
        3,
        "아래 행에 도착한 뒤에도 왼쪽의 아래 카드에 접근할 수 있어야 한다"
    );
}

#[test]
fn 키보드_이동은_포커스된_그리드_카드를_보이도록_스크롤한다() {
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
        "뷰포트 오른쪽 밖 포커스는 가로 스크롤을 움직여야 한다"
    );
    let visible = visible.expect("이동 후 선택된 셀이 보여야 한다");
    assert!(visible.width >= 3, "선택된 카드를 그릴 공간이 있어야 한다");
}

#[test]
fn 렌더는_자동_스크롤_후_키보드_포커스_카드를_보여준다() {
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
        "자동 스크롤 후 포커스된 카드는 그리드 안에 렌더링되어야 한다. grid: {grid_text}"
    );
    assert!(
        !grid_text.contains("test.simple"),
        "첫 번째 카드는 그리드 뷰포트 밖으로 스크롤되어야 한다. grid: {grid_text}"
    );
}

#[test]
fn 그리드가_가로로_넘치면_렌더는_가로_스크롤바를_보여준다() {
    // Canvas is fixed at BOARD_COLS=6 cells wide (≈113 chars), so on a
    // 60-wide area the horizontal scrollbar is the always-present axis.
    use ratatui::backend::TestBackend;

    let s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 60, 20);
    let layout = tui_layout(area);
    let scrollbar_area =
        grid_h_scrollbar_area(layout.left).expect("가로 스크롤바 영역이 있어야 한다");

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| render(f, &s, t)).unwrap();
    let scrollbar_text = buffer_region_text(terminal.backend().buffer(), scrollbar_area);

    assert!(
        scrollbar_text.contains('#'),
        "가로로 넘치는 페그보드는 가로 스크롤바를 렌더링해야 한다. scrollbar: {scrollbar_text}"
    );
}

#[test]
fn 그리드가_뷰포트에_들어가면_가로_스크롤바는_그려지지_않는다() {
    // Wide viewport: 캔버스가 모두 보이므로 가로 스크롤바 없어야 한다 —
    // negative complement of `그리드가_가로로_넘치면_렌더는_가로_스크롤바를_보여준다`.
    use ratatui::backend::TestBackend;

    let s = fresh();
    let t = fixture_tools();
    let t = t.as_slice();
    let area = Rect::new(0, 0, 200, 30);
    let layout = tui_layout(area);
    let scrollbar_area = grid_h_scrollbar_area(layout.left).expect("바닥 한 줄 영역은 항상 존재");

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| render(f, &s, t)).unwrap();
    let scrollbar_text = buffer_region_text(terminal.backend().buffer(), scrollbar_area);

    assert!(
        !scrollbar_text.contains('#'),
        "뷰포트가 캔버스를 다 담으면 가로 스크롤바는 그려지지 않아야 한다. scrollbar: {scrollbar_text}"
    );
}

#[test]
fn 마우스_클릭은_스크롤된_그리드_좌표를_사용한다() {
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
        .expect("스크롤 후 두 번째 카드가 보여야 한다");

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
