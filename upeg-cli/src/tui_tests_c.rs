//! TUI detail-view regression tests.
//!
//! Split from `tui_tests_b.rs` to keep each Rust test source within the
//! workspace file-size budget.

use crate::surfaces::tui::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn 상세_보기는_embed_메타데이터를_표시한다() {
    // Iter 142: parity with `upeg tool show` text (iter 91/93/124).    // An Embed tool inspected via TUI Detail must show its URL +    // bindings; pre-iter-142 they were silently absent.
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
        output_spec: OutputSpec::empty(),
        primary_output_id: None,
        source: Source::UserInput,
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
        }],
    );

    // The shared list only shows pinned tools; detail rendering still accepts
    // runtime metadata directly, which is the behavior this test exercises.
    let tools = vec![upeg_runtime::toolbox_tool(id).expect("tool registered")];
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
        "TUI Detail must surface embed_url; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("iter142.example"),
        "TUI Detail must show the URL value; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("selector_bindings"),
        "TUI Detail must surface selector_bindings header; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("input"),
        "TUI Detail must show binding field; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains(".q"),
        "TUI Detail must show binding selector; rendered: {buf_string}"
    );
}

#[test]
fn 상세_보기는_입력_schema를_표시한다() {
    // Iter 181: TUI Detail must show the tool's inputs section    // (mirroring the iter-180 CLI text fix). Pre-iter-181 users    // could see the schema only by pressing 'r' to enter Form
    // view, committing to run for what's just inspection.
    let tools = list_tools();
    let cursor = tools
        .iter()
        .position(|t| t.id == "num.hex_to_decimal")
        .expect("hex_to_decimal is a built-in");
    let state = State {
        cursor,
        view: View::Detail,
        ..State::default()
    };

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf_string.contains("inputs"),
        "TUI Detail must surface inputs header; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(string, required)"),
        "TUI Detail must show field type + required marker; rendered: {buf_string}"
    );
}

#[test]
fn schema가_비어_있으면_상세_보기는_입력_없음을_알린다() {
    // Iter 181 sibling case: zero-arg tools (id.uuid_v7) get the    // "(none)" placeholder, same convention as boards and the CLI text fix.
    let tools = list_tools();
    let cursor = tools
        .iter()
        .position(|t| t.id == "id.uuid_v7")
        .expect("uuid_v7 is a built-in");
    let state = State {
        cursor,
        view: View::Detail,
        ..State::default()
    };

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf_string = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf_string.contains("inputs"),
        "TUI Detail must surface inputs header even when schema is empty; rendered: {buf_string}"
    );
    assert!(
        buf_string.contains("(none)"),
        "TUI Detail must show `(none)` for zero-arg tools; rendered: {buf_string}"
    );
}
