//! Companion to the `input_tests` module — collects only pegboard
//! list/filter composition and keyboard focus-cycling regression tests.
//! Split out for the workspace 1000-LoC file-size budget.

use super::*;

#[test]
fn tool_list_follows_shared_pegboard_layout_order() {
    // Both queries must read the same UPEG_HOME. HTTP fixtures temporarily
    // select a store containing their own board and pin under this lock.
    let _home = crate::test_support::pegboard_home_test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    upeg_tools::register_all();
    let tools = list_tools();
    let ids: Vec<&str> = tools.iter().map(|t| t.id).collect();
    let state = upeg_sources::pegboard::load_state();
    let expected: Vec<&str> =
        upeg_sources::pegboard::placements_for_board_and_tag_in(&state, None, None)
            .into_iter()
            .map(|(_, tool)| tool.id)
            .collect();
    assert_eq!(ids, expected);
}

#[test]
fn tool_list_by_board_and_tag_composes_filters() {
    let id = "test.tui_filter_custom";
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id,
        toolkit: "test",
        local_id: "tui_filter_custom",
        tags: &["custom"],
        display_label: "TUI filter custom",
        description: "Filter composition fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &["dev"],
    });
    let mut state = upeg_sources::pegboard::default_state();
    state
        .layouts
        .entry("dev".into())
        .or_default()
        .push(upeg_core::Placement::new(id.to_string(), 9, 0));

    let dev_pure =
        upeg_sources::pegboard::tools_for_board_and_tag_in(&state, Some("dev"), Some("custom"));
    assert!(
        dev_pure.iter().any(|t| t.id == id),
        "dev + custom must include the registered fixture tool"
    );
    assert!(
        dev_pure
            .iter()
            .all(|t| t.is_on_board("dev") && t.has_tag("custom")),
        "every result must satisfy both filters"
    );

    let none = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &state,
        Some("dev"),
        Some("definitely-no-such-tag"),
    );
    assert!(
        none.is_empty(),
        "an unknown tag must narrow to an empty result"
    );
}

#[test]
fn tui_uses_shared_pegboard_boards_and_graphic_metadata() {
    // Bootstrap State the same way the runtime does, so the test
    // exercises the board/tag option path derived from the cache instead
    // of re-reading the disk.
    let shared = upeg_sources::pegboard::load_state();
    let mut state = State {
        boards: shared.boards,
        layouts: shared.layouts,
        ..State::default()
    };
    let boards = state.board_filter_options();
    assert!(
        boards.iter().any(|board| board == "trading"),
        "the TUI board bar must include the shared GUI board `trading`"
    );

    let trading = list_tools_for_board_and_tag(Some("trading"), None);
    // `eth.gas` used to be the GUI-only example here (gui_meta.rs,
    // `Invoker::Http`, no dispatcher); it was promoted to a real
    // `Invoker::Function` tool with headless surfaces (including Tui), so
    // `net.status` — still a genuine Live+Timer GUI-only meta on the same
    // `trading` board — is the honest stand-in now.
    let net_status = trading
        .iter()
        .find(|tool| tool.id == "net.status")
        .expect("shared trading layout must resolve the GUI-only net.status metadata");
    assert!(
        net_status.is_on_surface(Surface::Desktop),
        "net.status must remain a Desktop-capable GUI tool"
    );
    assert!(
        !net_status.is_on_surface(Surface::Tui),
        "the TUI must inspect GUI-only metadata without pretending it can run it"
    );

    state.filters.select_board("trading");
    let tags = state.tag_filter_options();
    assert!(
        tags.iter().any(|tag| tag == "eth"),
        "the TUI tag bar must derive tags from the selected shared board layout"
    );
}

#[test]
fn filter_keys_cycle_boards_and_tags_without_running_tools() {
    let mut state = State {
        cursor: 2,
        view: View::Detail,
        ..fresh()
    };

    assert!(apply_filter_key(&mut state, Key::Char('b')));
    assert!(
        state.filters.board.is_some(),
        "b must cycle from All to the first board"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.view, View::List);

    assert!(apply_filter_key(&mut state, Key::Char('t')));
    assert!(
        state.filters.tag.is_some(),
        "t must cycle from All to the first tag"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.view, View::List);

    assert!(apply_filter_key(&mut state, Key::Char('0')));
    assert!(state.filters.board.is_none(), "0 must return to all boards");
}

#[test]
fn tab_and_backtab_cycle_default_keyboard_focus() {
    let mut state = fresh();
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let area = Rect::new(0, 0, 100, 30);

    assert_eq!(state.focus, FocusArea::Grid);
    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools,
            area: Some(area),
        },
    );
    assert_eq!(state.focus, FocusArea::Boards);
    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools,
            area: Some(area),
        },
    );
    assert_eq!(state.focus, FocusArea::Tags);
    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools,
            area: Some(area),
        },
    );
    assert_eq!(state.focus, FocusArea::RightPane);
    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Tab),
            tools,
            area: Some(area),
        },
    );
    assert_eq!(state.focus, FocusArea::Grid);
    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::BackTab),
            tools,
            area: Some(area),
        },
    );
    assert_eq!(state.focus, FocusArea::RightPane);
}

#[test]
fn keyboard_board_focus_moves_and_applies_board_filter() {
    let mut state = State {
        cursor: 2,
        view: View::Detail,
        ..fresh()
    };
    let tools = list_tools();
    let area = Rect::new(0, 0, 100, 30);

    for key in [Key::Tab, Key::Right, Key::Enter] {
        update(
            &mut state,
            Msg::KeyPress {
                stroke: key.into(),
                tools: &tools,
                area: Some(area),
            },
        );
    }

    assert_eq!(state.focus, FocusArea::Boards);
    assert!(
        state.filters.board.is_some(),
        "board focus must apply the focused board with no mouse input"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.view, View::List);
}

#[test]
fn keyboard_tag_focus_moves_and_applies_tag_filter() {
    let mut state = State {
        cursor: 2,
        filters: TuiFilters::from_options(Some("dev"), None),
        view: View::Detail,
        ..fresh()
    };
    state.sync_filter_cursors();
    let tools = list_tools();
    let area = Rect::new(0, 0, 100, 30);

    for key in [Key::Tab, Key::Tab, Key::Right, Key::Enter] {
        update(
            &mut state,
            Msg::KeyPress {
                stroke: key.into(),
                tools: &tools,
                area: Some(area),
            },
        );
    }

    assert_eq!(state.focus, FocusArea::Tags);
    assert_eq!(state.filters.board.as_deref(), Some("dev"));
    assert!(
        state.filters.tag.is_some(),
        "tag focus must apply the focused tag with no mouse input"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.view, View::List);
}

#[test]
fn keyboard_tag_focus_scrolls_focused_option_into_view() {
    static TAGS: &[&str] = &[
        "zz-keyboard-overflow-00",
        "zz-keyboard-overflow-01",
        "zz-keyboard-overflow-02",
        "zz-keyboard-overflow-03",
    ];
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id: "test.tui_keyboard_tag_scroll",
        toolkit: "test",
        local_id: "tui_keyboard_tag_scroll",
        tags: TAGS,
        display_label: "Keyboard tag scroll",
        description: "Forces tag filter bar keyboard overflow",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });

    let mut state = fresh();
    let tools = list_tools();
    let area = Rect::new(0, 0, 36, 20);

    for key in [Key::Tab, Key::Tab, Key::End] {
        update(
            &mut state,
            Msg::KeyPress {
                stroke: key.into(),
                tools: &tools,
                area: Some(area),
            },
        );
    }

    assert_eq!(state.focus, FocusArea::Tags);
    assert!(
        state.tag_scroll > 0,
        "moving the keyboard to an off-screen tag must adjust horizontal scroll"
    );
}

#[test]
fn right_pane_focus_scrolls_result_with_keyboard() {
    let mut state = State {
        focus: FocusArea::RightPane,
        view: View::Result {
            tool_id: "x",
            outputs: Vec::new(),
            text: "line\n".repeat(20),
            is_error: false,
        },
        ..State::default()
    };
    let tools = fixture_tools();
    let tools = tools.as_slice();

    for key in [Key::Down, Key::PageDown] {
        update(
            &mut state,
            Msg::KeyPress {
                stroke: key.into(),
                tools,
                area: Some(Rect::new(0, 0, 80, 20)),
            },
        );
    }

    assert!(state.right_scroll > 0);
    assert!(matches!(state.view, View::Result { .. }));

    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::Home),
            tools,
            area: Some(Rect::new(0, 0, 80, 20)),
        },
    );
    assert_eq!(state.right_scroll, 0);

    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::End),
            tools,
            area: Some(Rect::new(0, 0, 80, 20)),
        },
    );
    assert_eq!(state.right_scroll, u16::MAX);

    update(
        &mut state,
        Msg::KeyPress {
            stroke: upeg_core::KeyStroke::from(Key::PageUp),
            tools,
            area: Some(Rect::new(0, 0, 80, 20)),
        },
    );
    assert!(state.right_scroll < u16::MAX);
}
