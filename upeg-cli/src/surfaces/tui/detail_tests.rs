//! 파일 크기 예산 때문에 분리한 TUI 상세 보기 회귀 테스트.

use crate::surfaces::tui::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use upeg_core::{InputFieldSpec, InputKind, InputName, InputSpec};

fn detail_input_tool() -> &'static upeg_core::ToolMeta {
    Box::leak(Box::new(upeg_core::ToolMeta {
        id: "num.hex_to_decimal",
        toolkit: "convert",
        local_id: "hex_to_decimal",
        tags: &[],
        display_label: "Hex to Dec",
        description: "Parse a hex string into decimal.",
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
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Tui],
        boards: &["dev"],
    }))
}

fn detail_no_input_tool() -> &'static upeg_core::ToolMeta {
    static TOOL: upeg_core::ToolMeta = upeg_core::ToolMeta {
        id: "id.uuid_v7",
        toolkit: "id",
        local_id: "uuid_v7",
        tags: &[],
        display_label: "UUID v7",
        description: "Generate a UUID v7.",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Tui],
        boards: &["dev"],
    };
    &TOOL
}

#[test]
fn 상세_보기는_embed_메타데이터를_표시한다() {
    let id = "test.iter142.tui_embed";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("테스트 ToolMeta id는 정규 형식이어야 한다")
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
    let state = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };

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
    assert!(
        buf_string.contains("(string, required)"),
        "TUI Detail은 필드 타입과 필수 표시를 보여줘야 한다. rendered: {buf_string}"
    );
}

#[test]
fn 상세_보기는_schema가_비어_있으면_없음을_표시한다() {
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
