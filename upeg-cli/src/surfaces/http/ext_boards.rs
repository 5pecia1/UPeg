//! Extension-only Board adapter.
//!
//! Browser extension calls are execution mode `ext`, not a claim about the
//! browser user's authenticated identity. This adapter therefore stamps
//! [`Surface::Ext`] while deriving the principal role from the bearer token.

use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Map, Value, json};
use upeg_core::{
    ArgsPreset, BoardKey, EXECUTION_CONTEXT_APPROVED_STEPS, EXECUTION_CONTEXT_ARG, InputKind,
    Surface,
};
use upeg_runtime::ToolMetaRuntimeExt;
use upeg_sources::pegboard::{self, PegboardState};

use super::{
    HttpState, dispatch_http_tool_with_context, not_found_response, parse_tool_call_body,
    principal_on_surface,
};

const APPROVE_ARG: &str = "approve";

pub(super) async fn ext_boards_list() -> Json<Value> {
    let state = pegboard::load_state();
    Json(json!({
        "boards": pegboard::board_keys_in(&state)
            .iter()
            .map(|board| ext_board_json(&state, board))
            .collect::<Vec<_>>(),
    }))
}

pub(super) async fn ext_board_show(Path(board): Path<String>) -> impl IntoResponse {
    let state = pegboard::load_state();
    if !pegboard::board_exists_in(&state, &board) {
        return not_found_response("board", &board);
    }
    (StatusCode::OK, Json(ext_board_json(&state, &board)))
}

pub(super) async fn ext_board_tools_call(
    State(state): State<HttpState>,
    Path((board, id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let Ok(board_key) = BoardKey::parse(&board) else {
        return not_found_response("board tool", &format!("{board}/{id}"));
    };
    let pegboard_state = pegboard::load_state();
    let Some(placement) = pegboard::board_placement_on_surface_in(
        &pegboard_state,
        board_key.as_str(),
        &id,
        Surface::Ext,
    ) else {
        return not_found_response("board tool", &format!("{board}/{id}"));
    };
    if upeg_runtime::tool_approval_policy(&id).requires_approval() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(crate::domain::execution::dispatch::dispatch_failure(
                "approval_unsupported",
                "the browser extension cannot run a tool that requires approval; open it in Desktop upeg",
            ).to_canonical_json()),
        );
    }
    let body = match parse_tool_call_body(&body) {
        Ok(args) => strip_extension_approval(args),
        Err(response) => return response,
    };
    let context = upeg_runtime::ExecutionContext::board(
        Surface::Ext,
        board_key,
        extension_preset(placement.args_preset.as_ref()),
    )
    .with_principal(principal_on_surface(&state, &headers, Surface::Ext));
    dispatch_http_tool_with_context(
        state.notifications_enabled,
        id,
        Bytes::from(body.to_string()),
        context,
        None,
    )
}

fn ext_board_json(state: &PegboardState, board: &str) -> Value {
    json!({
        "board": board,
        "tools": pegboard::board_entries_on_surface_in(state, board, None, Surface::Ext)
            .into_iter()
            .map(|(placement, tool)| ext_tool_json(tool, placement.args_preset.as_ref()))
            .collect::<Vec<_>>(),
    })
}

fn ext_tool_json(tool: &upeg_core::ToolMeta, preset: Option<&ArgsPreset>) -> Value {
    let mut metadata = tool.to_json_object("name");
    let Some(object) = metadata.as_object_mut() else {
        return metadata;
    };
    let preset = preset.map_or_else(Map::new, ArgsPreset::to_object);
    let mut args_preset = Map::new();
    let mut preset_fields = Vec::new();
    for field in &tool.input_spec.fields {
        let name = field.name.as_str();
        let Some(value) = preset.get(name) else {
            continue;
        };
        if name == APPROVE_ARG || name == EXECUTION_CONTEXT_ARG {
            continue;
        }
        preset_fields.push(Value::String(name.to_string()));
        if !matches!(field.kind, InputKind::File(_)) {
            args_preset.insert(name.to_string(), value.clone());
        }
    }
    object.insert("argsPreset".into(), Value::Object(args_preset));
    object.insert("presetFields".into(), Value::Array(preset_fields));
    object.insert(
        "requiresApproval".into(),
        Value::Bool(tool.requires_approval()),
    );
    metadata
}

fn extension_preset(preset: Option<&ArgsPreset>) -> ArgsPreset {
    let mut preset = preset.map_or_else(Map::new, ArgsPreset::to_object);
    preset.remove(APPROVE_ARG);
    ArgsPreset::from_object(preset).unwrap_or_else(|_| upeg_runtime::empty_args_preset())
}

fn strip_extension_approval(args: Value) -> Value {
    let Value::Object(mut args) = args else {
        return args;
    };
    args.remove(APPROVE_ARG);
    if let Some(Value::Object(context)) = args.get_mut(EXECUTION_CONTEXT_ARG) {
        context.remove(EXECUTION_CONTEXT_APPROVED_STEPS);
    }
    Value::Object(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use http::{Request, StatusCode};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    static EXT_DISPATCHES: AtomicUsize = AtomicUsize::new(0);

    fn field(name: &str, kind: InputKind) -> upeg_core::InputFieldSpec {
        upeg_core::InputFieldSpec::new(
            upeg_core::InputName::new(name).expect("test input name"),
            None,
            None,
            false,
            kind,
        )
        .expect("test input field")
    }

    fn tool_with_inputs() -> upeg_core::ToolMeta {
        upeg_core::ToolMeta {
            id: "extcontract.metadata",
            toolkit: "extcontract",
            local_id: "metadata",
            tags: &[],
            display_label: "Extension metadata contract",
            description: "test fixture",
            input_spec: upeg_core::InputSpec::new(vec![
                field("literal", InputKind::String),
                field("flag", InputKind::Boolean),
                field("count", InputKind::Integer),
                field("empty", InputKind::String),
                field(
                    "upload",
                    InputKind::File(upeg_core::FileInputPolicy::default()),
                ),
            ])
            .expect("test input spec"),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: &[Surface::Ext],
            boards: &[],
        }
    }

    #[test]
    fn metadata_exposes_declared_literal_presets_but_not_files_or_backend_values() {
        let preset = ArgsPreset::parse(
            r#"{"literal":"visible","flag":false,"count":0,"empty":"","upload":{"name":"secret.bin","content":{"kind":"bytes","bytes":"AA=="}},"credential":"backend-sentinel","approve":true,"undeclared":255}"#,
        )
        .expect("test preset");

        let json = ext_tool_json(&tool_with_inputs(), Some(&preset));

        assert_eq!(json["argsPreset"]["literal"], "visible");
        assert_eq!(json["argsPreset"]["flag"], false);
        assert_eq!(json["argsPreset"]["count"], 0);
        assert_eq!(json["argsPreset"]["empty"], "");
        assert!(json["argsPreset"].get("upload").is_none());
        assert!(json["argsPreset"].get("credential").is_none());
        assert!(json["argsPreset"].get("approve").is_none());
        assert!(json["argsPreset"].get("undeclared").is_none());
        assert_eq!(
            json["presetFields"],
            json!(["literal", "flag", "count", "empty", "upload"])
        );
    }

    #[test]
    fn extension_scrub_removes_both_approval_levers_without_losing_cwd() {
        let args = json!({
            "approve": true,
            "literal": "kept",
            "_upeg": { "approvedSteps": ["gate"], "cwd": "/tmp/kept" },
        });

        let args = strip_extension_approval(args);

        assert!(args.get("approve").is_none());
        assert!(args["_upeg"].get("approvedSteps").is_none());
        assert_eq!(args["_upeg"]["cwd"], "/tmp/kept");
        assert_eq!(
            extension_preset(Some(
                &ArgsPreset::parse(r#"{"approve":true,"literal":"kept"}"#).expect("preset")
            ))
            .to_object(),
            json!({ "literal": "kept" })
                .as_object()
                .expect("object")
                .clone()
        );
    }

    #[test]
    fn extension_adapter_lists_and_calls_only_extension_visible_pins() {
        EXT_DISPATCHES.store(0, Ordering::SeqCst);
        let ext_tool = tool_with_inputs();
        upeg_runtime::toolbox_add_tool(ext_tool);
        upeg_runtime::register_single_text_runtime_dispatcher("extcontract.metadata", |args| {
            EXT_DISPATCHES.fetch_add(1, Ordering::SeqCst);
            let literal = args
                .get("literal")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| "missing merged literal preset".to_string())?;
            let context = args
                .get(EXECUTION_CONTEXT_ARG)
                .ok_or_else(|| "missing extension context".to_string())?;
            Ok(format!(
                "{literal}:{}:{}",
                context["principal"]["role"]
                    .as_str()
                    .ok_or_else(|| "missing principal role".to_string())?,
                context["surface"]
                    .as_str()
                    .ok_or_else(|| "missing surface".to_string())?,
            ))
        });
        let mut http_only = tool_with_inputs();
        http_only.id = "extcontract.http_only";
        http_only.local_id = "http_only";
        http_only.surfaces = &[Surface::Http];
        upeg_runtime::toolbox_add_tool(http_only);

        crate::test_support::with_seeded_pegboard_home(
            "http-extension-board-adapter",
            |state| {
                state.boards.push(upeg_sources::pegboard::BoardData {
                    guidance: upeg_core::BoardGuidance::default(),
                    key: "ext-contract".into(),
                    title: "Ext contract".into(),
                });
                state.layouts.insert(
                    "ext-contract".into(),
                    vec![
                        upeg_core::Placement::new("extcontract.metadata", 0, 0).with_args_preset(
                            Some(ArgsPreset::parse(r#"{"literal":"saved"}"#).expect("test preset")),
                        ),
                        upeg_core::Placement::new("extcontract.http_only", 1, 0),
                    ],
                );
            },
            || {
                let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
                runtime.block_on(async {
                    let app = super::super::router();
                    let list = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .uri("/v1/ext/boards/ext-contract")
                                .body(Body::empty())
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(list.status(), StatusCode::OK);
                    let list: Value = serde_json::from_slice(
                        &to_bytes(list.into_body(), usize::MAX).await.expect("body"),
                    )
                    .expect("json");
                    assert_eq!(list["tools"].as_array().expect("tools").len(), 1);
                    assert_eq!(list["tools"][0]["name"], "extcontract.metadata");
                    assert_eq!(list["tools"][0]["argsPreset"]["literal"], "saved");

                    let call = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .method("POST")
                                .uri("/v1/ext/boards/ext-contract/tools/extcontract.metadata")
                                .header("content-type", "application/json")
                                .body(Body::from(
                                    r#"{"_upeg":{"principal":{"role":"operator","surface":"cli"},"approvedSteps":["gate"]},"approve":true}"#,
                                ))
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(call.status(), StatusCode::OK);
                    let call: Value = serde_json::from_slice(
                        &to_bytes(call.into_body(), usize::MAX).await.expect("body"),
                    )
                    .expect("json");
                    assert_eq!(call["outputs"][0]["value"], "saved:agent:ext");

                    let override_call = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .method("POST")
                                .uri("/v1/ext/boards/ext-contract/tools/extcontract.metadata")
                                .header("content-type", "application/json")
                                .body(Body::from(r#"{"literal":"override"}"#))
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(override_call.status(), StatusCode::OK);
                    let override_call: Value = serde_json::from_slice(
                        &to_bytes(override_call.into_body(), usize::MAX)
                            .await
                            .expect("body"),
                    )
                    .expect("json");
                    assert_eq!(
                        override_call["outputs"][0]["value"],
                        "override:agent:ext"
                    );

                    let unpinned = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .method("POST")
                                .uri("/v1/ext/boards/ext-contract/tools/extcontract.http_only")
                                .body(Body::from("{}"))
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(unpinned.status(), StatusCode::NOT_FOUND);

                    upeg_runtime::set_tool_approval_policy(
                        "extcontract.metadata",
                        upeg_runtime::ToolApprovalPolicy::gated(vec![Surface::Ext]),
                    );
                    let gated = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .method("POST")
                                .uri("/v1/ext/boards/ext-contract/tools/extcontract.metadata")
                                .body(Body::from(r#"{"approve":true}"#))
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(gated.status(), StatusCode::UNPROCESSABLE_ENTITY);
                    assert_eq!(EXT_DISPATCHES.load(Ordering::SeqCst), 2);
                    upeg_runtime::set_tool_approval_policy(
                        "extcontract.metadata",
                        upeg_runtime::ToolApprovalPolicy::none(),
                    );

                    let hidden = app
                        .oneshot(
                            Request::builder()
                                .uri("/v1/ext/boards/ext-contract")
                                .body(Body::empty())
                                .expect("request"),
                        )
                        .await
                        .expect("response");
                    assert_eq!(hidden.status(), StatusCode::OK);
                });
            },
        );
    }
}
