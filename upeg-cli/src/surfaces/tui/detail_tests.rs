//! TUI detail-view regression tests, split out for the file-size budget.

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
                InputName::new("input").expect("test input name"),
                None,
                None,
                true,
                InputKind::String,
            )
            .expect("test input field"),
        ])
        .expect("test input spec"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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
fn detail_view_shows_embed_metadata() {
    let id = "test.iter142.tui_embed";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be in canonical form")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "iter 142 test fixture",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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

    let tools = vec![upeg_runtime::toolbox_tool(id).expect("tool must be registered")];
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
        "TUI Detail must show embed_url. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("iter142.example"),
        "TUI Detail must show the URL value. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("controlled_embed.bindings"),
        "TUI Detail must show the controlled_embed.bindings header. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("input"),
        "TUI Detail must show the binding field. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains(".q"),
        "TUI Detail must show the binding selector. rendered: {buf_string}"
    );
}

#[test]
fn detail_view_shows_input_schema() {
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
        "TUI Detail must show the inputs header. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(string, required)"),
        "TUI Detail must show the field type and required marker. rendered: {buf_string}"
    );
}

#[test]
fn detail_view_shows_none_when_schema_empty() {
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
        "TUI Detail must show the inputs header even with an empty schema. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(none)"),
        "TUI Detail must show `(none)` for a no-arg tool. rendered: {buf_string}"
    );
}
