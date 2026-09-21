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

//! WASM Toolkit host integration test.
//!
//! Loads a precompiled extism plugin (`fixtures/test_plugin.wasm`) and
//! drives it through the iter 39 runtime dispatcher primitive — the same
//! primitive built-in tools, TOML External tools, and chain Tools all
//! flow through. Source under `tests/fixture-src/` rebuilds the .wasm
//! when the contract changes.

use upeg_wasm::register_from_bytes;

/// Embedded so tests don't need a build orchestration step.
static TEST_PLUGIN_WASM: &[u8] = include_bytes!("fixtures/test_plugin.wasm");

#[test]
fn plugin_load_registers_declared_tools() {
    let ids = register_from_bytes(TEST_PLUGIN_WASM).expect("test plugin should load + register");
    // The fixture's `manifest` export declares two tools whose ids
    // follow upeg's `{toolkit}.{tool}` convention while pointing
    // at differently-named wasm exports via the `export` field.
    assert!(ids.contains(&"test.wasm.echo"));
    assert!(ids.contains(&"test.wasm.shout"));

    // Both must show up in the global registry.
    assert!(upeg_runtime::toolbox_tool("test.wasm.echo").is_some());
    assert!(upeg_runtime::toolbox_tool("test.wasm.shout").is_some());
}

#[test]
fn wasm_tool_execution_runs_plugin_function() {
    let _ = register_from_bytes(TEST_PLUGIN_WASM).expect("load");

    let result = upeg_runtime::try_runtime_dispatch(
        "test.wasm.echo",
        &serde_json::json!({"input": "hello"}),
    );
    match result {
        Some(upeg_core::ToolResult::Success(success)) => {
            assert_eq!(
                upeg_runtime::tool_success_primary_text(&success),
                "echoed: hello"
            );
        }
        other => panic!("expected `echoed: hello`, got {other:?}"),
    }

    let upper = upeg_runtime::try_runtime_dispatch(
        "test.wasm.shout",
        &serde_json::json!({"input": "hello"}),
    );
    match upper {
        Some(upeg_core::ToolResult::Success(success)) => {
            assert_eq!(upeg_runtime::tool_success_primary_text(&success), "HELLO");
        }
        other => panic!("expected `HELLO`, got {other:?}"),
    }
}

#[test]
fn wasm_tool_meta_carries_manifest_description() {
    let _ = register_from_bytes(TEST_PLUGIN_WASM).expect("load");
    let meta = upeg_runtime::toolbox_tool("test.wasm.echo").expect("registered");
    assert!(meta.description.contains("Echo"));
    assert_eq!(meta.toolkit, "test");
}

#[test]
fn reregistering_same_plugin_keeps_lookup_idempotent() {
    // Loading the plugin twice should leave the registry consistent.
    // The second register replaces the dispatcher (tested in upeg-core);
    // we just confirm the tool stays callable.
    let _ = register_from_bytes(TEST_PLUGIN_WASM).expect("first load");
    let _ = register_from_bytes(TEST_PLUGIN_WASM).expect("second load");
    let result = upeg_runtime::try_runtime_dispatch(
        "test.wasm.echo",
        &serde_json::json!({"input": "twice"}),
    );
    match result {
        Some(upeg_core::ToolResult::Success(success)) => {
            assert_eq!(
                upeg_runtime::tool_success_primary_text(&success),
                "echoed: twice"
            );
        }
        other => panic!("expected `echoed: twice`, got {other:?}"),
    }
}
