//! `input_tests` 모듈의 짝 — 필터 막대(보드/태그) 클릭·스크롤 회귀
//! 테스트만 모은다. 워크스페이스 1000-LoC 파일 크기 예산 때문에 분리했다.

use super::*;

#[test]
fn 보드_막대_클릭은_다음_필터가_아니라_클릭한_라벨을_선택한다() {
    let area = tui_layout(Rect::new(0, 0, 100, 30)).boards;
    let options = vec!["all".to_string(), "dev".to_string(), "ops".to_string()];
    let column = filter_option_column(area, BOARD_FILTER_PREFIX, &options, 2);
    let hit = filter_option_at_bar_cell(
        area,
        BOARD_FILTER_PREFIX,
        &options,
        ScrollOffset::ZERO,
        column,
        area.y + 1,
    );
    assert_eq!(hit, Some("ops"));

    let mut state = State {
        cursor: 2,
        grid_scroll: ScrollOffset::new(12),
        right_scroll: ScrollOffset::new(9),
        board_scroll: ScrollOffset::new(4),
        tag_scroll: ScrollOffset::new(2),
        filters: TuiFilters::from_options(Some("dev"), Some("pure")),
        view: View::Detail,
        ..State::default()
    };

    assert!(apply_filter_click(
        &mut state,
        FilterBar::Board,
        FilterBarClick {
            area,
            prefix: BOARD_FILTER_PREFIX,
            options: &options,
            scroll: ScrollOffset::ZERO,
            column,
            row: area.y + 1,
        }
    ));
    assert_eq!(state.filters.board.as_deref(), Some("ops"));
    assert_eq!(state.filters.tag.as_deref(), None);
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.board_scroll, 4);
    assert_eq!(state.tag_scroll, 2);
    assert_eq!(state.view, View::List);
}

#[test]
fn 태그_막대_클릭은_전체_라벨을_필터_없음으로_선택한다() {
    let area = tui_layout(Rect::new(0, 0, 100, 30)).tags;
    let options = vec!["all".to_string(), "pure".to_string(), "text".to_string()];
    let column = filter_option_column(area, TAG_FILTER_PREFIX, &options, 0);

    let mut state = State {
        cursor: 1,
        grid_scroll: ScrollOffset::new(6),
        right_scroll: ScrollOffset::new(3),
        board_scroll: ScrollOffset::ZERO,
        tag_scroll: ScrollOffset::new(5),
        filters: TuiFilters::from_options(Some("dev"), Some("text")),
        view: View::Detail,
        ..State::default()
    };

    assert!(apply_filter_click(
        &mut state,
        FilterBar::Tag,
        FilterBarClick {
            area,
            prefix: TAG_FILTER_PREFIX,
            options: &options,
            scroll: ScrollOffset::ZERO,
            column,
            row: area.y + 1,
        }
    ));
    assert_eq!(state.filters.board.as_deref(), Some("dev"));
    assert!(state.filters.tag.is_none());
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.tag_scroll, 5);
    assert_eq!(state.view, View::List);
}

#[test]
fn 필터_막대_클릭은_라벨이_아닌_셀을_무시한다() {
    let area = tui_layout(Rect::new(0, 0, 100, 30)).boards;
    let options = vec!["all".to_string(), "dev".to_string()];
    let mut state = State {
        cursor: 2,
        grid_scroll: ScrollOffset::new(12),
        right_scroll: ScrollOffset::new(9),
        board_scroll: ScrollOffset::new(3),
        tag_scroll: ScrollOffset::ZERO,
        filters: TuiFilters::from_options(Some("dev"), None),
        view: View::Detail,
        ..State::default()
    };

    assert!(!apply_filter_click(
        &mut state,
        FilterBar::Board,
        FilterBarClick {
            area,
            prefix: BOARD_FILTER_PREFIX,
            options: &options,
            scroll: ScrollOffset::ZERO,
            column: area.x + 1,
            row: area.y + 1,
        }
    ));
    assert_eq!(state.filters.board.as_deref(), Some("dev"));
    assert_eq!(state.cursor, 2);
    assert_eq!(state.grid_scroll, 12);
    assert_eq!(state.right_scroll, 9);
    assert_eq!(state.board_scroll, 3);
    assert_eq!(state.view, View::Detail);
}

#[test]
fn 필터_막대_히트_테스트는_가로_스크롤_오프셋을_사용한다() {
    let area = tui_layout(Rect::new(0, 0, 36, 20)).boards;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let scroll = filter_bar_scroll_step();
    let scrolled_column =
        filter_option_column(area, BOARD_FILTER_PREFIX, &options, 3).saturating_sub(scroll);

    assert_eq!(
        filter_option_at_bar_cell(
            area,
            BOARD_FILTER_PREFIX,
            &options,
            ScrollOffset::new(scroll),
            scrolled_column,
            area.y + 1
        ),
        Some("cccccccc")
    );
    assert_ne!(
        filter_option_at_bar_cell(
            area,
            BOARD_FILTER_PREFIX,
            &options,
            ScrollOffset::ZERO,
            scrolled_column,
            area.y + 1
        ),
        Some("cccccccc")
    );
}

#[test]
fn 필터_막대_레이아웃은_스크롤바_행을_예약한다() {
    let layout = tui_layout(Rect::new(0, 0, 80, 24));

    assert_eq!(layout.boards.height, 4);
    assert_eq!(layout.tags.height, 4);
    assert_eq!(
        filter_bar_scrollbar_area(layout.boards),
        Some(Rect::new(
            1,
            layout.boards.y + 2,
            layout.boards.width - 2,
            1
        ))
    );
}

#[test]
fn 필터_막대_휠은_선택된_막대를_스크롤하고_범위를_고정한다() {
    let area = tui_layout(Rect::new(0, 0, 36, 20)).boards;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let max_scroll = filter_bar_max_scroll(area, BOARD_FILTER_PREFIX, &options);
    assert!(max_scroll > 0);

    let mut state = State::default();
    assert!(apply_filter_scroll(
        &mut state,
        FilterBar::Board,
        area,
        BOARD_FILTER_PREFIX,
        &options,
        ScrollDelta::VERTICAL_FORWARD
    ));
    assert_eq!(state.board_scroll, filter_bar_scroll_step().min(max_scroll));
    assert_eq!(state.tag_scroll, 0);

    for _ in 0..20 {
        apply_filter_scroll(
            &mut state,
            FilterBar::Board,
            area,
            BOARD_FILTER_PREFIX,
            &options,
            ScrollDelta::VERTICAL_FORWARD,
        );
    }
    assert_eq!(state.board_scroll, max_scroll);

    assert!(apply_filter_scroll(
        &mut state,
        FilterBar::Board,
        area,
        BOARD_FILTER_PREFIX,
        &options,
        ScrollDelta::VERTICAL_BACK
    ));
    assert!(state.board_scroll < max_scroll);
}

#[test]
fn 필터_막대_스크롤바_행은_필터_옵션에_맞지_않는다() {
    let area = tui_layout(Rect::new(0, 0, 36, 20)).boards;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let scrollbar =
        filter_bar_scrollbar_area(area).expect("필터 막대에는 스크롤바 행이 있어야 한다");
    let column = filter_option_column(area, BOARD_FILTER_PREFIX, &options, 1);

    assert_eq!(
        filter_option_at_bar_cell(
            area,
            BOARD_FILTER_PREFIX,
            &options,
            ScrollOffset::ZERO,
            column,
            scrollbar.y
        ),
        None
    );
}

#[test]
fn 필터_막대_가로_휠은_세로_휠과_동일한_방향으로_스크롤된다() {
    // 필터 막대 자체가 가로 스크롤 컨테이너이므로 ScrollLeft/Right는
    // ScrollUp/Down과 같은 축에 매핑되어야 한다 — 그래야 가로 휠 단말을
    // 가진 사용자도 동일한 mental model로 조작할 수 있다.
    let area = tui_layout(Rect::new(0, 0, 36, 20)).boards;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let max_scroll = filter_bar_max_scroll(area, BOARD_FILTER_PREFIX, &options);
    assert!(max_scroll > 0);

    let mut state = State::default();
    apply_filter_scroll(
        &mut state,
        FilterBar::Board,
        area,
        BOARD_FILTER_PREFIX,
        &options,
        ScrollDelta::HORIZONTAL_FORWARD,
    );
    let after_right = state.board_scroll;
    assert!(
        after_right > 0,
        "ScrollRight는 board_scroll을 증가시켜야 한다"
    );

    apply_filter_scroll(
        &mut state,
        FilterBar::Board,
        area,
        BOARD_FILTER_PREFIX,
        &options,
        ScrollDelta::HORIZONTAL_BACK,
    );
    assert!(
        state.board_scroll < after_right.get(),
        "ScrollLeft는 board_scroll을 감소시켜야 한다"
    );
}

#[test]
fn 태그_막대도_가로_휠에_대칭으로_반응한다() {
    // FilterBar::Tag 분기가 Board와 별도로 존재하므로 한쪽만 동작하는
    // 회귀가 가능. board용 테스트와 대칭 형태로 핀.
    let area = tui_layout(Rect::new(0, 0, 36, 20)).tags;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let max_scroll = filter_bar_max_scroll(area, TAG_FILTER_PREFIX, &options);
    assert!(max_scroll > 0);

    let mut state = State::default();
    apply_filter_scroll(
        &mut state,
        FilterBar::Tag,
        area,
        TAG_FILTER_PREFIX,
        &options,
        ScrollDelta::HORIZONTAL_FORWARD,
    );
    let after_right = state.tag_scroll;
    assert!(
        after_right > 0,
        "ScrollRight는 tag_scroll을 증가시켜야 한다"
    );
    assert_eq!(state.board_scroll, 0, "다른 축은 변하면 안 된다");

    apply_filter_scroll(
        &mut state,
        FilterBar::Tag,
        area,
        TAG_FILTER_PREFIX,
        &options,
        ScrollDelta::HORIZONTAL_BACK,
    );
    assert!(
        state.tag_scroll < after_right.get(),
        "ScrollLeft는 tag_scroll을 감소시켜야 한다"
    );
}
