//! Companion to the `render_tests` module — collects only the
//! Controlled Embed runtime-dispatch and render-contract regression
//! tests. Split out for the workspace 1000-LoC file-size budget.

use super::*;

#[test]
fn controlled_embed_runtime_success_delivers_canonical_output_to_tui_result_view() {
    use ratatui::backend::TestBackend;
    use std::sync::Arc;

    struct FakeControlledEmbedBackend;

    impl upeg_runtime::controlled_embed::ControlledEmbedBackend for FakeControlledEmbedBackend {
        fn run(
            &self,
            request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
        ) -> Result<
            upeg_runtime::controlled_embed::ControlledEmbedResponse,
            upeg_runtime::controlled_embed::ControlledEmbedError,
        > {
            assert_eq!(request.url, "https://example.test/tool");
            assert!(
                request
                    .inputs
                    .iter()
                    .any(|(field, value)| *field == "query" && *value == "upeg"),
                "runtime dispatch must pass CLI/TUI input pairs into the backend"
            );
            Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
                outputs: vec![("summary".into(), "첫 줄\n둘째 줄 🙂".into())],
            })
        }
    }

    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        FakeControlledEmbedBackend,
    ));

    const TOOL_ID: &str = "cetui.run";
    let dir =
        std::env::temp_dir().join(format!("upeg-controlled-embed-tui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    std::fs::write(
        dir.join("cetui.toml"),
        r##"id = "cetui"

[[tools]]
id = "run"
description = "Controlled Embed fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.test/tool"
surfaces = ["tui"]
primary_output_id = "summary"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "summary"
type = "string"
label = "Summary"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[tools.controlled_embed.bindings]]
role = "output"
field = "summary"
selector = "#summary"
"##,
    )
    .expect("write cetui fixture");
    let loaded = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        loaded.failed.is_empty(),
        "Controlled Embed TUI fixture must load without failures: {:?}",
        loaded.failed
    );

    let outcome = crate::dispatch_tool(TOOL_ID, &serde_json::json!({"query": "upeg"}));

    let mut state = State::default();
    apply_outcome(&mut state, TOOL_ID, outcome);

    // Verify state has Result view with canonical output
    match &state.view {
        View::Result {
            outputs,
            text,
            is_error,
            ..
        } => {
            assert!(!is_error, "should not be error");
            assert!(text.is_empty(), "should have no fallback text");
            assert_eq!(outputs.len(), 1);
            let out = &outputs[0];
            assert_eq!(out.id, "summary");
            assert_eq!(out.label.as_deref(), Some("Summary"));
            assert!(matches!(&out.value, OutputValue::String(s) if s == "첫 줄\n둘째 줄 🙂"));
        }
        other => panic!("Expected Result view, got: {other:?}"),
    }

    // Render and verify buffer contains output
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    assert!(buf.contains("Summary"), "rendered: {buf}");
    assert!(buf.contains("첫 줄"), "rendered: {buf}");
    assert!(buf.contains("둘째 줄"), "rendered: {buf}");
    // Result pane shows canonical output, not browser/url-specific fallback
    // Note: other tools may have "WebView" in their descriptions (left pane)
}

#[test]
fn controlled_embed_wait_timeout_dispatch_reaches_tui_result_view_as_error() {
    use ratatui::backend::TestBackend;
    use std::sync::Arc;

    const TOOL_ID: &str = "cetui.wait_timeout_dispatch";
    const WAIT_TIMEOUT_CODE: &str =
        upeg_runtime::controlled_embed::CONTROLLED_EMBED_WAIT_TIMEOUT_CODE;
    const WAIT_TIMEOUT_SELECTOR: &str = "#summary";
    const WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
    const WAIT_TIMEOUT_MS: u64 = 125;

    struct WaitTimeoutControlledEmbedBackend;

    impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitTimeoutControlledEmbedBackend {
        fn run(
            &self,
            request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
        ) -> Result<
            upeg_runtime::controlled_embed::ControlledEmbedResponse,
            upeg_runtime::controlled_embed::ControlledEmbedError,
        > {
            assert_eq!(request.url, "https://example.test/tool");
            assert!(
                request
                    .inputs
                    .iter()
                    .any(|(field, value)| *field == "query" && *value == "upeg"),
                "runtime dispatch must pass TUI input pairs into the backend"
            );
            Err(
                upeg_runtime::controlled_embed::ControlledEmbedError::WaitTimeout {
                    role: upeg_core::BindingRole::Output,
                    selector: WAIT_TIMEOUT_SELECTOR.into(),
                    for_selector: WAIT_TIMEOUT_FOR_SELECTOR.into(),
                    condition: upeg_core::BindingWaitCondition::Visible,
                    timeout_ms: WAIT_TIMEOUT_MS,
                },
            )
        }
    }

    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        WaitTimeoutControlledEmbedBackend,
    ));

    let dir = std::env::temp_dir().join(format!(
        "upeg-controlled-embed-tui-wait-timeout-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    std::fs::write(
        dir.join("cetui.toml"),
        r##"id = "cetui"

[[tools]]
id = "wait_timeout_dispatch"
description = "Controlled Embed wait-timeout fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.test/tool"
surfaces = ["tui"]
primary_output_id = "summary"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "summary"
type = "string"
label = "Summary"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[tools.controlled_embed.bindings]]
role = "output"
field = "summary"
selector = "#summary"
"##,
    )
    .expect("write cetui wait-timeout fixture");
    let loaded = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        loaded.failed.is_empty(),
        "Controlled Embed TUI wait-timeout fixture must load without failures: {:?}",
        loaded.failed
    );

    let outcome = crate::dispatch_tool(TOOL_ID, &serde_json::json!({"query": "upeg"}));

    let mut state = State::default();
    apply_outcome(&mut state, TOOL_ID, outcome);

    match &state.view {
        View::Result {
            outputs,
            text,
            is_error,
            ..
        } => {
            assert!(*is_error, "wait timeout must render as an error");
            assert!(outputs.is_empty(), "wait timeout must not invent outputs");
            assert!(text.contains(WAIT_TIMEOUT_CODE), "text: {text}");
            assert!(text.contains(WAIT_TIMEOUT_SELECTOR), "text: {text}");
            assert!(text.contains(WAIT_TIMEOUT_FOR_SELECTOR), "text: {text}");
            assert!(text.contains("Visible"), "text: {text}");
            assert!(text.contains("125ms"), "text: {text}");
        }
        other => panic!("Expected Result view, got: {other:?}"),
    }

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    assert!(buf.contains("ERROR"), "rendered: {buf}");
    assert!(buf.contains(WAIT_TIMEOUT_CODE), "rendered: {buf}");
    assert!(buf.contains(WAIT_TIMEOUT_FOR_SELECTOR), "rendered: {buf}");
    assert!(buf.contains("125ms"), "rendered: {buf}");
}

/// Controlled Embed success result: renders long unicode output in the
/// right pane
/// (Task 5: Direct canonical output rendering without fixture setup)
#[test]
fn controlled_embed_success_result_renders_long_unicode_output_in_right_pane() {
    use ratatui::backend::TestBackend;

    // Build direct ToolSuccess with OutputEntry containing Unicode multiline
    let success = ToolSuccess::new(
        Some("summary".into()),
        vec![OutputEntry {
            id: "summary".into(),
            label: Some("Summary".into()),
            kind: OutputKind::String,
            value: OutputValue::String("첫 줄\n둘째 줄 🙂".into()),
        }],
    )
    .unwrap();

    let mut state = State::default();
    apply_outcome(&mut state, "test.ce", Outcome::Success(success));

    // Render with TestBackend
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    // Verify output appears in buffer
    assert!(buf.contains("Summary"), "rendered: {buf}");
    assert!(buf.contains("첫 줄"), "rendered: {buf}");
    assert!(buf.contains("둘째 줄"), "rendered: {buf}");
}

/// Controlled Embed failure result: renders only the error message
/// (Task 5: Failure rendering without browser fallback)
#[test]
fn controlled_embed_failure_result_renders_only_error_message() {
    use ratatui::backend::TestBackend;

    // Build Outcome::Failure with controlled_embed_unavailable code
    let failure = dispatch_failure(
        "controlled_embed_unavailable",
        "Controlled Embed feature is disabled",
    );

    let mut state = State::default();
    apply_outcome(&mut state, "cetui.run", Outcome::Failure(failure));

    // Verify state shows error
    match &state.view {
        View::Result {
            outputs,
            text,
            is_error,
            ..
        } => {
            assert!(*is_error, "should be error");
            assert!(!text.is_empty(), "should have error text");
            assert!(
                text.contains("Controlled Embed feature is disabled"),
                "text: {text}"
            );
            assert!(outputs.is_empty(), "should have no outputs");
        }
        other => panic!("Expected Result view, got: {other:?}"),
    }

    // Render and verify buffer contains error message
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    assert!(
        buf.contains("Controlled Embed feature") || buf.contains("disabled"),
        "rendered: {buf}"
    );
}

#[test]
fn controlled_embed_wait_timeout_tui_renders_wait_timeout_error_as_standard_message() {
    use ratatui::backend::TestBackend;

    const WAIT_TIMEOUT_CODE: &str = "wait-timeout";
    const WAIT_TIMEOUT_MESSAGE: &str = "wait-timeout: Output binding `#result` waiting for `#ready` condition `Visible` timed out after 125ms";
    const FORBIDDEN_URL_FALLBACK: &str = "https://example.test/wait";

    let failure = dispatch_failure(WAIT_TIMEOUT_CODE, WAIT_TIMEOUT_MESSAGE);
    assert_eq!(failure.error.code, WAIT_TIMEOUT_CODE);

    let mut state = State::default();
    apply_outcome(&mut state, "cetui.wait_timeout", Outcome::Failure(failure));

    match &state.view {
        View::Result {
            outputs,
            text,
            is_error,
            ..
        } => {
            assert!(*is_error, "wait timeout must render as an error");
            assert!(outputs.is_empty(), "wait timeout must not invent outputs");
            assert_eq!(text, WAIT_TIMEOUT_MESSAGE);
            assert!(text.contains(WAIT_TIMEOUT_CODE), "text: {text}");
            assert!(text.contains("#ready"), "text: {text}");
            assert!(!text.contains(FORBIDDEN_URL_FALLBACK), "text: {text}");
        }
        other => panic!("Expected Result view, got: {other:?}"),
    }

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    assert!(buf.contains("ERROR"), "rendered: {buf}");
    assert!(buf.contains(WAIT_TIMEOUT_CODE), "rendered: {buf}");
    assert!(buf.contains("#ready"), "rendered: {buf}");
    assert!(buf.contains("125ms"), "rendered: {buf}");
    assert!(!buf.contains(FORBIDDEN_URL_FALLBACK), "rendered: {buf}");
}

#[test]
fn controlled_embed_wait_timeout_tui_renders_successful_wait_output_row() {
    use ratatui::backend::TestBackend;

    const OUTPUT_ID: &str = "waited_result";
    const OUTPUT_LABEL: &str = "Waited result";
    const OUTPUT_VALUE: &str = "ready after wait";

    let success = ToolSuccess::new(
        Some(OUTPUT_ID.into()),
        vec![OutputEntry {
            id: OUTPUT_ID.into(),
            label: Some(OUTPUT_LABEL.into()),
            kind: OutputKind::String,
            value: OutputValue::String(OUTPUT_VALUE.into()),
        }],
    )
    .unwrap();

    let mut state = State::default();
    apply_outcome(&mut state, "cetui.waited", Outcome::Success(success));

    match &state.view {
        View::Result {
            outputs,
            text,
            is_error,
            ..
        } => {
            assert!(!is_error, "waited output must render as success");
            assert!(
                text.is_empty(),
                "canonical outputs should avoid fallback text"
            );
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].label.as_deref(), Some(OUTPUT_LABEL));
            assert_eq!(outputs[0].value, OutputValue::String(OUTPUT_VALUE.into()));
        }
        other => panic!("Expected Result view, got: {other:?}"),
    }

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, &state, &tools)).unwrap();
    let buf = format!("{:?}", terminal.backend().buffer());

    assert!(buf.contains("OK"), "rendered: {buf}");
    assert!(buf.contains(OUTPUT_LABEL), "rendered: {buf}");
    assert!(buf.contains(OUTPUT_VALUE), "rendered: {buf}");
    assert!(!buf.contains("wait-timeout"), "rendered: {buf}");
}
