use super::*;

/// Same fixture `wasm_e2e.rs` uses — declares `test.wasm.echo` /
/// `test.wasm.shout` under toolkit `test`.
static TEST_PLUGIN_WASM: &[u8] = include_bytes!("../../tests/fixtures/test_plugin.wasm");

#[test]
fn 유효한_플러그인_바이트는_도구_id_목록을_돌려준다() {
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
fn 손상된_바이트는_검사_단계에서_에러를_반환한다() {
    let result = inspect_bytes(b"not a wasm module");
    assert!(matches!(result, Err(LoadError::Extism(_))));
}

#[test]
fn 검사는_레지스트리에_아무것도_등록하지_않는다() {
    // Neither id exists before the call...
    assert!(upeg_runtime::toolbox_tool("test.wasm.echo").is_none());
    assert!(upeg_runtime::toolbox_tool("test.wasm.shout").is_none());

    let _ = inspect_bytes(TEST_PLUGIN_WASM).expect("fixture plugin should validate");

    // ...and inspecting must not have registered them either.
    assert!(upeg_runtime::toolbox_tool("test.wasm.echo").is_none());
    assert!(upeg_runtime::toolbox_tool("test.wasm.shout").is_none());
}
