//! `input_tests` 모듈의 짝 — 페그보드 목록/필터 조합과 키보드 포커스
//! 순환 회귀 테스트만 모은다. 워크스페이스 1000-LoC 파일 크기 예산
//! 때문에 분리했다.

use super::*;

#[test]
fn 도구_목록은_공유_페그보드_레이아웃_순서를_따른다() {
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
fn 보드와_태그별_도구_목록은_필터를_조합한다() {
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
        "dev + custom은 등록된 fixture 도구를 포함해야 한다"
    );
    assert!(
        dev_pure
            .iter()
            .all(|t| t.is_on_board("dev") && t.has_tag("custom")),
        "모든 결과는 두 필터를 모두 만족해야 한다"
    );

    let none = upeg_sources::pegboard::tools_for_board_and_tag_in(
        &state,
        Some("dev"),
        Some("definitely-no-such-tag"),
    );
    assert!(none.is_empty(), "알 수 없는 태그는 빈 결과로 좁혀져야 한다");
}

#[test]
fn tui는_공유_페그보드_보드와_그래픽_메타데이터를_사용한다() {
    // 런타임과 같은 방식으로 State를 부트스트랩해서, 테스트가 디스크를
    // 다시 읽는 대신 캐시에서 파생된 보드/태그 옵션 경로를 검증한다.
    let shared = upeg_sources::pegboard::load_state();
    let mut state = State {
        boards: shared.boards,
        layouts: shared.layouts,
        ..State::default()
    };
    let boards = state.board_filter_options();
    assert!(
        boards.iter().any(|board| board == "trading"),
        "TUI 보드 막대는 공유 GUI 보드 `trading`을 포함해야 한다"
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
        .expect("공유 trading 레이아웃은 GUI 전용 net.status 메타데이터를 해석해야 한다");
    assert!(
        net_status.is_on_surface(Surface::Desktop),
        "net.status는 Desktop 가능 GUI 도구로 남아야 한다"
    );
    assert!(
        !net_status.is_on_surface(Surface::Tui),
        "TUI는 GUI 전용 메타데이터를 실행 가능한 척하지 않고 검사해야 한다"
    );

    state.filters.select_board("trading");
    let tags = state.tag_filter_options();
    assert!(
        tags.iter().any(|tag| tag == "eth"),
        "TUI 태그 막대는 선택된 공유 보드 레이아웃에서 태그를 파생해야 한다"
    );
}

#[test]
fn 필터키는_도구를_실행하지_않고_보드와_태그를_순환한다() {
    let mut state = State {
        cursor: 2,
        view: View::Detail,
        ..fresh()
    };

    assert!(apply_filter_key(&mut state, Key::Char('b')));
    assert!(
        state.filters.board.is_some(),
        "b는 전체에서 첫 보드로 순환해야 한다"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.view, View::List);

    assert!(apply_filter_key(&mut state, Key::Char('t')));
    assert!(
        state.filters.tag.is_some(),
        "t는 전체에서 첫 태그로 순환해야 한다"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.view, View::List);

    assert!(apply_filter_key(&mut state, Key::Char('0')));
    assert!(
        state.filters.board.is_none(),
        "0은 모든 보드로 돌아가야 한다"
    );
}

#[test]
fn 탭과_역탭은_기본_키보드_포커스를_순환한다() {
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
fn 키보드_보드_포커스는_이동하고_보드_필터를_적용한다() {
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
        "보드 포커스는 마우스 입력 없이 포커스된 보드를 적용해야 한다"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.view, View::List);
}

#[test]
fn 키보드_태그_포커스는_이동하고_태그_필터를_적용한다() {
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
        "태그 포커스는 마우스 입력 없이 포커스된 태그를 적용해야 한다"
    );
    assert_eq!(state.cursor, 0);
    assert_eq!(state.grid_scroll, 0);
    assert_eq!(state.right_scroll, 0);
    assert_eq!(state.view, View::List);
}

#[test]
fn 키보드_태그_포커스는_포커스된_옵션이_보이도록_스크롤한다() {
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
        "화면 밖 태그로 키보드 이동하면 가로 스크롤이 조정되어야 한다"
    );
}

#[test]
fn 오른쪽_패널_포커스는_키보드로_결과를_스크롤한다() {
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
