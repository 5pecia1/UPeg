#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Baseline architecture characterization tests.
//!
//! These pin the user-visible contracts that the command-runner migration and
//! follow-on refactors must preserve: CLI discovery/calls, protocol tools/list
//! shapes, and the pure TUI state machine.

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use std::process::Command;
use tower::ServiceExt;
use upeg_cli::{
    Action, Key, Outcome, State, View, apply_outcome, handle_key, handle_mcp_message, http_router,
};
use upeg_core::{InputSpec, Invoker, PinKind, Surface, ToolMeta};

const fn upeg_bin() -> &'static str {
    env!("CARGO_BIN_EXE_upeg")
}

fn run_upeg(args: &[&str]) -> String {
    let output = Command::new(upeg_bin())
        .args(args)
        // The repo root carries a dogfood `upeg.toml` (the README's
        // "Project Manifest" row). These baselines pin the *built-in*
        // toolbox, so detection has to be off or `dev.*` leaks in.
        .env(
            upeg_sources::project::PROJECT_MANIFEST_PATH_ENV,
            upeg_sources::project::PROJECT_MANIFEST_OVERRIDE_OFF,
        )
        .output()
        .unwrap_or_else(|err| panic!("run upeg {args:?}: {err}"));
    assert!(
        output.status.success(),
        "upeg {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("upeg stdout is utf-8")
}

fn find_tool<'a>(body: &'a Value, id: &str) -> &'a Value {
    body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|tool| tool["name"] == id)
        .unwrap_or_else(|| panic!("missing {id} in tools/list body: {body}"))
}

#[test]
fn cli의_도구_목록과_hex_to_dec_호출은_안정적이다() {
    let list = run_upeg(&["tool", "list"]);
    assert!(
        list.contains("num.hex_to_decimal"),
        "tool list missing num.hex_to_decimal:\n{list}"
    );

    let dynamic = run_upeg(&["num", "hex-to-decimal", "0xff"]);
    assert_eq!(dynamic.trim(), "255");

    let explicit = run_upeg(&["call", "num.hex_to_decimal", "-a", "input=0xff"]);
    assert_eq!(explicit.trim(), "255");
}

#[tokio::test]
async fn http_도구_목록은_내장_픽스처와_같은_형태로_노출된다() {
    let response = http_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/tools")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("http tools/list response");
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let body: Value = serde_json::from_slice(&bytes).expect("json body");
    let tool = find_tool(&body, "num.hex_to_decimal");
    assert_eq!(tool["toolkit"], "num");
    assert_eq!(tool["tool"], "hex_to_decimal");
    assert!(tool["inputSchema"].is_object(), "tool shape: {tool}");
    assert_eq!(tool["pegboardUnits"], "U2");
    assert_eq!(
        tool["pegboardSpan"],
        serde_json::json!({ "cols": 2, "rows": 1 })
    );
}

#[test]
fn mcp_도구_목록은_내장_픽스처와_같은_형태로_노출된다() {
    let body = handle_mcp_message(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    }))
    .expect("mcp tools/list response")["result"]
        .clone();

    let tool = find_tool(&body, "num.hex_to_decimal");
    assert_eq!(tool["toolkit"], "num");
    assert_eq!(tool["tool"], "hex_to_decimal");
    assert!(tool["inputSchema"].is_object(), "tool shape: {tool}");
    assert_eq!(tool["pegboardUnits"], "U2");
    assert_eq!(
        tool["pegboardSpan"],
        serde_json::json!({ "cols": 2, "rows": 1 })
    );
}

fn tui_fixture_tools() -> [&'static ToolMeta; 2] {
    static SIMPLE: ToolMeta = ToolMeta {
        id: "test.simple",
        toolkit: "test",
        local_id: "simple",
        tags: &[],
        display_label: "Test tool",
        description: "Simple no-arg tool",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    };
    let with_input = Box::leak(Box::new(ToolMeta {
        id: "test.with_input",
        toolkit: "test",
        local_id: "with_input",
        tags: &[],
        display_label: "Test tool",
        description: "Tool that takes an input string",
        input_spec: InputSpec::try_from(&json!({
            "type": "object",
            "properties": { "input": { "type": "string" } },
            "required": ["input"],
            "additionalProperties": false,
        }))
        .expect("test input spec should import"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    }));
    [&SIMPLE, with_input]
}

#[test]
fn tui_상태_머신_전환은_순수하고_안정적이다() {
    let tools = tui_fixture_tools();
    let mut state = State::default();

    assert_eq!(handle_key(&mut state, Key::Down, &tools), Action::None);
    assert_eq!(state.cursor, 1);

    // Enter always runs/confirms; inspecting a tool's manifest moved to
    // its own key, `o` (see upeg-core/src/keyboard.rs's Board scope).
    assert_eq!(handle_key(&mut state, Key::Char('o'), &tools), Action::None);
    assert_eq!(state.view, View::Detail);

    assert_eq!(handle_key(&mut state, Key::F(1), &tools), Action::None);
    match &state.view {
        View::Form { tool_id, form } => {
            assert_eq!(*tool_id, "test.with_input");
            assert_eq!(form.focused, 0);
            assert_eq!(form.fields[0].name.as_str(), "input");
        }
        other => panic!("expected form view, got {other:?}"),
    }

    for ch in ['0', 'x', 'f', 'f'] {
        assert_eq!(handle_key(&mut state, Key::Char(ch), &tools), Action::None);
    }
    let action = handle_key(&mut state, Key::Enter, &tools);
    // `Action::Dispatch` also carries the run identity the model minted
    // (`State::active_run`), which is per-session by construction — the
    // baseline pins the tool and the args it dispatches with.
    match &action {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(*tool_id, "test.with_input");
            assert_eq!(*args, json!({ "input": "0xff" }));
        }
        other => panic!("expected dispatch action, got {other:?}"),
    }

    let expected_success = upeg_cli::text_success("255");
    let expected_outputs = expected_success.outputs.clone();
    apply_outcome(
        &mut state,
        "test.with_input",
        Outcome::Success(expected_success),
    );
    assert_eq!(
        state.view,
        View::Result {
            tool_id: "test.with_input",
            outputs: expected_outputs,
            text: String::new(),
            is_error: false,
        }
    );

    assert_eq!(handle_key(&mut state, Key::Esc, &tools), Action::None);
    assert_eq!(state.view, View::List);
    // Modeless: Esc at the board root no longer quits; it is a no-op.
    assert_eq!(handle_key(&mut state, Key::Esc, &tools), Action::None);
    assert_eq!(state.view, View::List);
    // Quitting now routes through the confirm-quit overlay: `q` opens it,
    // then a confirm (`y`) yields the terminal Quit action.
    assert_eq!(handle_key(&mut state, Key::Char('q'), &tools), Action::None);
    assert_eq!(state.view, View::ConfirmQuit);
    assert_eq!(handle_key(&mut state, Key::Char('y'), &tools), Action::Quit);
}
