use super::*;
use syn::ItemFn;

#[test]
fn file_정책은_plugin_output에서_명시적으로_거부된다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.render", toolkit = "files", pegboard_units = U1, outputs = [
            result: File(extensions=["png"], max_count=2)
        ]"#,
    )
    .expect("공용 문법은 File 정책을 파싱해야 한다");
    let item_fn: ItemFn = syn::parse_str("pub fn render() -> String { String::new() }")
        .expect("도구 함수를 파싱해야 한다");

    let err = build_tool_decl_fn(&args, &item_fn, "Render", None)
        .expect_err("입력 전용 File 정책을 plugin output에서 거부해야 한다");

    assert!(
        err.to_string().contains("`File(...)` policy is input-only"),
        "오류는 File 정책이 입력 전용임을 설명해야 한다: {err}"
    );
}

#[test]
fn 지원되는_plugin_output_파라미터와_정책없는_file은_계속_허용된다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.render", toolkit = "files", pegboard_units = U1, outputs = [
            result: File,
            format: Options(["png", "jpg"]),
            preview: EmbeddedView("https://example.com/preview")
        ]"#,
    )
    .expect("지원되는 output 문법을 파싱해야 한다");
    let item_fn: ItemFn = syn::parse_str("pub fn render() -> String { String::new() }")
        .expect("도구 함수를 파싱해야 한다");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Render", None)
        .expect("기존 output 파라미터를 포함한 선언을 생성해야 한다")
        .to_string();

    assert!(tokens.contains("PluginOutputKind :: File"));
    assert!(tokens.contains("PluginOutputKind :: Options"));
    assert!(tokens.contains("\"png\""));
    assert!(tokens.contains("\"jpg\""));
    assert!(tokens.contains("PluginOutputKind :: EmbeddedView"));
}
