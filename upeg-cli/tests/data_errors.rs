#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests verify data-tool failure envelopes"
)]

use axum::body::{Body, to_bytes};
use clap::Parser as _;
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use upeg_cli::{Outcome, dispatch_tool, handle_mcp_message, http_router};

async fn assert_failure_on_all_surfaces(tool: &str, args: Value, reason: &str) {
    let Outcome::Failure(failure) = dispatch_tool(tool, &args) else {
        panic!("{tool} unexpectedly accepted {args}");
    };
    let message = failure.error.message;
    assert!(message.contains(reason), "{tool}: {message}");
    let mcp = handle_mcp_message(json!({
        "jsonrpc":"2.0","id":1,"method":"tools/call",
        "params":{"name":tool,"arguments":args}
    }))
    .expect("MCP response");
    assert_eq!(mcp["result"]["isError"], true);
    assert_eq!(mcp["result"]["content"][0]["text"], message);
    let response = http_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{tool}"))
                .header("content-type", "application/json")
                .body(Body::from(args.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(body["error"]["message"], message);
    assert!(
        body.get("outputs").is_none(),
        "failure must not contain a success result"
    );
}

#[tokio::test]
async fn malformed_documents_fail_consistently_across_surfaces() {
    for (args, reason) in [
        (json!({"left":"{","right":"null"}), "left JSON"),
        (
            json!({"left":"null","right":"a: 1\na: 2","right_format":"yaml"}),
            "duplicate YAML",
        ),
    ] {
        assert_failure_on_all_surfaces("data.value_diff", args, reason).await;
    }
}

#[tokio::test]
async fn sampling_and_schema_failures_remain_errors_across_surfaces() {
    assert_failure_on_all_surfaces(
        "data.schema_generate",
        json!({"input":"[]","sample_mode":"samples"}),
        "nonempty array",
    )
    .await;
    assert_failure_on_all_surfaces(
        "data.schema_diff",
        json!({
            "left":{"type":"number","minimum":0},"right":{"type":"number"}
        }),
        "minimum",
    )
    .await;
    assert_failure_on_all_surfaces(
        "data.schema_diff",
        json!({
            "left":{"version":2,"policy":"observed_types_v1","schema":{"type":"number"}},
            "right":{"type":"number"}
        }),
        "version 1",
    )
    .await;
}

#[test]
fn dispatch_rejects_wrong_input_types_missing_fields_and_unknown_options() {
    for (tool, args) in [
        ("data.value_diff", json!({"left":{},"right":"{}"})),
        ("data.value_diff", json!({"left":"{}"})),
        (
            "data.value_diff",
            json!({"left":"{}","right":"{}","left_format":"toml"}),
        ),
        (
            "data.schema_generate",
            json!({"input":"{}","sample_mode":"all"}),
        ),
        ("data.schema_diff", json!({"left":{"type":"string"}})),
    ] {
        assert!(
            matches!(dispatch_tool(tool, &args), Outcome::Failure(_)),
            "{tool} accepted {args}"
        );
    }
}

#[test]
fn cli_argument_validation_rejects_unknown_data_tool_fields() {
    // Unknown-key rejection belongs to CLI argument validation; the lower-level
    // dispatcher validates declared fields and permits execution context keys.
    let cli = upeg_cli::Cli::try_parse_from([
        "upeg",
        "call",
        "--local",
        "data.schema_generate",
        r#"{"input":"{}","unexpected":true}"#,
    ])
    .expect("CLI parses");
    let error = upeg_cli::run(cli).expect_err("unknown input must fail");
    assert!(
        error.message().contains("unknown input `unexpected`"),
        "{error:?}"
    );
}
