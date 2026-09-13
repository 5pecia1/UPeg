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

//! Integration tests for the MCP manager.
//!
//! Uses cargo's `CARGO_BIN_EXE_upeg` env var to find the just-built
//! `upeg` binary, then spawns `upeg mcp` itself as the upstream MCP
//! server. Eat-our-own-dogfood test: anything our MCP server emits, the
//! manager must successfully consume.

use serde_json::json;
use upeg_cli::{UpstreamMcpConfig, UpstreamMcpServer, register_upstream_mcp_server};

/// Path to the just-built `upeg` binary. Cargo sets this when running
/// integration tests in `tests/`.
fn upeg_mcp_config() -> UpstreamMcpConfig {
    UpstreamMcpConfig {
        command: env!("CARGO_BIN_EXE_upeg").to_string(),
        args: vec!["mcp".to_string()],
        reexport: false,
    }
}

#[test]
fn upeg_mcp는_자신을_대상으로_스폰과_목록을_수행한다() {
    let mut server =
        UpstreamMcpServer::spawn("self", &upeg_mcp_config()).expect("spawn `upeg mcp`");
    let listing = server.tools_list().expect("tools/list");
    // Dogfood gate: every tool our own MCP server emits must convert
    // to typed I/O — a skip here means upeg emits schemas its own
    // importer cannot express.
    assert!(
        listing.skipped.is_empty(),
        "own MCP tools must all convert; skipped: {:?}",
        listing.skipped
    );
    let ids: Vec<&str> = listing.decls.iter().map(|d| d.id.as_str()).collect();
    // Our own MCP server exposes the cross-surface built-ins.
    assert!(
        ids.contains(&"num.hex_to_decimal"),
        "expected num.hex_to_decimal, got: {ids:?}"
    );
    assert!(
        ids.contains(&"hash.sha256"),
        "expected hash.sha256 (registered iter 42), got: {ids:?}"
    );
}

#[test]
fn upeg_mcp는_프록시_호출을_통해_자신을_호출한다() {
    let mut server = UpstreamMcpServer::spawn("selfcall", &upeg_mcp_config()).expect("spawn");
    let result = server
        .call("num.hex_to_decimal", &json!({"input": "0xff"}))
        .expect("hex_to_decimal via proxy");
    assert_eq!(result, "255");
}

#[test]
fn 등록된_서버는_네임스페이스가_붙은_dispatch를_받는다() {
    // End-to-end: register `upeg mcp` itself under namespace `upself_iter50`.
    // After registration, `upself_iter50.num.hex_to_decimal` must be in the
    // global registry and dispatch back to the subprocess.
    let server_name = "upself_iter50";
    let outcome = register_upstream_mcp_server(server_name, &upeg_mcp_config()).expect("register");
    assert!(outcome.skipped.is_empty(), "own MCP tools must all convert");
    for id in outcome.registered_ids() {
        assert!(id.starts_with(&format!("{server_name}.")), "got: {id}");
    }
    let ns_id = format!("{server_name}.num.hex_to_decimal");
    assert!(upeg_runtime::toolbox_tool(&ns_id).is_some());

    let r = upeg_runtime::try_runtime_dispatch(&ns_id, &json!({"input": "0x10"}));
    match r.map(upeg_runtime::tool_result_text) {
        Some(Ok(out)) => assert_eq!(out, "16"),
        other => panic!("expected `16`, got {other:?}"),
    }
}

#[test]
fn 등록_서버는_점이_포함된_네임스페이스와_dispatch를_허용한다() {
    let server_name = "upself.iter50";
    let outcome = register_upstream_mcp_server(server_name, &upeg_mcp_config())
        .expect("register dotted namespace");
    for id in outcome.registered_ids() {
        assert!(id.starts_with(&format!("{server_name}.")), "got: {id}");
    }
    let ns_id = format!("{server_name}.num.hex_to_decimal");
    let meta = upeg_runtime::toolbox_tool(&ns_id).expect("dotted namespace tool registered");
    assert_eq!(meta.toolkit_id(), server_name);
    assert_eq!(meta.tool_id(), "num.hex_to_decimal");

    let r = upeg_runtime::try_runtime_dispatch(&ns_id, &json!({"input": "0x11"}));
    match r.map(upeg_runtime::tool_result_text) {
        Some(Ok(out)) => assert_eq!(out, "17"),
        other => panic!("expected `17`, got {other:?}"),
    }
}

#[test]
fn 상위_도구_오류는_dispatcher_오류로_표면화된다() {
    let server_name = "upself_err_iter50";
    let _outcome = register_upstream_mcp_server(server_name, &upeg_mcp_config()).expect("register");
    let ns_id = format!("{server_name}.num.hex_to_decimal");
    let r = upeg_runtime::try_runtime_dispatch(&ns_id, &json!({"input": "0xZZ"}));
    match r.map(upeg_runtime::tool_result_text) {
        Some(Err(msg)) => assert!(msg.contains("invalid hex"), "got: {msg}"),
        other => panic!("expected Err, got {other:?}"),
    }
}

// Fork-bomb regression: pre-fix, spawning `upeg mcp` from a parent that
// itself had `UPEG_MCP_IMPORTS_DIR` set caused the subprocess to auto-load the
// same MCP config and recursively spawn another `upeg mcp`, ad infinitum.
// Fix lives in `UpstreamMcpServer::spawn` (env_remove of upeg's three loader dirs).
// Verified via the bash smoke at iter 50 close — automating this here
// would require mutating process env, which `forbid(unsafe_code)`
// blocks. Logged in the iter 50 message instead.

/// Iter 57: when the subprocess dies mid-session, the next dispatch
/// should respawn from the stored config and retry once. Without the
/// recovery path, the server would be permanently broken until the
/// upeg process restarts.
#[test]
fn dispatch는_하위프로세스_충돌에서_복구한다() {
    let mut server =
        UpstreamMcpServer::spawn("crash_recovery", &upeg_mcp_config()).expect("initial spawn");
    // First call works normally.
    let r1 = server
        .call("num.hex_to_decimal", &json!({"input": "0xa"}))
        .expect("first call");
    assert_eq!(r1, "10");

    // Simulate a crash. The subprocess pipe goes dead.
    server.terminate();

    // Second call must detect death, respawn, retry — succeed.
    let r2 = server
        .call("num.hex_to_decimal", &json!({"input": "0xb"}))
        .expect("call after crash should auto-restart");
    assert_eq!(r2, "11");

    // And subsequent calls should work normally on the fresh subprocess.
    let r3 = server
        .call("num.hex_to_decimal", &json!({"input": "0xff"}))
        .expect("call after restart");
    assert_eq!(r3, "255");
}
