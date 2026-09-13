//! `input_tests` 모듈의 짝 — Detail 보기 회귀 테스트만 모은다.
//! 워크스페이스 1000-LoC 파일 크기 예산 때문에 분리했다.

use super::*;

#[test]
fn 상세_보기는_embed_메타데이터를_표시한다() {
    // 142회차: `upeg tool show` 텍스트와의 동등성(91/93/124회차).
    // TUI Detail에서 검사한 Embed 도구는 URL과 바인딩을 보여줘야 한다.
    // 142회차 전에는 이들이 조용히 빠져 있었다.
    use ratatui::backend::TestBackend;

    // embed_url과 바인딩이 있는 런타임 Embed 도구를 등록한다.
    let id = "test.iter142.tui_embed";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "iter 142 test fixture",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Embed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Static,
        surfaces: &[upeg_core::Surface::Tui],
        boards: &[],
    });
    upeg_runtime::register_embed_url(id, "https://iter142.example/");
    upeg_runtime::set_selector_bindings(
        id,
        vec![upeg_runtime::SelectorBinding {
            role: upeg_runtime::BindingRole::Input,
            field: "input".into(),
            selector: ".q".into(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        }],
    );

    let tools = vec![upeg_runtime::toolbox_tool(id).expect("도구가 등록되어야 한다")];

    // 커서를 해당 도구로 옮기고 Detail로 전환한다.
    let state = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };

    // Detail 본문이 들어가도록 넉넉한 크기로 렌더링한다.
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf_string.contains("embed_url"),
        "TUI Detail은 embed_url을 표시해야 한다. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("iter142.example"),
        "TUI Detail은 URL 값을 보여줘야 한다. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("controlled_embed.bindings"),
        "TUI Detail은 controlled_embed.bindings 헤더를 표시해야 한다. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("input"),
        "TUI Detail은 바인딩 필드를 보여줘야 한다. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains(".q"),
        "TUI Detail은 바인딩 선택자를 보여줘야 한다. rendered: {buf_string}"
    );
}

#[test]
fn 상세_보기는_입력_schema를_표시한다() {
    // 181회차: TUI Detail은 도구의 inputs 섹션을 보여줘야 한다
    // (180회차 CLI 텍스트 수정과 대응). 181회차 전 사용자는 단순 검사만
    // 하려 해도 'r'을 눌러 Form 보기로 들어가야 스키마를 볼 수 있었다.
    // hex_to_decimal에는 필수 문자열 필드 `input`이 하나 있다.
    use ratatui::backend::TestBackend;

    let tools = vec![detail_input_tool()];
    let state = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf_string.contains("inputs"),
        "TUI Detail은 inputs 헤더를 표시해야 한다. rendered: {buf_string}"
    );
    // 필드 행 형태: 이름 (타입, 필수 여부) — 설명.
    assert!(
        buf_string.contains("(string, required)"),
        "TUI Detail은 필드 타입과 필수 표시를 보여줘야 한다. rendered: {buf_string}"
    );
}

#[test]
fn 상세_보기는_schema가_비어_있으면_없음을_표시한다() {
    // 181회차 짝 케이스: 무인자 도구(id.uuid_v7)는 보드와 CLI 텍스트 수정과
    // 같은 관례로 "(none)" 자리표시자를 받는다.
    use ratatui::backend::TestBackend;

    let tools = vec![detail_no_input_tool()];
    let state = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf_string.contains("inputs"),
        "스키마가 비어 있어도 TUI Detail은 inputs 헤더를 표시해야 한다. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(none)"),
        "TUI Detail은 무인자 도구에 `(none)`을 보여줘야 한다. rendered: {buf_string}"
    );
}
