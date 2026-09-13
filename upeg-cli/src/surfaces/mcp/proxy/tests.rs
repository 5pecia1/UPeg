//! Proxy-lane tests: Project Manifest routing (D-1), `_upeg.cwd`
//! stamping (D-2), and `tools/list` merging.
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

/// One Project Manifest fixture id per test. The provenance map is
/// process-global and cargo runs these tests in parallel, so a shared id
/// would let one test's teardown clear the provenance another test is
/// still asserting on.
const PROJECT_TOOL_ROUTE_ID: &str = "test.mcp_proxy.project_route";
const PROJECT_TOOL_DISPATCH_ID: &str = "test.mcp_proxy.project_dispatch";
const PROJECT_TOOL_LIST_ID: &str = "test.mcp_proxy.project_list";
const HOST_TOOL_ID: &str = "test.mcp_proxy.host_owned";
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

// ─── 라우팅 (순수) ─────────────────────────────────────

#[test]
fn 라우팅은_tools_list를_병합_대상_tools_call을_cwd_각인으로_본다() {
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
fn 라우팅은_project_manifest_provenance_도구를_로컬_dispatch로_보낸다() {
    let _fixture = ProjectManifestTool::install(PROJECT_TOOL_ROUTE_ID);

    assert_eq!(
        proxy_route(&json!({
            "method": "tools/call", "id": 4,
            "params": { "name": PROJECT_TOOL_ROUTE_ID },
        })),
        ProxyRoute::LocalDispatch
    );
}

// ─── D-2: `_upeg.cwd` 각인 ─────────────────────────────

#[test]
fn cwd_각인은_arguments가_없어도_객체로_승격해_넣는다() {
    let stamped = stamp_caller_cwd_on_call(json!({
        "method": "tools/call", "id": 5,
        "params": { "name": HOST_TOOL_ID },
    }));

    let cwd = stamped["params"]["arguments"][upeg_core::EXECUTION_CONTEXT_ARG]
        [EXECUTION_CONTEXT_CWD]
        .as_str()
        .expect("호출자 cwd가 각인되어야 한다");
    assert!(!cwd.is_empty());
}

#[test]
fn cwd_각인은_기존_인자를_보존한다() {
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
fn cwd_각인은_객체가_아닌_arguments를_건드리지_않는다() {
    // 프로토콜 위반은 host가 그대로 보고해야 한다 — 여기서 고쳐 쓰면
    // 클라이언트가 보는 오류가 달라진다.
    let original = json!({
        "method": "tools/call", "id": 7,
        "params": { "name": HOST_TOOL_ID, "arguments": "not-an-object" },
    });

    assert_eq!(stamp_caller_cwd_on_call(original.clone()), original);
}

// ─── tools/list 병합 ───────────────────────────────────

#[test]
fn 병합은_중복_이름을_host_항목으로_유지하고_이름순을_지킨다() {
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

    let tools = merged["result"]["tools"].as_array().expect("tools 배열");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, vec!["a.tool", "b.tool"]);
    assert_eq!(
        tools[1]["from"], "host",
        "이름이 겹치면 host 항목이 남아야 한다"
    );
}

#[test]
fn 병합은_결과가_없는_프레임을_그대로_둔다() {
    let error_frame = json!({
        "jsonrpc": "2.0", "id": 9,
        "error": { "code": -32601, "message": "Method not found" },
    });

    assert_eq!(
        merge_tool_entries(error_frame.clone(), vec![json!({ "name": "a.tool" })]),
        error_frame
    );
}

// ─── proxy_line 통합 (스크립트된 host) ─────────────────

#[test]
fn proxy_line은_host_소유_tools_call에_cwd를_실어_전달한다() {
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
        serde_json::from_str(&seen.expect("host가 호출되어야 한다")).expect("전달된 프레임");
    assert!(
        forwarded["params"]["arguments"][upeg_core::EXECUTION_CONTEXT_ARG][EXECUTION_CONTEXT_CWD]
            .is_string(),
        "host로 나가는 프레임에 _upeg.cwd가 있어야 한다: {forwarded}"
    );
    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        format!("{canned}\n")
    );
}

#[test]
fn proxy_line은_project_manifest_도구를_host에_보내지_않고_직접_실행한다() {
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
    assert!(seen.is_none(), "host는 호출되지 않아야 한다 (D-1)");
    let response: Value =
        serde_json::from_str(&String::from_utf8(out).expect("utf-8")).expect("응답 프레임");
    assert_eq!(
        response["result"]["content"][0]["text"],
        PROJECT_TOOL_OUTPUT
    );
}

#[test]
fn proxy_line은_host_tools_list에_project_manifest_도구를_합쳐준다() {
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
        serde_json::from_str(&String::from_utf8(out).expect("utf-8")).expect("응답 프레임");
    let names: Vec<&str> = response["result"]["tools"]
        .as_array()
        .expect("tools 배열")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(
        names.contains(&PROJECT_TOOL_LIST_ID),
        "host가 모르는 project manifest 도구가 병합되어야 한다: {names:?}"
    );
    assert!(names.contains(&HOST_TOOL_ID), "host 항목은 유지된다");
}

#[test]
fn proxy_line은_host_실패를_host_gone으로_보고한다() {
    let mut out = Vec::new();
    let mut seen = None;

    let outcome = proxy_line(
        &json!({ "jsonrpc": "2.0", "id": 13, "method": "initialize" }).to_string(),
        &mut out,
        None,
        scripted_host(&mut seen, Err(std::io::Error::other("host gone"))),
    );

    assert_eq!(outcome, LineOutcome::HostGone);
    assert!(out.is_empty(), "실패한 프레임에는 아무것도 쓰지 않는다");
}

#[test]
fn proxy_line은_파싱되지_않는_줄을_그대로_전달한다() {
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
