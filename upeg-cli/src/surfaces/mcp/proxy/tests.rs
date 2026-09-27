//! Proxy-lane tests: Project Manifest routing, `_upeg.cwd`
//! stamping, and `tools/list` merging.
//!
//! The host is scripted through [`proxy_line`]'s `forward` parameter
//! rather than a real listener: a same-process HTTP host would answer
//! `/mcp` from the *same* global Toolbox, so its `tools/list` would
//! already contain the Project Manifest tools and the merge under test
//! would be a no-op. A scripted host is the only way to reproduce the
//! real asymmetry (host resolved a different `upeg.toml`, or none).

use super::*;
use serde_json::json;
use upeg_core::EXECUTION_CONTEXT_CWD;

#[test]
fn changed_discovery_returns_mcp_error_without_local_fallback() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let info = crate::infrastructure::discovery::ServerInfo::new(endpoint, "old-token");
    let host = crate::infrastructure::discovery::DiscoveredHost::for_test(info.clone());
    let mut replacement = info;
    replacement.token = "new-token".into();
    host.replace_for_test(&replacement);
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\"}\n";
    let mut output = Vec::new();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(500)))
                .unwrap();
            let mut request = [0_u8; 512];
            assert_eq!(
                std::io::Read::read(&mut stream, &mut request).unwrap_or(0),
                0
            );
        });
        proxy_loop(&mut std::io::Cursor::new(input), &mut output, &host, None);
    });

    let response: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(response["id"], 7);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("discovery changed")
    );
    assert!(response.get("result").is_none());
}

#[test]
fn changed_discovery_notification_sends_no_reply() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let info = crate::infrastructure::discovery::ServerInfo::new(
        format!("http://{}", listener.local_addr().unwrap()),
        "old-token",
    );
    let host = crate::infrastructure::discovery::DiscoveredHost::for_test(info.clone());
    let mut replacement = info;
    replacement.token = "new-token".into();
    host.replace_for_test(&replacement);
    let input = b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
    let mut output = Vec::new();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 512];
            assert_eq!(
                std::io::Read::read(&mut stream, &mut request).unwrap_or(0),
                0
            );
        });
        proxy_loop(&mut std::io::Cursor::new(input), &mut output, &host, None);
    });

    assert!(output.is_empty(), "notifications have no response frame");
}

#[test]
fn lost_host_response_never_replays_a_tool_call_locally() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    register_tool(AMBIGUOUS_CALL_ID);
    let observed = std::sync::Arc::clone(&calls);
    upeg_runtime::register_single_text_runtime_dispatcher(AMBIGUOUS_CALL_ID, move |_| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("ran locally".to_string())
    });

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let info = crate::infrastructure::discovery::ServerInfo::new(
        format!("http://{}", listener.local_addr().unwrap()),
        "test-token",
    );
    let host = crate::infrastructure::discovery::DiscoveredHost::for_test(info);
    let input = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"tools/call\",\"params\":{{\"name\":\"{AMBIGUOUS_CALL_ID}\",\"arguments\":{{}}}}}}\n"
    );
    let mut output = Vec::new();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut request).unwrap();
            // The host disappears without a response after receiving the call.
        });
        proxy_loop(&mut std::io::Cursor::new(input), &mut output, &host, None);
    });

    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    let response: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(response["id"], 11);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("may have run")
    );
}

#[test]
fn lost_host_response_tool_notification_sends_no_reply() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let info = crate::infrastructure::discovery::ServerInfo::new(
        format!("http://{}", listener.local_addr().unwrap()),
        "test-token",
    );
    let host = crate::infrastructure::discovery::DiscoveredHost::for_test(info);
    let input = b"{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"test.mcp_proxy.unknown\",\"arguments\":{}}}\n";
    let mut output = Vec::new();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 512];
            let _ = std::io::Read::read(&mut stream, &mut request).unwrap();
        });
        proxy_loop(&mut std::io::Cursor::new(input), &mut output, &host, None);
    });

    assert!(output.is_empty(), "notifications have no response frame");
}

/// One Project Manifest fixture id per test. The provenance map is
/// process-global and cargo runs these tests in parallel, so a shared id
/// would let one test's teardown clear the provenance another test is
/// still asserting on.
const PROJECT_TOOL_ROUTE_ID: &str = "test.mcp_proxy.project_route";
const PROJECT_TOOL_DISPATCH_ID: &str = "test.mcp_proxy.project_dispatch";
const PROJECT_TOOL_LIST_ID: &str = "test.mcp_proxy.project_list";
const HOST_TOOL_ID: &str = "test.mcp_proxy.host_owned";
const AMBIGUOUS_CALL_ID: &str = "test.mcp_proxy.ambiguous_call";
const PROJECT_MANIFEST_PATH: &str = "/workspaces/upeg/upeg.toml";
const PROJECT_TOOL_OUTPUT: &str = "dispatched in-process";

/// Register a tool in the global Toolbox with the given surfaces, the
/// same way the TOML loader path does.
fn register_tool(id: &'static str) {
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Proxy fixture",
        description: "mcp proxy fixture",
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
}

/// RAII fixture: a registered, dispatchable tool whose provenance says
/// it came from the Project Manifest. Dropping it clears the provenance
/// so the process-global map never leaks into another test.
struct ProjectManifestTool {
    id: &'static str,
}

impl ProjectManifestTool {
    fn install(id: &'static str) -> Self {
        register_tool(id);
        upeg_runtime::register_single_text_runtime_dispatcher(id, |_| {
            Ok(PROJECT_TOOL_OUTPUT.to_string())
        });
        upeg_runtime::register_tool_provenance(
            id,
            upeg_runtime::ToolProvenance::ProjectManifest {
                path: PROJECT_MANIFEST_PATH.to_string(),
            },
        );
        Self { id }
    }
}

impl Drop for ProjectManifestTool {
    fn drop(&mut self) {
        upeg_runtime::clear_tool_provenance(self.id);
    }
}

/// A host that answers every frame with `canned` and records what it
/// was asked. `Ok`/`Err` is chosen by the caller.
fn scripted_host(
    seen: &mut Option<String>,
    canned: std::io::Result<String>,
) -> impl FnOnce(&str) -> std::io::Result<String> + '_ {
    move |body| {
        *seen = Some(body.to_string());
        canned
    }
}

fn host_tools_list_response() -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": { "tools": [{ "name": HOST_TOOL_ID }] },
    })
    .to_string()
}

// ─── Routing (pure) ──────────────────────────────────

#[test]
fn routing_sees_tools_list_as_merge_and_tools_call_as_cwd_stamp() {
    assert_eq!(
        proxy_route(&json!({ "method": "tools/list", "id": 1 })),
        ProxyRoute::ForwardAndMergeProjectTools
    );
    assert_eq!(
        proxy_route(&json!({
            "method": "tools/call", "id": 2,
            "params": { "name": HOST_TOOL_ID },
        })),
        ProxyRoute::ForwardWithCallerCwd
    );
    assert_eq!(
        proxy_route(&json!({ "method": "initialize", "id": 3 })),
        ProxyRoute::Forward
    );
    assert_eq!(
        proxy_route(&json!({ "method": "notifications/initialized" })),
        ProxyRoute::Forward
    );
}

#[test]
fn routing_sends_project_manifest_provenance_tools_to_local_dispatch() {
    let _fixture = ProjectManifestTool::install(PROJECT_TOOL_ROUTE_ID);

    assert_eq!(
        proxy_route(&json!({
            "method": "tools/call", "id": 4,
            "params": { "name": PROJECT_TOOL_ROUTE_ID },
        })),
        ProxyRoute::LocalDispatch
    );
}

// ─── `_upeg.cwd` stamping ────────────────────────────

#[test]
fn cwd_stamp_promotes_missing_arguments_to_object() {
    let stamped = stamp_caller_cwd_on_call(json!({
        "method": "tools/call", "id": 5,
        "params": { "name": HOST_TOOL_ID },
    }));

    let cwd = stamped["params"]["arguments"][upeg_core::EXECUTION_CONTEXT_ARG]
        [EXECUTION_CONTEXT_CWD]
        .as_str()
        .expect("caller cwd must be stamped");
    assert!(!cwd.is_empty());
}

#[test]
fn cwd_stamp_preserves_existing_arguments() {
    let stamped = stamp_caller_cwd_on_call(json!({
        "method": "tools/call", "id": 6,
        "params": { "name": HOST_TOOL_ID, "arguments": { "input": "hi" } },
    }));

    assert_eq!(stamped["params"]["arguments"]["input"], "hi");
    assert!(
        stamped["params"]["arguments"][upeg_core::EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_CWD]
            .is_string()
    );
}

#[test]
fn cwd_stamp_leaves_non_object_arguments_untouched() {
    // Protocol violations are reported by the host verbatim —
    // rewriting them here would change the error the client sees.
    let original = json!({
        "method": "tools/call", "id": 7,
        "params": { "name": HOST_TOOL_ID, "arguments": "not-an-object" },
    });

    assert_eq!(stamp_caller_cwd_on_call(original.clone()), original);
}

// ─── tools/list merge ────────────────────────────────

#[test]
fn merge_keeps_host_entry_on_name_collision_and_sorts_by_name() {
    let response = json!({
        "jsonrpc": "2.0", "id": 8,
        "result": { "tools": [{ "name": "b.tool", "from": "host" }] },
    });

    let merged = merge_tool_entries(
        response,
        vec![
            json!({ "name": "b.tool", "from": "local" }),
            json!({ "name": "a.tool", "from": "local" }),
        ],
    );

    let tools = merged["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, vec!["a.tool", "b.tool"]);
    assert_eq!(
        tools[1]["from"], "host",
        "the host entry must win on a name collision"
    );
}

#[test]
fn merge_leaves_frames_without_result_untouched() {
    let error_frame = json!({
        "jsonrpc": "2.0", "id": 9,
        "error": { "code": -32601, "message": "Method not found" },
    });

    assert_eq!(
        merge_tool_entries(error_frame.clone(), vec![json!({ "name": "a.tool" })]),
        error_frame
    );
}

// ─── proxy_line integration (scripted host) ──────────

#[test]
fn proxy_line_forwards_host_owned_tools_call_with_cwd() {
    let mut out = Vec::new();
    let mut seen = None;
    let canned = json!({ "jsonrpc": "2.0", "id": 10, "result": { "content": [] } }).to_string();

    let outcome = proxy_line(
        &json!({
            "jsonrpc": "2.0", "id": 10, "method": "tools/call",
            "params": { "name": HOST_TOOL_ID, "arguments": { "input": "x" } },
        })
        .to_string(),
        &mut out,
        None,
        scripted_host(&mut seen, Ok(canned.clone())),
    );

    assert_eq!(outcome, LineOutcome::Proxied);
    let forwarded: Value =
        serde_json::from_str(&seen.expect("host must be called")).expect("forwarded frame");
    assert!(
        forwarded["params"]["arguments"][upeg_core::EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_CWD]
            .is_string(),
        "the frame sent to the host must carry _upeg.cwd: {forwarded}"
    );
    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        format!("{canned}\n")
    );
}

#[test]
fn proxy_line_runs_project_manifest_tool_locally_without_host() {
    let _fixture = ProjectManifestTool::install(PROJECT_TOOL_DISPATCH_ID);
    let mut out = Vec::new();
    let mut seen = None;

    let outcome = proxy_line(
        &json!({
            "jsonrpc": "2.0", "id": 11, "method": "tools/call",
            "params": { "name": PROJECT_TOOL_DISPATCH_ID, "arguments": {} },
        })
        .to_string(),
        &mut out,
        None,
        scripted_host(&mut seen, Ok(String::new())),
    );

    assert_eq!(outcome, LineOutcome::Proxied);
    assert!(seen.is_none(), "host must not be called");
    let response: Value =
        serde_json::from_str(&String::from_utf8(out).expect("utf-8")).expect("response frame");
    assert_eq!(
        response["result"]["content"][0]["text"],
        PROJECT_TOOL_OUTPUT
    );
}

#[test]
fn proxy_line_merges_project_manifest_tools_into_host_tools_list() {
    let _fixture = ProjectManifestTool::install(PROJECT_TOOL_LIST_ID);
    let mut out = Vec::new();
    let mut seen = None;

    let outcome = proxy_line(
        &json!({ "jsonrpc": "2.0", "id": 12, "method": "tools/list" }).to_string(),
        &mut out,
        None,
        scripted_host(&mut seen, Ok(host_tools_list_response())),
    );

    assert_eq!(outcome, LineOutcome::Proxied);
    let response: Value =
        serde_json::from_str(&String::from_utf8(out).expect("utf-8")).expect("response frame");
    let names: Vec<&str> = response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(
        names.contains(&PROJECT_TOOL_LIST_ID),
        "a project manifest tool unknown to the host must be merged in: {names:?}"
    );
    assert!(names.contains(&HOST_TOOL_ID), "host entries are preserved");
}

#[test]
fn proxy_line_reports_host_failure_as_host_gone() {
    let mut out = Vec::new();
    let mut seen = None;

    let outcome = proxy_line(
        &json!({ "jsonrpc": "2.0", "id": 13, "method": "initialize" }).to_string(),
        &mut out,
        None,
        scripted_host(&mut seen, Err(std::io::Error::other("host gone"))),
    );

    assert_eq!(outcome, LineOutcome::HostGone);
    assert!(out.is_empty(), "nothing is written for a failed frame");
}

#[test]
fn proxy_line_forwards_unparseable_line_verbatim() {
    let mut out = Vec::new();
    let mut seen = None;
    let canned = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700}}"#;

    let outcome = proxy_line(
        "not json",
        &mut out,
        None,
        scripted_host(&mut seen, Ok(canned.to_string())),
    );

    assert_eq!(outcome, LineOutcome::Proxied);
    assert_eq!(seen.as_deref(), Some("not json"));
    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        format!("{canned}\n")
    );
}
