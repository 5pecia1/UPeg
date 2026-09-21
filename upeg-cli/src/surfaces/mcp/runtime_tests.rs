//! Runtime registration, surface-gating, and extra tools/call MCP tests.

use crate::surfaces::mcp::*;
use serde_json::json;

#[test]
fn server_info_version_matches_cargo_package() {
    // env!("CARGO_PKG_VERSION") drives serverInfo.version. If the upeg-cli
    // version diverges from this constant, MCP clients will see stale data.
    let resp = handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize"})).unwrap();
    let v = resp["result"]["serverInfo"]["version"].as_str().unwrap();
    assert!(!v.is_empty());
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
}

#[test]
fn tool_call_supports_base64_encode_round_trip() {
    let enc = handle(json!({
        "jsonrpc": "2.0", "id": 100, "method": "tools/call",
        "params": { "name": "convert.base64_encode", "arguments": { "input": "hello" } },
    }))
    .unwrap();
    assert_eq!(enc["result"]["content"][0]["text"], "aGVsbG8=");

    let dec = handle(json!({
        "jsonrpc": "2.0", "id": 101, "method": "tools/call",
        "params": { "name": "convert.base64_decode", "arguments": { "input": "aGVsbG8=" } },
    }))
    .unwrap();
    assert_eq!(dec["result"]["content"][0]["text"], "hello");
}

#[test]
fn base64_decode_tool_call_marks_invalid_input_as_error() {
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 102, "method": "tools/call",
        "params": { "name": "convert.base64_decode", "arguments": { "input": "!!!" } },
    }))
    .unwrap();
    assert_eq!(resp["result"]["isError"], true);
}

// The local raw-schema parsing wrapper was dropped after MCP
// tools/list switched to `to_json_object`. Schema object parsing is now
// covered by upeg-core's typed input adapter tests.

#[test]
fn tools_list_input_schema_comes_from_tool_meta() {
    let resp = handle(
        json!({        "jsonrpc":"2.0","id":110,"method":"tools/list",
        }),
    )
    .unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();

    // hex_to_decimal's generated schema should declare a required "input" string.
    let hex = tools
        .iter()
        .find(|t| t["name"] == "num.hex_to_decimal")
        .unwrap();
    assert_eq!(hex["inputSchema"]["type"], "object");
    assert_eq!(hex["inputSchema"]["required"][0], "input");
    assert_eq!(hex["inputSchema"]["properties"]["input"]["type"], "string");
    assert_eq!(hex["inputSchema"]["additionalProperties"], false);
    assert_eq!(hex["outputSchema"]["type"], "object");
    assert_eq!(hex["outputSchema"]["additionalProperties"], false);

    // uuid_v7 takes no args.
    let uuid = tools.iter().find(|t| t["name"] == "id.uuid_v7").unwrap();
    assert_eq!(uuid["inputSchema"]["type"], "object");
    assert!(
        uuid["inputSchema"]["required"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(uuid["inputSchema"]["additionalProperties"], false);
    assert_eq!(uuid["outputSchema"]["type"], "object");
    assert_eq!(uuid["outputSchema"]["additionalProperties"], false);
}

#[test]
fn tools_list_description_comes_from_tool_meta_not_hardcoded() {
    // Descriptions live on `ToolMeta::description` and flow through
    // automatically. Each annotated tool should produce a non-empty,
    // distinct description.
    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 103, "method": "tools/list",
    }))
    .unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();

    let descriptions: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["description"].as_str())
        .filter(|s| !s.is_empty())
        .collect();
    assert!(
        descriptions.len() >= 4,
        "expected ≥4 non-empty descriptions, got {}: {tools:?}",
        descriptions.len(),
    );

    // Spot-check one specific description matches the macro annotation.
    let hex = tools
        .iter()
        .find(|t| t["name"] == "num.hex_to_decimal")
        .unwrap();
    assert!(hex["description"].as_str().unwrap().contains("hex"));
}

/// a tool registered at runtime (TOML loader path, simulated
/// here directly via `toolbox_add_tool` + `register_runtime_dispatcher`)
/// must surface in MCP `tools/list` AND be callable via `tools/call`.
/// This is the cross-surface contract the dispatcher primitive promises.
#[test]
fn tools_list_includes_runtime_registered_tool_with_description() {
    let id = "test.runtime.mcp_visible";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "runtime visibility check",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });

    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 200, "method": "tools/list",
    }))
    .unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    let entry = tools
        .iter()
        .find(|t| t["name"] == id)
        .unwrap_or_else(|| panic!("runtime tool missing from tools/list:\n{tools:?}"));
    assert_eq!(entry["description"], "runtime visibility check");
    // Empty input_schema falls back to permissive object shape.
    assert_eq!(entry["inputSchema"]["type"], "object");
    assert_eq!(entry["outputSchema"]["type"], "object");
}

#[test]
fn tool_call_routes_to_runtime_dispatcher() {
    let id = "test.runtime.mcp_dispatch";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        let n = args
            .get("n")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        Ok(format!("doubled: {}", n * 2))
    });

    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 201, "method": "tools/call",
        "params": { "name": id, "arguments": { "n": 21 } },
    }))
    .unwrap();
    assert!(resp["result"]["isError"].is_null());
    assert_eq!(resp["result"]["content"][0]["text"], "doubled: 42");
}

#[test]
fn tool_call_marks_runtime_dispatcher_error_as_error() {
    let id = "test.runtime.mcp_dispatch_err";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |_| Err("plugin refused".into()));

    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 202, "method": "tools/call",
        "params": { "name": id, "arguments": {} },
    }))
    .unwrap();
    assert_eq!(resp["result"]["isError"], true);
    assert_eq!(resp["result"]["content"][0]["text"], "plugin refused");
}

// ─── Surface gating  ─────────────────────────────────

#[test]
fn tools_list_excludes_tools_without_mcp_surface() {
    let id = "test.surfacegate.mcp_excluded_from_list";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Cli], // cli-only
        boards: &[],
    });

    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 300, "method": "tools/list",
    }))
    .unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        !names.contains(&id),
        "tool with surfaces=[cli] must not appear in MCP tools/list"
    );
}

#[test]
fn surface_specific_handle_lists_only_requested_surface() {
    let id = "test.http_surface.handle_for_surface_http_only";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });

    let mcp_resp = handle(json!({
        "jsonrpc": "2.0", "id": 310, "method": "tools/list",
    }))
    .unwrap();
    let mcp_tools = mcp_resp["result"]["tools"].as_array().unwrap();
    assert!(
        !mcp_tools.iter().any(|tool| tool["name"] == id),
        "default handle() must remain scoped to MCP"
    );

    let http_resp = handle_for_surface(
        json!({
            "jsonrpc": "2.0", "id": 311, "method": "tools/list",
        }),
        upeg_core::Surface::Http,
    )
    .unwrap();
    let http_tools = http_resp["result"]["tools"].as_array().unwrap();
    assert!(
        http_tools.iter().any(|tool| tool["name"] == id),
        "handle_for_surface(..., Http) must expose HTTP tools"
    );
}

#[test]
fn tools_call_method_not_found_branches_are_indistinguishable() {
    // Info-leak invariant (MCP half), mirrored by the HTTP test.
    // The two -32601 paths in tools_call (surface-gate refusal vs
    // genuine NotFound) must produce byte-identical error responses
    // (after id-normalisation); otherwise a client could learn that
    // a tool exists on another surface.
    let gated_id = "test.infleak.mcp_surface_gated";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: gated_id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(gated_id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    let absent_id = "test.infleak.mcp_totally_missing";

    let gated = handle(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": gated_id, "arguments": {} },
    }))
    .unwrap();
    let absent = handle(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": absent_id, "arguments": {} },
    }))
    .unwrap();

    assert_eq!(gated["error"]["code"], -32601);
    assert_eq!(absent["error"]["code"], -32601);

    let gated_msg = gated["error"]["message"].as_str().expect("message");
    let absent_msg = absent["error"]["message"].as_str().expect("message");
    let gated_norm = gated_msg.replace(gated_id, "<ID>");
    let absent_norm = absent_msg.replace(absent_id, "<ID>");
    assert_eq!(
        gated_norm, absent_norm,
        "the two -32601 branches must use the same template; \
         gated=`{gated_msg}` absent=`{absent_msg}`"
    );
}

#[test]
fn call_for_tool_without_mcp_surface_is_rejected_as_method_not_found() {
    let id = "test.surfacegate.mcp_call_refused";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |_| Ok("would-have-run".into()));

    let resp = handle(json!({
        "jsonrpc": "2.0", "id": 301, "method": "tools/call",
        "params": { "name": id, "arguments": {} },
    }))
    .unwrap();
    assert_eq!(
        resp["error"]["code"], -32601,
        "non-mcp tool must surface as method-not-found, not as a successful call"
    );
}
