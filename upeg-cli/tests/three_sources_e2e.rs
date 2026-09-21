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

//! Three-source coexistence: TOML External + WASM plugin + MCP-managed.
//!
//! All three runtime sources funnel through the same iter 39 dispatcher
//! primitive (`toolbox_add_tool_with_dispatcher`).
//! This test loads one tool from each source into a single process and
//! verifies they're independently dispatchable with the right results —
//! no id collisions, no cross-routing, no shared-state interference.

#![cfg(feature = "wasm-plugin")]

use serde_json::json;
use upeg_cli::{Outcome, UpstreamMcpConfig, dispatch_tool, register_upstream_mcp_server};

static WASM_FIXTURE: &[u8] = include_bytes!("../../upeg-wasm/tests/fixtures/test_plugin.wasm");

fn upeg_mcp_config() -> UpstreamMcpConfig {
    UpstreamMcpConfig {
        command: env!("CARGO_BIN_EXE_upeg").to_string(),
        args: vec!["mcp".to_string()],
        reexport: false,
    }
}

#[test]
fn toml_wasm_and_mcp_tools_each_dispatch_independently() {
    // ─── 1. TOML External ───────────────────────────────────────
    let toml_dir = std::env::temp_dir().join("upeg_three_sources_toml");
    let _ = std::fs::remove_dir_all(&toml_dir);
    std::fs::create_dir_all(&toml_dir).unwrap();
    let toml_id = "iter53.toml.echo";
    // Tokens of form `{key}` substitute as a whole — concatenation
    // ("prefix-{key}") isn't supported per iter 37's contract. Keep
    // literal prefix and substitution token as separate args.
    std::fs::write(
        toml_dir.join("echo.toml"),
        r#"id = "iter53"

[[tools]]
id = "toml.echo"
invoker = "External"
pegboard_units = "U1"
command = "echo"
args_template = ["from-toml", "{tag}"]

[[tools.inputs]]
name = "tag"
type = "string""#,
    )
    .unwrap();
    let outcome = upeg_loader::load_and_register_dir_verbose(&toml_dir);
    assert_eq!(
        outcome.loaded.len(),
        1,
        "TOML registration: {:?}",
        outcome.failed
    );

    // ─── 2. WASM plugin ────────────────────────────────────────
    let wasm_ids =
        upeg_wasm::register_from_bytes(WASM_FIXTURE).expect("wasm plugin should register");
    assert!(
        wasm_ids.contains(&"test.wasm.echo"),
        "wasm fixture must declare test.wasm.echo, got {wasm_ids:?}"
    );

    // ─── 3. MCP-managed (`upeg mcp` self-fixture) ─────────────
    let mcp_ns = "iter53self";
    let mcp_registration = register_upstream_mcp_server(mcp_ns, &upeg_mcp_config())
        .expect("mcp server should register");
    assert!(
        mcp_registration
            .registered_ids()
            .iter()
            .any(|i| i.contains("num.hex_to_decimal")),
        "expected mcp server to expose num.hex_to_decimal"
    );

    // ─── Dispatch each source — every one returns its own result ──
    match dispatch_tool(toml_id, &json!({"tag": "alpha"})) {
        // `echo from-toml alpha\n` — both args echoed, separated by a space.
        Outcome::Success(success) => assert!(
            upeg_runtime::tool_success_primary_text(&success).contains("from-toml")
                && upeg_runtime::tool_success_primary_text(&success).contains("alpha"),
            "TOML dispatch: {:?}",
            upeg_runtime::tool_success_primary_text(&success)
        ),
        other => panic!("TOML route: {other:?}"),
    }

    match dispatch_tool("test.wasm.echo", &json!({"input": "hi"})) {
        Outcome::Success(success) => {
            assert_eq!(
                upeg_runtime::tool_success_primary_text(&success),
                "echoed: hi"
            );
        }
        other => panic!("WASM route: {other:?}"),
    }

    let mcp_id = format!("{mcp_ns}.num.hex_to_decimal");
    match dispatch_tool(&mcp_id, &json!({"input": "0x2a"})) {
        Outcome::Success(success) => {
            assert_eq!(upeg_runtime::tool_success_primary_text(&success), "42");
        }
        other => panic!("MCP route: {other:?}"),
    }

    // ─── Cross-talk check: WASM tool dispatched with TOML's args
    //     does its own thing (uses input.input), not echo's behavior.
    match dispatch_tool("test.wasm.echo", &json!({"tag": "alpha"})) {
        // wasm tool reads `input` field; with no `input` key it falls back
        // to empty string per the fixture source, so we expect "echoed: ".
        Outcome::Success(success) => assert_eq!(
            upeg_runtime::tool_success_primary_text(&success),
            "echoed: ",
            "WASM route should NOT have run TOML's echo — that would be cross-talk"
        ),
        other => panic!("WASM cross-talk check: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&toml_dir);
}
