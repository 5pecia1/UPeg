use super::*;
use syn::ItemFn;

#[test]
fn file_policy_is_explicitly_rejected_on_plugin_output() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.render", toolkit = "files", pegboard_units = U1, outputs = [
            result: File(extensions=["png"], max_count=2)
        ]"#,
    )
    .expect("the shared grammar must parse the File policy");
    let item_fn: ItemFn = syn::parse_str("pub fn render() -> String { String::new() }")
        .expect("must parse the tool function");

    let err = build_tool_decl_fn(&args, &item_fn, "Render", None)
        .expect_err("input-only File policy must be rejected on plugin output");

    assert!(
        err.to_string().contains("`File(...)` policy is input-only"),
        "the error must explain the File policy is input-only: {err}"
    );
}

#[test]
fn supported_plugin_output_params_and_policy_free_file_still_allowed() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.render", toolkit = "files", pegboard_units = U1, outputs = [
            result: File,
            format: Options(["png", "jpg"]),
            preview: EmbeddedView("https://example.com/preview")
        ]"#,
    )
    .expect("must parse the supported output grammar");
    let item_fn: ItemFn = syn::parse_str("pub fn render() -> String { String::new() }")
        .expect("must parse the tool function");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Render", None)
        .expect("must generate the decl including existing output params")
        .to_string();

    assert!(tokens.contains("PluginOutputKind :: File"));
    assert!(tokens.contains("PluginOutputKind :: Options"));
    assert!(tokens.contains("\"png\""));
    assert!(tokens.contains("\"jpg\""));
    assert!(tokens.contains("PluginOutputKind :: EmbeddedView"));
}
