use crate::surfaces::mcp::*;
use serde_json::json;

#[test]
fn tools_list_exposes_external_readiness_only_for_visible_external_tools() {
    let dir = std::env::temp_dir().join(format!("upeg-mcp-readiness-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    std::fs::write(
        dir.join("readiness.toml"),
        r#"id = "readiness"
[[tools]]
id = "missing"
description = "readiness fixture"
pegboard_units = "U1"
invoker = "External"
command = "__upeg_missing_mcp_readiness__"
surfaces = ["mcp"]
"#,
    )
    .expect("fixture manifest");
    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "fixture should load: {:?}",
        outcome.failed
    );

    let response = handle(json!({ "jsonrpc": "2.0", "id": 884, "method": "tools/list" }))
        .expect("tools list response");
    let tool = response["result"]["tools"]
        .as_array()
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool["name"] == "readiness.missing")
        })
        .expect("loaded external tool");
    assert_eq!(
        tool["_meta"]["upeg/readiness"]["status"],
        "missing_executable"
    );
    assert_eq!(
        tool["_meta"]["upeg/readiness"]["command"],
        "__upeg_missing_mcp_readiness__"
    );
}
