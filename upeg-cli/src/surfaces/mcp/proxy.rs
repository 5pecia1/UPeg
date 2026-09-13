//! `upeg mcp` proxy lane: stdio JSON-RPC ↔ a running host's HTTP `/mcp`.
//!
//! The proxy is not a dumb pipe. It applies the same two Project
//! Manifest rules the CLI and TUI apply on their own attach paths
//! (docs/architecture/project-manifest.md):
//!
//!   - **D-1** — a tool whose provenance is `project-manifest:*` exists
//!     only in *this* process's Toolbox (the host resolved its own
//!     `upeg.toml`, or none). Forwarding its `tools/call` would earn an
//!     `unknown tool` from the host, so it is dispatched in-process and
//!     merged into `tools/list` on the way back.
//!   - **D-2** — every other `tools/call` carries the caller's absolute
//!     working directory in `_upeg.cwd`, so an `External` tool runs
//!     where the agent is, not where the host happens to live.
//!
//! [`proxy_line`] takes the host call as a closure so the routing,
//! rewriting and merging can be tested against a scripted host without
//! binding a socket or publishing a `server.json`.

use serde_json::Value;
use upeg_core::{BoardKey, Surface};

use super::{
    FIELD_METHOD, FIELD_PARAMS, FIELD_RESULT, FIELD_TOOLS, LineReadOutcome, MAX_LINE_BYTES,
    METHOD_TOOLS_CALL, METHOD_TOOLS_LIST, PARAMS_ARGUMENTS, PARAMS_NAME, handle_with_board,
    parse_error_response, read_line_capped, serve_loop_once, serve_loop_with_board,
    tools_list_result,
};
use crate::domain::execution::context;

/// What the proxy does with one inbound JSON-RPC frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyRoute {
    /// Forward verbatim — handshake frames, notifications, unknown
    /// methods, and anything that failed to parse (the host owns the
    /// protocol error).
    Forward,
    /// `tools/call` the host owns: forward with `_upeg.cwd` stamped
    /// into `params.arguments` (D-2).
    ForwardWithCallerCwd,
    /// `tools/call` for a Project Manifest tool (D-1): the host has
    /// never heard of it, so dispatch in-process.
    LocalDispatch,
    /// `tools/list`: forward, then merge this process's Project
    /// Manifest tools into the host's list (D-1's listing half).
    ForwardAndMergeProjectTools,
}

/// Whether the host is still usable for the rest of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineOutcome {
    Proxied,
    HostGone,
}

/// PRD §5.7 proxy: stdio JSON-RPC ↔ host's HTTP `/mcp`. Any HTTP
/// failure falls through to in-process dispatch for the offending
/// request and every request after it, so a flaky host doesn't break
/// the stdio session.
pub(super) fn proxy_loop<R: std::io::BufRead, W: std::io::Write>(
    reader: &mut R,
    out: &mut W,
    host: &crate::infrastructure::discovery::ServerInfo,
    board: Option<&BoardKey>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match read_line_capped(reader, &mut line) {
            Ok(LineReadOutcome::Eof) => break,
            Ok(LineReadOutcome::Line { .. }) => {}
            Ok(LineReadOutcome::CapHit) => {
                let err =
                    parse_error_response(format!("request line exceeds {MAX_LINE_BYTES} bytes"));
                let _ = writeln!(out, "{err}");
                let _ = out.flush();
                break;
            }
            Err(_) => break,
        }

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        let outcome = proxy_line(&trimmed, out, board, |body| {
            crate::infrastructure::attach::post_mcp_with_board(host, body, board)
        });
        if outcome == LineOutcome::HostGone {
            // Host went away mid-session. Handle the offending request
            // in-process and stay in-process for the rest of the
            // session — one process, no reconnect storm, and the
            // operator sees consistent behaviour.
            serve_loop_once(trimmed, out, board);
            serve_loop_with_board(reader, out, board);
            return;
        }
    }
}

/// Route, rewrite, forward and merge one already-trimmed JSON-RPC line.
///
/// `forward` is the host call (`POST /mcp`); it is a parameter so tests
/// can script the host's replies.
fn proxy_line<W, F>(trimmed: &str, out: &mut W, board: Option<&BoardKey>, forward: F) -> LineOutcome
where
    W: std::io::Write,
    F: FnOnce(&str) -> std::io::Result<String>,
{
    // Parse only far enough to route. An unparseable line is forwarded
    // verbatim so the host still owns the parse-error response.
    let request: Option<Value> = serde_json::from_str(trimmed).ok();
    let route = request.as_ref().map_or(ProxyRoute::Forward, proxy_route);

    if route == ProxyRoute::LocalDispatch {
        if let Some(request) = request
            && let Some(response) = handle_with_board(request, Surface::Mcp, board)
        {
            write_frame(out, &response.to_string());
        }
        return LineOutcome::Proxied;
    }

    let body = match (route, request) {
        (ProxyRoute::ForwardWithCallerCwd, Some(request)) => {
            stamp_caller_cwd_on_call(request).to_string()
        }
        _ => trimmed.to_string(),
    };

    match forward(&body) {
        // 204 No Content — notification, no body owed.
        Ok(response) if response.is_empty() => LineOutcome::Proxied,
        Ok(response) => {
            let response = response.trim_end_matches(['\r', '\n']);
            write_frame(out, &merged_or_verbatim(route, response, board));
            LineOutcome::Proxied
        }
        Err(_) => LineOutcome::HostGone,
    }
}

fn write_frame<W: std::io::Write>(out: &mut W, frame: &str) {
    let _ = writeln!(out, "{frame}");
    let _ = out.flush();
}

/// Pure routing decision for one parsed JSON-RPC frame.
fn proxy_route(request: &Value) -> ProxyRoute {
    match request.get(FIELD_METHOD).and_then(Value::as_str) {
        Some(METHOD_TOOLS_LIST) => ProxyRoute::ForwardAndMergeProjectTools,
        Some(METHOD_TOOLS_CALL) => call_route(request),
        _ => ProxyRoute::Forward,
    }
}

/// D-1 for a single `tools/call`: provenance is the authority, not the
/// id shape (`upeg-runtime::provenance`).
fn call_route(request: &Value) -> ProxyRoute {
    let local = request
        .get(FIELD_PARAMS)
        .and_then(|params| params.get(PARAMS_NAME))
        .and_then(Value::as_str)
        .is_some_and(context::requires_local_dispatch);
    if local {
        ProxyRoute::LocalDispatch
    } else {
        ProxyRoute::ForwardWithCallerCwd
    }
}

/// D-2: stamp `_upeg.cwd` into `params.arguments` before the frame goes
/// to the host. A `params.arguments` of any shape other than
/// object/null/absent is left alone — that is a protocol error the host
/// must report as such, and rewriting it would change what the client
/// sees.
fn stamp_caller_cwd_on_call(mut request: Value) -> Value {
    let Some(params) = request.get_mut(FIELD_PARAMS).and_then(Value::as_object_mut) else {
        return request;
    };
    let args = match params.get(PARAMS_ARGUMENTS) {
        None => Some(Value::Null),
        Some(value) if value.is_null() || value.is_object() => Some(value.clone()),
        Some(_) => None,
    };
    if let Some(args) = args {
        params.insert(PARAMS_ARGUMENTS.to_string(), context::with_caller_cwd(args));
    }
    request
}

/// Merge this process's Project Manifest tools into a proxied
/// `tools/list` response; every other route hands the host's frame
/// through untouched. An unparseable host frame is also handed through
/// — a merge is never worth swallowing whatever the host actually said.
fn merged_or_verbatim(route: ProxyRoute, response: &str, board: Option<&BoardKey>) -> String {
    if route != ProxyRoute::ForwardAndMergeProjectTools {
        return response.to_string();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(response) else {
        return response.to_string();
    };
    merge_tool_entries(parsed, project_manifest_tool_entries(board)).to_string()
}

/// This process's Project Manifest tools as `tools/list` entries,
/// through the same enumeration the in-process lane uses — so a
/// board-scoped proxy session merges only what that board pins.
fn project_manifest_tool_entries(board: Option<&BoardKey>) -> Vec<Value> {
    tools_list_result(Surface::Mcp, board)
        .get(FIELD_TOOLS)
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter(|tool| tool_name(tool).is_some_and(context::requires_local_dispatch))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Append `extra` entries to a `tools/list` response, deduped by name
/// (the host's own entry wins) and re-sorted so the merged list keeps
/// the by-id order every other `tools/list` produces.
fn merge_tool_entries(mut response: Value, extra: Vec<Value>) -> Value {
    if extra.is_empty() {
        return response;
    }
    let Some(tools) = response
        .get_mut(FIELD_RESULT)
        .and_then(|result| result.get_mut(FIELD_TOOLS))
        .and_then(Value::as_array_mut)
    else {
        return response;
    };
    let listed: std::collections::BTreeSet<String> = tools
        .iter()
        .filter_map(tool_name)
        .map(str::to_string)
        .collect();
    for entry in extra {
        if tool_name(&entry).is_some_and(|name| !listed.contains(name)) {
            tools.push(entry);
        }
    }
    tools.sort_by(|left, right| tool_name(left).cmp(&tool_name(right)));
    response
}

fn tool_name(tool: &Value) -> Option<&str> {
    tool.get(PARAMS_NAME).and_then(Value::as_str)
}

#[cfg(test)]
mod tests;
