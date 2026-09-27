//! Sync HTTP client for surfaces that attach to a running host.
//!
//! PRD §5.2 (L4 client modes) / §5.7 (MCP proxy). The workspace forbids
//! `unsafe` and we don't want a new dependency just for in-process
//! attachment, so this module builds minimal HTTP/1.0 requests on top
//! of `std::net::TcpStream`. Only the request shapes used by
//! `discovery::read_reachable` callers are supported (POST JSON,
//! Connection: close).
//!
//! Used by:
//!   - `upeg mcp` proxy mode (§5.7) — forwards stdin JSON-RPC lines
//!     to `/mcp` and echoes the body back to stdout.
//!   - Future: `upeg call` auto-attach, `upeg tui` attach dispatch.
//!
//! # Host topology
//!
//! Surfaces share **at most one** HTTP host per config root, on loopback
//! (the stdio MCP lane with agents is separate; Unix sockets and
//! `upeg daemon` are retired). `server.json` is the single source for
//! discovery, auth, and lifecycle; precedence resolves conflicts
//! deterministically so the user never picks; and explicit-start is
//! preserved — no surface implicitly spawns another. Two binaries cover
//! everything: `upeg` (headless-capable) + the desktop GUI.
//!
//! | Tier | Surface | Host eligibility | Lifecycle |
//! |---|---|---|---|
//! | L1 | `upeg host start --daemon` | yes — persistent, strong intent | until explicit stop |
//! | L2 | Desktop GUI + Tray | only when `Tweaks.local_http_host` is on (default OFF — no listener without an explicit decision) | user session |
//! | L3 | `upeg host start` (foreground) | yes | occupies the shell |
//! | L4 | `upeg call` / `upeg mcp` / `upeg tui` | no — client or in-process | per call / session |
//!
//! Startup algorithm (identical on every surface): no host → an L1/L3
//! surface becomes it, Desktop runs an in-process L2 host only when the
//! tweak is on (otherwise it comes up with no host), and any L4 runs
//! in-process without auto-spawning. Host present → shareable calls
//! attach while project-manifest Tools self-execute (`upeg_sources::
//! project`); `upeg mcp` proxies stdio↔`/mcp` without `--board`,
//! self-executes with it; `upeg host start` against a Desktop-held host
//! errors with "stop it first". When a host exits, clients do **not**
//! auto-take-over — they degrade explicitly or fall back to in-process.
//!
//! # Discovery file
//!
//! `~/.upeg/server.json` (Unix, mode `0600`) /
//! `%APPDATA%\upeg\server.json` (Windows, current-user ACL):
//! `{ endpoint, mcp_endpoint, token, pid, started_at_ms, origin }`.
//! `token` is a 32-byte URL-safe random value; `origin` is `explicit`
//! (a user-run `upeg host start`) or `embedded` (Desktop's in-process
//! host) — a file missing the key reads conservatively as `explicit`.
//!
//! Discovery checks the recorded PID before probing `/healthz`; a dead
//! PID cannot be validated by an unrelated listener on the same port.
//! Unknown process state prevents auto-attach and retains the record.
//! Every authenticated request from a discovered host rechecks the file
//! and PID before writing the bearer token. PID reuse and the gap between
//! this check and the write remain possible; `started_at_ms` is only an
//! application timestamp, not an OS process birth identity. A host
//! publishes the file at startup and removes it via RAII on clean exit.
//!
//! Multi-user: each OS user gets their own `~/.upeg/` and ephemeral
//! ports — a shared instance is out of scope. Per-platform tray: macOS
//! is menubar-only (`LSUIElement`, Dock visibility is a settings
//! toggle), Windows and SNI-capable Linux use the notification area,
//! and Linux without SNI has no tray (desktop window only).

use std::io::ErrorKind;

use serde_json::Value;
use upeg_core::{
    ChoiceSpec, FileValue, OutputEntry, OutputKind, OutputValue, Surface, ToolFailure, ToolSuccess,
    preflight_file_output_json,
};

use super::discovery::{self, DiscoveredHost, ServerInfo};
use crate::domain::execution::dispatch::Outcome;

/// Path of the host's unauthenticated liveness/pairing document.
/// Shared spelling with `discovery`'s probe and the route itself.
const HEALTHZ_PATH: &str = "/healthz";

mod http_transport;
mod stream;

use http_transport::{
    RequestBudget, request_discovered as http_request_discovered,
    request_within as http_request_within,
};
pub(crate) use stream::{
    LiveCall, StreamedDispatch, dispatch_tool_on_board_streamed, dispatch_tool_streamed,
};

/// Forward a single JSON-RPC line to the host's `/mcp` endpoint and
/// return the response body verbatim. Errors map to `io::Error` so the
/// caller can decide whether to fall through to in-process dispatch.
/// An optional board scope rides the `x-upeg-board` request header
/// (see the host's `/mcp` route) so a board-scoped `upeg mcp --board
/// <b>` proxy session keeps its pin gate and preset merge when the
/// dispatch happens host-side.
pub fn post_mcp_with_board(
    server: &DiscoveredHost,
    body: &str,
    board: Option<&upeg_core::BoardKey>,
) -> std::io::Result<String> {
    let board_header;
    let extra_headers: &[(&str, &str)] = match board {
        Some(board) => {
            board_header = [(crate::surfaces::http::BOARD_SCOPE_HEADER, board.as_str())];
            &board_header
        }
        None => &[],
    };
    let (status, response) =
        http_request_discovered("POST", &server.mcp_endpoint, server, extra_headers, body)?;
    if status == 204 {
        // JSON-RPC notification — no body owed.
        return Ok(String::new());
    }
    if !(200..300).contains(&status) {
        return Err(std::io::Error::other(format!(
            "host returned HTTP {status} for /mcp: {response}"
        )));
    }
    Ok(response)
}

/// Select a discovered host only when its process and `/healthz` probe
/// are healthy. The returned context rechecks both before authenticated
/// requests; `None` also covers an unknown process verdict.
pub fn current_host() -> Option<DiscoveredHost> {
    discovery::discover_reachable()
}

/// Read a host's `/healthz` document.
///
/// Unauthenticated on purpose: the route is the one endpoint a client
/// may read before pairing (see the host's `healthz` handler), so no
/// bearer token is sent and none is needed. `None` for every failure
/// mode — unreachable, non-2xx, or a body that is not JSON — which is
/// exactly what callers report as "unknown", never as a fabricated
/// value.
///
/// `discovery::read_reachable` also probes `/healthz`, but only for
/// the status line; this is the one path that reads the BODY, which is
/// how `upeg host status` learns the host's live MCP-import phase
/// (`infrastructure::mcp_imports`).
///
/// Runs on [`RequestBudget::HEALTH_PROBE`], not the generic dispatch
/// budget. `/healthz` is the route whose whole job is to answer at once;
/// spending the 30s budget sized for a running tool on it would let one
/// hung host stall `upeg host status` far past the 300ms the liveness
/// probe of the very same route already treats as dead.
pub fn healthz_json(server: &ServerInfo) -> Option<Value> {
    let url = format!("{}{HEALTHZ_PATH}", server.endpoint.trim_end_matches('/'));
    let (status, body) =
        http_request_within("GET", &url, None, &[], "", RequestBudget::HEALTH_PROBE).ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    serde_json::from_str(&body).ok()
}

/// Run a tool through the host's REST endpoint and translate the HTTP
/// response into the dispatch [`Outcome`] shape — same return type
/// in-process dispatchers produce, so call sites swap "in-process vs
/// attach" without API churn.
///
/// Path: `POST /v1/tools/{tool_id}` with `args` as JSON body.
///
/// `origin` is the surface this client is attaching FROM, sent as the
/// host's origin-surface header. Attach mode used to be answered as
/// plain `http`, which was true of the transport and false of the
/// person: a chain's approval gate then refused the terminal the human
/// was typing in. The host validates the header against its own allow
/// list before believing it (`surfaces/http/mod.rs`), so this is a
/// declaration, not an authorization.
pub fn dispatch_tool(
    server: &DiscoveredHost,
    tool_id: &str,
    args: &Value,
    origin: Surface,
) -> std::io::Result<Outcome> {
    let url = format!(
        "{}/v1/tools/{}",
        server.endpoint.trim_end_matches('/'),
        tool_id
    );
    dispatch_via_url(server, &url, args, origin)
}

/// Read a host's non-executing External readiness result. An attach failure
/// stays an attach failure: callers must not silently inspect another host.
pub fn tool_readiness(
    server: &DiscoveredHost,
    tool_id: &str,
    board: Option<&upeg_core::BoardKey>,
    origin: Surface,
) -> std::io::Result<Value> {
    let mut url = format!(
        "{}/v1/tools/{tool_id}/readiness",
        server.endpoint.trim_end_matches('/')
    );
    if let Some(board) = board {
        url.push_str("?board=");
        url.push_str(board.as_str());
    }
    let cwd = std::env::current_dir()
        .map_err(|error| std::io::Error::other(format!("readiness working directory: {error}")))?;
    let cwd = cwd.to_string_lossy();
    let (status, response) = http_request_discovered(
        "GET",
        &url,
        server,
        &[
            (crate::surfaces::http::ORIGIN_SURFACE_HEADER, origin.label()),
            (crate::surfaces::http::READINESS_CWD_HEADER, cwd.as_ref()),
        ],
        "",
    )?;
    if status != 200 {
        return Err(std::io::Error::other(format!(
            "host returned HTTP {status} for readiness: {}",
            extract_error(&response)
        )));
    }
    serde_json::from_str(&response).map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("attach: readiness JSON: {error}"),
        )
    })
}

/// Board-scoped twin of [`dispatch_tool`]: `POST
/// /v1/boards/{board}/tools/{tool_id}` so the host applies its own
/// board gate (user PegboardState) and pin-preset merge.
pub fn dispatch_tool_on_board(
    server: &DiscoveredHost,
    board: &str,
    tool_id: &str,
    args: &Value,
    origin: Surface,
) -> std::io::Result<Outcome> {
    let url = format!(
        "{}/v1/boards/{}/tools/{}",
        server.endpoint.trim_end_matches('/'),
        board,
        tool_id
    );
    dispatch_via_url(server, &url, args, origin)
}

fn dispatch_via_url(
    server: &DiscoveredHost,
    url: &str,
    args: &Value,
    origin: Surface,
) -> std::io::Result<Outcome> {
    let body = if args.is_null() {
        String::new()
    } else {
        args.to_string()
    };
    let (status, response) = http_request_discovered(
        "POST",
        url,
        server,
        &[(crate::surfaces::http::ORIGIN_SURFACE_HEADER, origin.label())],
        &body,
    )?;
    map_dispatch_response(status, &response)
}

/// Pure status→`Outcome` mapping helper, extracted for unit testing.
/// 200 → `Outcome::Success`; 404 → `Outcome::NotFound`;
/// 422 → `Outcome::Failure`; 401 → `Err` (`PermissionDenied`);
/// anything else → `Outcome::Failure` with the HTTP code attached.
pub(crate) fn map_dispatch_response(status: u16, response: &str) -> std::io::Result<Outcome> {
    match status {
        200 => {
            let v: Value = serde_json::from_str(response).map_err(|e| {
                std::io::Error::new(ErrorKind::InvalidData, format!("attach: parse 200: {e}"))
            })?;
            canonical_success_from_value(v).map(Outcome::Success)
        }
        404 => Ok(Outcome::NotFound),
        422 => Ok(Outcome::Failure(
            canonical_failure_from_response(response).unwrap_or_else(|| {
                crate::domain::execution::dispatch::dispatch_failure(
                    "tool_error",
                    extract_error(response),
                )
            }),
        )),
        401 => Err(std::io::Error::new(
            ErrorKind::PermissionDenied,
            "attach: host paused or token rejected",
        )),
        _ => Ok(Outcome::Failure(
            crate::domain::execution::dispatch::dispatch_failure(
                "host_error",
                format!("host returned HTTP {status}: {}", extract_error(response)),
            ),
        )),
    }
}

/// PRD §5.7 / §5.9. Fire a single heartbeat ping to the host's
/// `/v1/clients/heartbeat`. Used by long-running attach surfaces
/// (`upeg mcp` proxy) to surface themselves in the "Connected
/// clients" panel.
pub fn send_heartbeat(
    server: &DiscoveredHost,
    client_id: &str,
    label: &str,
) -> std::io::Result<()> {
    let url = format!(
        "{}/v1/clients/heartbeat",
        server.endpoint.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "client_id": client_id,
        "label": label,
    })
    .to_string();
    let (status, _) = http_request_discovered("POST", &url, server, &[], &body)?;
    if !(200..300).contains(&status) {
        return Err(std::io::Error::other(format!(
            "heartbeat returned HTTP {status}"
        )));
    }
    Ok(())
}

fn extract_error(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| match v.get("error") {
            Some(Value::String(message)) => Some(message.clone()),
            Some(Value::Object(error)) => error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        })
        .unwrap_or_else(|| body.to_string())
}

fn canonical_success_from_value(value: Value) -> std::io::Result<ToolSuccess> {
    if value.get("ok") != Some(&Value::Bool(true)) {
        return Err(invalid_canonical_success("missing ok=true"));
    }
    let primary_output_id = match value.get("primary_output_id") {
        Some(Value::Null) | None => None,
        Some(Value::String(id)) => Some(id.clone()),
        _ => {
            return Err(invalid_canonical_success(
                "primary_output_id must be string or null",
            ));
        }
    };
    let outputs = value
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_canonical_success("outputs must be an array"))?
        .iter()
        .map(canonical_output_entry_from_value)
        .collect::<std::io::Result<Vec<_>>>()?;

    ToolSuccess::new(primary_output_id, outputs)
        .map_err(|e| invalid_canonical_success(format!("invalid success envelope: {e}")))
}

fn canonical_output_entry_from_value(value: &Value) -> std::io::Result<OutputEntry> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_canonical_success("output entry must be an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_canonical_success("output id must be a string"))?
        .to_string();
    let label = match object.get("label") {
        Some(Value::Null) | None => None,
        Some(Value::String(label)) => Some(label.clone()),
        _ => {
            return Err(invalid_canonical_success(
                "output label must be string or null",
            ));
        }
    };
    let kind_label = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_canonical_success("output kind must be a string"))?;
    let raw_value = object
        .get("value")
        .ok_or_else(|| invalid_canonical_success("output value is required"))?;
    Ok(OutputEntry {
        id,
        label,
        kind: output_kind_from_label(kind_label)?,
        value: output_value_from_canonical(kind_label, raw_value)?,
    })
}

fn output_kind_from_label(kind: &str) -> std::io::Result<OutputKind> {
    Ok(match kind {
        "string" => OutputKind::String,
        "number" => OutputKind::Number,
        "integer" => OutputKind::Integer,
        "boolean" => OutputKind::Boolean,
        "options" => OutputKind::Options(ChoiceSpec { options: vec![] }),
        "multi_options" => OutputKind::MultiOptions(ChoiceSpec { options: vec![] }),
        "markdown" => OutputKind::Markdown,
        "json" => OutputKind::Json,
        "datetime" => OutputKind::DateTime,
        "file_path" => OutputKind::FilePath,
        "url" => OutputKind::Url,
        "file" => OutputKind::File,
        "embedded_view" => OutputKind::EmbeddedView { url: String::new() },
        _ => {
            return Err(invalid_canonical_success(format!(
                "unknown output kind `{kind}`"
            )));
        }
    })
}

fn output_value_from_canonical(kind: &str, value: &Value) -> std::io::Result<OutputValue> {
    Ok(match kind {
        "string" => OutputValue::String(required_string_value(kind, value)?),
        "number" => OutputValue::Number(
            value
                .as_number()
                .cloned()
                .ok_or_else(|| invalid_canonical_success("number output value must be a number"))?,
        ),
        "integer" => {
            OutputValue::Integer(value.as_i64().ok_or_else(|| {
                invalid_canonical_success("integer output value must be an integer")
            })?)
        }
        "boolean" => {
            OutputValue::Boolean(value.as_bool().ok_or_else(|| {
                invalid_canonical_success("boolean output value must be a boolean")
            })?)
        }
        "options" => OutputValue::Options(required_string_value(kind, value)?),
        "multi_options" => OutputValue::MultiOptions(
            value
                .as_array()
                .ok_or_else(|| {
                    invalid_canonical_success("multi_options output value must be an array")
                })?
                .iter()
                .map(|entry| {
                    entry.as_str().map(str::to_string).ok_or_else(|| {
                        invalid_canonical_success("multi_options entries must be strings")
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?,
        ),
        "markdown" => OutputValue::Markdown(required_string_value(kind, value)?),
        "json" => OutputValue::Json(value.clone()),
        "datetime" => OutputValue::DateTime(required_string_value(kind, value)?),
        "file_path" => OutputValue::FilePath(required_string_value(kind, value)?),
        "url" => OutputValue::Url(required_string_value(kind, value)?),
        "file" => {
            preflight_file_output_json(value).map_err(|error| {
                invalid_canonical_success(format!("file output failed resource preflight: {error}"))
            })?;
            OutputValue::File(serde_json::from_value::<FileValue>(value.clone()).map_err(
                |error| invalid_canonical_success(format!("file output value is invalid: {error}")),
            )?)
        }
        "embedded_view" => OutputValue::EmbeddedView(required_string_value(kind, value)?),
        _ => {
            return Err(invalid_canonical_success(format!(
                "unknown output kind `{kind}`"
            )));
        }
    })
}

fn required_string_value(kind: &str, value: &Value) -> std::io::Result<String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| invalid_canonical_success(format!("{kind} output value must be a string")))
}

fn canonical_failure_from_response(response: &str) -> Option<ToolFailure> {
    let value = serde_json::from_str::<Value>(response).ok()?;
    canonical_failure_from_value(&value)
}

fn canonical_failure_from_value(value: &Value) -> Option<ToolFailure> {
    if value.get("ok") != Some(&Value::Bool(false)) {
        return None;
    }
    let error = value.get("error")?.as_object()?;
    Some(ToolFailure {
        error: upeg_core::ToolError {
            code: error.get("code")?.as_str()?.to_string(),
            message: error.get("message")?.as_str()?.to_string(),
            details: error.get("details").cloned(),
        },
    })
}

/// One canonical envelope → the dispatch [`Outcome`] shape.
///
/// The buffered route reads the outcome off the HTTP status
/// ([`map_dispatch_response`]); a streamed one has no status to read —
/// headers went out before the tool finished — so the envelope on the
/// terminal line is the whole answer. Both end in the same type.
pub(crate) fn outcome_from_canonical_envelope(envelope: &Value) -> std::io::Result<Outcome> {
    match envelope.get("ok") {
        Some(Value::Bool(true)) => {
            canonical_success_from_value(envelope.clone()).map(Outcome::Success)
        }
        Some(Value::Bool(false)) => Ok(Outcome::Failure(
            canonical_failure_from_value(envelope).unwrap_or_else(|| {
                crate::domain::execution::dispatch::dispatch_failure(
                    "tool_error",
                    extract_error(&envelope.to_string()),
                )
            }),
        )),
        _ => Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "attach: result envelope has no `ok` flag",
        )),
    }
}

fn invalid_canonical_success(message: impl Into<String>) -> std::io::Error {
    let message = message.into();
    std::io::Error::new(ErrorKind::InvalidData, format!("attach: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT: u64 = upeg_core::MAX_FILE_OUTPUT_NODES;

    fn file_success_response_over_node_budget() -> String {
        let entries = (0..FILE_OUTPUT_NODE_BUDGET_EXCESS_CHILD_COUNT)
            .map(|index| {
                serde_json::json!({
                    "name": format!("{index}.bin"),
                    "is_dir": false,
                    "content": {
                        "kind": "bytes",
                        "bytes": "",
                    },
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "ok": true,
            "primary_output_id": "archive",
            "outputs": [{
                "id": "archive",
                "kind": "file",
                "value": {
                    "name": "bundle",
                    "is_dir": true,
                    "content": {
                        "kind": "directory",
                        "entries": entries,
                    },
                },
            }],
        })
        .to_string()
    }

    #[test]
    fn dispatch_response_200_mapping_returns_canonical_success() {
        let outcome = map_dispatch_response(
            200,
            r#"{"ok":true,"primary_output_id":"result","outputs":[{"id":"result","label":"Result","kind":"integer","value":255}]}"#,
        )
        .unwrap();
        assert_eq!(outcome.primary_text().as_deref(), Some("255"));
    }

    #[test]
    fn dispatch_response_200_mapping_errors_on_non_canonical_success() {
        let err = map_dispatch_response(200, "{}").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn dispatch_response_200_mapping_errors_on_invalid_json() {
        let err = map_dispatch_response(200, "garbage").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn attach_file_output_checks_aggregate_node_budget_before_decode() {
        let error = map_dispatch_response(200, &file_success_response_over_node_budget())
            .expect_err("attach File output must reject an aggregate node budget excess");

        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(error.to_string().contains("node count"), "{error}");
    }

    #[test]
    fn dispatch_response_404_mapping_returns_not_found() {
        let outcome = map_dispatch_response(404, r#"{"error":"no"}"#).unwrap();
        assert_eq!(outcome, Outcome::NotFound);
    }

    #[test]
    fn dispatch_response_422_mapping_returns_canonical_error() {
        let outcome = map_dispatch_response(
            422,
            r#"{"ok":false,"error":{"code":"tool_error","message":"bad hex"}}"#,
        )
        .unwrap();
        match outcome {
            Outcome::Failure(failure) => {
                assert_eq!(failure.error.code, "tool_error");
                assert_eq!(failure.error.message, "bad hex");
            }
            other => panic!("expected ToolError, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_response_401_mapping_returns_permission_error() {
        let err = map_dispatch_response(401, r#"{"error":"paused"}"#).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn dispatch_response_mapping_falls_back_to_tool_error_for_other_status_codes() {
        let outcome = map_dispatch_response(503, r#"{"error":"down"}"#).unwrap();
        match outcome {
            Outcome::Failure(failure) => {
                let msg = failure.error.message;
                assert!(msg.contains("HTTP 503"), "{msg}");
                assert!(msg.contains("down"), "{msg}");
            }
            other => panic!("expected ToolError, got {other:?}"),
        }
    }

    #[test]
    fn map_dispatch_response_falls_back_to_raw_body_without_error_field() {
        let outcome = map_dispatch_response(422, "raw text").unwrap();
        match outcome {
            Outcome::Failure(failure) => assert_eq!(failure.error.message, "raw text"),
            other => panic!("expected ToolError, got {other:?}"),
        }
    }
}
