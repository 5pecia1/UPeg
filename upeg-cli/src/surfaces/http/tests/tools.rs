use super::*;

async fn tools_body() -> Value {
    get_ok_json(router(), "/v1/tools").await
}

async fn post_tool(id: &str, body: &'static str) -> http::Response<Body> {
    post_json(format!("/v1/tools/{id}"), body).await
}

fn primary_value(body: &Value) -> &Value {
    let primary = body["primary_output_id"]
        .as_str()
        .expect("canonical response has primary_output_id");
    body["outputs"]
        .as_array()
        .expect("canonical response has outputs")
        .iter()
        .find(|entry| entry["id"].as_str() == Some(primary))
        .map(|entry| &entry["value"])
        .expect("canonical response has primary output entry")
}

const CONTROLLED_EMBED_HTTP_TOOL_ID: &str = "cehttp.wait_timeout";
const CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_CODE: &str =
    upeg_runtime::controlled_embed::CONTROLLED_EMBED_WAIT_TIMEOUT_CODE;
const CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_SELECTOR: &str = "#summary";
const CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
const CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_MS: u64 = 125;

const CONTROLLED_EMBED_HTTP_TOOLKIT_TOML: &str = r##"
id = "cehttp"

[[tools]]
id = "wait_timeout"
description = "Controlled Embed HTTP wait-timeout fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.test/tool"
surfaces = ["http"]
primary_output_id = "summary"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "summary"
type = "string"
label = "Summary"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[tools.controlled_embed.bindings]]
role = "output"
field = "summary"
selector = "#summary"
"##;

struct WaitTimeoutControlledEmbedBackend;

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitTimeoutControlledEmbedBackend {
    fn run(
        &self,
        _request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        Err(
            upeg_runtime::controlled_embed::ControlledEmbedError::WaitTimeout {
                role: upeg_core::BindingRole::Output,
                selector: CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_SELECTOR.into(),
                for_selector: CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_FOR_SELECTOR.into(),
                condition: upeg_core::BindingWaitCondition::Visible,
                timeout_ms: CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_MS,
            },
        )
    }
}

fn ensure_controlled_embed_http_fixture_loaded() {
    let dir =
        std::env::temp_dir().join(format!("upeg-controlled-embed-http-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("controlled embed HTTP fixture dir");
    std::fs::write(dir.join("cehttp.toml"), CONTROLLED_EMBED_HTTP_TOOLKIT_TOML)
        .expect("controlled embed HTTP fixture TOML");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "Controlled Embed HTTP fixture must load without failures: {:?}",
        outcome.failed
    );
    assert!(
        outcome.loaded.contains(&CONTROLLED_EMBED_HTTP_TOOL_ID),
        "Controlled Embed HTTP fixture tool must be loaded: {:?}",
        outcome.loaded
    );
}

#[tokio::test]
async fn tool_list_includes_registered_tools_sorted() {
    let body = tools_body().await;
    let tools = body["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();

    assert_eq!(names, sorted, "tools list must be sorted by name");
    assert!(names.contains(&"num.hex_to_decimal"));
    assert!(names.contains(&"id.uuid_v7"));
    assert!(names.contains(&"convert.base64_encode"));
}

#[tokio::test]
async fn each_tool_list_entry_has_embed_url_and_bindings() {
    let body = tools_body().await;
    let tools = body["tools"].as_array().expect("tools array");

    for entry in tools {
        assert!(
            entry.get("embedUrl").is_some(),
            "each /v1/tools entry must include `embedUrl` field; got {entry}"
        );
        assert!(
            entry.get("pegboardUnits").is_some(),
            "each /v1/tools entry must include `pegboardUnits` field; got {entry}"
        );
        assert!(
            entry.get("pegboardSpan").is_some(),
            "each /v1/tools entry must include `pegboardSpan` field; got {entry}"
        );
        assert!(
            entry.get("selectorBindings").is_some(),
            "each /v1/tools entry must include `selectorBindings` field; got {entry}"
        );
        assert!(
            entry["selectorBindings"].is_array(),
            "selectorBindings must always be an array; got {entry}"
        );
    }

    let hex = tools
        .iter()
        .find(|t| t["name"] == "num.hex_to_decimal")
        .expect("hex_to_decimal entry");
    assert_eq!(hex["pegboardUnits"], "U2");
    assert_eq!(
        hex["pegboardSpan"],
        serde_json::json!({ "cols": 2, "rows": 1 })
    );
    assert!(
        hex["embedUrl"].is_null(),
        "non-Embed tool must have null embedUrl; got {hex}"
    );
    assert_eq!(
        hex["selectorBindings"].as_array().unwrap().len(),
        0,
        "non-Embed tool must have empty selectorBindings; got {hex}"
    );
}

#[tokio::test]
async fn tool_list_exposes_the_registered_embed_url() {
    const ID: &str = "test.http_embed_with_url";

    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "test",
        local_id: "http_embed_with_url",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Embed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Static,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_embed_url(ID, "https://example.test/http-embed");

    let body = tools_body().await;
    let tools = body["tools"].as_array().expect("tools array");
    let entry = tools
        .iter()
        .find(|t| t["name"] == ID)
        .expect("test tool must show in tools list");
    assert_eq!(
        entry["embedUrl"], "https://example.test/http-embed",
        "registered embedUrl must round-trip; got {entry}"
    );
}

#[tokio::test]
async fn tool_call_hex_to_dec_returns_success() {
    let resp = post_tool("num.hex_to_decimal", r#"{"input":"0xff"}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(primary_value(&body), 255);
}

#[tokio::test]
async fn tool_call_rejects_url_encoded_whitespace_in_the_path() {
    let resp = post_json("/v1/tools/%20num.hex_to_decimal%20", r#"{"input":"0xff"}"#).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "URL-encoded whitespace around the id must not be normalized"
    );
    let body = body_to_value(resp.into_body()).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .expect("error string")
            .contains("unknown tool")
    );
}

#[tokio::test]
async fn tool_call_hex_to_dec_returns_422_for_invalid_input() {
    let resp = post_tool("num.hex_to_decimal", r#"{"input":"0xZZ"}"#).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_to_value(resp.into_body()).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid hex")
    );
}

#[tokio::test]
async fn tool_call_unknown_id_returns_404() {
    let resp = post_tool("no.such.tool", "{}").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(body["ok"], false);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );
}

#[tokio::test]
async fn tool_call_uuid_v7_works_without_a_body() {
    let resp = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/tools/id.uuid_v7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    let s = primary_value(&body).as_str().unwrap();
    assert_eq!(s.len(), 36);
    assert_eq!(s.chars().nth(14), Some('7'));
}

#[tokio::test]
async fn tool_call_returns_400_for_malformed_body() {
    for malformed in [r#"{ "input"]"#, r#"{"input":"0xff"#] {
        let resp = post_tool("num.hex_to_decimal", malformed).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "malformed JSON body must be 400: {malformed}"
        );
        let body = body_to_value(resp.into_body()).await;
        assert_eq!(body["ok"], false);
        let err = body["error"]["message"].as_str().expect("error string");
        assert!(
            err.contains("invalid JSON body"),
            "error must explain the cause; got {err:?}"
        );
    }
}

#[tokio::test]
async fn tool_call_returns_400_for_non_object_body() {
    for bad_body in [
        r#""hello""#, // string
        "42",         // number
        "true",       // boolean
        "[1, 2, 3]",  // array
    ] {
        let resp = post_tool("num.hex_to_decimal", bad_body).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "non-object body `{bad_body}` must be 400"
        );
        let body = body_to_value(resp.into_body()).await;
        assert_eq!(body["ok"], false);
        let err = body["error"]["message"].as_str().expect("error string");
        assert!(
            err.contains("object"),
            "error must explain the expected shape; got {err:?}"
        );
    }
}

#[tokio::test]
async fn tool_call_works_for_null_and_object_bodies() {
    for body_payload in [
        None,         // empty
        Some("null"), // explicit null
        Some("{}"),   // empty object
    ] {
        let mut req = Request::builder()
            .method("POST")
            .uri("/v1/tools/id.uuid_v7")
            .header("content-type", "application/json");
        if let Some(payload) = body_payload {
            req = req.header("content-length", payload.len().to_string());
        }
        let req = req.body(Body::from(body_payload.unwrap_or(""))).unwrap();
        let resp = router().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "body `{body_payload:?}` must succeed for zero-arg tool"
        );
        let body = body_to_value(resp.into_body()).await;
        let result = primary_value(&body).as_str().expect("result string");
        assert_eq!(
            result.len(),
            36,
            "uuid_v7 must produce a 36-char canonical form; got {result:?}"
        );
    }
}

#[tokio::test]
async fn tool_call_unknown_tool_includes_did_you_mean() {
    let resp = post_tool("num.hex_to_decimai", "{}").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_to_value(resp.into_body()).await;
    let err = body["error"]["message"].as_str().expect("error string");
    assert!(
        err.contains("did you mean"),
        "HTTP 404 must include suggestion for typo; got {err:?}"
    );
    assert!(
        err.contains("num.hex_to_decimal"),
        "the close match should appear; got {err:?}"
    );
}

#[tokio::test]
async fn tool_call_supports_a_base64_round_trip() {
    let enc = post_tool("convert.base64_encode", r#"{"input":"hello"}"#).await;
    let enc_body = body_to_value(enc.into_body()).await;
    assert_eq!(primary_value(&enc_body), "aGVsbG8=");

    let dec = post_tool("convert.base64_decode", r#"{"input":"aGVsbG8="}"#).await;
    let dec_body = body_to_value(dec.into_body()).await;
    assert_eq!(primary_value(&dec_body), "hello");
}

#[tokio::test]
async fn tool_list_includes_runtime_tools() {
    const ID: &str = "test.http_visible";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "test",
        local_id: "http_visible",
        tags: &[],
        display_label: "Test tool",
        description: "http visibility",
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

    let body = tools_body().await;
    let tools = body["tools"].as_array().unwrap();
    let entry = tools
        .iter()
        .find(|t| t["name"] == ID)
        .unwrap_or_else(|| panic!("runtime tool missing from /v1/tools"));
    assert_eq!(entry["description"], "http visibility");
    assert_eq!(entry["invoker"], "external");
}

#[tokio::test]
async fn tool_call_is_routed_to_the_runtime_dispatcher() {
    const ID: &str = "test.http_dispatch";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "test",
        local_id: "http_dispatch",
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
    upeg_runtime::register_single_text_runtime_dispatcher(ID, |args| {
        let s = args
            .get("greeting")
            .and_then(|v| v.as_str())
            .expect("greeting string");
        Ok(format!("hi-{s}"))
    });

    let resp = post_tool(ID, r#"{"greeting":"http"}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(body["outputs"][0]["value"], "hi-http");
}

#[tokio::test]
async fn tool_call_runtime_dispatcher_error_returns_422() {
    const ID: &str = "test.http_dispatch_err";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "test",
        local_id: "http_dispatch_err",
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
    upeg_runtime::register_single_text_runtime_dispatcher(ID, |_| Err("nope".into()));

    let resp = post_tool(ID, "{}").await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(body["error"]["message"], "nope");
}

#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "test holds the global controlled_embed backend lock across async HTTP assertions to prevent cross-test backend races"
)]
async fn controlled_embed_http_wait_timeout_returns_422_and_a_canonical_failure_envelope() {
    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    ensure_controlled_embed_http_fixture_loaded();
    upeg_runtime::controlled_embed::set_controlled_embed_backend(std::sync::Arc::new(
        WaitTimeoutControlledEmbedBackend,
    ));

    let resp = post_tool(CONTROLLED_EMBED_HTTP_TOOL_ID, r#"{"query":"upeg"}"#).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_to_value(resp.into_body()).await;

    assert_eq!(body["ok"], false);
    assert_eq!(
        body["error"]["code"],
        CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_CODE
    );
    let message = body["error"]["message"]
        .as_str()
        .expect("wait timeout message");
    assert!(
        message.contains(CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_SELECTOR),
        "message must include the timed-out binding selector: {message}"
    );
    assert!(
        message.contains(CONTROLLED_EMBED_HTTP_WAIT_TIMEOUT_FOR_SELECTOR),
        "message must include the waited selector: {message}"
    );
    assert!(message.contains("Visible"), "message: {message}");
    assert!(message.contains("125ms"), "message: {message}");
}

#[tokio::test]
async fn tool_list_excludes_tools_without_the_http_surface() {
    const ID: &str = "test.http_excluded_from_list";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: ID,
        toolkit: "test",
        local_id: "http_excluded_from_list",
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

    let body = tools_body().await;
    let tools = body["tools"].as_array().unwrap();
    assert!(
        tools.iter().all(|t| t["name"] != ID),
        "non-http tool must not appear in /v1/tools",
    );
}

#[tokio::test]
async fn tool_call_404_does_not_distinguish_surface_gate_from_absence() {
    const GATED_ID: &str = "test.surface_gated";
    const ABSENT_ID: &str = "test.totally_missing";

    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: GATED_ID,
        toolkit: "test",
        local_id: "surface_gated",
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

    let gated_resp = post_tool(GATED_ID, "{}").await;
    let gated_status = gated_resp.status();
    let gated_body = body_to_value(gated_resp.into_body()).await;

    let absent_resp = post_tool(ABSENT_ID, "{}").await;
    let absent_status = absent_resp.status();
    let absent_body = body_to_value(absent_resp.into_body()).await;

    assert_eq!(gated_status, StatusCode::NOT_FOUND);
    assert_eq!(absent_status, StatusCode::NOT_FOUND);

    let gated_err = gated_body["error"]["message"]
        .as_str()
        .expect("error string");
    let absent_err = absent_body["error"]["message"]
        .as_str()
        .expect("error string");
    let gated_norm = gated_err.replace(GATED_ID, "<ID>");
    let absent_norm = absent_err.replace(ABSENT_ID, "<ID>");
    assert_eq!(
        gated_norm, absent_norm,
        "the two 404 branches must use the same error template; \
         gated=`{gated_err}` absent=`{absent_err}`"
    );
}
