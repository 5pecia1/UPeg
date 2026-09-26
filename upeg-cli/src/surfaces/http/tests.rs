//! Source-located tests for the HTTP surface.
//!
//! Tests use axum's in-process `tower::ServiceExt::oneshot` so they do not
//! bind a real port.

use crate::surfaces::http::openapi::error_response;
use crate::surfaces::http::*;
use axum::{
    Json, Router,
    body::{Body, to_bytes},
};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

async fn body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
}

async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let body = body_to_value(resp.into_body()).await;
    (status, body)
}

async fn get_ok_json(app: Router, uri: &str) -> Value {
    let (status, body) = get_json(app, uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}");
    body
}

async fn post_json(uri: impl Into<String>, body: &'static str) -> http::Response<Body> {
    let uri = uri.into();
    router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn assert_get_array_field(app: Router, uri: &str, field: &str) {
    let body = get_ok_json(app, uri).await;
    assert!(body[field].is_array(), "{uri} must return `{field}` array");
}

#[test]
fn credentials_list_response_reports_registry_read_failure_as_an_error() {
    let (status, Json(body)) = credentials_list_response(Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid credential json",
    )));

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let error = body["error"].as_str().expect("error string");
    assert!(error.contains("read credential registry"));
    assert!(error.contains("invalid credential json"));
}

#[test]
fn logs_list_response_reports_store_read_errors() {
    let (status, Json(body)) = logs_list_response(Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "log file denied",
    )));

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let error = body["error"].as_str().expect("error string");
    assert!(error.contains("read execution log"));
    assert!(error.contains("log file denied"));
}

#[tokio::test]
async fn healthz_returns_200_and_a_pairing_hint() {
    let resp = router()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp.into_body()).await;
    assert_eq!(v["name"], "upeg");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        v.get("restApi").is_none(),
        "the desired-state gate is gone — a running host always serves /v1: {v}"
    );
    assert!(
        v.get("token").is_none(),
        "healthz must never leak the bearer token: {v}"
    );
}

#[tokio::test]
async fn healthz_carries_the_imports_pending_signal() {
    // This only checks that the route actually carries the fields. The
    // exact mapping is pinned by the `healthz_body` unit test below —
    // the phase is process-global, so its value can change under
    // parallel tests.
    let resp = router()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let v = body_to_value(resp.into_body()).await;

    assert!(
        v[mcp_imports::IMPORTS_PENDING_FIELD].is_boolean(),
        "an attached client must be able to tell 'still loading' from one bit: {v}"
    );
    assert!(
        v[mcp_imports::MCP_IMPORTS_FIELD].is_object(),
        "the import detail block must be present: {v}"
    );
}

#[test]
fn healthz_body_mirrors_the_phase() {
    use crate::infrastructure::mcp_imports::{McpImportPhase, McpImportTally};

    let loading = healthz_body(McpImportPhase::Loading);
    assert_eq!(loading[mcp_imports::IMPORTS_PENDING_FIELD], json!(true));
    assert_eq!(loading["name"], PRODUCT_NAME);

    let done = healthz_body(McpImportPhase::Done(McpImportTally {
        servers_loaded: 1,
        servers_failed: 0,
        tools: 5,
    }));
    assert_eq!(done[mcp_imports::IMPORTS_PENDING_FIELD], json!(false));
    assert_eq!(done[mcp_imports::MCP_IMPORTS_FIELD]["tools"], 5);
}

#[tokio::test]
async fn http_request_body_returns_413_at_the_first_byte_past_the_cap() {
    let body = vec![b'x'; MAX_HTTP_REQUEST_BODY_BYTES + 1];

    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/tools/num.hex_to_decimal")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("must build the HTTP request"),
        )
        .await
        .expect("must receive the HTTP response");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn tool_list_baseline_includes_the_builtin_http_tool_shape() {
    let body = get_ok_json(router(), "/v1/tools").await;
    let tools = body["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty(), "HTTP tools/list must not be empty");

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "HTTP tools/list must be sorted by tool id");

    let hex = tools
        .iter()
        .find(|tool| tool["name"] == "num.hex_to_decimal")
        .expect("num.hex_to_decimal must be exposed on HTTP tools/list");
    assert_eq!(hex["toolkit"], "num");
    assert_eq!(hex["tool"], "hex_to_decimal");
    assert_eq!(hex["inputSchema"]["type"], "object");
    assert_eq!(hex["outputSchema"]["type"], "object");
    assert!(
        hex["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "http"),
        "HTTP entry must advertise the http surface: {hex}"
    );
}

#[tokio::test]
async fn toolkit_tag_board_routes_expose_first_class_resources() {
    let app = router();

    let body = get_ok_json(app.clone(), "/v1/toolkits").await;
    let toolkits = body["toolkits"].as_array().expect("toolkits array");
    assert!(toolkits.iter().any(|t| t["id"] == "convert"));

    let body = get_ok_json(app.clone(), "/v1/toolkits/convert").await;
    assert_eq!(body["id"], "convert");
    assert!(body["toolCount"].as_u64().expect("toolCount number") > 0);
    assert!(
        body["tags"]
            .as_array()
            .expect("tags")
            .iter()
            .any(|tag| tag == "pure")
    );

    let body = get_ok_json(app.clone(), "/v1/toolkits/num/hex_to_decimal").await;
    assert_eq!(body["name"], "num.hex_to_decimal");
    assert_eq!(body["toolkit"], "num");
    assert_eq!(body["tool"], "hex_to_decimal");

    let dotted_id = "github.com.http.admin.tools.list";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: dotted_id,
        toolkit: "github.com",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(dotted_id, "github.com")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "dotted structured key",
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
    let body = get_ok_json(app.clone(), "/v1/toolkits/github.com/http.admin.tools.list").await;
    assert_eq!(body["name"], dotted_id);
    assert_eq!(body["toolkit"], "github.com");
    assert_eq!(body["tool"], "http.admin.tools.list");

    let body = get_ok_json(app.clone(), "/v1/tags/pure").await;
    assert_eq!(body["tag"], "pure");
    assert!(
        body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "num.hex_to_decimal")
    );

    let body = get_ok_json(app.clone(), "/v1/boards/dev").await;
    assert_eq!(body["board"], "dev");
    assert!(
        body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "num.hex_to_decimal")
    );

    for (uri, field) in [
        ("/v1/credentials", "credentials"),
        ("/v1/logs", "events"),
        ("/v1/triggers", "triggers"),
    ] {
        assert_get_array_field(app.clone(), uri, field).await;
    }
}

#[test]
fn board_context_route_dispatches_tools_gated_by_user_pins() {
    let mut env = BTreeMap::new();
    env.insert("PROFILE".to_string(), "dev".to_string());
    upeg_runtime::register_board_context(upeg_core::BoardExecutionContext {
        board: "ctx-dev".into(),
        env,
        project_manifest: Some("/tmp/upeg.ctx-dev/upeg.toml".into()),
    });
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "ctx.echo",
        toolkit: "ctx",
        local_id: "echo",
        tags: &["test"],
        display_label: "Test tool",
        description: "Echo board context",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Http],
        boards: &["ctx-dev"],
    });
    upeg_runtime::register_single_text_runtime_dispatcher("ctx.echo", |args| {
        let context = args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| "missing board context".to_string())?;
        Ok(format!(
            "{}:{}:{}",
            context["board"].as_str().expect("board string"),
            context["boardEnv"]["PROFILE"]
                .as_str()
                .expect("PROFILE string"),
            context["projectManifest"]
                .as_str()
                .expect("projectManifest string")
        ))
    });

    crate::test_support::with_seeded_pegboard_home(
        "http-board-dispatch",
        |state| {
            // The gate reads the *user's* pegboard state: add the
            // ctx-dev board with ctx.echo pinned. `dev` +
            // num.hex_to_decimal come from the default seeding.
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: "ctx-dev".into(),
                title: "Ctx Dev".into(),
            });
            state.layouts.insert(
                "ctx-dev".into(),
                vec![upeg_core::Placement::new("ctx.echo", 0, 0)],
            );
        },
        || {
            let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
            runtime.block_on(async {
                let uri = "/v1/boards/dev/tools/num.hex_to_decimal";
                let resp = post_json(uri, r#"{"input":"0xff"}"#).await;
                assert_eq!(resp.status(), StatusCode::OK, "{uri}");
                let body = body_to_value(resp.into_body()).await;
                assert_eq!(body["primary_output_id"], "result", "{uri}");
                assert_eq!(body["outputs"][0]["value"], 255, "{uri}");

                let resp = post_json("/v1/boards/ctx-dev/tools/ctx.echo", "{}").await;
                assert_eq!(resp.status(), StatusCode::OK);
                let body = body_to_value(resp.into_body()).await;
                assert_eq!(
                    body["outputs"][0]["value"], "ctx-dev:dev:/tmp/upeg.ctx-dev/upeg.toml",
                    "board route must inject BoardExecutionContext into dispatch args"
                );

                // Unpinned tool on an existing board → the same 404 an
                // unknown id produces (user-pin gate).
                let resp = post_json("/v1/boards/ctx-dev/tools/num.hex_to_decimal", "{}").await;
                assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            });
        },
    );
}

#[test]
fn board_routed_call_merges_the_pin_preset_as_defaults() {
    crate::test_support::with_seeded_pegboard_home(
        "http-board-preset",
        |state| {
            let preset = upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("valid preset");
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: "preset-dev".into(),
                title: "Preset Dev".into(),
            });
            state.layouts.insert(
                "preset-dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0)
                        .with_args_preset(Some(preset)),
                ],
            );
        },
        || {
            let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
            runtime.block_on(async {
                // Empty body → the pin preset supplies `input`.
                let resp = post_json("/v1/boards/preset-dev/tools/num.hex_to_decimal", "").await;
                assert_eq!(resp.status(), StatusCode::OK);
                let body = body_to_value(resp.into_body()).await;
                assert_eq!(
                    body["outputs"][0]["value"], 255,
                    "the preset becomes the default"
                );

                // Explicit caller args override the preset key.
                let resp = post_json(
                    "/v1/boards/preset-dev/tools/num.hex_to_decimal",
                    r#"{"input":"0x10"}"#,
                )
                .await;
                assert_eq!(resp.status(), StatusCode::OK);
                let body = body_to_value(resp.into_body()).await;
                assert_eq!(
                    body["outputs"][0]["value"], 16,
                    "caller args override the preset"
                );
            });
        },
    );
}

#[tokio::test]
async fn trigger_route_dispatches_a_declared_webhook_trigger_with_context() {
    const ID: &str = "httptrigger.webhook_echo";
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    CALLS.store(0, Ordering::SeqCst);

    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "httptrigger",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(ID, "httptrigger")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Webhook trigger echo",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(ID, |args| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        let context = args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| "missing execution context".to_string())?;
        Ok(format!(
            "{}:{}",
            context["surface"].as_str().expect("surface string"),
            context["trigger"].as_str().expect("trigger string")
        ))
    });
    upeg_runtime::set_trigger_bindings(
        ID,
        vec![upeg_runtime::TriggerBinding {
            tool_id: ID,
            source: "webhook".into(),
            condition: None,
        }],
    );

    let resp = post_json(format!("/v1/trigger/{ID}"), "{}").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    // `_upeg.trigger` names the trigger that fired, not the tool id the tool
    // already knows: a `webhook` source carries no condition to qualify it.
    assert_eq!(body["outputs"][0]["value"], "http:webhook");
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn trigger_route_rejects_http_tools_without_a_webhook_trigger_binding() {
    const PLAIN_ID: &str = "httptrigger.plain_http";
    const HOTKEY_ID: &str = "httptrigger.hotkey_only";
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    CALLS.store(0, Ordering::SeqCst);

    for id in [PLAIN_ID, HOTKEY_ID] {
        upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
            id,
            toolkit: "httptrigger",
            local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "httptrigger")
                .expect("test ToolMeta id must be canonical")
                .local(),
            tags: &["test"],
            display_label: "Test tool",
            description: "HTTP trigger guard fixture",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: &[upeg_core::Surface::Http],
            boards: &[],
        });
        upeg_runtime::register_single_text_runtime_dispatcher(id, |_| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok("ran".into())
        });
    }
    upeg_runtime::set_trigger_bindings(PLAIN_ID, vec![]);
    upeg_runtime::set_trigger_bindings(
        HOTKEY_ID,
        vec![upeg_runtime::TriggerBinding {
            tool_id: HOTKEY_ID,
            source: "hotkey".into(),
            condition: Some("Ctrl+Shift+H".into()),
        }],
    );

    for id in [PLAIN_ID, HOTKEY_ID] {
        let resp = post_json(format!("/v1/trigger/{id}"), "{}").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{id}");
        let body = body_to_value(resp.into_body()).await;
        let err = body["error"]["message"].as_str().expect("error string");
        assert!(
            err.contains("unknown webhook trigger"),
            "trigger route must reject non-webhook tool `{id}` before dispatch; got {err:?}"
        );
    }
    assert_eq!(CALLS.load(Ordering::SeqCst), 0, "must not dispatch");

    let resp = post_json(format!("/v1/tools/{PLAIN_ID}"), "{}").await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the fixture remains a normal HTTP Tool; only /v1/trigger is gated"
    );
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn unconfigured_board_context_detects_the_nearest_project_manifest() {
    struct CwdRestore(std::path::PathBuf);

    impl Drop for CwdRestore {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    // B-4: detection only walks ancestors that stay inside `$HOME`, so
    // this fixture tree must itself live under the real `$HOME` — a
    // bare `std::env::temp_dir()` ancestor is exactly the "world-writable
    // /tmp/x/upeg.toml auto-loaded from /tmp/x/anything" case the fix
    // closes (docs/architecture.md#security-absolutes).
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .expect("$HOME must be set for this test");
    let root = home.join("upeg_http_project_manifest_detect_test");
    let nested = root.join("nested/work");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&nested).unwrap();
    let manifest = root.join("upeg.toml");
    std::fs::write(
        &manifest,
        r#"id = "ctxproj"
tools = [{ id = "noop" }]"#,
    )
    .unwrap();

    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "ctxproj.echo",
        toolkit: "ctxproj",
        local_id: "echo",
        tags: &["test"],
        display_label: "Test tool",
        description: "Echo auto-detected project manifest",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Http],
        boards: &["auto-project"],
    });
    upeg_runtime::register_single_text_runtime_dispatcher("ctxproj.echo", |args| {
        let context = args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| "missing board context".to_string())?;
        Ok(context["projectManifest"]
            .as_str()
            .expect("projectManifest string")
            .to_string())
    });

    // `with_seeded_pegboard_home` holds the shared pegboard-env lock,
    // which doubles as the cwd lock this test used to take.
    crate::test_support::with_seeded_pegboard_home(
        "http-project-manifest",
        |state| {
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: "auto-project".into(),
                title: "Auto Project".into(),
            });
            state.layouts.insert(
                "auto-project".into(),
                vec![upeg_core::Placement::new("ctxproj.echo", 0, 0)],
            );
        },
        || {
            let old = std::env::current_dir().unwrap();
            let _cwd_restore = CwdRestore(old);
            std::env::set_current_dir(&nested).unwrap();
            // This test is ABOUT detection, so it opts back out of the
            // repo's `UPEG_PROJECT_MANIFEST_PATH=off` gate hermetics.
            crate::test_support::with_project_manifest_detection(|| {
                let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
                runtime.block_on(async {
                    let resp = router()
                        .oneshot(
                            Request::builder()
                                .method("POST")
                                .uri("/v1/boards/auto-project/tools/ctxproj.echo")
                                .header("content-type", "application/json")
                                .body(Body::from("{}"))
                                .unwrap(),
                        )
                        .await
                        .unwrap();

                    assert_eq!(resp.status(), StatusCode::OK);
                    let body = body_to_value(resp.into_body()).await;
                    assert_eq!(body["outputs"][0]["value"], manifest.display().to_string());
                });
            });
        },
    );

    let _ = std::fs::remove_dir_all(&root);
}

mod file_wire;
mod openapi;
mod tools;
