#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! MCP import against a REAL official-SDK server.
//!
//! `mcp_import_e2e.rs` imports `upeg mcp` itself, which is a fine
//! dogfood test and a useless spec test: both ends are ours, so a
//! frame both ends get wrong stays invisible. E-5 was exactly that —
//! upeg sent `"params": null` for `tools/list` and skipped
//! `notifications/initialized`; every fake stdio server in the suite
//! answered anyway, while the official `@modelcontextprotocol`
//! TypeScript SDK silently dropped the request and the import looked
//! like a timeout (`upeg_sources::mcp_import` module docs).
//!
//! So this file talks to a server nobody here wrote:
//! `@modelcontextprotocol/server-filesystem`, fetched through `npx`.
//!
//! Three levels of the same import: the raw handshake, in-process
//! registration + dispatch, and the real user lane — a `upeg host
//! start --daemon` in a scratch `UPEG_HOME` that a separate `upeg
//! call` process reaches the imported tool through.
//!
//! Every test is `#[ignore]` — they need network on a cold `npx`
//! cache and a Node toolchain, which no plain `cargo test` should
//! assume. `just mcp-import-real-smoke` runs them, and `just ci-smoke`
//! runs that. When `npx` is missing (or the package cannot be fetched)
//! they SKIP with a printed reason instead of failing: an absent Node
//! toolchain is not an upeg regression.
//!
//! That skip is a hole wherever the toolchain IS supposed to be there.
//! `UPEG_REQUIRE_REAL_MCP=1` closes it: every not-ready reason becomes
//! a panic instead of a silent pass, so a CI runner that quietly lost
//! its Node install fails loudly rather than reporting three green
//! tests that never ran. `just mcp-import-real-smoke` sets it whenever
//! `CI` is set.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::json;
use upeg_cli::{UpstreamMcpConfig, UpstreamMcpServer, register_upstream_mcp_server};

/// Node package runner. Found through `PATH`, like every other
/// developer tool this repo shells out to.
const NPX_BINARY: &str = "npx";
/// Answers `npx`'s "install this package?" prompt so the spawn is
/// non-interactive — without it a cold cache blocks forever on stdin,
/// which the MCP client owns for JSON-RPC.
const NPX_AUTO_INSTALL_FLAG: &str = "-y";
/// The reference server: official SDK, strict JSON-RPC frame
/// validation, no upeg code anywhere in it.
const FILESYSTEM_SERVER_PACKAGE: &str = "@modelcontextprotocol/server-filesystem";

/// Upstream tools this smoke pins. Both take a single `path` string,
/// so both must survive conversion to upeg typed I/O.
const LIST_DIRECTORY_TOOL: &str = "list_directory";
const READ_FILE_TOOL: &str = "read_file";

const SAMPLE_FILE_NAME: &str = "hello.txt";
const SAMPLE_FILE_BODY: &str = "upeg real-server smoke\n";

/// Budget for the one-time `npx` package download. Deliberately far
/// larger than the import layer's own 5s handshake timeouts: this
/// pre-warm exists precisely so the timed import never pays for a
/// network fetch (see [`prewarm_package`]).
const PREWARM_TIMEOUT: Duration = Duration::from_secs(180);
const PREWARM_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A directory the filesystem server is allowed to touch, holding one
/// known file. `TempDir` removes it when the test ends.
fn sandbox() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("scratch sandbox dir");
    std::fs::write(dir.path().join(SAMPLE_FILE_NAME), SAMPLE_FILE_BODY).expect("write sample file");
    dir
}

fn upstream_config(sandbox: &Path) -> UpstreamMcpConfig {
    UpstreamMcpConfig {
        command: NPX_BINARY.to_string(),
        args: vec![
            NPX_AUTO_INSTALL_FLAG.to_string(),
            FILESYSTEM_SERVER_PACKAGE.to_string(),
            sandbox.display().to_string(),
        ],
        reexport: false,
    }
}

/// Env var that turns every skip below into a failure. Set by `just
/// mcp-import-real-smoke` whenever `CI` is set: on a runner that is
/// SUPPOSED to have Node, "skipped" is indistinguishable from "passed"
/// in the log, and this lane's whole point is that a non-upeg MCP
/// implementation stayed in the loop.
const REQUIRE_REAL_MCP_ENV: &str = "UPEG_REQUIRE_REAL_MCP";
const REQUIRE_REAL_MCP_ON: &str = "1";

/// Why the reference server cannot run here. Split per cause so a log
/// reader can tell "no Node toolchain" from "npm registry unreachable"
/// from "the server itself refused to start" — three very different
/// things that all used to print as one undifferentiated skip.
#[derive(Debug, Clone)]
enum NotReady {
    /// No `npx` on `PATH`: no Node toolchain at all.
    NpxMissing,
    /// `npx` could not be spawned, or the package could not be fetched
    /// within [`PREWARM_TIMEOUT`].
    PrewarmFetchFailed(String),
    /// The pre-warm ran to completion but the server exited non-zero —
    /// the package is there and refuses to start.
    PrewarmExitFailed(String),
}

impl NotReady {
    fn reason(&self) -> String {
        match self {
            Self::NpxMissing => format!(
                "`{NPX_BINARY}` not on PATH — cannot run {FILESYSTEM_SERVER_PACKAGE}. \
                 Install Node (the devcontainer and the CI runner both have it)."
            ),
            Self::PrewarmFetchFailed(detail) => format!(
                "could not fetch {FILESYSTEM_SERVER_PACKAGE} via `{NPX_BINARY}` within {}s \
                 — offline or npm registry unreachable ({detail})",
                PREWARM_TIMEOUT.as_secs()
            ),
            Self::PrewarmExitFailed(detail) => format!(
                "{FILESYSTEM_SERVER_PACKAGE} was fetched but the pre-warm run failed \
                 ({detail}) — the package is present and will not start"
            ),
        }
    }
}

/// `true` when this machine can run the reference server at all.
///
/// Two gates:
///   1. `npx` on `PATH` — no Node toolchain, nothing to test;
///   2. a completed pre-warm run — the package is now in the `npx`
///      cache, so the actual import spawns in well under the import
///      layer's `MCP_INITIALIZE_TIMEOUT` (5s). Without this, a cold
///      cache would spend the whole handshake budget downloading and
///      report a network problem as an upeg timeout bug.
///
/// A failed gate skips by default and PANICS under
/// [`REQUIRE_REAL_MCP_ENV`], carrying the specific reason either way.
fn reference_server_ready(sandbox: &Path) -> bool {
    let Some(not_ready) = reference_server_readiness(sandbox) else {
        return true;
    };
    let reason = not_ready.reason();
    assert!(
        std::env::var(REQUIRE_REAL_MCP_ENV).as_deref() != Ok(REQUIRE_REAL_MCP_ON),
        "{REQUIRE_REAL_MCP_ENV}={REQUIRE_REAL_MCP_ON} but the reference server is not runnable: \
         {reason}"
    );
    eprintln!("skip: {reason}");
    false
}

/// `None` when the reference server can run; the specific obstacle
/// otherwise. The pre-warm result is computed at most once per test
/// process — it is a package download, and three tests paying for it
/// serially would triple the cold-cache cost of the lane.
fn reference_server_readiness(sandbox: &Path) -> Option<NotReady> {
    static READINESS: OnceLock<Option<NotReady>> = OnceLock::new();
    READINESS
        .get_or_init(|| {
            if !npx_on_path() {
                return Some(NotReady::NpxMissing);
            }
            prewarm_package(sandbox).err()
        })
        .clone()
}

fn npx_on_path() -> bool {
    Command::new(NPX_BINARY)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Run the server once with a closed stdin so it starts, sees EOF, and
/// exits — the cheapest way to make `npx` populate its package cache.
/// Bounded: a hung fetch is killed and reported, never left to stall
/// the suite.
///
/// The failure is classified, not collapsed to a bool: "could not
/// fetch" and "fetched, then exited non-zero" point at completely
/// different machines to go look at.
fn prewarm_package(sandbox: &Path) -> Result<(), NotReady> {
    let config = upstream_config(sandbox);
    let spawned = Command::new(&config.command)
        .args(&config.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(err) => {
            return Err(NotReady::PrewarmFetchFailed(format!("spawn failed: {err}")));
        }
    };
    let deadline = Instant::now() + PREWARM_TIMEOUT;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(NotReady::PrewarmExitFailed(format!("exit {status}")));
            }
            Ok(None) => std::thread::sleep(PREWARM_POLL_INTERVAL),
            Err(err) => {
                return Err(NotReady::PrewarmFetchFailed(format!("wait failed: {err}")));
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(NotReady::PrewarmFetchFailed("timed out".to_string()))
}

fn dispatch_text(id: &str, args: &serde_json::Value) -> String {
    let outcome = upeg_runtime::try_runtime_dispatch(id, args)
        .unwrap_or_else(|| panic!("`{id}` must be registered in the toolbox"));
    match upeg_runtime::tool_result_text(outcome) {
        Ok(text) => text,
        Err(msg) => panic!("`{id}` dispatch failed: {msg}"),
    }
}

/// The E-5 regression in its natural habitat: a strict SDK server must
/// answer our `initialize` + `tools/list` handshake and hand back its
/// real tool list.
#[test]
#[ignore = "needs `npx` + network; run via `just mcp-import-real-smoke`"]
fn the_real_filesystem_server_answers_the_handshake_and_tool_list() {
    let sandbox = sandbox();
    if !reference_server_ready(sandbox.path()) {
        return;
    }

    let mut server = UpstreamMcpServer::spawn("fsreal_list", &upstream_config(sandbox.path()))
        .expect("spawn the official filesystem MCP server");
    let listing = server
        .tools_list()
        .expect("tools/list from a real SDK server");

    let ids: Vec<&str> = listing.decls.iter().map(|d| d.id.as_str()).collect();
    assert!(
        !ids.is_empty(),
        "a real SDK server must yield at least one tool"
    );
    for expected in [LIST_DIRECTORY_TOOL, READ_FILE_TOOL] {
        assert!(
            ids.contains(&expected),
            "expected `{expected}`, got: {ids:?}"
        );
    }
    // Partial success is by design; print what did not convert so a
    // schema regression is visible in the smoke log.
    for skipped in &listing.skipped {
        println!("note: skipped `{}`: {}", skipped.id, skipped.reason);
    }
}

/// End-to-end: declare-shaped import (namespacing, registration,
/// provenance) plus a real `tools/call` round trip through the
/// registered dispatcher.
#[test]
#[ignore = "needs `npx` + network; run via `just mcp-import-real-smoke`"]
fn real_filesystem_server_tools_are_registered_under_a_namespace_and_called() {
    let sandbox = sandbox();
    if !reference_server_ready(sandbox.path()) {
        return;
    }

    let namespace = "fsreal";
    // Held for the whole test: the registration is an OWNING handle —
    // dropping it deregisters every imported tool and kills the
    // subprocess (see `retain_for_process_lifetime` on the host side).
    let outcome = register_upstream_mcp_server(namespace, &upstream_config(sandbox.path()))
        .expect("register the official filesystem MCP server");

    let ids = outcome.registered_ids();
    assert!(
        !ids.is_empty(),
        "import must register at least one tool, skipped: {:?}",
        outcome.skipped
    );
    for id in ids {
        assert!(
            id.starts_with(&format!("{namespace}.")),
            "imported ids are namespaced by the declaration stem: {id}"
        );
    }

    let list_id = format!("{namespace}.{LIST_DIRECTORY_TOOL}");
    let listing = dispatch_text(&list_id, &json!({ "path": sandbox.path() }));
    assert!(
        listing.contains(SAMPLE_FILE_NAME),
        "`{list_id}` must list the sandbox file, got: {listing}"
    );

    let read_id = format!("{namespace}.{READ_FILE_TOOL}");
    let contents = dispatch_text(
        &read_id,
        &json!({ "path": sandbox.path().join(SAMPLE_FILE_NAME) }),
    );
    assert!(
        contents.contains(SAMPLE_FILE_BODY.trim()),
        "`{read_id}` must return the file body, got: {contents}"
    );
}

// === Host lane ===

/// Env var pinning the scratch `~/.upeg` every spawned `upeg` process
/// uses. Passed per-command through `Command::env` — the workspace
/// forbids `unsafe`, so a test can never mutate its OWN environment.
const UPEG_HOME_ENV: &str = "UPEG_HOME";
/// Project-manifest detection is per working directory and this test
/// runs inside the repo, whose own `upeg.toml` would otherwise load 15
/// unrelated tools into every assertion's blast radius.
const UPEG_PROJECT_MANIFEST_ENV: &str = "UPEG_PROJECT_MANIFEST_PATH";
const UPEG_PROJECT_MANIFEST_OFF: &str = "off";
/// Declaration stem = import namespace (`upeg_sources::mcp_import` module docs).
const HOST_IMPORT_NAMESPACE: &str = "fsdemo";
const MCP_IMPORTS_DIR_NAME: &str = "mcp-imports";
const DECLARATION_EXTENSION: &str = "toml";

/// How long the host gets to publish `server.json` and finish its
/// eager import load. The `upeg host start` lane loads BEFORE opening
/// the listener, so "running" already implies "imports done" — this
/// budget covers process start plus one upstream spawn.
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(30);
const HOST_READY_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// A `upeg host start --daemon` in a scratch `UPEG_HOME`, stopped on
/// drop — including on a panicking assertion, which is exactly when a
/// leaked daemon would poison the next run.
struct ScratchHost {
    home: tempfile::TempDir,
}

impl ScratchHost {
    /// Declare `<home>/mcp-imports/fsdemo.toml` against the reference
    /// server, then start the daemon and wait for it to report running.
    fn start(sandbox: &Path) -> Self {
        let home = tempfile::tempdir().expect("scratch UPEG_HOME");
        let imports = home.path().join(MCP_IMPORTS_DIR_NAME);
        std::fs::create_dir_all(&imports).expect("mcp-imports dir");
        let config = upstream_config(sandbox);
        let quoted: Vec<String> = config.args.iter().map(|arg| format!("{arg:?}")).collect();
        std::fs::write(
            imports.join(format!("{HOST_IMPORT_NAMESPACE}.{DECLARATION_EXTENSION}")),
            format!(
                "command = {:?}\nargs = [{}]\n",
                config.command,
                quoted.join(", ")
            ),
        )
        .expect("write import declaration");

        let host = Self { home };
        let started = host.upeg(&["host", "start", "--daemon"]);
        assert!(
            started.status.success(),
            "`upeg host start --daemon` failed: {}",
            String::from_utf8_lossy(&started.stderr)
        );
        host.wait_until_running();
        host
    }

    fn upeg(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_upeg"))
            .args(args)
            .env(UPEG_HOME_ENV, self.home.path())
            .env(UPEG_PROJECT_MANIFEST_ENV, UPEG_PROJECT_MANIFEST_OFF)
            .stdin(Stdio::null())
            .output()
            .expect("run the upeg binary")
    }

    fn status_json(&self) -> serde_json::Value {
        let out = self.upeg(&["host", "status", "--json"]);
        serde_json::from_slice(&out.stdout).unwrap_or_else(|err| {
            panic!(
                "`upeg host status --json` emitted non-JSON ({err}): {}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    fn wait_until_running(&self) {
        let deadline = Instant::now() + HOST_READY_TIMEOUT;
        while Instant::now() < deadline {
            if self.status_json()["running"] == serde_json::Value::Bool(true) {
                return;
            }
            std::thread::sleep(HOST_READY_POLL_INTERVAL);
        }
        panic!(
            "host did not come up within {}s",
            HOST_READY_TIMEOUT.as_secs()
        );
    }
}

impl Drop for ScratchHost {
    fn drop(&mut self) {
        let _ = self.upeg(&["host", "stop"]);
    }
}

/// The lane a user actually takes: declare an upstream, start the
/// host, call the imported tool from a separate one-shot process.
///
/// It also pins the imports-pending signal end to end — `upeg host
/// status --json` reports the HOST's live phase, read back off its
/// `/healthz` (`upeg_cli::infrastructure::mcp_imports` module docs).
#[test]
#[ignore = "needs `npx` + network; run via `just mcp-import-real-smoke`"]
fn a_real_server_import_is_called_from_another_process_via_the_host() {
    let sandbox = sandbox();
    if !reference_server_ready(sandbox.path()) {
        return;
    }

    let host = ScratchHost::start(sandbox.path());

    // `upeg host start` loads imports BEFORE it opens the listener, so
    // a reachable host is necessarily past its load.
    let status = host.status_json();
    let imports = &status["mcpImports"];
    assert_eq!(
        imports["importsPending"],
        serde_json::Value::Bool(false),
        "an eagerly-loading host is never pending once reachable: {status}"
    );
    assert_eq!(imports["state"], "done", "{status}");
    assert_eq!(imports["serversLoaded"], 1, "{status}");
    assert_eq!(imports["serversFailed"], 0, "{status}");
    assert!(
        imports["tools"].as_u64().is_some_and(|tools| tools > 0),
        "the host must report the tools it imported: {status}"
    );

    let called = host.upeg(&[
        "call",
        &format!("{HOST_IMPORT_NAMESPACE}.{LIST_DIRECTORY_TOOL}"),
        "-a",
        &format!("path={}", sandbox.path().display()),
    ]);
    let stdout = String::from_utf8_lossy(&called.stdout);
    assert!(
        called.status.success(),
        "`upeg call` failed: {}",
        String::from_utf8_lossy(&called.stderr)
    );
    assert!(
        stdout.contains(SAMPLE_FILE_NAME),
        "the imported tool must run through the host: {stdout}"
    );
}
