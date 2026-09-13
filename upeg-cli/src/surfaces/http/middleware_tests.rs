//! HTTP middleware + router tests. Extracted from `surfaces/http.rs`
//! to keep that file under the workspace 1000-line file-size budget.
//!
//! Mounted into `surfaces/http.rs` via `#[path]` so the tests can keep
//! poking at module-private helpers (`pause_decision`, `HttpState`,
//! `router_with_state`).

use super::*;
use axum::body::{Body, to_bytes};
use http::Request;
use tower::ServiceExt;

const T: &str = "test-token";

fn auth_router() -> Router {
    auth_router_with_cors_origins(Vec::new())
}

fn auth_router_with_cors_origins(web_origins: Vec<String>) -> Router {
    router_with_state(HttpState {
        tokens: Arc::new(crate::infrastructure::auth::HostTokens::with_agents(
            T,
            Vec::new(),
        )),
        skip_origin_guard: false,
        notifications_enabled: false,
        origin_policy: Arc::new(
            cors::OriginPolicy::new(web_origins).expect("test web origins must be valid"),
        ),
    })
}

async fn body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn 상태확인은_베어러와_오리진_검사를_우회한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn 누락된_토큰은_거부된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn 올바른_토큰은_성공한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn 외부_오리진은_금지된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "http://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn 루프백_오리진은_허용된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "http://localhost:1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost:1234"),
        "loopback web origin must also get the ACAO header so a locally-served PWA can read the response"
    );
}

// === CORS (Task B1) ===
//
// Preflight (`OPTIONS`) behavior comes straight from `tower_http`'s
// `CorsLayer`: it always answers `200` (it never 4xx/5xxs a preflight —
// that's how the crate implements the spec), and either includes
// `Access-Control-Allow-Origin` (origin permitted) or omits it (origin
// refused). A browser treats a response with no ACAO header as
// "refused" even though the raw status is 200 — so these tests assert
// on header presence/value, not status code, for the "refused" cases.

#[tokio::test]
async fn 확장_프로그램_오리진의_사전요청은_토큰_없이_성공한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/tools")
                .header("origin", "chrome-extension://abcdefghijklmnop")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("chrome-extension://abcdefghijklmnop"),
        "preflight from an allowed extension origin must carry ACAO, and must succeed without a Bearer token"
    );
}

#[tokio::test]
async fn 허용되지_않은_웹_오리진의_사전요청은_에이씨에이오_헤더_없이_거부된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/tools")
                .header("origin", "https://evil.example.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "disallowed web origin must not receive an ACAO header, even on a 200 preflight response"
    );
}

#[tokio::test]
async fn 허용된_오리진과_유효한_토큰의_실제_요청은_에이씨에이오_헤더를_포함한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "chrome-extension://abcdefghijklmnop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("chrome-extension://abcdefghijklmnop")
    );
}

#[tokio::test]
async fn 설정된_cors_오리진_웹_클라이언트는_토큰이_있으면_허용된다() {
    let app = auth_router_with_cors_origins(vec!["https://app.example.com".to_string()]);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "https://app.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://app.example.com"),
        "--cors-origin must add the configured web origin to both CORS and the guard"
    );
}

#[tokio::test]
async fn 설정되지_않은_다른_웹_오리진은_cors_origin_설정과_무관하게_거부된다() {
    let app = auth_router_with_cors_origins(vec!["https://app.example.com".to_string()]);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "https://other.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "the origin guard (not just CORS headers) must actually refuse an unconfigured web origin"
    );
}

/// Everything `/healthz` is allowed to say: `name`, `version`, and the
/// two MCP-import fields (`importsPending` + its counts block). The
/// route is unauthenticated, so this cap is a real budget — growing it
/// means deciding, again, that the new field is safe pre-pairing.
const HEALTHZ_MAX_FIELDS: usize = 4;

#[tokio::test]
async fn 상태확인_응답은_페어링_힌트_형태를_반환한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(body["name"], "upeg");
    assert!(
        body.get("restApi").is_none(),
        "desired-state gating is gone; a running host always serves /v1: {body}"
    );
    assert!(
        body.get("token").is_none()
            && body
                .as_object()
                .is_some_and(|obj| obj.len() <= HEALTHZ_MAX_FIELDS),
        "healthz must stay a minimal, non-secret pairing hint: {body}"
    );
}

#[tokio::test]
async fn chrome_extension_origin은_loopback에서_허용된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "chrome-extension://abcdefghijklmnop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn chrome_extension_origin이어도_비루프백_호스트는_거부된다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .header("origin", "chrome-extension://abcdefghijklmnop")
                .header("host", "evil.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mcp_경로는_json_알피시를_실행한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    assert_eq!(body["id"], 1);
    assert!(body["result"]["tools"].as_array().is_some());
}

#[test]
fn 일시정지된_상태에서_일시정지_결정은_상태확인을_허용한다() {
    assert!(!pause_decision("/healthz", true));
}

#[test]
fn 일시정지된_상태에서_일시정지_결정은_상태확인이_아닌_경로를_막는다() {
    assert!(pause_decision("/v1/tools", true));
    assert!(pause_decision("/mcp", true));
    assert!(pause_decision("/v1/clients/heartbeat", true));
}

#[test]
fn 일시정지되지_않은_상태에서_일시정지_결정은_그대로_통과시킨다() {
    assert!(!pause_decision("/healthz", false));
    assert!(!pause_decision("/v1/tools", false));
}

#[tokio::test]
async fn 하트비트_기록은_클라이언트_목록에_나타난다() {
    let app = auth_router();
    let unique = format!("test-{}-{}", std::process::id(), line!());
    let body = format!(r#"{{"client_id":"{unique}","label":"test"}}"#);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/clients/heartbeat")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/clients")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_to_value(resp.into_body()).await;
    let clients = body["clients"].as_array().expect("clients array");
    assert!(
        clients.iter().any(|c| c["client_id"] == unique),
        "registered heartbeat must surface in /v1/clients: {body}"
    );
}

#[tokio::test]
async fn 하트비트는_잘못된_형식의_본문을_거부한다() {
    let resp = auth_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/clients/heartbeat")
                .header(header::AUTHORIZATION, format!("Bearer {T}"))
                .body(Body::from("not-json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_to_value(resp.into_body()).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invalid heartbeat")
    );
}
