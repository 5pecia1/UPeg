use super::*;

// ─── OpenAPI generation  ─────────────────────────────

async fn fetch_openapi() -> Value {
    let resp = router()
        .oneshot(
            Request::builder()
                .uri("/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    body_to_value(resp.into_body()).await
}

#[tokio::test]
async fn openapi는_상단_수준_필수_필드를_가진다() {
    let spec = fetch_openapi().await;
    // OpenAPI 3.0 spec mandates these. Swagger UI rejects docs without them.
    assert_eq!(spec["openapi"], "3.0.3");
    assert!(spec["info"].is_object());
    assert!(spec["info"]["title"].as_str().unwrap().contains("upeg"));
    assert!(spec["info"]["version"].is_string());
    assert!(spec["paths"].is_object());
}

#[tokio::test]
async fn openapi는_알려진_내장_도구를_포함한다() {
    let spec = fetch_openapi().await;
    let paths = spec["paths"].as_object().unwrap();
    // Every registered cross-surface tool gets a concrete path —
    // OpenAPI doesn't expand `{id}` templates server-side.
    for id in ["num.hex_to_decimal", "id.uuid_v7", "hash.sha256"] {
        let path = format!("/v1/tools/{id}");
        assert!(
            paths.contains_key(&path),
            "expected `{path}` in OpenAPI paths, got keys: {:?}",
            paths.keys().collect::<Vec<_>>(),
        );
    }
}

#[tokio::test]
async fn openapi는_v21_리소스_경로를_포함한다() {
    let spec = fetch_openapi().await;
    let paths = spec["paths"].as_object().unwrap();
    for path in [
        "/v1/toolkits",
        "/v1/toolkits/{toolkit}",
        "/v1/toolkits/{toolkit}/{tool}",
        "/v1/tags",
        "/v1/tags/{tag}",
        "/v1/boards",
        "/v1/boards/{board}",
        "/v1/boards/{board}/tools/{id}",
        "/v1/credentials",
        "/v1/logs",
        "/v1/triggers",
        "/v1/trigger/{id}",
    ] {
        assert!(
            paths.contains_key(path),
            "PRD v2.1 resource route `{path}` missing from OpenAPI"
        );
    }
}

#[tokio::test]
async fn openapi_경로_템플릿은_필수_매개변수를_선언한다() {
    let spec = fetch_openapi().await;
    for (path, item) in spec["paths"].as_object().unwrap() {
        let names = path_template_names(path);
        if names.is_empty() {
            continue;
        }
        for method in ["get", "post"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let params = operation["parameters"]
                .as_array()
                .unwrap_or_else(|| panic!("{method} `{path}` missing path parameters"));
            assert_eq!(
                params.len(),
                names.len(),
                "{method} `{path}` must declare exactly its template parameters"
            );
            for name in &names {
                let param = params
                    .iter()
                    .find(|param| param["name"] == *name)
                    .unwrap_or_else(|| panic!("{method} `{path}` missing parameter `{name}`"));
                assert_eq!(param["in"], "path", "{method} `{path}` parameter `{name}`");
                assert_eq!(
                    param["required"], true,
                    "{method} `{path}` parameter `{name}`"
                );
                assert_eq!(
                    param["schema"]["type"], "string",
                    "{method} `{path}` parameter `{name}`"
                );
            }
        }
    }
}

fn path_template_names(path: &str) -> Vec<&str> {
    path.split('{')
        .skip(1)
        .filter_map(|tail| tail.split_once('}').map(|(name, _)| name))
        .collect()
}

#[test]
fn 오류_응답_도우미는_안정적인_형태를_유지한다() {
    // pin the helper's output shape so a future edit
    // that drops/renames the `error` field is a loud diff. Adding
    // a new field (e.g., `code`) to all three status responses
    // intentionally ripples through this helper — update this
    // test in the same diff.
    let v = error_response("test description");
    assert_eq!(v["description"], "test description");
    let schema = &v["content"]["application/json"]["schema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["required"], json!(["ok", "error"]));
    assert_eq!(schema["properties"]["ok"]["enum"][0], false);
    assert_eq!(
        schema["properties"]["error"]["required"],
        json!(["code", "message"])
    );
}

#[tokio::test]
async fn openapi의_404_응답은_오류_본문_schema를_알린다() {
    // The 404 response must advertise the `{"error": "..."}` body
    // schema; otherwise generated clients don't know to deserialise
    // it. Pin parity with the 400/422 schema shape so a future spec
    // edit that drops it (or differs from sibling error responses)
    // is a loud diff.
    let spec = fetch_openapi().await;
    let post = &spec["paths"]["/v1/tools/num.hex_to_decimal"]["post"];
    let r404 = &post["responses"]["404"];

    // Sibling 400/422 shape: `content -> application/json -> schema`
    // with `{type: object, required: [error], properties: {error: {type: string}}}`.
    let schema = &r404["content"]["application/json"]["schema"];
    assert_eq!(
        schema["type"], "object",
        "404 response schema must be an object; got {schema}"
    );
    assert_eq!(
        schema["required"],
        json!(["ok", "error"]),
        "404 response must require the `error` field; got {schema}"
    );
    assert_eq!(
        schema["properties"]["error"]["properties"]["message"]["type"], "string",
        "404 `error.message` field must be string-typed; got {schema}"
    );

    // Sanity: the parallel 400/422 responses still have their schemas
    // (this test would already fail if they were dropped, but pin
    // explicitly so a future edit that drops error bodies wholesale
    // shows up here, not just in fragile downstream codegen).
    for code in ["400", "422"] {
        let s = &post["responses"][code]["content"]["application/json"]["schema"];
        assert_eq!(
            s["type"], "object",
            "{code} response schema must remain an object; got {s}"
        );
    }
}

#[tokio::test]
async fn openapi는_상태_경로의_저장소_읽기_오류를_문서화한다() {
    let spec = fetch_openapi().await;
    for path in ["/v1/credentials", "/v1/logs"] {
        let r500 = &spec["paths"][path]["get"]["responses"]["500"];
        let schema = &r500["content"]["application/json"]["schema"];
        assert_eq!(
            schema["required"],
            json!(["ok", "error"]),
            "{path} 500 response must document the shared error shape"
        );
    }
}

#[tokio::test]
async fn openapi는_http_표면_없이_도구를_제외한다() {
    let id = "test.iter44.openapi_excluded";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "should not appear",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Cli], // cli-only
        boards: &[],
    });

    let spec = fetch_openapi().await;
    let paths = spec["paths"].as_object().unwrap();
    let bad_path = format!("/v1/tools/{id}");
    assert!(
        !paths.contains_key(&bad_path),
        "non-http tool must not appear in /v1/openapi.json"
    );
}

#[tokio::test]
async fn openapi_경로_항목은_각_작업의_정상_형태를_가진다() {
    let spec = fetch_openapi().await;
    let item = &spec["paths"]["/v1/tools/num.hex_to_decimal"];
    let op = &item["post"];
    assert_eq!(op["operationId"], "num.hex_to_decimal");
    assert!(
        op["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag == "num"),
        "OpenAPI tags must include the owning Toolkit tag"
    );
    // requestBody schema is generated from the tool's typed InputSpec.
    let req_schema = &op["requestBody"]["content"]["application/json"]["schema"];
    assert_eq!(req_schema["type"], "object");
    assert_eq!(req_schema["properties"]["input"]["type"], "string");
    assert_eq!(req_schema["required"][0], "input");
    assert_eq!(req_schema["additionalProperties"], false);
    // 200 + 422 + 404 must all be present.
    assert!(op["responses"]["200"].is_object());
    assert!(op["responses"]["422"].is_object());
    assert!(op["responses"]["404"].is_object());
    let ok_schema = &op["responses"]["200"]["content"]["application/json"]["schema"];
    assert_eq!(ok_schema["properties"]["ok"]["enum"][0], true);
    assert_eq!(
        ok_schema["required"],
        json!(["ok", "primary_output_id", "outputs"])
    );
    assert_eq!(ok_schema["properties"]["outputs"]["type"], "array");
    assert!(ok_schema["properties"].get("structuredContent").is_none());
}

#[tokio::test]
async fn openapi_경로_항목은_x_확장_필드를_가진다() {
    let spec = fetch_openapi().await;
    let op = &spec["paths"]["/v1/tools/num.hex_to_decimal"]["post"];
    // OpenAPI 3.0 explicitly permits `x-*` extension fields.
    // We expose the rest of ToolMeta so clients can inspect it
    // without an extra round trip to /v1/tools.
    assert_eq!(op["x-pin"], "Inline");
    assert_eq!(op["x-pegboard-units"], "U2");
    assert_eq!(op["x-invoker"], "function");
    let surfaces = op["x-surfaces"].as_array().expect("x-surfaces array");
    assert!(
        surfaces.iter().any(|s| s == "http"),
        "this tool's x-surfaces must include `http` (it's the surface serving this spec)"
    );
    // boards is an array (possibly empty); for hex_to_decimal it's `["dev"]`.
    assert!(op["x-boards"].is_array());
}

#[tokio::test]
async fn openapi의_x_확장은_모든_경로에_존재한다() {
    // Every emitted path must carry the same x-* keys — mismatched
    // shapes break tooling that walks the spec uniformly. x-embed-url
    // and x-selector-bindings mirror /v1/tools.
    let spec = fetch_openapi().await;
    for (path, item) in spec["paths"].as_object().unwrap() {
        // Only the per-tool paths carry tool metadata. Templated paths
        // (`/v1/tools/{id}/stream`) describe one shape shared by every
        // tool, so there is no single tool whose pin/units/invoker they
        // could name — those live on the tool's own path.
        if !path.starts_with("/v1/tools/") || path.contains('{') {
            continue;
        }
        let op = &item["post"];
        for k in [
            "x-pin",
            "x-pegboard-units",
            "x-invoker",
            "x-surfaces",
            "x-boards",
            "x-embed-url",
            "x-selector-bindings",
        ] {
            assert!(op.get(k).is_some(), "path `{path}` missing `{k}`: {item}");
        }
        // Selector bindings must always be an array (empty when none),
        // matching /v1/tools' contract — so codegen can rely on the
        // shape without conditional handling.
        assert!(
            op["x-selector-bindings"].is_array(),
            "x-selector-bindings must be an array on every path; got {item}"
        );
    }
}

#[tokio::test]
async fn openapi는_잘못된_형식_본문에_대한_400_응답을_문서화한다() {
    // /v1/tools/{id} returns 400 for malformed JSON bodies. The    // OpenAPI spec must document it so Swagger UI / Postman users
    // see the full response surface.
    let spec = fetch_openapi().await;
    let op = &spec["paths"]["/v1/tools/num.hex_to_decimal"]["post"];
    let r400 = &op["responses"]["400"];
    assert!(
        r400.is_object(),
        "400 response must be present in OpenAPI for every path; got {op}"
    );
    let schema = &r400["content"]["application/json"]["schema"];
    assert_eq!(
        schema["properties"]["error"]["properties"]["message"]["type"], "string",
        "400 body must declare {{error: string}} shape; got {schema}"
    );
    assert_eq!(schema["required"], json!(["ok", "error"]));
}

#[tokio::test]
async fn openapi의_x_embed_필드는_등록된_embed_정보를_왕복한다() {
    // Register an embed-tool + URL + bindings; OpenAPI must reflect
    // both in the corresponding path item's x-* fields.
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "test.iter129.openapi_embed",
        toolkit: "test",
        local_id: "iter129.openapi_embed",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Embed,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Static,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_embed_url(
        "test.iter129.openapi_embed",
        "https://example.test/openapi-embed",
    );
    upeg_runtime::set_selector_bindings(
        "test.iter129.openapi_embed",
        vec![upeg_runtime::SelectorBinding {
            role: upeg_runtime::BindingRole::Input,
            field: "input".into(),
            selector: ".q".into(),
            trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
            wait: None,
        }],
    );

    let spec = fetch_openapi().await;
    let op = &spec["paths"]["/v1/tools/test.iter129.openapi_embed"]["post"];
    assert_eq!(
        op["x-embed-url"], "https://example.test/openapi-embed",
        "x-embed-url must round-trip the registered URL; got {op}"
    );
    let bindings = op["x-selector-bindings"].as_array().expect("array");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["field"], "input");
    assert_eq!(bindings[0]["selector"], ".q");
}

#[tokio::test]
async fn description이_비어_있으면_openapi는_id로_대체한다() {
    // A runtime tool with empty description must still produce a valid    // path item — `summary` falls through to the tool id rather than    // emitting an empty string (Swagger UI rendered "" gets ugly).
    let id = "test.iter44.empty_desc";
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
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });

    let spec = fetch_openapi().await;
    let item = &spec["paths"][format!("/v1/tools/{id}")];
    assert_eq!(item["post"]["summary"], id);
}
