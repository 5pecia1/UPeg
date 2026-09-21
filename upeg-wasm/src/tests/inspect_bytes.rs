use super::*;

/// Same fixture `wasm_e2e.rs` uses — declares `test.wasm.echo` /
/// `test.wasm.shout` under toolkit `test`.
static TEST_PLUGIN_WASM: &[u8] = include_bytes!("../../tests/fixtures/test_plugin.wasm");

#[test]
fn valid_plugin_bytes_return_tool_id_list() {
    let inspection = inspect_bytes(TEST_PLUGIN_WASM).expect("fixture plugin should validate");
    assert_eq!(
        inspection,
        PluginInspection {
            toolkit: "test".to_string(),
            tool_ids: vec!["test.wasm.echo".to_string(), "test.wasm.shout".to_string()],
        }
    );
}

#[test]
fn corrupted_bytes_error_at_inspect_stage() {
    let result = inspect_bytes(b"not a wasm module");
    assert!(matches!(result, Err(LoadError::Extism(_))));
}

#[test]
fn inspect_registers_nothing_in_registry() {
    // Neither id exists before the call...
    assert!(upeg_runtime::toolbox_tool("test.wasm.echo").is_none());
    assert!(upeg_runtime::toolbox_tool("test.wasm.shout").is_none());

    let _ = inspect_bytes(TEST_PLUGIN_WASM).expect("fixture plugin should validate");

    // ...and inspecting must not have registered them either.
    assert!(upeg_runtime::toolbox_tool("test.wasm.echo").is_none());
    assert!(upeg_runtime::toolbox_tool("test.wasm.shout").is_none());
}
