//! Companion to the `input_tests` module — collects only filter-bar
//! (board/tag) click/scroll regression tests. Split out for the
//! workspace 1000-LoC file-size budget.

use super::*;

#[test]
fn board_bar_click_selects_clicked_label_not_next_filter() {
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
fn tag_bar_click_selects_all_label_as_no_filter() {
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
fn filter_bar_click_ignores_non_label_cells() {
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
fn filter_bar_hit_test_uses_horizontal_scroll_offset() {
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
fn filter_bar_layout_reserves_scrollbar_row() {
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
fn filter_bar_wheel_scrolls_selected_bar_and_clamps_range() {
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
fn filter_bar_scrollbar_row_does_not_hit_filter_options() {
    let area = tui_layout(Rect::new(0, 0, 36, 20)).boards;
    let options = vec![
        "all".to_string(),
        "aaaaaaaa".to_string(),
        "bbbbbbbb".to_string(),
        "cccccccc".to_string(),
        "dddddddd".to_string(),
    ];
    let scrollbar = filter_bar_scrollbar_area(area).expect("filter bar must have a scrollbar row");
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
fn filter_bar_horizontal_wheel_scrolls_same_direction_as_vertical() {
    // The filter bar itself is a horizontal scroll container, so
    // ScrollLeft/Right must map onto the same axis as ScrollUp/Down —
    // that way users with horizontal-wheel terminals drive it with the
    // same mental model.
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
    assert!(after_right > 0, "ScrollRight must increase board_scroll");

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
        "ScrollLeft must decrease board_scroll"
    );
}

#[test]
fn tag_bar_responds_symmetrically_to_horizontal_wheel() {
    // The FilterBar::Tag branch exists separately from Board, so a
    // regression where only one side works is possible. Pinned
    // symmetrically with the board test.
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
    assert!(after_right > 0, "ScrollRight must increase tag_scroll");
    assert_eq!(state.board_scroll, 0, "the other axis must not change");

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
        "ScrollLeft must decrease tag_scroll"
    );
}
