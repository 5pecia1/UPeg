//! `tui_tests.rs`에서 분리한 TUI 렌더/소스 계약 테스트.

use super::state_tests::fresh;
use crate::Outcome;
use crate::domain::execution::dispatch::dispatch_failure;
use crate::surfaces::tui::*;
use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use upeg_core::{
    InputFieldSpec, InputKind, InputName, InputSpec, Invoker, OutputEntry, OutputKind, OutputValue,
    PinColorHex, PinKind, Placement, Surface, ToolMeta, ToolSuccess,
};

fn buffer_region_has_modifier(buffer: &Buffer, area: Rect, modifier: Modifier) -> bool {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if buffer[(x, y)].modifier.contains(modifier) {
                return true;
            }
        }
    }
    false
}

fn buffer_region_has_symbol_with_fg(buffer: &Buffer, area: Rect, symbol: &str, fg: Color) -> bool {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &buffer[(x, y)];
            if cell.symbol() == symbol && cell.fg == fg {
                return true;
            }
        }
    }
    false
}

#[test]
fn 테스트_백엔드_렌더는_제목을_포함한다() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(buf_string.contains("Pegboard"));
    assert!(buf_string.contains("Boards"));
    assert!(buf_string.contains("Tags"));
    assert!(buf_string.contains("navigate"));
}

#[test]
fn 렌더는_키보드로_포커스된_필터_옵션을_표시한다() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &["zz-focus-render"];
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id: "test.tui_focus_render",
        toolkit: "test",
        local_id: "tui_focus_render",
        tags: TAGS,
        display_label: "Focus render",
        description: "Forces a tag option for focus rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });

    let area = Rect::new(0, 0, 100, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = list_tools();
    let state = State {
        focus: FocusArea::Tags,
        tag_cursor: 1,
        ..fresh()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let layout = tui_layout(area);
    let buffer = terminal.backend().buffer();

    assert!(
        buffer_region_has_modifier(buffer, layout.tags, Modifier::UNDERLINED),
        "포커스된 태그 옵션은 보이는 포커스 표시로 렌더링되어야 한다"
    );
    assert!(
        !buffer_region_has_modifier(buffer, layout.boards, Modifier::UNDERLINED),
        "포커스되지 않은 보드 막대는 키보드 포커스 밑줄을 렌더링하면 안 된다"
    );
}

#[test]
fn 보드_목록이_넓으면_보드_스크롤바를_렌더링한다() {
    use ratatui::backend::TestBackend;

    static BOARDS: &[&str] = &[
        "zz-tui-overflow-00",
        "zz-tui-overflow-01",
        "zz-tui-overflow-02",
        "zz-tui-overflow-03",
    ];
    upeg_runtime::toolbox_add_tool(ToolMeta {
        id: "test.tui_board_scrollbar",
        toolkit: "test",
        local_id: "tui_board_scrollbar",
        tags: &[],
        display_label: "Board scrollbar",
        description: "Forces the board filter bar to overflow",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: BOARDS,
    });

    let backend = TestBackend::new(36, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = fresh();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf_string.contains('#') && buf_string.contains('>'),
        "넓은 보드 필터 막대는 가로 스크롤바를 보여줘야 한다. rendered: {buf_string}"
    );
}

#[test]
fn 필터를_적용한_렌더는_활성_보드와_태그를_표시한다() {
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let filters = TuiFilters::from_options(Some("dev"), Some("pure"));
    let tools = list_tools_for_board_and_tag(filters.board.as_deref(), filters.tag.as_deref());
    let state = fresh();

    terminal
        .draw(|f| render_with_filters(f, &state, &tools, &filters))
        .unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());
    assert!(buf_string.contains("Pegboard grid"));
    assert!(buf_string.contains("Boards"));
    assert!(buf_string.contains(" dev "));
    assert!(buf_string.contains("Tags"));
    assert!(buf_string.contains("pure"));
}

#[test]
fn 렌더는_공유_표시_라벨과_결과_상태를_사용한다() {
    use ratatui::backend::TestBackend;

    let hex: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "num.hex_to_decimal",
        toolkit: "convert",
        local_id: "hex_to_decimal",
        tags: &[],
        display_label: "Hex → Dec",
        description: "Convert hexadecimal to decimal",
        input_spec: InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("input").expect("테스트 입력 이름"),
                None,
                None,
                true,
                InputKind::String,
            )
            .expect("테스트 입력 필드"),
        ])
        .expect("테스트 입력 명세"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    }));
    let tools = [hex];

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|f| render(f, &State::default(), &tools))
        .unwrap();
    let list_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        list_buf.contains("Hex → Dec"),
        "TUI 목록은 ToolMeta 표시 라벨을 사용해야 한다. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("↵ run") && list_buf.contains("o inspect"),
        "Enter는 실행이고 상세 검사는 o로 분리되어야 한다. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("MCP off") && list_buf.contains("HTTP off"),
        "푸터는 가짜 백그라운드 작업 자리표시자 대신 실제 인터페이스 상태를 노출해야 한다. rendered: {list_buf}"
    );
    assert!(
        !list_buf.contains("0 bg tasks") && !list_buf.contains("net OK"),
        "푸터는 비활성 상태 자리표시자를 다시 도입하면 안 된다. rendered: {list_buf}"
    );
    assert!(
        list_buf.contains("num.hex_to_decimal"),
        "TUI는 기술용 도구 id를 보조 메타데이터로 계속 노출해야 한다. rendered: {list_buf}"
    );

    let ok_state = State {
        cursor: 0,
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "255".into(),
            is_error: false,
        },
        ..State::default()
    };
    terminal.draw(|f| render(f, &ok_state, &tools)).unwrap();
    let ok_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        ok_buf.contains("OK"),
        "성공 결과는 OK를 보여줘야 한다. rendered: {ok_buf}"
    );

    let err_state = State {
        cursor: 0,
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "bad input".into(),
            is_error: true,
        },
        ..State::default()
    };
    terminal.draw(|f| render(f, &err_state, &tools)).unwrap();
    let err_buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        err_buf.contains("ERROR"),
        "오류 결과는 ERROR를 보여줘야 한다. rendered: {err_buf}"
    );
}

#[test]
fn tui_결과_상태_라벨은_코어_사용자경험_계약에서_온다() {
    let view_src = include_str!("view.rs");
    assert!(
        view_src.contains("upeg_core::ux::result_status_label"),
        "TUI view는 공유 ux 결과 상태 헬퍼를 가져와야 한다"
    );
    assert!(
        !view_src.contains("const fn result_status_label"),
        "TUI view는 로컬 result_status_label 복사본을 유지하면 안 된다"
    );
}

#[test]
fn 성공_결과는_정규_output의_라벨과_값을_모두_그린다() {
    use ratatui::backend::TestBackend;

    let success = ToolSuccess::new(
        Some("primary".into()),
        vec![
            OutputEntry {
                id: "primary".into(),
                label: Some("Primary label".into()),
                kind: OutputKind::String,
                value: OutputValue::String("main value".into()),
            },
            OutputEntry {
                id: "secondary".into(),
                label: Some("Secondary label".into()),
                kind: OutputKind::Integer,
                value: OutputValue::Integer(42),
            },
        ],
    )
    .unwrap();
    let mut state = State::default();
    apply_outcome(
        &mut state,
        "test.multi_output",
        crate::domain::execution::dispatch::Outcome::Success(success),
    );
    match &state.view {
        View::Result { outputs, text, .. } => {
            assert_eq!(outputs.len(), 2);
            assert!(text.is_empty());
        }
        other => panic!("Result 보기를 기대했다: {other:?}"),
    }

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(buf.contains("Primary label"), "rendered: {buf}");
    assert!(buf.contains("main value"), "rendered: {buf}");
    assert!(buf.contains("Secondary label"), "rendered: {buf}");
    assert!(buf.contains("42"), "rendered: {buf}");
}

#[test]
fn 폼_보기_제목과_본문은_일관된_뒤로가기_동사를_사용한다() {
    // Form 보기의 제목("esc back")과 본문 푸터("[Esc] back")는 같은 동사를
    // 사용해야 한다. 핸들러는 `back_one_level`이므로 "back"은 Detail의
    // "q back" 및 Result의 "↩ list"와 맞다. 이후 어느 문자열을 고치든
    // 불일치가 크게 드러나도록 계약을 고정한다.
    use ratatui::backend::TestBackend;
    let mut s = State {
        cursor: 0,
        view: View::Form {
            tool_id: "test.iter160",
            form: TuiFormState::new(
                InputSpec::new(vec![
                    InputFieldSpec::new(
                        InputName::new("input").expect("테스트 입력 이름"),
                        None,
                        None,
                        true,
                        InputKind::String,
                    )
                    .expect("테스트 입력 필드"),
                ])
                .expect("테스트 입력 명세"),
            ),
        },
        ..State::default()
    };
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &s, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 두 표면(블록 테두리의 제목 + 본문 푸터) 모두 "back"을 보여줘야 한다.
    // 두 부분 문자열을 모두 고정한다.
    assert!(
        buf.contains("Esc] back"),
        "Form 보기 본문은 160회차 이후 `[Esc] back`이라고 말해야 한다. got: {buf}"
    );
    assert!(
        buf.contains("esc back"),
        "Form 보기 제목은 `esc back`이라고 말해야 한다. got: {buf}"
    );
    // 부정 고정: 오래된 "cancel" 동사는 돌아오면 안 된다.
    assert!(
        !buf.contains("Esc] cancel"),
        "160회차에서 `[Esc] cancel`을 제거했다. 되돌리면 제목/본문 불일치가 돌아온다"
    );
    let _ = &mut s; // 이후 State 편집이 mutate하지 않아도 unused-mut 경고를 막는다.
}

// ─── 6-9단계 오버레이 렌더러 ─────────────────────────────────
//
// 새 전체 본문 보기(BoardEditor / ConfirmDeleteBoard / ToolPicker)는 각각
// TestBackend 스냅샷을 가진다. 렌더 쪽 회귀(누락된 프롬프트, 잘못된 제목,
// 누락된 고정 표시)가 CI를 지나가지 않고 테스트를 깨뜨리게 하기 위함이다.

#[test]
fn 보드_편집기_추가_모드_렌더는_프롬프트와_버퍼를_보여준다() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer: "Alpha".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 전체 본문 패널은 제목, 추가 프롬프트, 입력된 버퍼를 가져야 한다.
    // 페그보드 그리드는 보이면 안 된다.
    assert!(
        buf.contains("Edit board"),
        "BoardEditor 제목이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("New board title:"),
        "추가 모드 프롬프트가 나타나야 한다. got: {buf}"
    );
    assert!(
        buf.contains("Alpha"),
        "입력된 버퍼가 렌더링되어야 한다. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "BoardEditor는 전체 본문을 차지해야 하며 그리드 제목이 새면 안 된다"
    );
}

#[test]
fn 보드_편집기_이름변경_모드_렌더는_원래_제목을_미리_채운다() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::Rename {
                key: "dev".to_string(),
                original_title: "Dev".to_string(),
            },
            buffer: "Development".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 이름 변경 프롬프트는 원래 제목을 담아 사용자가 무엇을 바꾸는지 보게 하고,
    // 버퍼는 편집 중인 값을 담는다.
    assert!(
        buf.contains("Dev"),
        "이름 변경 프롬프트는 원래 제목을 포함해야 한다. got: {buf}"
    );
    assert!(
        buf.contains("Development"),
        "이름 변경 버퍼가 렌더링되어야 한다. got: {buf}"
    );
}

#[test]
fn 보드_편집기_빈_버퍼_렌더는_커서만_아니라_힌트를_보여준다() {
    use crate::surfaces::tui::model::BoardEditMode;
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::BoardEditor {
            mode: BoardEditMode::AddBoard,
            buffer: String::new(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 빈 버퍼는 사용자가 무엇을 해야 할지 알 수 있도록 힌트를 표시해야 한다.
    // 수정 전 패널은 지시 없이 프롬프트와 외로운 커서 블록만 렌더링했다.
    assert!(
        buf.contains("type a title"),
        "빈 버퍼 힌트는 해야 할 일을 알려야 한다. got: {buf}"
    );
}

#[test]
fn 보드_삭제_확인_렌더는_대상_제목과_선택지를_보여준다() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::ConfirmDeleteBoard {
            key: "trading".to_string(),
            title: "Trading".to_string(),
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 사용자가 y를 누르기 전에 다시 확인할 수 있도록 프롬프트는 대상 제목을
    // 그대로 인용해야 한다.
    assert!(
        buf.contains("Trading"),
        "확인 프롬프트는 대상 제목을 포함해야 한다. got: {buf}"
    );
    assert!(
        buf.contains("[y/F1] delete"),
        "확인 프롬프트는 [y/F1] 동작을 표시해야 한다 (F1도 어디서나 확인 키). got: {buf}"
    );
    assert!(
        buf.contains("[n] keep"),
        "확인 프롬프트는 [n] 동작을 표시해야 한다. got: {buf}"
    );
}

#[test]
fn 도구_선택기_렌더는_검색어와_고정_표시를_보여준다() {
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();

    // 선택기에서 ★ 표시가 붙을 대상을 갖도록 `dev`에 알려진 고정 도구를 심는다.
    let mut state = fresh();
    state.filters.select_board("dev");
    state.layouts.insert(
        "dev".to_string(),
        vec![Placement::new("num.hex_to_decimal", 0, 0)],
    );
    state.view = View::ToolPicker {
        mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
        return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
        query: "hex".to_string(),
        cursor: 0,
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // 제목 + 검색어 + 고정 표시 + 필터링된 결과 하나.
    assert!(
        buf.contains("Add tool"),
        "ToolPicker 제목이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("Filter:"),
        "검색어 입력 라벨이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("hex"),
        "입력한 검색어가 렌더링되어야 한다. got: {buf}"
    );
    assert!(
        buf.contains('★'),
        "이 보드에 고정됨 표시는 num.hex_to_decimal 옆에 렌더링되어야 한다. got: {buf}"
    );
    assert!(
        buf.contains("num.hex_to_decimal"),
        "필터링된 툴박스 id가 렌더링되어야 한다. got: {buf}"
    );
}

#[test]
fn 도구_선택기는_고정된_도구를_검색_결과_위쪽에_그린다() {
    use ratatui::backend::TestBackend;

    const TAGS: &[&str] = &["zz-tui-render-pinned-rank-tag"];
    let _unpinned = upeg_runtime::toolbox_add_tool_managed(ToolMeta {
        id: "zz_tui_render_rank.first",
        toolkit: "zz_tui_render_rank",
        local_id: "first",
        tags: TAGS,
        display_label: "Render rank first",
        description: "Render shared search rank fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });
    let _pinned = upeg_runtime::toolbox_add_tool_managed(ToolMeta {
        id: "zz_tui_render_rank.pinned",
        toolkit: "zz_tui_render_rank",
        local_id: "pinned",
        tags: TAGS,
        display_label: "Render rank pinned",
        description: "Render shared search rank fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    });
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let mut state = fresh();
    state.filters.select_board("dev");
    state.layouts.insert(
        "dev".to_string(),
        vec![Placement::new("zz_tui_render_rank.pinned", 0, 0)],
    );
    state.view = View::ToolPicker {
        mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
        return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
        query: "zz-tui-render-pinned-rank-tag".to_string(),
        cursor: 0,
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    let pinned = buf
        .find("zz_tui_render_rank.pinned")
        .expect("pinned fixture should render");
    let unpinned = buf
        .find("zz_tui_render_rank.first")
        .expect("unpinned fixture should render");

    assert!(
        pinned < unpinned,
        "pinned shared-search result must render before unpinned result. got: {buf}"
    );
    assert!(
        buf.contains('★'),
        "pinned marker should stay visible on ranked shared-search rows. got: {buf}"
    );
}

#[test]
fn 도구_선택기_빈_검색어_렌더는_본문을_비워두지_않는다() {
    // 빈 검색어는 사용자를 빈 패널에 남겨두면 안 된다. 패널 본문 안의 버퍼
    // 셀을 훑어 공백도 테두리도 아닌 기호가 적어도 하나 렌더링되는지
    // 확인한다. 이는 빈 상태 힌트이거나 필터링되지 않은 툴박스 행이다.
    use ratatui::backend::TestBackend;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        filters: crate::surfaces::tui::model::TuiFilters::from_options(Some("dev"), None),
        view: View::ToolPicker {
            mode: crate::surfaces::tui::model::ToolPickerMode::Pin,
            return_to: crate::surfaces::tui::model::ToolPickerReturn::list(FocusArea::Grid),
            query: String::new(),
            cursor: 0,
        },
        ..fresh()
    };
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buffer = terminal.backend().buffer();
    let border_chars = ['╭', '╮', '╰', '╯', '─', '│', '█'];
    let mut content_cells = 0_usize;
    // 본문은 헤더/필터 행 아래에 있다. 8..20행을 스캔해 헤더, 필터 막대,
    // 패널 테두리를 건너뛴다.
    for y in 8_u16..20 {
        for x in 1_u16..79 {
            let sym = buffer[(x, y)].symbol();
            if sym.trim().is_empty() {
                continue;
            }
            if sym.chars().all(|c| border_chars.contains(&c)) {
                continue;
            }
            content_cells += 1;
        }
    }
    assert!(
        content_cells > 0,
        "빈 검색어 선택기는 본문 영역에 콘텐츠를 렌더링해야 한다. \
         테두리도 공백도 아닌 셀을 {content_cells}개 찾았다",
    );
}

// ──────────────────────────────────────────────────────────────
// Narrow-body (좁은 화면) 렌더 분기. GUI의 `overflow:auto +
// width:max-content` 와 마찬가지로 보드가 본진. List/Detail 에서는
// 우측 패널이 접히고 보드가 body 전체를 차지하며, Form/Result 에서는
// 사용자가 작업 중인 우측 콘텐츠가 풀-바디로 노출된다.

#[test]
fn 좁은_화면_list는_보드를_풀_바디로_그리고_우측_패널을_숨긴다() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50; // < MIN_DUAL_PANE_WIDTH(60)
    let backend = TestBackend::new(width, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default(); // View::List

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Pegboard"),
        "좁은 화면 List에서도 보드 제목은 보여야 한다. got: {buf}"
    );
    // 우측 패널의 list 제목(" Tool · ↵ run · o inspect · Space run · / search ")
    // 이 노출되면 안 된다. "inspect"는 modeless 헤더 힌트("o inspect")에도
    // 등장하므로, 우측 패널 제목에만 있는 "Space run"으로 판별한다.
    assert!(
        !buf.contains("Space run"),
        "좁은 화면 List에서는 우측 list 패널이 숨겨져야 한다. got: {buf}"
    );
}

#[test]
fn 좁은_화면_detail은_보드를_풀_바디로_그린다() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::Detail,
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Pegboard"),
        "좁은 화면 Detail에서도 보드가 본진이어야 한다. got: {buf}"
    );
    // Detail 패널 제목 "Manifest · F1/↵ ..." 가 노출되면 안 된다.
    assert!(
        !buf.contains("Manifest"),
        "좁은 화면 Detail에서는 매니페스트 우측 패널이 숨겨져야 한다. got: {buf}"
    );
}

#[test]
fn 좁은_화면_form은_풀_바디로_그려지고_보드를_숨긴다() {
    use crate::surfaces::tui::model::TuiFormState;
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    // 사용자가 작업 중인 surface (Form) 가 본진. 보드는 양보.
    let state = State {
        view: View::Form {
            tool_id: "num.hex_to_decimal",
            form: TuiFormState::new(upeg_core::InputSpec::empty()),
        },
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    // Form 의 우측 패널 제목 ("Form · ↵ run · ...") 이 풀-바디로 그려져야.
    assert!(
        buf.contains("Form"),
        "좁은 화면 Form에서는 폼 풀-바디가 보여야 한다. got: {buf}"
    );
    // 보드 grid 제목은 노출되면 안 된다.
    assert!(
        !buf.contains("Pegboard grid"),
        "좁은 화면 Form에서는 보드 grid가 숨겨져야 한다. got: {buf}"
    );
}

#[test]
fn 좁은_화면_result는_결과를_풀_바디로_그린다() {
    use ratatui::backend::TestBackend;

    let width: u16 = 50;
    let backend = TestBackend::new(width, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State {
        view: View::Result {
            tool_id: "num.hex_to_decimal",
            outputs: Vec::new(),
            text: "0x12 = 18".into(),
            is_error: false,
        },
        ..State::default()
    };

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(
        buf.contains("Result") || buf.contains("결과"),
        "좁은 화면 Result는 결과 패널이 풀-바디로 보여야 한다. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "좁은 화면 Result에서는 보드 grid가 숨겨져야 한다. got: {buf}"
    );
}

#[test]
fn 넓은_화면은_여전히_좌우_듀얼_패널을_보여준다() {
    use ratatui::backend::TestBackend;

    // 회귀 가드: narrow 분기를 도입했지만 표준 80x24 에서는 듀얼 유지.
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    let state = State::default();

    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());
    assert!(buf.contains("Pegboard"), "보드 제목은 보여야 한다");
    assert!(
        buf.contains("inspect") || buf.contains("Tool"),
        "넓은 화면 List는 우측 패널도 노출해야 한다. got: {buf}"
    );
}

#[test]
fn 핀_색상이_있으면_그리드_카드는_실제_rgb_색을_렌더링한다() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &[];
    let tool: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "test.pin_color_card",
        toolkit: "test",
        local_id: "pin_color_card",
        tags: TAGS,
        display_label: "Pin Color Card",
        description: "Test card for pin color rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    }));

    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = [tool];
    let state = fresh();
    let pin_color = PinColorHex::parse("#112233").expect("테스트 핀 색상");
    let pin_colors = [Some(pin_color)];
    let expected = Color::Rgb(0x11, 0x22, 0x33);

    terminal
        .draw(|f| render_with_pin_colors(f, &state, &tools, &pin_colors))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let layout = tui_layout(area);
    let grid = grid_content_area(layout.body());

    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "I", expected),
        "핀 종류 라벨은 custom RGB 전경색을 써야 한다"
    );
    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "─", expected)
            || buffer_region_has_symbol_with_fg(buffer, grid, "│", expected),
        "카드 border는 custom RGB 전경색을 써야 한다"
    );
}

#[test]
fn 핀_색상이_없으면_기존_강조색_렌더링을_유지한다() {
    use ratatui::backend::TestBackend;

    static TAGS: &[&str] = &[];
    let tool: &'static ToolMeta = Box::leak(Box::new(ToolMeta {
        id: "test.pin_color_default_card",
        toolkit: "test",
        local_id: "pin_color_default_card",
        tags: TAGS,
        display_label: "Default Pin Color Card",
        description: "Test card for default pin color rendering",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U2T,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &[],
    }));

    let area = Rect::new(0, 0, 80, 24);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let tools = [tool];
    let state = fresh();
    let pin_colors = [None];

    terminal
        .draw(|f| render_with_pin_colors(f, &state, &tools, &pin_colors))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let layout = tui_layout(area);
    let grid = grid_content_area(layout.body());

    assert!(
        buffer_region_has_symbol_with_fg(buffer, grid, "─", Color::DarkGray)
            || buffer_region_has_symbol_with_fg(buffer, grid, "│", Color::DarkGray),
        "custom 색상이 없으면 unselected border는 기존 DarkGray를 유지해야 한다"
    );
}

mod approval_running;
mod controlled_embed;
mod form_constraints;
