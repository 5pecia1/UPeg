//! Companion to the `input_tests` module — collects only Detail-view
//! regression tests. Split out for the workspace 1000-LoC file-size budget.

use super::*;

#[test]
fn detail_view_shows_embed_metadata() {
    // Iter 142: parity with `upeg tool show` text (iters 91/93/124).
    // An Embed tool inspected in TUI Detail must show its URL and
    // bindings. Before iter 142 they were silently omitted.
    use ratatui::backend::TestBackend;

    // Register a runtime Embed tool that has embed_url and bindings.
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

    // Move the cursor to that tool and switch to Detail.
    let state = State {
        cursor: 0,
        view: View::Detail,
        ..State::default()
    };

    // Render large enough to fit the Detail body.
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
    // Iter 181: TUI Detail must show the tool's inputs section
    // (counterpart to the iter-180 CLI text fix). Before iter 181 a user
    // who only wanted to inspect still had to press 'r' into the Form
    // view to see the schema. hex_to_decimal has one required string
    // field, `input`.
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
        "TUI Detail must show the inputs header. rendered: {buf_string}"
    );
    // Field row shape: name (type, required) — description.
    assert!(
        buf_string.contains("(string, required)"),
        "TUI Detail must show the field type and required marker. rendered: {buf_string}"
    );
}

#[test]
fn detail_view_shows_none_when_schema_empty() {
    // Iter-181 companion case: a no-arg tool (id.uuid_v7) gets the
    // "(none)" placeholder, the same convention as the board and CLI
    // text fixes.
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
        "TUI Detail must show the inputs header even with an empty schema. rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(none)"),
        "TUI Detail must show `(none)` for a no-arg tool. rendered: {buf_string}"
    );
}
