//! MCP import. Proxies upstream MCP servers so their tools
//! appear in the upeg toolbox alongside built-ins, TOML loaders, and
//! WASM plugins.
//!
//! Lifecycle:
//!   1. A TOML config under `$UPEG_MCP_IMPORTS_DIR` (or `~/.upeg/mcp-imports/`) declares
//!      `command` + `args` for an MCP server.
//!   2. `register_dir` spawns each subprocess, sends `initialize` and
//!      `tools/list` over stdio, and registers every remote tool with id
//!      `<server_name>.<original_id>` (namespaced to avoid collisions).
//!      Import is tool-level partial success: a tool whose schema fails
//!      conversion to upeg typed I/O is skipped with a structured
//!      [`SkipReason`] while the rest of the server registers; the
//!      server as a whole fails only on protocol violations, namespace
//!      collisions, or when every tool was skipped.
//!   3. Each registered dispatcher holds an `Arc<Mutex<UpstreamServer>>` and
//!      proxies `tools/call` back over the long-lived subprocess. The
//!      caller owns the returned [`Registration`]; dropping it
//!      tears down the dispatchers (subprocess remains until the `Arc`
//!      is released by the last dispatcher closure).
//!
//! Threading: the subprocess is `!Sync` from upeg's perspective (one
//! stdin, one stdout, request/response order matters), so dispatches
//! against a single server serialize through a Mutex. Different servers
//! run independently.
//!
//! # Load contract
//!
//! Imports load only in long-lived server processes, differently per
//! lane — all funnel through `upeg-cli`'s `infrastructure::mcp_imports`:
//!
//! | Lane | When | Exposure while loading |
//! |---|---|---|
//! | `upeg host start` (fg/`--daemon`) | synchronous, before the listener opens | requests accepted only after loading finishes |
//! | desktop embedded host | `Loading` stamped *before* the embed thread spawns; load runs in the background | `/healthz`'s `importsPending` is true until done — counts only, never upstream names |
//! | in-process `upeg mcp` | conditional + background | `notifications/tools/list_changed` on completion |
//!
//! One-shot CLI commands, the TUI, and proxy-mode `upeg mcp` never load
//! imports — they reach imported tools through the attached host. There
//! is no mid-run reload: reload = host restart.
//!
//! An imported tool is **not** re-exposed on upeg's own `mcp` surface
//! unless its server's TOML opts in with `reexport = true` — the
//! default-off blocks proxy chains and self-import loops. A skipped
//! tool registers on no surface.
//!
//! # Subprocess and handshake safety
//!
//! Bounded timeouts: `initialize`/`tools/list` 5s, `tools/call` 30s,
//! shutdown 500ms; a timeout kills the child and waits on it. The first
//! spawn + handshake retries transient failures (timeout, pipe closing
//! early) with fixed attempts/backoff (`spawn_retry`); permanent
//! failures — a bad command, an explicit RPC error — return
//! immediately.
//!
//! Strict JSON-RPC peers (the official SDK) drop malformed frames
//! silently: a request with no params *omits* the `params` member
//! (`"params": null` ≠ absent), and `notifications/initialized` goes
//! out right after the `initialize` response, before any other request
//! — a best-effort write whose failure surfaces on the next request.
//!
//! Self-import recursion (upeg declared as its own upstream) is cut by a
//! marker env var stamped on every spawned upstream — see `child_env`.

use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use upeg_core::{InputSpec, Invoker, OutputSpec, PinKind, Surface, ToolKey, ToolMeta};
use upeg_runtime::{
    ToolboxRegistrationGroup,
    manifest::{RuntimeToolManifest, lower_runtime_tool_manifest},
    toolbox_add_single_text_tool_with_dispatcher_managed, validate_toolbox_addition,
};

mod child_env;
mod inventory;
mod output_schema;
mod skip;
mod spawn_retry;
mod wire;

pub use child_env::{MCP_IMPORT_CHILD_ENV, is_mcp_import_child};
pub use inventory::interface_inventory_entries;
pub use output_schema::OutputSchemaError;
pub use skip::{ImportOutcome, ParsedToolsList, SkipReason, SkippedTool};

use skip::skipped_summary;
use spawn_retry::spawn_and_list_with_bounded_retry;
use wire::{
    PendingRequest, block_on_runtime, kill_and_wait, request_async, write_notification_async,
};

/// Filename extension every MCP-import declaration carries. Shared with
/// `upeg_sources::sources`' directory survey so the survey can never
/// count a different set of files than the loader actually reads.
pub const DECLARATION_EXTENSION: &str = "toml";

/// Namespace used when a declaration file's stem is not valid UTF-8.
/// `register_server` then rejects the (non-canonical) name explicitly,
/// which reports better than silently skipping the file.
const FALLBACK_SERVER_NAME: &str = "server";

const MCP_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_TOOLS_LIST_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_TOOLS_CALL_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// Per the MCP spec, the client sends this notification right after the
/// `initialize` response, before any other request (`tools/list`
/// included). It carries no `id` and expects no response. Some upstream
/// SDKs (e.g. the official TypeScript SDK) enforce the ordering
/// strictly, so skipping it can leave a server silently ignoring every
/// later request (E-5).
const MCP_NOTIFICATION_INITIALIZED: &str = "notifications/initialized";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpTimeouts {
    pub initialize: Duration,
    pub tools_list: Duration,
    pub tools_call: Duration,
}

impl Default for McpTimeouts {
    fn default() -> Self {
        Self {
            initialize: MCP_INITIALIZE_TIMEOUT,
            tools_list: MCP_TOOLS_LIST_TIMEOUT,
            tools_call: MCP_TOOLS_CALL_TIMEOUT,
        }
    }
}

impl McpTimeouts {
    fn for_method(self, method: &'static str) -> Duration {
        match method {
            "initialize" => self.initialize,
            "tools/list" => self.tools_list,
            "tools/call" => self.tools_call,
            _ => self.tools_call,
        }
    }
}

/// TOML schema for `~/.upeg/mcp-imports/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamConfig {
    /// Subprocess to spawn (e.g. `"npx"`).
    pub command: String,
    /// Args to pass to the subprocess (e.g. `["-y", "@modelcontextprotocol/server-github"]`).
    #[serde(default)]
    pub args: Vec<String>,
    /// Re-expose imported tools over upeg's own MCP surface. Default
    /// `false`: imported tools register on every surface EXCEPT MCP so
    /// an import cannot silently loop another server's tools back out
    /// as upeg MCP tools. Set `reexport = true` to opt in.
    #[serde(default)]
    pub reexport: bool,
}

impl UpstreamConfig {
    /// Typed view of the `reexport` flag.
    pub const fn reexport_policy(&self) -> McpReexport {
        if self.reexport {
            McpReexport::OptedIn
        } else {
            McpReexport::Blocked
        }
    }
}

/// Whether imported tools may be re-exposed over upeg's MCP surface.
/// `Blocked` is the default: an MCP-imported tool stays off the MCP
/// surface unless the import config opts in with `reexport = true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpReexport {
    Blocked,
    OptedIn,
}

impl McpReexport {
    const fn surfaces(self) -> &'static [Surface] {
        match self {
            Self::Blocked => upeg_core::ALL_SURFACES_EXCEPT_MCP,
            Self::OptedIn => upeg_core::ALL_SURFACES,
        }
    }
}

/// Live handle to a running MCP server subprocess. Wraps the stdin/stdout
/// pipes so the dispatcher can issue `tools/call` requests.
///
/// Holds the original `UpstreamConfig` so `respawn` can rebuild the
/// subprocess after a crash without going back through `register_dir`.
pub struct UpstreamServer {
    /// UpstreamServer name used for namespacing tool ids in the registry — kept
    /// alongside the handle for diagnostics and respawn.
    name: String,
    config: UpstreamConfig,
    timeouts: McpTimeouts,
    runtime: tokio::runtime::Runtime,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("spawn `{command}`: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },
    #[error("rpc `{method}` failed (code {code}): {message}")]
    Rpc {
        method: &'static str,
        code: i64,
        message: String,
    },
    #[error("rpc `{method}` timed out for server `{server}` after {timeout_ms}ms")]
    Timeout {
        server: String,
        method: &'static str,
        timeout_ms: u64,
    },
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("bad tool meta: {0}")]
    BadToolMeta(String),
    #[error(
        "register_server: server name must be non-empty (used as the namespace prefix for upstream tool ids)"
    )]
    EmptyServerName,
    #[error("register_server: server name `{0}` must be canonical and unpadded")]
    NonCanonicalServerName(String),
    #[error(
        "tool from server `{server}` would namespace as `{ns_id}` which shadows a built-in; rename the server config (e.g. `my_{server}` instead of `{server}`) so the namespace prefix doesn't collide"
    )]
    IdShadowsBuiltIn { server: String, ns_id: String },
    #[error(
        "tool from server `{server}` would namespace as `{ns_id}`, but that id is already registered under Toolkit `{existing_toolkit}`; rename the server config or upstream tool so the full id is unique"
    )]
    IdConflictsWithRegisteredTool {
        server: String,
        ns_id: String,
        existing_toolkit: String,
    },
    #[error(
        "server `{server}` has no importable tools — all {} tool(s) were skipped: {}",
        .skipped.len(),
        skipped_summary(.skipped)
    )]
    AllToolsSkipped {
        server: String,
        skipped: Vec<SkippedTool>,
    },
}

impl UpstreamServer {
    /// Spawn the configured subprocess and run the MCP handshake
    /// (`initialize`). Stdio pipes are kept alive on `Self`. The returned
    /// `UpstreamServer` is ready for `tools_list` / `call`.
    pub fn spawn(name: impl Into<String>, config: &UpstreamConfig) -> Result<Self, ImportError> {
        Self::spawn_with_timeouts(name, config, McpTimeouts::default())
    }

    pub fn spawn_with_timeouts(
        name: impl Into<String>,
        config: &UpstreamConfig,
        timeouts: McpTimeouts,
    ) -> Result<Self, ImportError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(ImportError::Io)?;
        let (child, stdin, stdout) = {
            let _runtime_guard = runtime.enter();
            spawn_child(config)?
        };

        let mut srv = Self {
            name: name.into(),
            config: config.clone(),
            timeouts,
            runtime,
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };

        // MCP `initialize` handshake — required before tools/list per
        // the protocol. We don't actually use the response shape; just
        // confirm the server responded without an error frame.
        let _ = srv.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "upeg-mcp-import", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        // Spec-required follow-up notification (E-5): send it before
        // any further request. Best-effort — a write failure here
        // must not fail the handshake; the very next request
        // (`tools/list`) will surface any real connection problem.
        srv.notify_initialized();
        Ok(srv)
    }

    /// Recover from a dead subprocess by spawning a fresh one with the
    /// stored config. Reuses the existing `name` so the registry's
    /// namespace stays stable. Caller is responsible for retrying any
    /// in-flight request that triggered the respawn.
    fn respawn(&mut self) -> Result<(), ImportError> {
        self.shutdown();
        let fresh = Self::spawn_with_timeouts(self.name.clone(), &self.config, self.timeouts)?;
        // Whole-struct assignment — `*self = fresh` triggers Drop on the
        // old self (kill+wait again, no-op on the already-dead process).
        *self = fresh;
        Ok(())
    }

    /// Forcibly terminate the running subprocess. After this, subsequent
    /// `call`s will hit a broken pipe and trigger the respawn-and-retry
    /// recovery path. Public so integration tests can simulate crashes;
    /// production code uses Drop semantics or per-server shutdown.
    pub fn terminate(&mut self) {
        self.shutdown();
    }

    /// Send a JSON-RPC request and read the next response line. The
    /// server's `id` is required to match the request id; mismatches
    /// surface as `ImportError::Protocol`.
    fn request(&mut self, method: &'static str, params: Value) -> Result<Value, ImportError> {
        let id = self.next_id;
        self.next_id += 1;
        let server = self.name.clone();
        let timeout = self.timeouts.for_method(method);
        let runtime = &self.runtime;
        let child = &mut self.child;
        let stdin = &mut self.stdin;
        let stdout = &mut self.stdout;
        let request = PendingRequest {
            server,
            id,
            method,
            params,
            timeout,
        };
        block_on_runtime(runtime, async move {
            request_async(child, stdin, stdout, request).await
        })
    }

    /// Send a JSON-RPC notification (no `id`, no response expected).
    /// Used for `notifications/initialized` right after the
    /// `initialize` handshake (E-5). Best-effort: swallows write
    /// failures instead of returning `Err`, since a dead pipe here
    /// will surface loudly on the caller's very next `request`.
    fn notify(&mut self, method: &'static str) {
        let runtime = &self.runtime;
        let stdin = &mut self.stdin;
        block_on_runtime(runtime, async move {
            let _ = write_notification_async(stdin, method).await;
        });
    }

    /// Convenience wrapper pinning the one notification upeg's import
    /// client actually sends.
    fn notify_initialized(&mut self) {
        self.notify(MCP_NOTIFICATION_INITIALIZED);
    }

    /// Ask the server for its tool catalog. Tools whose schemas fail
    /// conversion surface in [`ParsedToolsList::skipped`] instead of
    /// failing the listing.
    pub fn tools_list(&mut self) -> Result<ParsedToolsList, ImportError> {
        let result = self.request("tools/list", Value::Null)?;
        let raw = extract_tools_array(&result)?;
        parse_tools_list_entries(&raw, &self.name)
    }

    /// Proxy a `tools/call` request to the subprocess. `tool_id` is the
    /// server's own id (without the `<server_name>.` prefix the importer
    /// adds when registering).
    ///
    /// If the subprocess has died (Io / Protocol error on the write or
    /// read), respawn once via the stored config and retry the same
    /// `tools/call`. Tool calls succeed even across upstream crashes
    /// for callers that don't care; if the second attempt also fails,
    /// the second error is returned (no further retries).
    pub fn call(&mut self, tool_id: &str, args: &Value) -> Result<String, ImportError> {
        let params = json!({
            "name": tool_id,
            "arguments": args,
        });
        let first = self.request("tools/call", params.clone());
        let result = match first {
            Ok(v) => v,
            Err(e) if is_subprocess_dead(&e) => {
                // Subprocess crashed mid-flight — restart and retry once.
                self.respawn()?;
                self.request("tools/call", params)?
            }
            Err(e) => return Err(e),
        };
        // Iter 186: shared `mcp::extract_text_content` helper. Pre-iter-186
        // this loop and the parallel one in `client.rs:translate_response`
        // had drifted (see iter 185 for the silent data-loss bug). One
        // implementation now serves both proxy and CLI-via-daemon paths.
        let buf = crate::protocol::extract_text_content(&result);
        // `isError` from the upstream surfaces as a tool error to the
        // caller. We map it into ImportError::Rpc with code 0 so the
        // caller can format it the same way other rpc failures format.
        if result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(ImportError::Rpc {
                method: "tools/call",
                code: 0,
                message: buf,
            });
        }
        Ok(buf)
    }

    /// Best-effort terminate. Returns nothing useful — the subprocess
    /// may have already exited.
    pub fn shutdown(&mut self) {
        let runtime = &self.runtime;
        let child = &mut self.child;
        block_on_runtime(runtime, async move {
            let _ = kill_and_wait(child).await;
        });
    }
}

fn spawn_child(config: &UpstreamConfig) -> Result<(Child, ChildStdin, ChildStdout), ImportError> {
    let mut child = child_env::stamp(
        Command::new(&config.command)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherit stderr so subprocess startup errors surface in the
            // user's terminal — easier to diagnose `command not found`.
            .stderr(Stdio::inherit())
            // Scrub env vars that would make the subprocess auto-load
            // ANOTHER copy of the same server config and fork-bomb us.
            // The subprocess gets a clean slate for upeg's own loaders;
            // it should declare its own tools (or be a non-upeg server,
            // in which case these variables mean nothing to it). This
            // alone does not stop a self-import (E-6): the child still
            // finds the SAME default `~/.upeg/mcp-imports/` via
            // `$UPEG_HOME`/`$HOME`, which is exactly what
            // `child_env::stamp` above guards against.
            .env_remove("UPEG_MCP_IMPORTS_DIR")
            .env_remove("UPEG_TOOLKITS_DIR")
            .env_remove("UPEG_WASM_DIR"),
    )
    .spawn()
    .map_err(|e| ImportError::Spawn {
        command: config.command.clone(),
        source: e,
    })?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ImportError::Protocol("subprocess stdin pipe missing".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ImportError::Protocol("subprocess stdout pipe missing".into()))?;
    Ok((child, stdin, stdout))
}

impl Drop for UpstreamServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// `true` for errors that look like the subprocess crashed: broken pipe
/// on write, EOF on read, or a generic Protocol error mentioning closed
/// stdout. Used by `UpstreamServer::call` to decide whether respawn-and-retry
/// is appropriate.
fn is_subprocess_dead(err: &ImportError) -> bool {
    match err {
        ImportError::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionAborted,
        ),
        ImportError::Protocol(msg) => {
            msg.contains("closed stdout") || msg.contains("invalid JSON line")
        }
        _ => false,
    }
}

/// Tool declaration extracted from an MCP `tools/list` response.
#[derive(Debug, Clone)]
pub struct RemoteToolDecl {
    pub id: String,
    pub description: String,
    /// Typed MCP input metadata imported from the upstream `inputSchema`.
    pub input_spec: InputSpec,
    /// Typed MCP output metadata imported from the upstream `outputSchema`.
    pub output_spec: OutputSpec,
}

/// Owning handle returned by [`register_server`] / [`register_dir`].
///
/// While the value is live, the runtime dispatchers it created are
/// reachable through the global registry; dropping it removes those
/// dispatchers. The MCP subprocess survives until the last dispatcher
/// closure (each holds an `Arc<Mutex<UpstreamServer>>`) is dropped.
#[derive(Debug)]
pub struct Registration {
    ids: Vec<&'static str>,
    _registrations: Vec<ToolboxRegistrationGroup>,
}

impl Registration {
    pub fn ids(&self) -> &[&'static str] {
        &self.ids
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Dispatcher/tool teardown is owned by the
        // `ToolboxRegistrationGroup`s; the provenance record is ours.
        for id in &self.ids {
            upeg_runtime::clear_tool_provenance(id);
        }
    }
}

/// Iter 216: truncate JSON Value displays in error messages so a
/// pathological upstream entry (multi-MB inputSchema for example)
/// doesn't bloat the response. Same reasoning as iter 214's
/// `display_id`. Reuses `serde_json`'s `to_string` then char-truncates
/// — same UTF-8-safe approach iter 215 documented.
///
/// Iter 253: shape-parity with `display_id` (lib.rs).
/// Iter 254: delegated to `crate::truncate_for_display` so the two
/// callers can't drift on the byte-pre-filter / char-check / suffix
/// shape. MAX=200 is per-call, picked to give error responses room
/// for an offending JSON entry without bloating.
pub fn display_value(v: &Value) -> String {
    crate::truncate_for_display(&v.to_string(), 200)
}

/// Extract the `tools` array from a `tools/list` response body.
/// Missing-vs-wrong-type lens applied to the array-level field:
///   - Missing entirely → empty `Vec` (a server with no tools is
///     legitimate; not an error).
///   - Present but not an array → `ImportError::Protocol` (the
///     upstream violates the MCP spec). Silently swallowing this
///     case as empty would leave users puzzled about why their
///     server registered zero tools.
pub fn extract_tools_array(result: &Value) -> Result<Vec<Value>, ImportError> {
    match result.get("tools") {
        None => Ok(Vec::new()),
        Some(v) => v.as_array().cloned().ok_or_else(|| {
            ImportError::Protocol(format!(
                "`result.tools` must be an array, got {}",
                display_value(v)
            ))
        }),
    }
}

/// parse the upstream `tools/list` response's `tools` array
/// into `RemoteToolDecl`s. Pure (no subprocess, no I/O).
///
/// Two failure tiers:
///   - **Protocol violations → `Err` (whole listing).** `name` field
///     absent, non-string, or padded with whitespace. An upstream
///     that violates the MCP spec (or emits non-canonical ids — upeg
///     stores ids exactly, never normalizes) is broken in a way that
///     deserves a loud fail, not a workaround.
///   - **Conversion failures → per-tool skip.** Empty/whitespace-only
///     `name` (the namespaced id `<server>.` would be unidentifiable
///     downstream) and `inputSchema`/`outputSchema` shapes upeg's
///     typed I/O subset cannot express (`$ref`, `oneOf`, nested
///     objects, ...). These are expressiveness gaps, not spec
///     violations; the other tools from the same server still
///     register. Skips are recorded in [`ParsedToolsList::skipped`]
///     with a structured [`SkipReason`] so callers (startup log, MCP
///     manager surface, tests) can report which tool was dropped and
///     why. The `_server_name` arg is kept for future log/telemetry
///     use.
pub fn parse_tools_list_entries(
    raw: &[Value],
    _server_name: &str,
) -> Result<ParsedToolsList, ImportError> {
    let mut out = ParsedToolsList::default();
    for entry in raw {
        // Disambiguate missing vs wrong-type `name` — same pattern
        // as `mcp.rs` invalid-request method and `params.name` in
        // tools_call. A `name: 42` entry returning "missing `name`"
        // would mislead the upstream server author about whether
        // the field was absent or mistyped.
        let id = match entry.get("name") {
            None => {
                return Err(ImportError::Protocol(
                    // truncate entry display so a pathologically-
                    // large entry doesn't bloat the error response.
                    format!("tool entry missing `name`: {}", display_value(entry)),
                ));
            }
            Some(v) => v
                .as_str()
                .ok_or_else(|| {
                    ImportError::Protocol(format!(
                        "tool entry `name` must be a string, got {}: {}",
                        display_value(v),
                        display_value(entry)
                    ))
                })?
                .to_string(),
        };
        if id.trim().is_empty() {
            out.skipped.push(SkippedTool {
                id,
                reason: SkipReason::EmptyName,
            });
            continue;
        }
        if id != id.trim() {
            return Err(ImportError::Protocol(format!(
                "tool entry `name` must be canonical and unpadded, got `{}`: {}",
                crate::truncate_for_display(&id, 80),
                display_value(entry)
            )));
        }
        let description = entry
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let input_spec = match entry.get("inputSchema") {
            None => InputSpec::empty(),
            Some(v) => match InputSpec::try_from(v) {
                Ok(spec) => spec,
                Err(error) => {
                    out.skipped.push(SkippedTool {
                        id,
                        reason: SkipReason::UnsupportedInputSchema(error),
                    });
                    continue;
                }
            },
        };
        let output_spec = match entry.get("outputSchema") {
            None => OutputSpec::empty(),
            Some(v) => match output_schema::output_spec_from_json_schema(v) {
                Ok(spec) => spec,
                Err(error) => {
                    out.skipped.push(SkippedTool {
                        id,
                        reason: SkipReason::UnsupportedOutputSchema(error),
                    });
                    continue;
                }
            },
        };
        out.decls.push(RemoteToolDecl {
            id,
            description,
            input_spec,
            output_spec,
        });
    }
    Ok(out)
}

/// Translate a `RemoteToolDecl` into a `ToolMeta` registered under the
/// namespace `<server_name>.<original_id>`. Strings are leaked (same
/// pattern as TOML/WASM loaders) so the registry holds `&'static`.
/// `reexport` decides whether the imported tool appears on upeg's own
/// MCP surface (default: [`McpReexport::Blocked`]).
pub fn decl_to_meta(
    server_name: &str,
    decl: &RemoteToolDecl,
    reexport: McpReexport,
) -> Result<ToolMeta, ImportError> {
    let primary_output_id = decl
        .output_spec
        .fields
        .first()
        .map(|field| field.name.clone());
    lower_runtime_tool_manifest(RuntimeToolManifest {
        id: format!("{server_name}.{}", decl.id),
        toolkit: server_name.to_string(),
        tags: Vec::new(),
        display_label: None,
        description: Some(decl.description.clone()),
        input_spec: decl.input_spec.clone(),
        output_spec: decl.output_spec.clone(),
        primary_output_id,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::External,
        surfaces: reexport.surfaces().to_vec(),
        boards: Vec::new(),
    })
    .map_err(|error| ImportError::BadToolMeta(error.to_string()))
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Pre-validation for `register_server`'s inventory-shadow check,
/// extracted so it can be unit-tested without spawning a real
/// subprocess. Pure: takes the would-be server name and the decls
/// slice; returns Err on the first collision found.
///
/// A namespaced id `<server_name>.<original_id>` collides if either the
/// transport id or the structured Toolkit/local key is already in static
/// inventory, or if a runtime entry with the same id already belongs to
/// another Toolkit. The check is atomic per-server (first collision aborts
/// the whole server's registration) so the user can rename and retry without
/// partial state.
/// Same iter-201 spirit as `upeg-loader`'s extracted `validate_toml`.
///
/// Collisions deliberately stay server-level even though schema
/// conversion failures are tool-level skips ([`SkipReason`]): a
/// collision is a trust/config problem, not an expressiveness gap.
/// Silently skipping a colliding tool could mask an upstream that
/// names a tool to shadow a built-in, and the remedy — renaming the
/// server config — applies to the whole namespace, so partial
/// registration would only leave misleading state behind.
pub fn check_no_inventory_shadows(
    server_name: &str,
    decls: &[RemoteToolDecl],
) -> Result<(), ImportError> {
    upeg_toolkit_native::register_native_toolkits().map_err(ImportError::BadToolMeta)?;
    for decl in decls {
        let ns_id_str = format!("{server_name}.{}", decl.id);
        let key = ToolKey::parse_canonical(server_name, &decl.id).map_err(|error| {
            ImportError::BadToolMeta(format!(
                "tool from server `{server_name}` has invalid structured key `{server_name}/{}`: {error}",
                decl.id
            ))
        })?;
        validate_toolbox_addition(&key).map_err(|error| match error {
            upeg_runtime::manifest::CollisionError::ShadowsBuiltIn { .. } => {
                ImportError::IdShadowsBuiltIn {
                    server: server_name.to_string(),
                    ns_id: ns_id_str,
                }
            }
            upeg_runtime::manifest::CollisionError::DuplicateTool { .. } => {
                let existing_toolkit = upeg_runtime::toolbox_tool(&ns_id_str)
                    .map_or_else(|| server_name.to_string(), |tool| tool.toolkit.to_string());
                ImportError::IdConflictsWithRegisteredTool {
                    server: server_name.to_string(),
                    ns_id: ns_id_str,
                    existing_toolkit,
                }
            }
        })?;
    }
    Ok(())
}

/// Spawn the configured server, list its tools, and register each in the
/// global runtime dispatcher with id `<server_name>.<original_id>`.
///
/// Tools whose schemas fail conversion are skipped and reported in
/// [`ImportOutcome::skipped`]; the server errors only when a protocol
/// violation or namespace collision occurs, or when *every* tool was
/// skipped ([`ImportError::AllToolsSkipped`]).
///
/// The returned [`ImportOutcome`] owns the dispatcher registrations;
/// drop it to deregister the tools.
pub fn register_server(
    server_name: &str,
    config: &UpstreamConfig,
) -> Result<ImportOutcome, ImportError> {
    let (server, parsed) = spawn_server_and_list_tools(server_name, config)?;
    let ParsedToolsList { decls, skipped } = parsed;
    let server_arc = Arc::new(Mutex::new(server));
    let mut ids = Vec::with_capacity(decls.len());
    let mut registrations = Vec::with_capacity(decls.len());

    for decl in decls {
        let meta = decl_to_meta(server_name, &decl, config.reexport_policy())?;
        let ns_id: &'static str = meta.id;

        // Strip the server prefix at dispatch time so the upstream sees
        // its own original tool id.
        let original_id_static: &'static str = leak(decl.id.clone());
        let server_arc = Arc::clone(&server_arc);
        let group = toolbox_add_single_text_tool_with_dispatcher_managed(meta, move |args| {
            let mut guard = server_arc
                .lock()
                .map_err(|_| "mcp server mutex poisoned".to_string())?;
            guard
                .call(original_id_static, args.as_value())
                .map_err(|e| format!("{e}"))
        });
        upeg_runtime::register_tool_provenance(
            ns_id,
            upeg_runtime::ToolProvenance::McpImport {
                server: server_name.to_string(),
            },
        );
        ids.push(ns_id);
        registrations.push(group);
    }
    Ok(ImportOutcome {
        registration: Registration {
            ids,
            _registrations: registrations,
        },
        skipped,
    })
}

fn spawn_server_and_list_tools(
    server_name: &str,
    config: &UpstreamConfig,
) -> Result<(UpstreamServer, ParsedToolsList), ImportError> {
    // Validate the server name BEFORE spawning the subprocess. An
    // empty / whitespace-only name would cause every upstream tool
    // to register as `.<original_id>` (leading dot) — usable
    // downstream but confusing in `tool list`. Same boundary-
    // validation lens applied elsewhere to ToolMeta fields and tool
    // entries.
    if server_name.trim().is_empty() {
        return Err(ImportError::EmptyServerName);
    }
    if ToolKey::parse_canonical(server_name, "tool").is_err() {
        return Err(ImportError::NonCanonicalServerName(server_name.to_string()));
    }

    // Bounded retry + backoff (E-6): a transient hiccup (upstream
    // hasn't started answering stdio yet) gets a few attempts before
    // this gives up; a permanently failing upstream still fails in
    // bounded time.
    let (server, parsed) = spawn_and_list_with_bounded_retry(server_name, config)?;

    // All-or-nothing only in the degenerate direction: if every tool
    // was skipped there is nothing to register and the server import
    // fails loudly (a server with zero tools upstream is still fine —
    // both lists empty).
    if parsed.decls.is_empty() && !parsed.skipped.is_empty() {
        return Err(ImportError::AllToolsSkipped {
            server: server_name.to_string(),
            skipped: parsed.skipped,
        });
    }

    // Pre-validate every namespaced id against the link-time
    // built-in inventory BEFORE registering anything. Extracted as
    // `check_no_inventory_shadows` so the pure collision logic can
    // be unit-tested directly (without spawning a real subprocess) —
    // same extract-for-testability reason as upeg-loader's
    // `validate_toml`.
    check_no_inventory_shadows(server_name, &parsed.decls)?;
    Ok((server, parsed))
}

/// Every `*.toml` declaration under `dir`, as `(server name, path)`
/// pairs — the file stem is the namespace prefix for that server's tool
/// ids. An unreadable directory yields no declarations.
///
/// Shared by [`register_dir`] (which spawns each declared server) and
/// [`scan_dir_reexport_policy`] (which only reads the files), so "what
/// counts as a declaration file" is decided in exactly one place.
fn declaration_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == DECLARATION_EXTENSION)
        })
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(FALLBACK_SERVER_NAME)
                .to_string();
            (name, path)
        })
        .collect()
}

/// Walk every `*.toml` under `dir`, spawn + register each declared
/// server. Per-file failures are surfaced inline via `Err` entries; the
/// caller decides whether to abort or continue.
pub fn register_dir(dir: &Path) -> Vec<(String, Result<ImportOutcome, ImportError>)> {
    declaration_files(dir)
        .into_iter()
        .map(|(name, path)| {
            let result = (|| -> Result<ImportOutcome, ImportError> {
                let raw = std::fs::read_to_string(&path).map_err(ImportError::Io)?;
                let cfg: UpstreamConfig = toml::from_str(&raw).map_err(ImportError::Toml)?;
                register_server(&name, &cfg)
            })();
            (name, result)
        })
        .collect()
}

/// Cheap pre-scan: does ANY declaration under `dir` opt into
/// `reexport = true`?
///
/// Reads and parses the declaration files and nothing else — no
/// subprocess is spawned and no upstream handshake happens, so this
/// costs one `read_dir` plus a few small file reads even when the real
/// load would take tens of seconds. The in-process `upeg mcp` lane uses
/// it to skip eager import loading entirely when none of the imported
/// tools could appear on its own surface anyway
/// (this module's docs, "Load contract").
///
/// An unreadable or unparseable declaration counts as `Blocked`: the
/// real load reports it as a per-server failure, and a broken file must
/// not buy a load whose result this surface cannot use.
pub fn scan_dir_reexport_policy(dir: &Path) -> McpReexport {
    for (_, path) in declaration_files(dir) {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(config) = toml::from_str::<UpstreamConfig>(&raw) else {
            continue;
        };
        if config.reexport_policy() == McpReexport::OptedIn {
            return McpReexport::OptedIn;
        }
    }
    McpReexport::Blocked
}

#[cfg(test)]
mod tests;
