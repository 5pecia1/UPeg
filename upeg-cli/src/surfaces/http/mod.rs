//! HTTP surface (PRD §6.8, §5.6, §5.7).
//!
//! Axum router shared by every cross-process consumer:
//!   - REST shape (`/v1/...`) for scripts / browsers / SDK consumers
//!   - JSON-RPC shape (`/mcp`) for MCP-over-HTTP attachers (incl. the
//!     `upeg mcp` proxy fallback in §5.7)
//!   - `/healthz` for liveness probes (unauthenticated by design — used
//!     by `discovery::read_reachable`)
//!
//! Authentication: every route except `/healthz` requires
//! `Authorization: Bearer <token>`. Tokens are auto-generated per run
//! and published to `~/.upeg/server.json` (`discovery::publish`) so
//! same-OS-user clients auto-attach without UX. Override with
//! `UPEG_HTTP_TOKEN` or `--token-file <path>` (PRD §5.4).
//!
//! Origin/Host guard: every non-`/healthz` request must come from
//! loopback (`127.0.0.1`, `::1`, `localhost`), a `chrome-extension://`
//! popup, or an operator-configured `--cors-origin` web origin (Task
//! B1) — protects browsers from DNS-rebinding attacks. Non-loopback
//! binds require explicit `UPEG_HTTP_ALLOW_NON_LOOPBACK=1` consent plus
//! an injected bearer token (never a generated one), and skip the Origin
//! check (operator opted in deliberately) — see [`bind_policy`]. The
//! `cors` submodule's `OriginPolicy` is the single source of truth this
//! guard and the `tower_http` CORS layer both read.
//!
//! Lifecycle: [`serve_with_options`] is the canonical entrypoint;
//! [`serve`] preserves the iter-pre-tray signature by wrapping it with
//! defaults. Ephemeral bind (`127.0.0.1:0`) is the default — explicit
//! `--addr` only matters for non-loopback / fixed-port deployments.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use upeg_core::Surface;
use upeg_runtime::{
    ExecutionContext, ToolMetaRuntimeExt, tags_for_surface, toolbox_tool_in_toolkit,
    toolkits_for_surface, tools_for_toolkit_on_surface, tools_with_tag_on_surface,
};

use crate::adapters::credentials;
use crate::app;
use crate::domain::execution::dispatch::{Outcome, dispatch_failure};
use crate::infrastructure::{auth, discovery, mcp_imports, notify};
/// Default loopback bind. Used when no `--addr` flag is passed; ephemeral
/// port discovered via `local_addr()` and written to `server.json`.
pub const DEFAULT_BIND: &str = "127.0.0.1:0";

pub(crate) const MAX_HTTP_REQUEST_BODY_BYTES: usize = 1_000_000;

/// All options for starting an HTTP server. Default = ephemeral
/// loopback bind + auto-generated bearer + discovery publish.
///
/// Callers (the `upeg http` subcommand, the Flutter desktop tray via
/// upeg-frb) construct one of these and hand it to [`serve_with_options`]
/// — keeping the "who can host" decision out of this module.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub addr: String,
    pub token: auth::ResolvedToken,
    pub publish_discovery: bool,
    /// Consent to a non-loopback bind (the in-process twin of the
    /// `UPEG_HTTP_ALLOW_NON_LOOPBACK` env var). Consent alone is not
    /// enough — the token must also be injected ([`bind_policy`]) — and
    /// it changes nothing for a loopback bind: the Origin/Host guard is
    /// skipped only when the listener actually binds off loopback.
    pub allow_non_loopback: bool,
    /// PRD §5.9 — fire OS notifications on tool dispatch outcomes.
    /// False by default so `upeg http --daemon` stays silent; desktop
    /// tray hosts pass `true`. The fire-vs-skip branch lives inside
    /// [`crate::notify::fire_if`], which is the only public path to
    /// `notify_rust`; the dispatch handler simply hands its bool over.
    pub notifications_enabled: bool,
    /// Extra web origins (`--cors-origin`, exact match, no wildcard)
    /// allowed to call the REST data plane from a browser. Loopback
    /// and `chrome-extension://` origins are always allowed regardless
    /// of this list — see [`cors::OriginPolicy`]. Validated (rejecting
    /// `*` and scheme-less values) when the server actually binds.
    pub cors_origins: Vec<String>,
    /// RC-6 disable-teardown policy (PRD §5.9): how this bring-up lane
    /// came up. Stamped into the published `server.json` (see
    /// [`discovery::ServerInfo::with_origin`]) so a later disable can
    /// decide whether to auto-stop the host.
    pub origin: discovery::HostOrigin,
}

impl ServerOptions {
    /// Construct with default token (auto-generated) and discovery
    /// publish enabled — the data-plane default. In-process desktop
    /// embed, so `origin` is always [`discovery::HostOrigin::Embedded`]
    /// — never pid-killed by the RC-6 teardown policy (it IS the
    /// desktop process).
    pub fn loopback_ephemeral() -> std::io::Result<Self> {
        Ok(Self {
            addr: DEFAULT_BIND.to_string(),
            token: auth::resolve_token(None, None)?,
            publish_discovery: true,
            allow_non_loopback: false,
            notifications_enabled: false,
            cors_origins: Vec::new(),
            origin: discovery::HostOrigin::Embedded,
        })
    }

    /// Builder-style toggle for [`Self::notifications_enabled`].
    #[must_use]
    pub fn with_notifications(mut self, enabled: bool) -> Self {
        self.notifications_enabled = enabled;
        self
    }
}

#[derive(Clone)]
struct HttpState {
    /// The bearer tokens this host answers to, and the authority each
    /// one proves ([`auth::HostTokens`]). One field rather than two so
    /// "is this request authenticated" and "as whom" can never be
    /// answered against different data.
    tokens: Arc<auth::HostTokens>,
    /// When true, skip the Origin/Host loopback check (non-loopback
    /// deployment opted in via env var).
    skip_origin_guard: bool,
    /// Materialized from [`ServerOptions::notifications_enabled`].
    /// Handed to [`notify::fire_if`] on every dispatch outcome — that
    /// function owns the fire-vs-skip branch.
    notifications_enabled: bool,
    /// Single source of truth for "which browser Origins may reach
    /// this surface" — read by both the CORS layer and [`origin_guard`]
    /// so the two can never disagree. See [`cors`] module doc.
    origin_policy: Arc<cors::OriginPolicy>,
}

const BEARER_PREFIX: &str = "Bearer ";

/// Request header carrying an optional board scope on `/mcp` JSON-RPC
/// requests. Set by the `upeg mcp --board <b>` proxy so the host-side
/// dispatch keeps the same pin gate + preset merge the stdio server
/// applies in-process.
pub(crate) const BOARD_SCOPE_HEADER: &str = "x-upeg-board";

mod bind_policy;
mod cancel_on_drop;
mod cors;
mod interface_inventory;
mod origin_surface;
mod pairing;
pub(crate) use interface_inventory::interface_inventory_entries;
pub(crate) use origin_surface::ORIGIN_SURFACE_HEADER;
use origin_surface::{origin_surface_from_headers, principal_from_headers, principal_on_surface};
pub(crate) use pairing::pairing_status_block;

/// Build the canonical router. A [`auth::HostTokens`] holding no tokens
/// at all means "no bearer required" — used only by the legacy
/// [`router`] entry point that some tests still use.
fn router_with_state(state: HttpState) -> Router {
    // The CORS layer is the OUTERMOST `.layer()` call (added last — in
    // axum/tower, the last-added layer wraps every prior one, so it
    // sees the request first). That lets `tower_http`'s built-in
    // preflight short-circuit answer `OPTIONS` before it ever reaches
    // `require_bearer`: per the Fetch spec a CORS preflight carries no
    // credentials, so it must succeed without a bearer token, but it
    // must not leak data either — the layer only ever adds response
    // headers, it never bypasses `origin_guard`/`require_bearer` for
    // non-OPTIONS requests. See `surfaces/http/cors.rs`.
    let cors_layer = state.origin_policy.cors_layer();
    Router::new()
        .route("/healthz", get(healthz))
        .merge(rest_routes())
        .merge(mcp_routes())
        .layer(DefaultBodyLimit::max(MAX_HTTP_REQUEST_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), origin_guard))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ))
        .layer(middleware::from_fn(pause_guard))
        .layer(cors_layer)
        .with_state(state)
}

/// Iter-pre-tray-era entrypoint kept for tests and the no-auth bring-up
/// path. Production must call [`serve_with_options`].
pub fn router() -> Router {
    router_with_state(HttpState {
        tokens: Arc::new(auth::HostTokens::with_agents("", Vec::new())),
        skip_origin_guard: true,
        notifications_enabled: false,
        origin_policy: Arc::new(cors::OriginPolicy::default()),
    })
}

/// Bearer-guarded twin of [`router`], for tests outside this module that
/// need the authenticated paths (the origin-surface header is only
/// honored on an authenticated request, so an unauthenticated router
/// cannot exercise it at all).
#[cfg(test)]
pub(crate) fn router_with_token(token: &str) -> Router {
    router_with_tokens(auth::HostTokens::with_agents(token, Vec::new()))
}

/// Bearer-guarded router with an explicit token set, so a test can give
/// the host an agent token as well as the operator one.
#[cfg(test)]
pub(crate) fn router_with_tokens(tokens: auth::HostTokens) -> Router {
    router_with_state(HttpState {
        tokens: Arc::new(tokens),
        skip_origin_guard: true,
        notifications_enabled: false,
        origin_policy: Arc::new(cors::OriginPolicy::default()),
    })
}

fn rest_routes() -> Router<HttpState> {
    Router::new()
        .route("/v1/toolkits", get(toolkits_list))
        .route("/v1/toolkits/{toolkit}", get(toolkit_show))
        .route("/v1/toolkits/{toolkit}/{tool}", get(toolkit_tool_show))
        .route("/v1/tags", get(tags_list))
        .route("/v1/tags/{tag}", get(tag_show))
        .route("/v1/boards", get(boards_list))
        .route("/v1/boards/{board}", get(board_show))
        .route("/v1/boards/{board}/tools/{id}", post(board_tools_call))
        .route(
            "/v1/boards/{board}/tools/{id}/stream",
            post(stream::board_tools_call_stream),
        )
        .route("/v1/credentials", get(credentials_list))
        .route("/v1/logs", get(logs_list))
        .route("/v1/tools", get(tools_list))
        .route("/v1/tools/{id}", post(tools_call))
        // Sibling path rather than a `?stream=1` flag on the route
        // above: the two answer with different media types and different
        // status-code semantics, so they are different contracts and the
        // OpenAPI document says so.
        .route("/v1/tools/{id}/stream", post(stream::tools_call_stream))
        .route("/v1/triggers", get(triggers_list))
        .route("/v1/trigger/{id}", post(trigger_call))
        .route("/v1/clients", get(clients_list))
        .route("/v1/clients/heartbeat", post(clients_heartbeat))
        .route(
            "/v1/openapi.json",
            get(crate::surfaces::http::openapi::openapi_spec),
        )
}

#[derive(serde::Deserialize)]
struct HeartbeatBody {
    client_id: String,
    #[serde(default)]
    label: String,
}

async fn clients_heartbeat(body: Bytes) -> impl IntoResponse {
    let parsed: HeartbeatBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("invalid heartbeat body: {e}") })),
            );
        }
    };
    let label = if parsed.label.trim().is_empty() {
        "unknown".to_string()
    } else {
        parsed.label
    };
    crate::infrastructure::clients::record_heartbeat(parsed.client_id, label);
    (StatusCode::OK, Json(json!({ "ok": true })))
}

async fn clients_list() -> Json<Value> {
    Json(json!({ "clients": crate::infrastructure::clients::live_clients() }))
}

fn mcp_routes() -> Router<HttpState> {
    rpc::routes()
}

// === Middleware ===

/// Every non-`/healthz` route requires a bearer this host recognizes —
/// the operator token, or one of the configured agent tokens.
///
/// Both kinds pass this gate: authentication and authority are separate
/// questions, and the second one is answered per route by
/// [`origin_surface::principal_from_headers`]. An agent token buys entry
/// to the data plane and nothing else — it cannot claim an origin
/// surface, and it cannot approve a gated Chain step.
async fn require_bearer(State(state): State<HttpState>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/healthz" || !state.tokens.requires_bearer() {
        return next.run(request).await;
    }
    if !state
        .tokens
        .accepts(bearer_token(request.headers()).unwrap_or_default())
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized" })),
        )
            .into_response();
    }
    next.run(request).await
}

/// PRD §5.9 Pause: while the process pause flag is set, respond with
/// `401 Paused` to every non-`/healthz` request. Discovery stays
/// published so other surfaces still see the host endpoint and can
/// flag the "paused" state to the user. The flag is process-local
/// (see [`crate::infrastructure::pause`]) — no filesystem state.
async fn pause_guard(request: Request, next: Next) -> Response {
    if pause_decision(request.uri().path(), crate::is_paused()) {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "paused" }))).into_response();
    }
    next.run(request).await
}

/// Pure decision helper: returns `true` when the middleware should
/// reject the request with `401 Paused`. `/healthz` is always
/// allowed so liveness probes still work while paused.
fn pause_decision(uri_path: &str, paused: bool) -> bool {
    uri_path != "/healthz" && paused
}

async fn origin_guard(State(state): State<HttpState>, request: Request, next: Next) -> Response {
    if state.skip_origin_guard || request.uri().path() == "/healthz" {
        return next.run(request).await;
    }
    if !origin_and_host_allowed(request.headers(), &state.origin_policy) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "origin not allowed (loopback only)" })),
        )
            .into_response();
    }
    next.run(request).await
}

/// The bearer credential a request presents, or `None` when it presents
/// none. Extraction only — comparing it against this host's tokens is
/// [`auth::HostTokens`]'s job, which does it in constant time.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix(BEARER_PREFIX)
}

/// The authority this request proves, or `None` when it authenticates as
/// nobody (no bearer, an unrecognized one, or a tokenless host that can
/// vouch for no one).
fn bearer_role(state: &HttpState, headers: &HeaderMap) -> Option<upeg_core::PrincipalRole> {
    state.tokens.role_for(bearer_token(headers)?)
}

/// Allow if Origin header is absent (typical for `curl`/scripts) OR if
/// `policy` allows it (chrome-extension, loopback, or the configured
/// web-origin allowlist — see `surfaces/http/cors.rs`). Same
/// loopback rule for Host. Browsers always send Origin on
/// cross-origin requests, so a foreign-Origin request is rejected even
/// if it shares loopback. PRD §5.4.
///
/// This is the enforcement half of the CORS story: the `tower_http`
/// CORS layer (`cors::OriginPolicy::cors_layer`) only controls what a
/// *browser* will expose to page JS. A non-browser HTTP client ignores
/// missing CORS headers entirely, so this guard — not the CORS layer —
/// is what actually keeps a foreign Origin from reaching the data
/// plane. Both consult the same [`cors::OriginPolicy`] so they can
/// never disagree about which origins are in scope.
fn origin_and_host_allowed(headers: &HeaderMap, policy: &cors::OriginPolicy) -> bool {
    if let Some(raw) = headers.get("origin")
        && let Ok(value) = raw.to_str()
        && !policy.allows(value)
    {
        return false;
    }
    if let Some(raw) = headers.get("host")
        && let Ok(value) = raw.to_str()
        && !cors::is_loopback_authority(value)
    {
        return false;
    }
    true
}

// === Routes ===

/// Product name reported by [`healthz`]. Constant (not a magic
/// string) because it's compared against in tests and quoted in the
/// pairing-hint doc.
const PRODUCT_NAME: &str = "upeg";

/// Unauthenticated liveness probe AND pairing hint (Task B1): a
/// browser surface (PWA, chrome-ext popup) can `fetch('/healthz')`
/// before the user has pasted a token to learn "an upeg host is here",
/// without exposing the token or any tool data. Kept unauthenticated
/// and Origin/Host-guard-exempt (see [`origin_guard`]) so this
/// detection works even pre-pairing.
///
/// There is no service-desired field: a running host always serves
/// `/v1/*`. Starting the host process IS the explicit activation
/// (FR-16); loopback-first binding is unchanged.
///
/// It also carries `importsPending` (+ a counts block): the
/// desktop-embedded host registers MCP imports on a background thread
/// AFTER it starts answering, so for a few seconds `/v1/tools` and
/// `/mcp`'s `tools/list` are short of the imported tools. This is the
/// channel for clients that poll; one that holds a `GET /mcp` stream is
/// *told* when the window closes instead
/// (docs/architecture/mcp.md). Counts only, never upstream names: the
/// route is unauthenticated.
async fn healthz() -> Json<Value> {
    Json(healthz_body(mcp_imports::import_phase()))
}

/// Pure body builder for [`healthz`] — the phase comes in as an
/// argument so the shape is testable without a running host.
fn healthz_body(phase: mcp_imports::McpImportPhase) -> Value {
    let mut body = json!({
        "name": PRODUCT_NAME,
        "version": env!("CARGO_PKG_VERSION"),
    });
    if let Some(object) = body.as_object_mut() {
        object.append(&mut mcp_imports::healthz_fields(phase));
    }
    body
}

async fn tools_list() -> Json<Value> {
    Json(app::tools_list_json_for_surface(Surface::Http))
}

async fn toolkits_list() -> Json<Value> {
    Json(json!({
        "toolkits": toolkits_for_surface(Surface::Http)
            .into_iter()
            .map(toolkit_json)
            .collect::<Vec<_>>()
    }))
}

async fn toolkit_show(Path(toolkit): Path<String>) -> impl IntoResponse {
    let tools = tools_for_toolkit_on_surface(&toolkit, Surface::Http);
    if tools.is_empty() {
        return not_found_response("toolkit", &toolkit);
    }
    (StatusCode::OK, Json(toolkit_json(&toolkit)))
}

async fn toolkit_tool_show(Path((toolkit, tool)): Path<(String, String)>) -> impl IntoResponse {
    match toolbox_tool_in_toolkit(&toolkit, &tool).filter(|t| t.is_on_surface(Surface::Http)) {
        Some(t) => (StatusCode::OK, Json(t.to_json_object("name"))),
        None => not_found_response("tool", &format!("{toolkit}/{tool}")),
    }
}

async fn tags_list() -> Json<Value> {
    Json(json!({
        "tags": tags_for_surface(Surface::Http)
            .into_iter()
            .map(|tag| tag_json(&tag))
            .collect::<Vec<_>>()
    }))
}

async fn tag_show(Path(tag): Path<String>) -> impl IntoResponse {
    let tools = tools_with_tag_on_surface(&tag, Surface::Http);
    if tools.is_empty() {
        return not_found_response("tag", &tag);
    }
    (StatusCode::OK, Json(tag_json(&tag)))
}

use boards::{board_show, board_tools_call, boards_list};

async fn tools_call(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let surface = origin_surface_from_headers(&state, &headers);
    dispatch_http_tool_with_context(
        state.notifications_enabled,
        id,
        body,
        ExecutionContext::global(surface).with_principal(principal_from_headers(&state, &headers)),
        None,
    )
}

/// `POST /v1/trigger/{id}`
///
/// The surface is the request's own, exactly as on [`tools_call`]: a
/// webhook is still delivered over this listener, but an attached local
/// client firing its own trigger is the caller its header declares. It
/// is resolved once and used for the context *and* the principal, so
/// `_upeg.surface` and `_upeg.principal.surface` cannot name two
/// different callers.
async fn trigger_call(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if !has_webhook_trigger(&id) {
        return (
            StatusCode::NOT_FOUND,
            Json(
                dispatch_failure(
                    "unknown_trigger",
                    format!("unknown webhook trigger `{}`", crate::display_id(&id)),
                )
                .to_canonical_json(),
            ),
        );
    }
    let surface = origin_surface_from_headers(&state, &headers);
    dispatch_http_tool_with_context(
        state.notifications_enabled,
        id.clone(),
        body,
        ExecutionContext::global(surface).with_principal(principal_from_headers(&state, &headers)),
        upeg_runtime::webhook_trigger_label(&id),
    )
}

async fn triggers_list() -> Json<Value> {
    Json(json!({
        "triggers": upeg_runtime::registered_trigger_bindings()
            .into_iter()
            .map(|trigger| json!({
                "toolId": trigger.tool_id,
                "source": trigger.source,
                "condition": trigger.condition,
                "runtimeSupported": upeg_runtime::trigger_source_has_builtin_runtime(&trigger.source),
                "diagnostic": upeg_runtime::trigger_source_diagnostic(&trigger.source),
            }))
            .collect::<Vec<_>>()
    }))
}

fn has_webhook_trigger(id: &str) -> bool {
    upeg_runtime::trigger_bindings_for(id)
        .iter()
        .any(|trigger| trigger.source.trim() == "webhook")
}

async fn credentials_list() -> impl IntoResponse {
    credentials_list_response(app::list_credentials())
}

pub(crate) fn credentials_list_response(
    credentials: std::io::Result<Vec<credentials::CredentialRecord>>,
) -> (StatusCode, Json<Value>) {
    let credentials = match credentials {
        Ok(credentials) => credentials,
        Err(err) => return storage_error_response("read credential registry", err),
    };
    (
        StatusCode::OK,
        Json(json!({
        "credentials": credentials
            .iter()
            .map(|record| json!({
                "name": record.name,
                "type": record.value_type,
                "store": record.store,
                "env": record.env,
                "keychainService": record.keychain_service,
                "keychainAccount": record.keychain_account,
                "target": record.target,
            "status": credentials::credential_status(record),
            }))
            .collect::<Vec<_>>()
        })),
    )
}

async fn logs_list() -> impl IntoResponse {
    logs_list_response(crate::adapters::execution_log::read_records(
        &crate::adapters::execution_log::LogFilter {
            limit: Some(100),
            ..crate::adapters::execution_log::LogFilter::default()
        },
    ))
}

pub(crate) fn logs_list_response(
    records: std::io::Result<Vec<crate::adapters::execution_log::ExecutionLogRecord>>,
) -> (StatusCode, Json<Value>) {
    let records = match records {
        Ok(records) => records,
        Err(err) => return storage_error_response("read execution log", err),
    };
    (StatusCode::OK, Json(json!({ "events": records })))
}

fn storage_error_response(context: &str, err: std::io::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": format!("{context}: {err}") })),
    )
}

/// Build the standard `404 NOT_FOUND` body used by every `*_show` and
/// `*_call` handler when a path parameter doesn't resolve to anything
/// the HTTP surface exposes. Centralizes the `"unknown {kind} \`{id}\`"`
/// error-message shape so it stays consistent across resources.
fn not_found_response(kind: &str, id: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": format!("unknown {kind} `{id}`") })),
    )
}

fn toolkit_json(toolkit: &str) -> Value {
    let tools = tools_for_toolkit_on_surface(toolkit, Surface::Http);
    let mut tags: Vec<String> = tools
        .iter()
        .flat_map(|t| t.tag_labels().into_iter())
        .collect();
    tags.sort();
    tags.dedup();
    json!({
        "id": toolkit,
        "tags": tags,
        "toolCount": tools.len(),
        "tools": tools.iter().map(|t| t.to_json_object("name")).collect::<Vec<_>>(),
    })
}

fn tag_json(tag: &str) -> Value {
    json!({
        "tag": tag,
        "tools": tools_with_tag_on_surface(tag, Surface::Http)
            .iter()
            .map(|t| t.to_json_object("name"))
            .collect::<Vec<_>>(),
    })
}

/// Decode a tool-call request body into dispatch arguments.
///
/// Shared by the buffered routes and the streaming ones so the two
/// cannot disagree about what an acceptable body is: an empty body and
/// an explicit `null` both mean "no arguments" (zero-arg tools), an
/// object is the arguments, and anything else is a `400` naming which of
/// the two mistakes was made.
pub(super) fn parse_tool_call_body(body: &Bytes) -> Result<Value, (StatusCode, Json<Value>)> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(value) if value.is_null() || value.is_object() => Ok(value),
        Ok(_) => Err((
            StatusCode::BAD_REQUEST,
            Json(dispatch_failure(
                "invalid_request",
                "request body must be a JSON object or null (zero-arg tools may also post an empty body)",
            )
            .to_canonical_json()),
        )),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(
                dispatch_failure("invalid_json", format!("invalid JSON body: {e}"))
                    .to_canonical_json(),
            ),
        )),
    }
}

fn dispatch_http_tool_with_context(
    notifications_enabled: bool,
    id: String,
    body: Bytes,
    context: ExecutionContext,
    trigger: Option<String>,
) -> (StatusCode, Json<Value>) {
    let surface = context.surface();
    let args = match parse_tool_call_body(&body) {
        Ok(args) => args,
        Err(response) => return response,
    };
    match app::dispatch_tool_call(&id, args, &context, trigger.as_deref()) {
        Outcome::Success(success) => {
            let text = crate::domain::execution::dispatch::success_primary_text(&success);
            notify::fire_if(notifications_enabled, &id, true, &text);
            (StatusCode::OK, Json(success.to_canonical_json()))
        }
        Outcome::Failure(failure) => {
            let msg = failure.error.message.clone();
            notify::fire_if(notifications_enabled, &id, false, &msg);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(failure.to_canonical_json()),
            )
        }
        Outcome::NotFound => {
            // The hint enumerates what the *caller* can see, so it reads
            // the context's surface rather than the transport's — an
            // attached `cli` must not be told about a tool list it does
            // not have, nor kept from one it does.
            let hint = crate::unknown_tool_hint(&id, Some(surface));
            (
                StatusCode::NOT_FOUND,
                Json(
                    dispatch_failure(
                        "unknown_tool",
                        format!("unknown tool `{}`{hint}", crate::display_id(&id)),
                    )
                    .to_canonical_json(),
                ),
            )
        }
    }
}

// === Bind & serve ===

/// Canonical entrypoint. Honors every field of [`ServerOptions`]:
/// bind, bearer token, discovery publish, non-loopback consent.
pub fn serve_with_options(opts: ServerOptions) -> std::io::Result<()> {
    serve_inner(opts, |_| {})
}

/// Same as [`serve_with_options`] but also accepts a notification
/// channel that fires once the listener has bound and the discovery
/// file is published. Used by the Flutter desktop tray (via upeg-frb)
/// to spawn the server on a worker thread and continue UI initialization
/// once the endpoint is known. The channel sends the bound endpoint
/// string.
pub fn serve_with_ready(
    opts: ServerOptions,
    ready: std::sync::mpsc::Sender<std::io::Result<String>>,
) -> std::io::Result<()> {
    serve_inner(opts, move |result| {
        let _ = ready.send(result.map(str::to_string));
    })
}

/// Common server bootstrap. Both public entrypoints differ only in how
/// they react to the listener becoming ready (or failing to bind) — a
/// FnOnce callback receives the eventual outcome so each wrapper can
/// dispatch the signal in its own way (no-op for the CLI, channel
/// `Sender::send` for the tray-attached desktop UI).
fn serve_inner<R>(opts: ServerOptions, on_ready: R) -> std::io::Result<()>
where
    R: FnOnce(std::io::Result<&str>) + Send + 'static,
{
    // Policy is decided from the address that will actually be bound,
    // not from the consent flag: consent without a non-loopback bind
    // must neither skip the origin guard nor suppress discovery.
    let consent = opts.allow_non_loopback || bind_policy::consent_from_env();
    let non_loopback = match bind_policy::evaluate(&opts.addr, consent, opts.token.source) {
        Ok(scope) => scope == bind_policy::BindScope::NonLoopback,
        Err(refusal) => {
            let e = refusal.into_error(&opts.addr);
            on_ready(Err(std::io::Error::new(e.kind(), e.to_string())));
            return Err(e);
        }
    };
    let origin_policy = match cors::OriginPolicy::new(opts.cors_origins.clone()) {
        Ok(policy) => policy,
        Err(e) => {
            let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string());
            on_ready(Err(std::io::Error::new(err.kind(), err.to_string())));
            return Err(err);
        }
    };

    let state = HttpState {
        tokens: Arc::new(auth::HostTokens::new(opts.token.token.as_str())),
        skip_origin_guard: non_loopback,
        notifications_enabled: opts.notifications_enabled,
        origin_policy: Arc::new(origin_policy),
    };
    let publish = effective_publish(&opts, non_loopback);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::bind(&opts.addr).await {
            Ok(l) => l,
            Err(e) => {
                on_ready(Err(std::io::Error::new(e.kind(), e.to_string())));
                return Err(e);
            }
        };
        let bound: SocketAddr = listener.local_addr()?;
        let endpoint = format!("http://{bound}");

        // Discovery: best-effort UNLESS a live owner already holds the
        // `create_new` mutex while this lane intended to be discoverable
        // — that combination means a second, undiscoverable host would
        // otherwise start serving (RC-7), so `publish_failure_action`
        // aborts instead. Every other failure only affects auto-attach,
        // not the listener — log and continue.
        let _discovery_guard = if publish {
            let info = discovery::ServerInfo::with_origin(
                &endpoint,
                opts.token.token.clone(),
                opts.origin,
            );
            match discovery::publish(&info) {
                Ok(guard) => Some(guard),
                Err(e) => match publish_failure_action(publish, e.kind()) {
                    PublishFailure::Abort => {
                        let abort_err = std::io::Error::new(
                            e.kind(),
                            format!(
                                "upeg http: another host already owns discovery; \
                                 refusing to serve as a headless duplicate ({e})"
                            ),
                        );
                        on_ready(Err(std::io::Error::new(
                            abort_err.kind(),
                            abort_err.to_string(),
                        )));
                        return Err(abort_err);
                    }
                    PublishFailure::Tolerate => {
                        eprintln!("upeg http: discovery publish skipped ({e})");
                        None
                    }
                },
            }
        } else {
            None
        };

        let _status_guard = upeg_runtime::mark_http_active();
        let _ = startup_log(&mut std::io::stderr().lock(), &opts, &endpoint, publish);
        on_ready(Ok(&endpoint));

        axum::serve(listener, router_with_state(state))
            .with_graceful_shutdown(shutdown_signal())
            .await
    })
}

/// Effective discovery-publish decision: intent (`opts.publish_discovery`)
/// gated by the non-loopback rule (`publish_discovery_allowed`). The
/// single source of truth used by both serve paths and the startup
/// logger so the message can never disagree with what actually happened.
fn effective_publish(opts: &ServerOptions, non_loopback: bool) -> bool {
    opts.publish_discovery && publish_discovery_allowed(non_loopback)
}

/// What a `discovery::publish` failure should do to this serve lane.
/// `Abort` only when this lane actually intended to be the discoverable
/// host AND a live owner already holds the `create_new` mutex
/// (`AlreadyExists`) — serving on in that case would leave a second,
/// undiscoverable ("headless") host process running (RC-7). Every
/// other failure (permission errors, or a lane that never intended to
/// publish) is `Tolerate`: log and keep serving, matching the
/// pre-existing best-effort discovery contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishFailure {
    Abort,
    Tolerate,
}

const fn publish_failure_action(
    publish_requested: bool,
    kind: std::io::ErrorKind,
) -> PublishFailure {
    match (publish_requested, kind) {
        (true, std::io::ErrorKind::AlreadyExists) => PublishFailure::Abort,
        _ => PublishFailure::Tolerate,
    }
}

/// Presentation-only: report what the listener decided. Branches off the
/// effective `publish` (not `opts.publish_discovery`) so the message
/// never disagrees with what happened. The token itself is never written
/// here: under `--daemon` this stream is the log file, and a log is not a
/// credential store. A non-loopback bind always runs on an injected token
/// ([`bind_policy`]), so there is no generated token to hand back either.
fn startup_log<W: std::io::Write>(
    out: &mut W,
    opts: &ServerOptions,
    endpoint: &str,
    publish: bool,
) -> std::io::Result<()> {
    writeln!(out, "upeg http listening on {endpoint}")?;
    if opts.publish_discovery && !publish {
        writeln!(
            out,
            "upeg http: discovery publish suppressed \
             (non-loopback bind without UPEG_PUBLISH_DISCOVERY=1)"
        )?;
    }
    if matches!(opts.token.source, auth::TokenSource::Generated) {
        if publish {
            writeln!(
                out,
                "upeg http: bearer token published to server.json (auto-discovery)"
            )?;
        } else {
            writeln!(
                out,
                "upeg http: generated bearer token was not published; \
                 no client can pair with this run"
            )?;
        }
    }
    Ok(())
}

/// PRD §5.5: SIGTERM/SIGINT/Ctrl-Break grace shutdown. Returns when
/// either signal fires; `axum::serve.with_graceful_shutdown` then
/// drains in-flight requests before exiting. RAII Drop of the
/// discovery guard cleans up `server.json`.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let Ok(mut sigterm) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Discovery publish is auto-disabled on non-loopback binds unless
/// explicitly requested. Server operators rarely want a discovery
/// file written into someone's home; respect the consent boundary.
fn publish_discovery_allowed(non_loopback: bool) -> bool {
    if !non_loopback {
        return true;
    }
    std::env::var(crate::infrastructure::paths::env::PUBLISH_DISCOVERY)
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

mod boards;
pub(crate) mod openapi;
mod rpc;
mod stream;

#[cfg(test)]
mod identity_parity_tests;
#[cfg(test)]
mod middleware_tests;
#[cfg(test)]
mod serve_guard_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod tests;
