//! MCP server surface for the shared Toolbox.
//!
//! Speaks JSON-RPC 2.0 over stdio (one request per line). Methods:
//!   - `initialize` → handshake
//!   - `tools/list` → enumerate every `#[upeg::tool]` from the inventory
//!   - `tools/call` → dispatch to the named Tool's implementation
//!
//! `handle` is a pure function (`Value` in, `Option<Value>` out) so the
//! protocol can be unit-tested without spinning up stdin/stdout. `serve`
//! is the I/O wrapper used by the `upeg mcp` subcommand.

use serde_json::{Value, json};
use upeg_core::{
    BoardKey, Principal, Surface,
    interface_inventory::{
        Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
        InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
    },
};
use upeg_runtime::ExecutionContext;

use crate::app;
use crate::domain::execution::dispatch::Outcome;

mod board_guidance;
mod logging;
mod proxy;

pub(crate) use logging::{
    LogLevelGate, McpLogWriter, McpNotificationSink, dropped_notifications_frame,
};

pub(crate) const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "upeg";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const MCP_SURFACE_VERSION: &str = "v1";
const MCP_OWNER_PATH: &str = "upeg-cli/Cargo.toml";
const MCP_DOCS_PATH: &str = "README.md";
const MCP_TEST_PATH: &str = "upeg-cli/src/inventory/tests.rs";
/// The test inside [`MCP_TEST_PATH`] that actually asserts these
/// entries — pinned by the inventory honesty check.
const MCP_TEST_NAME: &str = "interface_inventory_covers_cli_http_and_mcp";

/// JSON-RPC 2.0 protocol version string every frame carries.
const JSON_RPC_VERSION: &str = "2.0";

/// JSON-RPC methods this surface answers, plus the one it emits. Named
/// constants so the dispatch match, the interface-inventory
/// declarations, and the proxy router (`proxy.rs`) can never drift into
/// naming different strings for the same method.
const METHOD_INITIALIZE: &str = "initialize";
pub(crate) const METHOD_TOOLS_LIST: &str = "tools/list";
pub(crate) const METHOD_TOOLS_CALL: &str = "tools/call";
const METHOD_NOTIFICATIONS_INITIALIZED: &str = "notifications/initialized";
const METHOD_NOTIFICATIONS_CANCELLED: &str = "notifications/cancelled";
/// Server→client notification: "re-read `tools/list`". Emitted once the
/// deferred MCP-import load finishes (see [`spawn_deferred_import_load`]).
const METHOD_TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
/// Server→client and client→server logging frames live in
/// [`logging`] — the module that also owns the severity type and the
/// per-session floor.
use logging::{METHOD_LOGGING_SET_LEVEL, set_level_result};

/// JSON-RPC / MCP frame fields read by more than one module.
pub(crate) const FIELD_ID: &str = "id";
pub(crate) const FIELD_METHOD: &str = "method";
pub(crate) const FIELD_PARAMS: &str = "params";
pub(crate) const FIELD_RESULT: &str = "result";
pub(crate) const FIELD_TOOLS: &str = "tools";
pub(crate) const PARAMS_NAME: &str = "name";
pub(crate) const PARAMS_ARGUMENTS: &str = "arguments";

struct McpMethodDeclaration {
    id: &'static str,
    jsonrpc_method: &'static str,
}

struct McpToolExposureDeclaration {
    id: &'static str,
    jsonrpc_method: &'static str,
}

const MCP_METHODS: &[McpMethodDeclaration] = &[McpMethodDeclaration {
    id: "mcp.tools.list",
    jsonrpc_method: METHOD_TOOLS_LIST,
}];

const MCP_TOOL_EXPOSURES: &[McpToolExposureDeclaration] = &[
    McpToolExposureDeclaration {
        id: "mcp.tools.call",
        jsonrpc_method: METHOD_TOOLS_CALL,
    },
    McpToolExposureDeclaration {
        id: "mcp.board.context",
        jsonrpc_method: METHOD_TOOLS_CALL,
    },
];

pub(crate) fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    MCP_METHODS
        .iter()
        .map(McpMethodDeclaration::interface_entry)
        .chain(
            MCP_TOOL_EXPOSURES
                .iter()
                .map(McpToolExposureDeclaration::interface_entry),
        )
        .collect()
}

impl McpMethodDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        mcp_entry(
            self.id,
            InterfaceKind::McpMethod,
            self.jsonrpc_method,
            "src/surfaces/mcp/mod.rs",
        )
    }
}

impl McpToolExposureDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        mcp_entry(
            self.id,
            InterfaceKind::McpTool,
            self.jsonrpc_method,
            "src/surfaces/mcp/mod.rs",
        )
    }
}

fn mcp_entry(
    id: &str,
    kind: InterfaceKind,
    jsonrpc_method: &str,
    source_path: &str,
) -> InterfaceEntry {
    InterfaceEntry {
        id: id.to_string(),
        surfaces: SurfaceSet::single(Surface::Mcp),
        kind,
        contract: ContractShape::new(
            ContractLocator::jsonrpc(jsonrpc_method),
            ContractIo::declared(
                ContractIoKind::JsonRpcParams,
                None,
                None,
                "JSON-RPC params contract",
            ),
            ContractIo::not_declared(
                "MCP responses follow JSON-RPC/MCP protocol; tools/call exposes canonical ToolResult as structuredContent",
            ),
        ),
        version: MCP_SURFACE_VERSION.to_string(),
        compatibility: Compatibility::Stable,
        owner: OwnerRef {
            path: Some(MCP_OWNER_PATH.to_string()),
            url: None,
        },
        docs: DocRef {
            path: Some(MCP_DOCS_PATH.to_string()),
            url: None,
        },
        source: SourceRef {
            path: Some(format!("upeg-cli/{source_path}")),
            url: None,
        },
        tests: TestMapping::covered(MCP_TEST_PATH, Some(MCP_TEST_NAME.to_string())),
    }
}

/// Pure MCP dispatch. Keeps the public stdio/Unix-socket contract on the
/// MCP surface while daemon HTTP can opt into another surface via
/// [`handle_for_surface`].
pub fn handle(request: Value) -> Option<Value> {
    handle_for_surface(request, Surface::Mcp)
}

/// Surface-parametrized JSON-RPC dispatch without a board scope.
pub fn handle_for_surface(request: Value, surface: Surface) -> Option<Value> {
    handle_with_board(request, surface, None)
}

/// Surface- and board-parametrized JSON-RPC dispatch. Keeps the daemon
/// at the shared Toolbox boundary while letting transports reuse the
/// MCP-shaped protocol without hard-coding the MCP surface.
///
/// With a board scope ("board = server", `upeg mcp --board <b>`):
///   - `tools/list` exposes only the tools the user pinned on that
///     board (PegboardState.layouts), still filtered to this surface.
///   - `tools/call` refuses unpinned tools with the same
///     `-32601 Method not found` shape an unknown id produces, and
///     merges the pin's saved args preset as argument defaults.
///
/// Returns `None` for valid JSON-RPC notifications (no `id`)
/// where the spec requires no response. Returns `Some(Value)` otherwise —
/// including the `-32600 Invalid Request` error response when a request
/// (has `id`) is missing its `method` field. Returning `None` here
/// would leave the client hung waiting.
pub fn handle_with_board(
    request: Value,
    surface: Surface,
    board: Option<&BoardKey>,
) -> Option<Value> {
    handle_in_lane(request, Principal::for_surface(surface), board, None, None)
}

/// Is this frame the handshake?
///
/// Read by two lanes that both have to act on it outside the dispatch:
/// the stdio loop opens its deferred-import gate strictly after
/// answering it, and the HTTP lane mints a session id for it.
pub(crate) fn is_initialize_request(request: &Value) -> bool {
    request.get(FIELD_METHOD).and_then(Value::as_str) == Some(METHOD_INITIALIZE)
}

/// Does this frame expect a response?
///
/// JSON-RPC's own rule — a request carries an `id`, a notification does
/// not — hoisted out of [`handle_in_lane`] because a transport has to
/// answer the same question *before* dispatching: the HTTP SSE lane will
/// not open a stream for a frame that has no response to carry.
pub(crate) fn expects_response(request: &Value) -> bool {
    request.get(FIELD_ID).is_some()
}

/// [`handle_with_board`] plus the lane's log stream, when it has one.
///
/// `log` is what makes `logging` a real capability rather than a claim:
/// it is `Some` exactly on the lanes that can write a server-originated
/// frame — the stdio in-process server and the HTTP `/mcp` transport,
/// which can open a `text/event-stream` body. So it decides two things
/// at once — whether `initialize` advertises `capabilities.logging`, and
/// whether `logging/setLevel` is a method that exists here.
fn handle_in_lane(
    request: Value,
    caller: Principal,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
    board_state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Option<Value> {
    let surface = caller.surface;
    let id = request.get(FIELD_ID).cloned();
    let is_notification = !expects_response(&request);

    // missing/wrong-type `method` is an Invalid Request per
    // JSON-RPC 2.0. Notifications without `method` are still dropped
    // (the spec also classifies them as malformed but no response is
    // owed for any notification).
    let Some(method) = request.get(FIELD_METHOD).and_then(Value::as_str) else {
        if is_notification {
            return None;
        }
        return Some(json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": invalid_request("missing or non-string `method`"),
        }));
    };

    let result = match method {
        METHOD_INITIALIZE => board_guidance::initialize(
            initialize_result(LoggingCapability::for_lane(log)),
            surface,
            board,
            board_state,
        ),
        METHOD_TOOLS_LIST => board_guidance::validate_board(surface, board, board_state)
            .map(|()| tools_list_result_in(surface, board, board_state)),
        METHOD_TOOLS_CALL => match request.get(FIELD_PARAMS) {
            Some(params) => tools_call_result(params, caller, board, board_state),
            None => Err(invalid_params("missing `params`")),
        },
        // Exists exactly where the capability was advertised. A lane
        // that cannot emit a log frame answers the same `Method not
        // found` it answers for anything else it never claimed.
        METHOD_LOGGING_SET_LEVEL => match log {
            Some(log) => set_level_result(request.get(FIELD_PARAMS), log),
            None => Err(method_not_found(method)),
        },
        // Some MCP clients send these — we have nothing to broadcast.
        METHOD_NOTIFICATIONS_INITIALIZED | METHOD_NOTIFICATIONS_CANCELLED => return None,
        _ => Err(method_not_found(method)),
    };

    if is_notification {
        return None;
    }

    Some(match result {
        Ok(value) => json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "result": value }),
        Err(err) => json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "error": err }),
    })
}

/// Whether this lane may declare `capabilities.logging`.
///
/// Declaring it is a promise the lane can keep: `tools/call` streams the
/// running tool's output as `notifications/message` frames, and a client
/// that sees no logging capability is entitled to treat such a frame as
/// a protocol violation. The inverse is just as binding — a lane with
/// nowhere to put an unsolicited frame (the stdio proxy lane, where the
/// host runs the tool) must not advertise a capability it can never
/// exercise, or a client will sit waiting for log frames that cannot
/// arrive.
///
/// The unit is the lane, not the request. On HTTP a single plain-JSON
/// `POST` opened no stream, but the transport it arrived on can open
/// one — so it still holds a writer (a discarding one) and still
/// advertises. See `surfaces/http/rpc.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoggingCapability {
    Advertised,
    Absent,
}

impl LoggingCapability {
    /// A lane advertises `logging` exactly when it holds a log stream.
    const fn for_lane(log: Option<&McpLogWriter>) -> Self {
        match log {
            Some(_) => Self::Advertised,
            None => Self::Absent,
        }
    }
}

fn initialize_result(logging: LoggingCapability) -> Value {
    let mut capabilities = json!({ "tools": {} });
    if logging == LoggingCapability::Advertised {
        capabilities[LOGGING_CAPABILITY] = json!({});
    }
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": capabilities,
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
    })
}

/// `capabilities` key the `logging` capability is declared under.
const LOGGING_CAPABILITY: &str = "logging";

pub(super) fn tools_list_result(surface: Surface, board: Option<&BoardKey>) -> Value {
    tools_list_result_in(surface, board, None)
}

fn tools_list_result_in(
    surface: Surface,
    board: Option<&BoardKey>,
    board_state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Value {
    match board {
        None => app::tools_list_json_for_surface(surface),
        Some(board) => board_state.map_or_else(
            || board_guidance::tools_list(surface, board),
            |state| board_guidance::tools_list_in(surface, board, state),
        ),
    }
}

/// One `tools/call`, dispatched as `caller`.
///
/// The whole [`Principal`] rather than its surface alone: the surface
/// decides visibility and the pin gate, and the role decides whether a
/// Chain's approval barrier can be lifted. A lane that authenticates its
/// callers (`surfaces/http/rpc.rs`) knows the second answer and nothing
/// downstream can re-derive it, so it travels in the execution context
/// the dispatch is given.
fn tools_call_result(
    params: &Value,
    caller: Principal,
    board: Option<&BoardKey>,
    board_state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Result<Value, Value> {
    let surface = caller.surface;
    // Align with the `"missing or non-string"` disambiguation used
    // for the top-level method field. A `params.name = 42` request
    // must not return just "missing `params.name`" — that would
    // mislead the client into looking for the field rather than
    // checking its type. The combined message accurately covers
    // both failure modes (None vs. wrong-type).
    let name = params
        .get(PARAMS_NAME)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("missing or non-string `params.name`"))?;
    if name != name.trim() {
        return Err(invalid_params(
            "`params.name` must be canonical and unpadded",
        ));
    }
    // Strict-validate the arguments shape. Per MCP spec,
    // `params.arguments` is an object (or absent/null for zero-arg
    // tools). Accepting any JSON value (e.g., `arguments: "hello"`)
    // would make `args.get("input")` return None, the tool would run
    // with empty args, and the client would see a confusing "empty
    // input" tool error instead of a clear protocol error pointing at
    // the actual mistake. Same strict-validate-at-the-protocol-
    // boundary lens as the `method` and `params.name` checks.
    let args = match params.get(PARAMS_ARGUMENTS) {
        None => Value::Null,
        Some(v) if v.is_null() || v.is_object() => v.clone(),
        Some(_) => {
            return Err(invalid_params(
                "`params.arguments` must be an object or null/absent (zero-arg tools)",
            ));
        }
    };
    if name == crate::board_agent::BOARD_CONTEXT_TOOL && board.is_some() && surface == Surface::Mcp
    {
        return board_guidance::context_result(&args, surface, board, board_state);
    }
    let args = if board.is_some() && surface == Surface::Mcp {
        crate::domain::execution::context::with_caller_cwd(args)
    } else {
        args
    };
    let context = match board {
        None => ExecutionContext::global(surface),
        Some(board) => board_guidance::call_context(name, surface, board, board_state)?,
    }
    .with_principal(caller);
    board_guidance::validate_call_args(name, &args, &context)?;

    let (text, is_error, structured_content) =
        match app::dispatch_tool_call(name, args, &context, None) {
            Outcome::Success(success) => {
                let text = crate::domain::execution::dispatch::success_primary_text(&success);
                (text, false, Some(success.to_canonical_json()))
            }
            Outcome::Failure(failure) => {
                // Agents read `content[0].text`, so the diagnostics the
                // command actually wrote have to be in it — the
                // `structuredContent` envelope carries the same streams
                // under `error.details` for clients that parse it.
                let text = crate::domain::execution::dispatch::failure_text(&failure);
                let structured_content = failure.to_canonical_json();
                (text, true, Some(structured_content))
            }
            Outcome::NotFound => return Err(method_not_found_with_hint(name, surface)),
        };

    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
    });
    if let Some(structured_content) = structured_content {
        result["structuredContent"] = structured_content;
    }
    if is_error {
        result["isError"] = Value::Bool(true);
    }
    Ok(result)
}

fn method_not_found(method: &str) -> Value {
    // truncate method name in display.
    json!({ "code": -32601, "message": format!("Method not found: {}", crate::display_id(method)) })
}

/// Same as `method_not_found` but appends "did you mean ..." hints
/// scoped to tools registered on the active surface. Used on both
/// the tools/call `NotFound` path *and* the surface-gate refusal
/// path; both branches must produce indistinguishable responses
/// (otherwise MCP and HTTP could leak which tools exist on other
/// surfaces). The hint's surface filter prevents the hint itself
/// from revealing a tool hidden from this transport.
fn method_not_found_with_hint(method: &str, surface: Surface) -> Value {
    let hint = crate::unknown_tool_hint(method, Some(surface));
    json!({
        "code": -32601,
        "message": format!("Method not found: {}{hint}", crate::display_id(method)),
    })
}

fn invalid_params(msg: &str) -> Value {
    json!({ "code": -32602, "message": format!("Invalid params: {msg}") })
}

/// JSON-RPC 2.0 -32600 Invalid Request: the JSON sent is not a valid
/// Request object (missing/wrong `method`, etc)..
fn invalid_request(msg: &str) -> Value {
    json!({ "code": -32600, "message": format!("Invalid Request: {msg}") })
}

/// Shared `Parse error: <detail>` response shape used when JSON-RPC
/// framing fails to deserialize. Centralising the shape removes the
/// drift surface between `mcp::serve` (stdio path) and
/// `daemon::handle_connection` (Unix socket path); any tweak to the
/// parse-error shape (e.g., adding a `code.kind` field) ripples to
/// both transports from one place.
///
/// Per JSON-RPC 2.0: parse errors use `id: null` (the framer can't
/// know the request id when the JSON was malformed).
///
/// Visibility is `pub(crate)` — only daemon and `serve_loop` call
/// it, both inside this crate.
pub(crate) fn parse_error_response(detail: impl std::fmt::Display) -> Value {
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": null,
        "error": { "code": -32700, "message": format!("Parse error: {detail}") },
    })
}

/// Extract the concatenated text from an MCP `tools/call` result
/// body. Centralised so `client.rs:translate_response` (CLI → daemon)
/// and `mcp_import.rs::UpstreamServer::call` (proxy to upstream MCP server)
/// cannot drift; both callers route through this helper.
///
/// Contract:
///   - Iterates every entry in `result.content` (an array, per MCP spec).
///   - Collects only entries whose `type == "text"` — image / resource
///     parts are skipped (upeg's dispatch contract is text-only).
///   - Joins multiple text parts with `\n` so chunked output reads
///     naturally on a single-line tool result body.
///   - Missing/non-array `content`, or zero text parts, → empty string
///     (matches the "empty content surfaces as `Ok("")`" contract).
///
/// tightened to `pub(crate)` — only `client.rs` and
/// `mcp_import.rs` consume it, both within this crate.
#[cfg(test)]
pub(crate) fn extract_text_content(result: &Value) -> String {
    let mut text = String::new();
    if let Some(arr) = result.get("content").and_then(Value::as_array) {
        for part in arr {
            if part.get("type").and_then(Value::as_str) == Some("text")
                && let Some(t) = part.get("text").and_then(Value::as_str)
            {
                // Skip empty-text parts entirely so that
                // `[{text:"hello"}, {text:""}]` joins to `"hello"`
                // rather than `"hello\n"` (a dangling trailing newline
                // from entering the join branch on an empty part).
                if t.is_empty() {
                    continue;
                }
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
    }
    text
}

/// Shared per-line cap across every JSON-RPC framing surface —
/// daemon socket reads, MCP stdio reads, upstream MCP server stdout,
/// CLI daemon-client reads. 1 MB is generous; real `tools/list` and
/// `tools/call` requests/responses are typically <100 KB. Centralised
/// here so a future change ripples across all 4 sites.
///
/// Visibility is `pub(crate)` — internal framing detail.
pub(crate) const MAX_LINE_BYTES: usize = 1_000_000;

/// Outcome of a capped line read. Centralises the take + check +
/// early-exit shape so each caller pattern-matches and returns its
/// own error variant for `CapHit`.
///
/// Derived `Debug + PartialEq + Eq` so unit tests can `assert_eq!`
/// rather than `matches!`, getting a real diff on failure. Cheap
/// derives — no runtime cost on the production path.
///
/// tightened to `pub(crate)` — internal framing detail.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LineReadOutcome {
    /// Reader returned 0 bytes — end of stream.
    Eof,
    /// Read a complete line of `bytes` bytes (terminator included
    /// in the buffer if present).
    Line { bytes: usize },
    /// Hit `MAX_LINE_BYTES` without seeing a newline. The buffer
    /// holds the partial bytes; the underlying reader is now
    /// positioned mid-request — callers should close/discard the
    /// connection rather than try to recover.
    CapHit,
}

/// Bounded `read_line` shared by the framing call sites.
/// The `take(MAX)` wrapper prevents buffer
/// growth on a malicious/buggy peer that sends unbounded bytes
/// without `\n`; the `CapHit` discriminant tells the caller to
/// surface a clean error and (typically) close the connection.
pub(crate) fn read_line_capped<R: std::io::BufRead>(
    reader: &mut R,
    line: &mut String,
) -> std::io::Result<LineReadOutcome> {
    use std::io::{BufRead, Read};
    let n = (&mut *reader).take(MAX_LINE_BYTES as u64).read_line(line)?;
    Ok(match (n, line.ends_with('\n')) {
        (0, _) => LineReadOutcome::Eof,
        (n, false) if n >= MAX_LINE_BYTES => LineReadOutcome::CapHit,
        (n, _) => LineReadOutcome::Line { bytes: n },
    })
}

/// Run the MCP server on stdin/stdout. Board-scoped sessions dispatch
/// in-process to retain their launch directory and loaded configuration.
/// Unscoped sessions proxy through a reachable host's HTTP `/mcp` endpoint,
/// or dispatch in-process when there is no host. Neither lane spawns a host.
/// In-process sessions own their MCP-import registration through
/// [`spawn_deferred_import_load`], off the request path.
///
/// `board` scopes the whole session ("board = server", `--board`).
/// Returns when stdin reaches EOF.
pub fn serve(board: Option<BoardKey>) {
    let stdin = std::io::stdin();
    // Deliberately the `Stdout` handle, NOT `stdout().lock()`: the
    // deferred import loader writes its `tools/list_changed`
    // notification from another thread, and a lock held for the whole
    // session would deadlock it. `Stdout`'s `Write` impl takes the lock
    // per call (`write_fmt` for a whole `writeln!`), so frames from the
    // two threads still never interleave mid-line.
    let mut out = std::io::stdout();
    let mut reader = stdin.lock();
    let _status_guard = upeg_runtime::mark_mcp_active();

    if let Some(host) = crate::infrastructure::attach::current_host().filter(|_| board.is_none()) {
        spawn_heartbeat_thread(host.clone(), "mcp");
        proxy::proxy_loop(&mut reader, &mut out, &host, board.as_ref());
    } else {
        // In-process lane only: this stdio server is the long-lived
        // process that owns its Toolbox, so it registers MCP imports
        // itself. The proxy lane above skips it — the host already
        // imported them (docs/architecture/mcp.md).
        let gate = std::sync::Arc::new(InitializeGate::default());
        // Dropped on the spot, which detaches the thread. This lane must
        // never join it: a stdio session ends at stdin EOF, which can
        // arrive while the loader is still inside its bounded
        // spawn+handshake attempts against an unreachable upstream, and
        // joining there would hold `upeg mcp` open for that whole window.
        // The loader owns nothing that outlives the process.
        drop(spawn_deferred_import_load(
            crate::infrastructure::mcp_imports::reexport_policy_from_env(),
            std::sync::Arc::clone(&gate),
            std::io::stdout(),
        ));
        // Same destination as `out`, held separately because the
        // progress frames are written from the invoker's reader threads.
        let log = McpLogWriter::new(std::sync::Arc::new(
            std::sync::Mutex::new(std::io::stdout()),
        ));
        serve_loop_gated(
            &mut reader,
            &mut out,
            board.as_ref(),
            Some(&gate),
            Some(&log),
        );
    }
}

/// One-shot gate the deferred import loader waits on.
///
/// An MCP client is entitled to see nothing but its own `initialize`
/// response until the handshake completes; a server-originated
/// notification arriving first is at best ignored and at worst treated
/// as a protocol violation. The stdio loop opens this gate the moment it
/// has written the `initialize` response, and the loader thread parks
/// here until then.
#[derive(Default)]
struct InitializeGate {
    answered: std::sync::Mutex<bool>,
    opened: std::sync::Condvar,
}

impl InitializeGate {
    /// Open the gate. Idempotent — a client that (wrongly) sends
    /// `initialize` twice just re-opens an already-open gate.
    fn open(&self) {
        if let Ok(mut answered) = self.answered.lock() {
            *answered = true;
        }
        self.opened.notify_all();
    }

    /// Block until [`Self::open`] runs. A poisoned mutex returns
    /// immediately: the notification is best-effort, and blocking a
    /// worker thread forever on a poisoned lock helps nobody.
    fn wait_until_open(&self) {
        let Ok(mut answered) = self.answered.lock() else {
            return;
        };
        while !*answered {
            match self.opened.wait(answered) {
                Ok(next) => answered = next,
                Err(_) => return,
            }
        }
    }
}

/// The in-process lane's MCP-import registration, deferred off the
/// request path.
///
/// Two decisions live here:
///   - `McpReexport::Blocked` (the default for every declaration) means
///     imported tools register on `ALL_SURFACES_EXCEPT_MCP`, so this
///     surface could not list one of them even after paying for the
///     load. Nothing is spawned at all.
///   - Otherwise the load runs on a worker thread, and once it finishes
///     — and once `initialize` has been answered — the client is told to
///     re-read `tools/list`.
///
/// Returns the loader's join handle (`None` when nothing was scheduled)
/// so tests can wait for it; `serve` drops it.
fn spawn_deferred_import_load<W: std::io::Write + Send + 'static>(
    policy: upeg_sources::mcp_import::McpReexport,
    gate: std::sync::Arc<InitializeGate>,
    mut out: W,
) -> Option<std::thread::JoinHandle<()>> {
    if policy == upeg_sources::mcp_import::McpReexport::Blocked {
        return None;
    }
    crate::infrastructure::mcp_imports::spawn_load_for_host(move || {
        gate.wait_until_open();
        write_tools_list_changed(&mut out);
    })
}

/// "Re-read `tools/list`" as a JSON-RPC notification.
///
/// Shared rather than built at each site: the stdio lane writes it as a
/// line and the HTTP lane wraps it in an SSE event, and the two must
/// never drift into naming different methods for the same fact.
pub(crate) fn tools_list_changed_frame() -> Value {
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        FIELD_METHOD: METHOD_TOOLS_LIST_CHANGED,
    })
}

/// Tell the client its tool list changed. Best-effort, like every other
/// write on this surface: a client that already hung up must not crash
/// the loader thread.
fn write_tools_list_changed<W: std::io::Write>(out: &mut W) {
    let notification = tools_list_changed_frame();
    let _ = writeln!(out, "{notification}");
    let _ = out.flush();
}

/// Long-running attach surfaces announce themselves in the
/// host's "Connected clients" panel via a periodic heartbeat. 25 s
/// interval beats the 30 s TTL with margin; once the surface exits
/// the entry ages out naturally.
fn spawn_heartbeat_thread(host: crate::infrastructure::discovery::ServerInfo, label: &'static str) {
    let pid = std::process::id();
    let start_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let client_id = format!("{label}-{pid}-{start_ms}");
    let _ = std::thread::Builder::new()
        .name(format!("upeg-heartbeat-{label}"))
        .spawn(move || {
            loop {
                let _ = crate::infrastructure::attach::send_heartbeat(&host, &client_id, label);
                std::thread::sleep(std::time::Duration::from_secs(25));
            }
        });
}

/// The tool a request is asking to call, when it is a well-formed
/// `tools/call`. `None` for every other method — nothing else has a
/// running tool to report progress for.
fn tools_call_target(request: &Value) -> Option<String> {
    if request.get(FIELD_METHOD).and_then(Value::as_str)? != METHOD_TOOLS_CALL {
        return None;
    }
    request
        .get(FIELD_PARAMS)?
        .get(PARAMS_NAME)?
        .as_str()
        .map(str::to_string)
}

/// Answer one request, streaming live tool output as log notifications
/// when this lane has somewhere to write them.
///
/// The sink is installed only around `tools/call`: it is the one method
/// that runs a tool, and installing it more broadly would put an unused
/// capability on every `tools/list`. `log` is handed on either way — it
/// is also what tells [`handle_in_lane`] that `logging` exists here.
fn handle_with_progress(
    request: Value,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
) -> Option<Value> {
    handle_with_progress_in(request, board, log, None)
}

fn handle_with_progress_in(
    request: Value,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
    board_state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Option<Value> {
    handle_with_log_in(
        request,
        Principal::for_surface(Surface::Mcp),
        board,
        log,
        board_state,
    )
}

/// [`handle_with_board`] plus the lane's server→client stream: while a
/// `tools/call` runs, its output travels into `log` as
/// `notifications/message` frames.
///
/// Principal-parametrized because the HTTP `/mcp` route reaches it too:
/// that lane dispatches as `mcp` like the stdio one, but it
/// authenticates its callers, so an agent token there is an
/// [`upeg_core::PrincipalRole::Agent`] rather than the in-process
/// [`upeg_core::PrincipalRole::Local`] the surface alone implies. The
/// progress sink is installed on the calling thread — the ambient sink is
/// per-thread — so a transport that dispatches on a worker must call this
/// *from* that worker.
pub(crate) fn handle_with_log(
    request: Value,
    caller: Principal,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
) -> Option<Value> {
    handle_with_log_in(request, caller, board, log, None)
}

fn handle_with_log_in(
    request: Value,
    caller: Principal,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
    board_state: Option<&upeg_sources::pegboard::PegboardState>,
) -> Option<Value> {
    match (log, tools_call_target(&request)) {
        (Some(log), Some(tool_id)) => {
            upeg_runtime::with_progress_sink(log.progress_sink(&tool_id), || {
                handle_in_lane(request, caller, board, Some(log), board_state)
            })
        }
        _ => handle_in_lane(request, caller, board, log, board_state),
    }
}

pub(super) fn serve_loop_once<W: std::io::Write>(
    line: String,
    out: &mut W,
    board: Option<&BoardKey>,
) {
    serve_loop_once_logged(line, out, board, None);
}

/// [`serve_loop_once`] plus the log stream `tools/call` progress frames
/// are written to.
pub(super) fn serve_loop_once_logged<W: std::io::Write>(
    line: String,
    out: &mut W,
    board: Option<&BoardKey>,
    log: Option<&McpLogWriter>,
) {
    let request: Value = match serde_json::from_str(&line) {
        Ok(v) => v,
        Err(e) => {
            let err = parse_error_response(e);
            let _ = writeln!(out, "{err}");
            let _ = out.flush();
            return;
        }
    };
    if let Some(response) = handle_with_progress(request, board, log) {
        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }
}

/// Extracted from `serve()` for testability. Takes generic
/// reader+writer so tests can inject pathological inputs without
/// mocking stdin, plus an optional session-wide board scope. `serve()`
/// is the thin wrapper that wires `stdin.lock()` to the process
/// `Stdout` handle.
pub fn serve_loop_with_board<R: std::io::BufRead, W: std::io::Write>(
    reader: &mut R,
    out: &mut W,
    board: Option<&BoardKey>,
) {
    serve_loop_gated(reader, out, board, None, None);
}

/// [`serve_loop_with_board`] plus the handshake gate the deferred
/// MCP-import loader waits on. Only `serve`'s in-process lane passes a
/// gate; every other caller (the proxy lane's in-process fallback,
/// tests) passes `None` because nothing is waiting on it.
fn serve_loop_gated<R: std::io::BufRead, W: std::io::Write>(
    reader: &mut R,
    out: &mut W,
    board: Option<&BoardKey>,
    gate: Option<&InitializeGate>,
    log: Option<&McpLogWriter>,
) {
    let mut line = String::new();
    let board_session = board_guidance::BoardSession::new(board);

    loop {
        line.clear();
        // shared `read_line_capped` helper.
        let n = match read_line_capped(reader, &mut line) {
            Ok(LineReadOutcome::Eof) => break,
            Ok(LineReadOutcome::Line { bytes }) => bytes,
            Ok(LineReadOutcome::CapHit) => {
                let err =
                    parse_error_response(format!("request line exceeds {MAX_LINE_BYTES} bytes"));
                let _ = writeln!(out, "{err}");
                let _ = out.flush();
                // Reader state is poisoned (mid-request bytes still pending);
                // exit cleanly so the parent process sees EOF.
                break;
            }
            Err(_) => break, // io error (broken stdin) → exit cleanly
        };
        let _ = n; // shadow retained for clarity

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                // shared helper — see `parse_error_response` for the
                // drift history that motivated centralisation.
                let err = parse_error_response(e);
                let _ = writeln!(out, "{err}");
                let _ = out.flush();
                continue;
            }
        };

        let is_initialize = is_initialize_request(&request);
        if let Some(response) = board_session.handle(request, board, log) {
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        }
        // Strictly after the response is on the wire: the deferred
        // loader's `tools/list_changed` must never overtake the
        // handshake it belongs to.
        if is_initialize && let Some(gate) = gate {
            gate.open();
        }
    }
}

#[cfg(test)]
mod board_guidance_tests;
#[cfg(test)]
mod board_scope_tests;
#[cfg(test)]
mod broken_pipe_tests;
#[cfg(test)]
mod deferred_import_tests;
#[cfg(test)]
mod external_failure_tests;
#[cfg(test)]
mod file_wire_test;
#[cfg(test)]
mod log_notification_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod tests;
