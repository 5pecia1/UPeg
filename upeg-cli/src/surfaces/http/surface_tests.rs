//! Focused HTTP surface-gating regression tests split from `http_tests.rs`.

use crate::surfaces::http::router;
use axum::body::Body;
use http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn tool_without_http_surface_returns_404_on_call() {
    let id = "test.iter41.http_call_refused";
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

    let resp = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "non-http tool must 404 — don't leak existence to unauthorized callers"
    );
}
