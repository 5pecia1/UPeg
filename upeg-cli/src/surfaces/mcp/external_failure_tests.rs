//! MCP `tools/call` behaviour for a failing `External` tool.
//!
//! `content[0].text` is what an MCP client actually reads, so a failing
//! command has to hand the agent the diagnostics it wrote — `cargo`,
//! clippy, and `flutter analyze` all put their findings on stdout,
//! which the pre-envelope dispatcher discarded outright.

use super::*;
use serde_json::json;

const TOOL_ID: &str = "extfail.run";
const STDOUT_MARKER: &str = "warning-on-stdout";
const STDERR_MARKER: &str = "error-on-stderr";
const EXIT_CODE: i64 = 3;

fn toolkit_toml() -> String {
    format!(
        r#"
id = "extfail"

[[tools]]
id = "run"
description = "External failure fixture"
pegboard_units = "U1"
invoker = "External"
command = "sh"
args_template = ["-c", "echo {STDOUT_MARKER}; echo {STDERR_MARKER} >&2; exit {EXIT_CODE}"]
surfaces = ["cli", "mcp"]
"#
    )
}

fn ensure_fixture_loaded() {
    let dir =
        std::env::temp_dir().join(format!("upeg-external-failure-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir is created");
    std::fs::write(dir.join("extfail.toml"), toolkit_toml()).expect("fixture TOML is written");
    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "External failure fixture must load: {:?}",
        outcome.failed
    );
}

#[test]
fn failing_external_tool_returns_diagnostic_text_and_structured_details() {
    ensure_fixture_loaded();

    let response = handle(json!({
        "jsonrpc": "2.0", "id": 900, "method": "tools/call",
        "params": { "name": TOOL_ID, "arguments": {} },
    }))
    .expect("JSON-RPC response");
    let result = &response["result"];

    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"]
        .as_str()
        .expect("error content is text");
    assert!(text.starts_with("`sh` exited with code 3"), "{text}");
    assert!(text.contains(STDERR_MARKER), "{text}");
    assert!(
        text.contains(STDOUT_MARKER),
        "agents need the stdout diagnostics: {text}"
    );

    let details = &result["structuredContent"]["error"]["details"];
    assert_eq!(details["exit_code"], EXIT_CODE);
    assert!(
        details["stdout"]
            .as_str()
            .expect("stdout detail is a string")
            .contains(STDOUT_MARKER)
    );
}
