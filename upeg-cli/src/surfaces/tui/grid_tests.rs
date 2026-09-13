//! Direct unit tests for the geometry helpers. Update-path tests
//! cover the orchestration; these pin the math so an off-by-one in
//! clipping or scroll-clamping surfaces here, not three layers up.

use super::*;

/// 한 칸짜리 placement hint. `(x, y)`만 관심 있는 기존 테스트용.
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
fn grid_canvas_width는_여섯_셀과_다섯_갭의_합이다() {
    let expected = (BOARD_COLS * GRID_CELL_WIDTH) + (BOARD_COLS - 1) * GRID_GAP;
    assert_eq!(grid_canvas_width(), expected);
}

#[test]
fn grid_h_scroll_step은_셀_폭과_갭의_합이다() {
    assert_eq!(grid_h_scroll_step(), GRID_CELL_WIDTH + GRID_GAP);
}

#[test]
fn grid_max_h_scroll은_캔버스가_뷰포트에_들어가면_영이다() {
    let area = Rect::new(0, 0, grid_canvas_width().saturating_add(4), 30);
    assert_eq!(grid_max_h_scroll(area), 0);
}

#[test]
fn grid_max_h_scroll은_정확히_캔버스_폭일_때도_영이다() {
    let area = Rect::new(0, 0, grid_canvas_width().saturating_add(2), 30);
    assert_eq!(grid_max_h_scroll(area), 0);
}

#[test]
fn grid_max_h_scroll은_뷰포트가_좁을수록_양수다() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    assert_eq!(
        grid_max_h_scroll(area),
        grid_canvas_width().saturating_sub(content.width)
    );
}

#[test]
fn clamp_grid_h_scroll은_max를_초과하지_않는다() {
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
fn grid_h_scrollbar_area는_바닥_한_줄이다() {
    let area = Rect::new(0, 0, 60, 20);
    let bar = grid_h_scrollbar_area(area).expect("정상 영역");
    assert_eq!(bar.height, 1);
    assert_eq!(bar.width, area.width.saturating_sub(2));
    assert_eq!(bar.y, area.y + area.height - 1);
}

#[test]
fn grid_h_scrollbar_area는_너무_좁으면_none() {
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 2, 5)).is_none());
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 1, 5)).is_none());
    assert!(grid_h_scrollbar_area(Rect::new(0, 0, 30, 0)).is_none());
}

#[test]
fn ensure_grid_cursor_visible_h는_오른쪽_밖이면_스크롤한다() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(5, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    assert!(scroll > 0, "오른쪽 끝 셀은 스크롤을 양수로 만들어야 한다");
    let cells = pegboard_cells_inner(&tools, area);
    let right = cells[0].rect.x - content.x + cells[0].rect.width;
    assert_eq!(scroll.get(), right.saturating_sub(content.width));
}

#[test]
fn ensure_grid_cursor_visible_h는_왼쪽_밖이면_왼쪽으로_되돌린다() {
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(0, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(50));
    assert_eq!(scroll.get(), 0, "왼쪽 끝 셀은 스크롤을 0으로 되돌려야 한다");
}

#[test]
fn ensure_grid_cursor_visible_h는_이미_보이면_그대로_둔다() {
    let area = Rect::new(0, 0, 200, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(2, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    assert_eq!(scroll.get(), 0, "이미 보이는 셀은 스크롤을 안 바꿔야 한다");
}

#[test]
fn ensure_grid_cursor_visible_h는_셀이_뷰포트보다_크면_왼쪽_가장자리를_보인다() {
    // 셀 폭 18 + 갭 1 = 19 → 셀 두 개(span 2)는 37폭. content.width를 그보다
    // 좁게 만들면(area.width=20, content.width=18) 셀이 뷰포트 보다 큼.
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
        "뷰포트보다 큰 셀은 왼쪽 가장자리를 보여야 한다"
    );
}

#[test]
fn grid_visible_rect는_완전히_왼쪽이면_none() {
    let area = Rect::new(0, 0, 60, 20);
    let c = cell(0, 0, 1, 1);
    let scroll = (c.rect.x - grid_content_area(area).x) + c.rect.width + 5;
    assert!(grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::new(scroll)).is_none());
}

#[test]
fn grid_visible_rect는_완전히_오른쪽이면_none() {
    let area = Rect::new(0, 0, 60, 20);
    let c = cell(5, 0, 1, 1);
    assert!(grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::ZERO).is_none());
}

#[test]
fn grid_visible_rect는_왼쪽이_잘리면_좌측_가장자리에_맞춘다() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(0, 0, 1, 1);
    let h_scroll = 5_u16;
    let r = grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::new(h_scroll))
        .expect("부분적으로 보여야 한다");
    assert_eq!(r.x, content.x);
    assert_eq!(r.width, c.rect.width.saturating_sub(h_scroll));
}

#[test]
fn grid_visible_rect는_오른쪽이_잘리면_컨텐츠_경계에_맞춘다() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(1, 0, 1, 1);
    let r = grid_visible_rect(c, area, ScrollOffset::ZERO, ScrollOffset::ZERO)
        .expect("일부라도 보여야 한다");
    let visible_right = content.x.saturating_add(content.width);
    assert!(r.x + r.width <= visible_right);
    assert!(r.x >= c.rect.x);
}

#[test]
fn ensure_grid_cursor_visible_h는_right_edge가_정확히_뷰포트와_같으면_그대로_둔다() {
    // 셀의 right_edge가 viewport_right와 정확히 같은 경계. 비교가
    // `>` 가 아니라 `>=` 로 바뀌면 불필요한 스크롤이 발생함.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(0, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let content = grid_content_area(area);
    let cell_right = cells[0].rect.x.saturating_sub(content.x) + cells[0].rect.width;
    // scroll을 잡아 viewport_right == cell_right이 되도록 설정.
    let scroll = cell_right.saturating_sub(content.width);
    let next = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(scroll));
    assert_eq!(next.get(), scroll, "경계에서는 스크롤을 바꾸면 안 된다");
}

#[test]
fn ensure_grid_cursor_visible_h는_left_edge가_h_scroll과_같으면_그대로_둔다() {
    // 셀의 left_edge가 h_scroll과 정확히 같은 경계. 비교가 `<`가
    // 아니라 `<=` 로 바뀌면 0으로 한 칸 더 스크롤됨.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(2, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let content = grid_content_area(area);
    let cell_left = cells[0].rect.x.saturating_sub(content.x);
    let next = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::new(cell_left));
    assert_eq!(next.get(), cell_left, "경계에서는 스크롤을 바꾸면 안 된다");
}

#[test]
fn grid_max_h_scroll은_극단_area에서도_panic_없이_정의된_값을_반환한다() {
    // width 0/1/2 모두 content.width == 0 (테두리 2칸 빼면 음수→0).
    // saturating_sub 보호로 panic 없이 canvas_width를 그대로 반환해야 함.
    // 실제 렌더에선 grid_h_scrollbar_area가 None이라 표시되지 않음.
    for width in [0_u16, 1, 2] {
        assert_eq!(
            grid_max_h_scroll(Rect::new(0, 0, width, 5)),
            grid_canvas_width(),
            "width={width}에서 panic 없이 canvas_width를 반환해야 한다"
        );
    }
}

#[test]
fn placement_hints_guard는_panic에서도_hints를_복구한다() {
    use std::panic::AssertUnwindSafe;
    // `PlacementHintsGuard::set`을 잡은 채 패닉을 발생시키고
    // catch_unwind로 가로채면 Drop이 hints를 vec![]로 되돌렸어야 한다.
    let _outer_guard = PlacementHintsGuard::set(vec![]);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = PlacementHintsGuard::set(vec![hint_1x1(3, 3)]);
        assert!(
            placement_hint(0).is_some(),
            "패닉 직전에는 hints가 살아 있어야 한다"
        );
        panic!("의도된 패닉");
    }));
    assert!(result.is_err(), "패닉이 catch_unwind로 잡혔어야 한다");
    assert!(
        placement_hint(0).is_none(),
        "가드 Drop이 hints를 비웠어야 한다 (panic-safe)"
    );
}

#[test]
fn tool_index_at_grid_cell는_가로_스크롤된_셀을_매핑한다() {
    // h_scroll > 0 인 상태에서 클릭 좌표가 "스크롤된 후 가시 위치"
    // 의 셀과 매핑되는지 확인. 통합 테스트와 별도로 hit-rect 계산을
    // 직접 핀.
    let area = Rect::new(0, 0, 60, 20);
    let _guard = PlacementHintsGuard::set(vec![hint_1x1(5, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let h_scroll = ensure_grid_cursor_visible_h(&tools, area, 0, ScrollOffset::ZERO);
    let cells = pegboard_cells_inner(&tools, area);
    let visible = grid_visible_rect(cells[0], area, ScrollOffset::ZERO, h_scroll)
        .expect("스크롤 후에는 셀이 일부라도 보여야 한다");

    let hit = tool_index_at_grid_cell(
        &tools,
        area,
        ScrollOffset::ZERO,
        h_scroll,
        visible.x + 1,
        visible.y + 1,
    );
    assert_eq!(hit, Some(0));

    // 스크롤되어 더 이상 보이지 않는 (0,0) 영역을 클릭하면 hit miss.
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
fn grid_visible_rect는_양축_동시_클립을_적용한다() {
    let area = Rect::new(0, 0, 60, 20);
    let content = grid_content_area(area);
    let c = cell(1, 1, 1, 1);
    let r = grid_visible_rect(c, area, ScrollOffset::new(2), ScrollOffset::new(5))
        .expect("일부 보여야 한다");
    assert!(r.x >= content.x);
    assert!(r.y >= content.y);
    assert!(r.x + r.width <= content.x.saturating_add(content.width));
    assert!(r.y + r.height <= content.y.saturating_add(content.height));
}

#[test]
fn tui_layout은_넓은_터미널에서_좌우_두_패널을_둔다() {
    let layout = tui_layout(Rect::new(0, 0, 120, 30));
    assert!(layout.left.width > 0, "보드 영역이 있어야 한다");
    assert!(layout.right.width > 0, "우측 패널 영역이 있어야 한다");
    assert!(!layout.is_narrow(), "넓은 터미널은 narrow가 아니다");
}

#[test]
fn tui_layout은_경계_바로_위에서_우측_패널을_유지한다() {
    let layout = tui_layout(Rect::new(0, 0, MIN_DUAL_PANE_WIDTH, 30));
    assert!(
        layout.right.width > 0,
        "MIN_DUAL_PANE_WIDTH({MIN_DUAL_PANE_WIDTH})에서는 듀얼 패널이어야 한다",
    );
    assert!(!layout.is_narrow());
}

#[test]
fn tui_layout은_경계_바로_아래에서_우측_패널을_접는다() {
    let narrow = MIN_DUAL_PANE_WIDTH.saturating_sub(1);
    let layout = tui_layout(Rect::new(0, 0, narrow, 30));
    assert_eq!(layout.right.width, 0, "narrow에서는 우측이 접혀야 한다");
    assert_eq!(
        layout.left.width, narrow,
        "보드가 body 전체 폭을 차지해야 한다",
    );
    assert!(layout.is_narrow());
}

#[test]
fn tui_layout_body는_좌우_합과_같다() {
    // body() 는 dialog 풀-바디 렌더와 narrow swap 양쪽에서 쓰이므로
    // narrow든 dual이든 항상 좌+우 폭의 합이어야 한다.
    for width in [40_u16, MIN_DUAL_PANE_WIDTH, 120] {
        let layout = tui_layout(Rect::new(0, 0, width, 30));
        assert_eq!(
            layout.body().width,
            layout.left.width.saturating_add(layout.right.width),
            "width={width}: body가 좌우 합과 일치해야 한다",
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
    source: upeg_core::Source::UserInput,
    pin: PinKind::Inline,
    pegboard_units: PegboardUnits::U2,
    invoker: Invoker::Function,
    surfaces: &[Surface::Tui],
    boards: &[],
};

#[test]
fn 힌트가_있으면_셀_크기는_manifest가_아니라_힌트_w_h를_따른다() {
    // span override가 있는 placement는 effective_size를 담은 힌트로
    // 들어온다. U1 manifest(1x1) 도구라도 힌트가 2x2면 2x2로 렌더된다 —
    // 리사이즈 프리뷰가 같은 경로로 그려지는 근거.
    let area = Rect::new(0, 0, 200, 40);
    let _guard = PlacementHintsGuard::set(vec![hint(0, 0, 2, 2)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    assert_eq!(
        (cells[0].col_span, cells[0].row_span),
        (2, 2),
        "힌트 w/h가 manifest grid_span보다 우선해야 한다"
    );
}

#[test]
fn 힌트가_없으면_셀_크기는_manifest_grid_span으로_돌아간다() {
    // placement 없는 도구(힌트 None)는 기존처럼 pegboard_units를 쓴다.
    let area = Rect::new(0, 0, 200, 40);
    let _guard = PlacementHintsGuard::set(vec![None]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U2_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    let (w, h) = STUB_U2_TOOL.pegboard_units.grid_span();
    assert_eq!(
        (cells[0].col_span, cells[0].row_span),
        (w as usize, h as usize),
        "힌트 None이면 manifest 크기로 폴백해야 한다"
    );
}

#[test]
fn 힌트_w가_보드_열수를_넘으면_열수로_클램프된다() {
    // effective_size는 ColSpan으로 6 이하가 보장되지만, 렌더 클램프는
    // 손상된 힌트에도 방어적으로 남는다.
    let area = Rect::new(0, 0, 200, 40);
    let oversized = GRID_COLS as u16 + 2;
    let _guard = PlacementHintsGuard::set(vec![hint(0, 0, oversized, 0)]);
    let tools: [&'static ToolMeta; 1] = [&STUB_U1_TOOL];

    let cells = pegboard_cells_inner(&tools, area);
    assert_eq!(cells[0].col_span, GRID_COLS, "w는 GRID_COLS로 클램프");
    assert_eq!(cells[0].row_span, 1, "h 0은 최소 1로 클램프");
}
