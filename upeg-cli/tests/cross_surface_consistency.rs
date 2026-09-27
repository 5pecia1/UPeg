#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Cross-surface dispatch consistency.
//!
//! The architecture guarantees identical results across CLI / MCP /
//! HTTP because all three call into `upeg_cli::dispatch_tool`,
//! which is the single point of dispatch (iter 39). This test puts that
//! claim under load: one tool, three surfaces, identical args, asserts
//! the output text is the same. If it ever diverges, the surface that
//! drifted will fail the equality check.

use axum::body::{Body, to_bytes};
use clap::Parser as _;
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use upeg_cli::board_agent::BOARD_CONTEXT_TOOL;
use upeg_cli::{
    Cli, Outcome, dispatch_tool, handle_mcp_message, handle_mcp_message_with_board, http_router,
    run,
};
use upeg_core::{
    BoardKey, InputSpec, Invoker, PegboardUnits, PinKind, PinnedSignal, RecentSignal, SearchQuery,
    SearchSignals, Surface, ToolId, ToolMeta,
};
use upeg_runtime::{search_toolbox_tools, toolbox_add_tool_managed, toolbox_tool};

const SEARCH_CONSISTENCY_TAG: &[&str] = &["search-cross-surface-consistency-fixture"];
const SEARCH_CONSISTENCY_SURFACES: &[Surface] = &[Surface::Tui, Surface::Desktop, Surface::Ext];

async fn body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn output_value_text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

/// Drive one tool through the three concrete surfaces and return the
async fn dispatch_three_ways(tool_id: &str, args: Value) -> (String, String, String) {
    let direct = match dispatch_tool(tool_id, &args) {
        Outcome::Success(success) => upeg_runtime::tool_success_primary_text(&success),
        other => panic!("direct dispatch failed: {other:?}"),
    };

    // 2. MCP `tools/call` over the in-process JSON-RPC handler.
    let mcp_resp = handle_mcp_message(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool_id, "arguments": args },
    }))
    .expect("mcp response");
    assert!(
        mcp_resp["result"]["isError"].is_null(),
        "mcp surfaced error: {mcp_resp}"
    );
    let via_mcp = mcp_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();

    // 3. HTTP POST /v1/tools/{id}.
    let http_resp = http_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{tool_id}"))
                .header("content-type", "application/json")
                .body(Body::from(args.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        http_resp.status(),
        StatusCode::OK,
        "http non-200: {tool_id}"
    );
    let http_body = body_to_value(http_resp.into_body()).await;
    let primary = http_body["primary_output_id"].as_str().unwrap();
    let via_http = http_body["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"].as_str() == Some(primary))
        .map(|entry| output_value_text(&entry["value"]))
        .unwrap();

    (direct, via_mcp, via_http)
}

fn local_id_for(id: &'static str, toolkit: &'static str) -> &'static str {
    ToolId::parse_canonical_in_toolkit(id, toolkit)
        .expect("test ToolMeta ids must be canonical")
        .local()
}

fn search_fixture_tool(id: &'static str, display_label: &'static str) -> ToolMeta {
    ToolMeta {
        id,
        toolkit: "cross",
        local_id: local_id_for(id, "cross"),
        tags: SEARCH_CONSISTENCY_TAG,
        display_label,
        description: "cross-surface shared search consistency fixture",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: SEARCH_CONSISTENCY_SURFACES,
        boards: &[],
    }
}

fn shared_search_ids_for_surface(
    needle: &str,
    surface: Surface,
    limit: Option<usize>,
    signals: SearchSignals,
) -> Vec<&'static str> {
    search_toolbox_tools(
        SearchQuery {
            needle,
            surface: Some(surface),
            limit,
        },
        signals,
    )
    .into_iter()
    .map(|result| result.tool.id)
    .collect()
}

#[test]
fn search_with_the_same_query_and_signals_returns_the_same_ordered_ids_on_tui_desktop_and_popup() {
    let first_id = "cross.search_alpha";
    let second_id = "cross.search_beta";
    let third_id = "cross.search_gamma";
    let _first = toolbox_add_tool_managed(search_fixture_tool(first_id, "Search alpha"));
    let _second = toolbox_add_tool_managed(search_fixture_tool(second_id, "Search beta"));
    let _third = toolbox_add_tool_managed(search_fixture_tool(third_id, "Search gamma"));

    let signals = SearchSignals {
        pinned: vec![
            PinnedSignal {
                tool_id: first_id.to_string(),
                rank: 3,
            },
            PinnedSignal {
                tool_id: second_id.to_string(),
                rank: 0,
            },
            PinnedSignal {
                tool_id: first_id.to_string(),
                rank: 1,
            },
        ],
        recent: vec![RecentSignal {
            tool_id: third_id.to_string(),
            rank: 0,
        }],
    };
    let expected = vec![second_id, first_id, third_id];

    let tui_ids = shared_search_ids_for_surface(
        SEARCH_CONSISTENCY_TAG[0],
        Surface::Tui,
        None,
        signals.clone(),
    );
    let desktop_ids = shared_search_ids_for_surface(
        SEARCH_CONSISTENCY_TAG[0],
        Surface::Desktop,
        Some(9),
        signals.clone(),
    );
    let popup_ids =
        shared_search_ids_for_surface(SEARCH_CONSISTENCY_TAG[0], Surface::Ext, Some(6), signals);

    assert_eq!(tui_ids, expected, "TUI adapter order drifted");
    assert_eq!(desktop_ids, expected, "Desktop adapter order drifted");
    assert_eq!(popup_ids, expected, "Popup adapter order drifted");
    assert_eq!(tui_ids, desktop_ids, "TUI/Desktop search results differ");
    assert_eq!(
        desktop_ids, popup_ids,
        "Desktop/Popup search results differ"
    );
}

/// Iter-era cross-crate contract inherited from the retired Dioxus
/// surface's toolbox-visibility tests: `num.hex_to_decimal` is the
/// canonical example used by every dispatch test in this file, so its full
/// ToolMeta contract is pinned once here rather than assumed implicitly.
#[test]
fn the_hex_to_dec_tool_is_exposed_in_the_registry() {
    upeg_toolkit_native::register_native_toolkits().expect("generated builtins register");
    let tool = toolbox_tool("num.hex_to_decimal")
        .expect("num.hex_to_decimal must be in the toolbox registry");
    assert_eq!(tool.toolkit, "num");
    assert_eq!(tool.pin, PinKind::Inline);
    assert_eq!(tool.invoker, Invoker::Function);
    assert_eq!(tool.boards, &["dev"]);
    // Default surfaces = ALL_SURFACES (PRD §6.2).
    assert_eq!(tool.surfaces.len(), 7);
    assert!(tool.surfaces.contains(&Surface::Cli));
    assert!(tool.surfaces.contains(&Surface::Desktop));
    assert!(tool.surfaces.contains(&Surface::Mcp));
}

#[tokio::test]
async fn a_builtin_tool_produces_identical_results_on_every_surface() {
    // hex_to_decimal is the canonical example: deterministic, simple input,
    // exercises arg-extraction in every surface.
    let (direct, via_mcp, via_http) =
        dispatch_three_ways("num.hex_to_decimal", json!({"input": "0xff"})).await;
    assert_eq!(direct, "255");
    assert_eq!(direct, via_mcp, "MCP differs from direct dispatch");
    assert_eq!(direct, via_http, "HTTP differs from direct dispatch");
}

#[tokio::test]
async fn a_two_arg_builtin_tool_produces_consistent_results_on_every_surface() {
    // Two-arg tool — exercises `read_str(args, "a")` + `read_str(args, "b")`.
    let (direct, via_mcp, via_http) =
        dispatch_three_ways("text.diff", json!({"left": "alpha", "right": "alpha"})).await;
    // `text.diff` of identical strings produces no diff lines.
    assert_eq!(direct, via_mcp);
    assert_eq!(direct, via_http);
}

/// Iter 235: error-path cross-surface consistency. Pre-iter-235 only
/// the Ok path was pinned — a regression that wrapped the tool's
/// error string on one surface (e.g., "Error: invalid hex" on MCP
/// while HTTP kept the bare "invalid hex") would have shipped silently.
/// The text inside each surface's error envelope must match the
/// underlying canonical failure message verbatim.
#[tokio::test]
async fn tool_error_text_is_identical_on_every_surface() {
    let tool_id = "num.hex_to_decimal";
    let bad_args = json!({"input": "0xZZ"});

    let direct_msg = match dispatch_tool(tool_id, &bad_args) {
        Outcome::Failure(failure) => failure.error.message,
        other => panic!("expected ToolError, got {other:?}"),
    };

    // 2. MCP: `result.isError = true`, `result.content[0].text` carries
    //    the same message.
    let mcp_resp = handle_mcp_message(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool_id, "arguments": bad_args },
    }))
    .expect("mcp response");
    assert_eq!(
        mcp_resp["result"]["isError"], true,
        "MCP must mark `isError: true` for ToolError outcomes"
    );
    let mcp_msg = mcp_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
        .to_string();
    assert_eq!(
        mcp_msg, direct_msg,
        "MCP error text must match direct dispatch"
    );

    let http_resp = http_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{tool_id}"))
                .header("content-type", "application/json")
                .body(Body::from(bad_args.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        http_resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "HTTP must surface ToolError as 422 Unprocessable Entity"
    );
    let http_msg = body_to_value(http_resp.into_body()).await["error"]["message"]
        .as_str()
        .expect("error field")
        .to_string();
    assert_eq!(
        http_msg, direct_msg,
        "HTTP error text must match direct dispatch"
    );
}

/// Iter 255: `NotFound` cross-surface consistency. iter-132 enriched
/// the unknown-tool error with `did you mean` hints across CLI/MCP/HTTP,
/// each using `unknown_tool_hint` filtered by the calling surface.
/// Pre-iter-255 only the per-surface tests pinned each one in
/// isolation — a regression that wired one surface to a different
/// hint source (or an unfiltered one) would have shipped silently.
/// Now: dispatch the same unknown id through both protocol-style
/// surfaces, parse out the suggestions from each envelope, and assert
/// the suggested-id sets are identical.
#[tokio::test]
async fn unknown_tool_suggestions_are_identical_across_mcp_and_http() {
    let typo = "num.hex_to_de"; // truncated — near hex_to_decimal

    // 1. Direct dispatch returns NotFound (no message — the surface
    //    builds the user-facing text).
    let outcome = dispatch_tool(typo, &json!({}));
    assert!(
        matches!(outcome, Outcome::NotFound),
        "typo'd id must surface as NotFound from direct dispatch; got {outcome:?}"
    );

    // 2. MCP `tools/call` returns -32601 with the message embedded.
    let mcp_resp = handle_mcp_message(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": typo, "arguments": {} },
    }))
    .expect("mcp response");
    assert_eq!(
        mcp_resp["error"]["code"], -32601,
        "MCP unknown tool must surface as -32601 Method not found"
    );
    let mcp_msg = mcp_resp["error"]["message"]
        .as_str()
        .expect("message")
        .to_string();
    assert!(
        mcp_msg.contains("did you mean"),
        "MCP must surface the iter-132 hint; got `{mcp_msg}`"
    );

    // 3. HTTP POST /v1/tools/{typo} returns 404 with the message in
    //    `body.error`.
    let http_resp = http_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tools/{typo}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        http_resp.status(),
        StatusCode::NOT_FOUND,
        "HTTP unknown tool must surface as 404"
    );
    let http_msg = body_to_value(http_resp.into_body()).await["error"]["message"]
        .as_str()
        .expect("error field")
        .to_string();
    assert!(
        http_msg.contains("did you mean"),
        "HTTP must surface the iter-132 hint; got `{http_msg}`"
    );

    // 4. Extract the backtick-quoted suggested ids from each message
    //    and assert the two sets are identical. The suggestion format
    //    is `\`<id>\`, \`<id>\`, ...` per `unknown_tool_hint`.
    fn extract_suggestions(msg: &str) -> Vec<String> {
        let after_hint = match msg.find("did you mean ") {
            Some(i) => &msg[i + "did you mean ".len()..],
            None => return vec![],
        };
        // Scan for backtick-wrapped tokens.
        let mut out = Vec::new();
        let mut in_tick = false;
        let mut acc = String::new();
        for ch in after_hint.chars() {
            if ch == '`' {
                if in_tick && !acc.is_empty() {
                    out.push(std::mem::take(&mut acc));
                }
                in_tick = !in_tick;
            } else if in_tick {
                acc.push(ch);
            }
        }
        out
    }
    let mcp_suggestions = extract_suggestions(&mcp_msg);
    let http_suggestions = extract_suggestions(&http_msg);
    assert!(
        !mcp_suggestions.is_empty(),
        "MCP suggestion list must be non-empty for a near-miss typo"
    );
    assert_eq!(
        mcp_suggestions, http_suggestions,
        "MCP and HTTP must surface identical suggestion lists for the same typo"
    );
    // Sanity: the obvious correction must appear.
    assert!(
        mcp_suggestions.iter().any(|s| s == "num.hex_to_decimal"),
        "expected `num.hex_to_decimal` in suggestions for `{typo}` typo; got {mcp_suggestions:?}"
    );
}

/// Same idea but for a TOML-loaded External tool. Confirms that runtime
/// dispatchers (registered via `register_runtime_dispatcher`) are
/// dispatched identically by every surface — there's no path that
/// accidentally checks `inventory` only and skips runtime entries.
#[tokio::test]
async fn an_external_toml_tool_produces_consistent_results_on_every_surface() {
    use upeg_loader::load_and_register_dir_verbose;
    // Drop a TOML External tool into a temp dir and load it.
    let dir = std::env::temp_dir().join("upeg_iter64_external");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "iter64.external.echo";
    std::fs::write(
        dir.join("echo.toml"),
        r#"id = "iter64"

[[tools]]
id = "external.echo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["[%s]", "{tag}"]

[[tools.inputs]]
name = "tag"
type = "string""#,
    )
    .unwrap();
    let outcome = load_and_register_dir_verbose(&dir);
    assert_eq!(outcome.loaded.len(), 1, "TOML failed: {:?}", outcome.failed);

    // Dispatch through every surface. printf wraps `tag` in brackets, so
    // we get `[hi]` regardless of which surface we route through.
    let (direct, via_mcp, via_http) = dispatch_three_ways(id, json!({"tag": "hi"})).await;
    assert!(direct.contains("[hi]"), "got: {direct:?}");
    assert_eq!(direct, via_mcp);
    assert_eq!(direct, via_http);

    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Board enumeration parity (CLI / HTTP / MCP) ───────────────────
//
// A board's listing used to be reimplemented per surface:
// `upeg-cli/src/app/board_scope.rs`'s `format_board_pins` skipped the
// `is_on_surface` filter entirely, so `upeg board <b> list --json`
// could list a pin (e.g. the GUI-only `memo.scratch`) that
// `upeg board <b> call` could never dispatch. All three surfaces now
// route through `upeg_sources::pegboard::board_entries_on_surface_in`,
// the single enumeration entry point. These tests seed one shared
// PegboardState via a scratch `UPEG_HOME` and assert the three
// surfaces list the same *set of ids* for it.

/// Serializes tests in this file that point `UPEG_HOME` at a scratch
/// pegboard store. `upeg_cli::test_support::with_seeded_pegboard_home`
/// is the in-crate equivalent, but it's `#[cfg(test)] pub(crate)` —
/// unreachable from this external integration-test crate — so this
/// file carries its own copy of the same lock-a-scratch-home pattern.
fn board_home_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// RAII scratch `UPEG_HOME`: seeds a fresh pegboard store (the shared
/// default boards plus whatever `build` adds) and points `UPEG_HOME`
/// at it; restores the previous value and removes the scratch dir on
/// drop. Callers must hold `board_home_lock` for the guard's whole
/// lifetime — the env var is process-global.
struct ScratchPegboardHome {
    dir: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl ScratchPegboardHome {
    #[allow(
        unsafe_code,
        reason = "std::env::set_var is unsafe since edition 2024; every caller holds \
                  `board_home_lock` for the whole guard lifetime, so no other test in \
                  this binary can observe the scratch value"
    )]
    fn seed(label: &str, build: impl FnOnce(&mut upeg_sources::pegboard::PegboardState)) -> Self {
        let dir =
            std::env::temp_dir().join(format!("upeg-board-parity-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch UPEG_HOME");

        let mut state = upeg_sources::pegboard::default_state();
        build(&mut state);
        let store = upeg_sources::pegboard::state_path_from_root(&dir);
        upeg_sources::pegboard::save_state_to_path(&store, &state).expect("seed pegboard store");

        let previous = std::env::var_os(upeg_core::paths::env::UPEG_HOME);
        unsafe {
            std::env::set_var(upeg_core::paths::env::UPEG_HOME, &dir);
        }
        Self { dir, previous }
    }
}

impl Drop for ScratchPegboardHome {
    #[allow(
        unsafe_code,
        reason = "std::env::set_var/remove_var are unsafe since edition 2024; restoring the \
                  pre-guard value here is the counterpart of `seed`'s set, done under the \
                  same `board_home_lock` hold"
    )]
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe {
                std::env::set_var(upeg_core::paths::env::UPEG_HOME, value);
            },
            None => unsafe {
                std::env::remove_var(upeg_core::paths::env::UPEG_HOME);
            },
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `upeg board <board> list --json` entry ids, run in-process through
/// `upeg_cli::run` (the same entry point `main.rs` calls) so this test
/// doesn't need to spawn the binary.
fn cli_board_list_ids(board: &str) -> Vec<String> {
    let cli = Cli::parse_from(["upeg", "board", board, "list", "--json"]);
    let out = run(cli).expect("cli board list --json");
    let entries: Value = serde_json::from_str(&out).expect("json array");
    entries
        .as_array()
        .expect("array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name").to_string())
        .collect()
}

/// `GET /v1/boards/{board}` tool ids.
async fn http_board_show_ids(board: &str) -> Vec<String> {
    let resp = http_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/boards/{board}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET /v1/boards/{board}");
    let body = body_to_value(resp.into_body()).await;
    body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name").to_string())
        .collect()
}

/// Board-scoped MCP pinned execution ids. The separate guidance tool is
/// asserted here and excluded from the cross-surface pin comparison.
fn mcp_board_pin_ids(board: &str) -> Vec<String> {
    let board_key = BoardKey::parse(board).expect("board key");
    let resp = handle_mcp_message_with_board(
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        Surface::Mcp,
        Some(&board_key),
    )
    .expect("mcp response owed");
    let mut ids: Vec<String> = resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("name").to_string())
        .collect();
    assert_eq!(
        ids.iter().filter(|id| *id == BOARD_CONTEXT_TOOL).count(),
        1,
        "MCP offers the board-guidance lookup exactly once, separate from the pins",
    );
    ids.retain(|id| id != BOARD_CONTEXT_TOOL);
    ids
}

#[tokio::test]
async fn board_listings_exclude_pins_not_on_that_surface_on_every_surface() {
    let _lock = board_home_lock().lock().await;
    let _home = ScratchPegboardHome::seed("mixed", |state| {
        state.boards.push(upeg_sources::pegboard::BoardData {
            guidance: upeg_core::BoardGuidance::default(),
            key: "parity-mixed".into(),
            title: "Parity Mixed".into(),
        });
        state.layouts.insert(
            "parity-mixed".into(),
            vec![
                // ALL_SURFACES — every surface must list it.
                upeg_core::Placement::new("num.hex_to_decimal", 0, 0),
                // GUI_SURFACES only (Desktop/Pwa/Ext) — no Cli/Mcp/Http.
                // This is the exact bug: the CLI copy used to list it
                // anyway because it skipped the `is_on_surface` filter.
                upeg_core::Placement::new("memo.scratch", 1, 0),
            ],
        );
    });

    let cli_ids = cli_board_list_ids("parity-mixed");
    let http_ids = http_board_show_ids("parity-mixed").await;
    let mcp_ids = mcp_board_pin_ids("parity-mixed");

    for (surface_name, ids) in [("cli", &cli_ids), ("http", &http_ids), ("mcp", &mcp_ids)] {
        assert!(
            ids.iter().any(|id| id == "num.hex_to_decimal"),
            "{surface_name} must list the all-surface pin: {ids:?}"
        );
        assert!(
            !ids.iter().any(|id| id == "memo.scratch"),
            "{surface_name} must exclude the GUI-only pin: {ids:?}"
        );
    }
}

#[tokio::test]
async fn a_pin_on_every_surface_comes_out_as_the_same_id_set_on_all_three_surfaces() {
    let _lock = board_home_lock().lock().await;
    let _home = ScratchPegboardHome::seed("uniform", |state| {
        state.boards.push(upeg_sources::pegboard::BoardData {
            guidance: upeg_core::BoardGuidance::default(),
            key: "parity-uniform".into(),
            title: "Parity Uniform".into(),
        });
        state.layouts.insert(
            "parity-uniform".into(),
            vec![
                upeg_core::Placement::new("num.hex_to_decimal", 0, 0),
                upeg_core::Placement::new("text.diff", 1, 0),
            ],
        );
    });

    let mut cli_ids = cli_board_list_ids("parity-uniform");
    let mut http_ids = http_board_show_ids("parity-uniform").await;
    let mut mcp_ids = mcp_board_pin_ids("parity-uniform");
    cli_ids.sort();
    http_ids.sort();
    mcp_ids.sort();

    let expected = vec!["num.hex_to_decimal".to_string(), "text.diff".to_string()];
    assert_eq!(cli_ids, expected, "CLI id set drifted");
    assert_eq!(cli_ids, http_ids, "CLI/HTTP id sets differ");
    assert_eq!(cli_ids, mcp_ids, "CLI/MCP id sets differ");
}

// ─── Board editing parity: `upeg board <b> pin|unpin` ──────────────
//
// The headless half of the pin lifecycle. `upeg board <b> pin` writes
// the same shared store Desktop's `p` key writes, so a CLI pin has to
// show up in HTTP `/v1/boards/{b}` and board-scoped MCP `tools/list`
// with no further ceremony — that is the whole point of the backlog
// item it closes. These tests drive the CLI through `upeg_cli::run`
// (the entry point `main.rs` calls) and then read the other two
// surfaces in-process.

/// `upeg board <board> <verb> <tool> [args…]` through `upeg_cli::run`.
fn cli_board_edit(board: &str, verb: &str, tool_id: &str, extra: &[&str]) -> String {
    let mut argv = vec!["upeg", "board", board, verb, tool_id];
    argv.extend_from_slice(extra);
    let cli = Cli::parse_from(argv);
    run(cli).unwrap_or_else(|e| panic!("upeg board {board} {verb} {tool_id}: {e:?}"))
}

#[tokio::test]
async fn a_cli_pin_shows_up_verbatim_in_the_http_and_mcp_board_listings() {
    let _lock = board_home_lock().lock().await;
    let _home = ScratchPegboardHome::seed("cli-pin", |state| {
        state.boards.push(upeg_sources::pegboard::BoardData {
            guidance: upeg_core::BoardGuidance::default(),
            key: "cli-pin".into(),
            title: "CLI Pin".into(),
        });
        state.layouts.insert("cli-pin".into(), Vec::new());
    });

    assert!(
        cli_board_list_ids("cli-pin").is_empty(),
        "starts with an empty board"
    );

    let out = cli_board_edit("cli-pin", "pin", "num.hex_to_decimal", &["--at", "1,2"]);
    assert!(out.contains("pinned"), "pin output: {out}");

    let expected = vec!["num.hex_to_decimal".to_string()];
    assert_eq!(cli_board_list_ids("cli-pin"), expected, "CLI listing");
    assert_eq!(
        http_board_show_ids("cli-pin").await,
        expected,
        "HTTP /v1/boards/{{b}} cannot see the CLI pin"
    );
    assert_eq!(
        mcp_board_pin_ids("cli-pin"),
        expected,
        "MCP board tools/list cannot see the CLI pin"
    );

    let out = cli_board_edit("cli-pin", "unpin", "num.hex_to_decimal", &[]);
    assert!(out.contains("unpinned"), "unpin output: {out}");

    assert!(cli_board_list_ids("cli-pin").is_empty(), "CLI listing");
    assert!(
        http_board_show_ids("cli-pin").await.is_empty(),
        "HTTP does not reflect the unpin"
    );
    assert!(
        mcp_board_pin_ids("cli-pin").is_empty(),
        "MCP does not reflect the unpin"
    );
}

#[tokio::test]
async fn a_cli_pin_coordinates_and_size_are_saved_in_board_state() {
    let _lock = board_home_lock().lock().await;
    let _home = ScratchPegboardHome::seed("cli-pin-geometry", |state| {
        state.boards.push(upeg_sources::pegboard::BoardData {
            guidance: upeg_core::BoardGuidance::default(),
            key: "cli-geo".into(),
            title: "CLI Geometry".into(),
        });
        state.layouts.insert("cli-geo".into(), Vec::new());
    });

    cli_board_edit(
        "cli-geo",
        "pin",
        "num.hex_to_decimal",
        &["--at", "2,1", "--units", "U2"],
    );

    let state = upeg_sources::pegboard::load_state();
    let placement = upeg_sources::pegboard::placement_in(&state, "cli-geo", "num.hex_to_decimal")
        .expect("the pin must be saved");
    assert_eq!((placement.x, placement.y), (1, 2), "--at is <row>,<col>");
    assert_eq!(
        placement.span.map(upeg_core::PinSpan::grid_span),
        Some((2, 1)),
        "--units U2 is a 2×1 span"
    );

    cli_board_edit("cli-geo", "move", "num.hex_to_decimal", &["--at", "0,0"]);
    let state = upeg_sources::pegboard::load_state();
    let placement = upeg_sources::pegboard::placement_in(&state, "cli-geo", "num.hex_to_decimal")
        .expect("the pin must remain");
    assert_eq!((placement.x, placement.y), (0, 0), "coordinates after move");
}

/// RAII holder for the process-global project board scope. Every test
/// that sets it holds `board_home_lock` for the guard's lifetime, and
/// the guard clears the scope on drop so a panic cannot leak project
/// boards into the next test in this binary.
struct ScratchProjectBoards;

impl ScratchProjectBoards {
    fn declare(manifest: &str, id: &str, label: &str) -> Self {
        upeg_runtime::pegboard_project::set_project_board_scope(
            upeg_runtime::pegboard_project::ProjectBoardScope::for_manifest(
                std::path::Path::new(manifest),
                vec![upeg_runtime::pegboard_project::ProjectBoardDecl::new(
                    BoardKey::parse(id).expect("board id"),
                    label.to_string(),
                )],
            ),
        );
        Self
    }
}

impl Drop for ScratchProjectBoards {
    fn drop(&mut self) {
        upeg_runtime::pegboard_project::clear_project_board_scope();
    }
}

#[tokio::test]
async fn a_project_board_is_enumerated_on_all_three_surfaces_and_disappears_outside() {
    let _lock = board_home_lock().lock().await;
    let _home = ScratchPegboardHome::seed("project-board", |_state| {});
    let project =
        ScratchProjectBoards::declare("/scratch/proj/upeg.toml", "scratch-proj", "Scratch");

    cli_board_edit("scratch-proj", "pin", "num.hex_to_decimal", &[]);

    let expected = vec!["num.hex_to_decimal".to_string()];
    assert_eq!(cli_board_list_ids("scratch-proj"), expected, "CLI");
    assert_eq!(http_board_show_ids("scratch-proj").await, expected, "HTTP");
    assert_eq!(mcp_board_pin_ids("scratch-proj"), expected, "MCP");

    // Once the manifest is no longer detected, the board itself disappears.
    drop(project);

    let boards = upeg_sources::pegboard::board_keys_in(&upeg_sources::pegboard::load_state());
    assert!(
        !boards.iter().any(|board| board == "scratch-proj"),
        "must not be enumerated outside the project: {boards:?}"
    );
    let resp = http_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/boards/scratch-proj")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "HTTP must 404 outside the project too"
    );
}
