//! Reproduction + regression tests for the real-world dogfooding bugs
//! fixed here:
//!   - E-5: `"params":null` must be omitted (real SDK servers drop the
//!     frame silently), and `notifications/initialized` must be sent
//!     right after `initialize`.
//!   - E-6: bounded retry + backoff for a permanently failing import.
//!
//! Fake upstream MCP servers are tiny POSIX shell read/printf loops —
//! a real subprocess speaking the real wire format, no network, no
//! mocking. Same lens as `upeg-cli::adapters::mcp_import_tests`'s
//! `write_unix_script`/`unix_script_config` (one layer up, testing the
//! CLI-facing re-export); duplicated here in miniature because these
//! tests exercise the wire-format fix directly, in the crate that owns
//! it.

use super::*;

fn short_timeouts() -> McpTimeouts {
    McpTimeouts {
        initialize: std::time::Duration::from_millis(500),
        tools_list: std::time::Duration::from_millis(500),
        tools_call: std::time::Duration::from_millis(500),
    }
}

#[cfg(unix)]
fn write_script(name: &str, body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "upeg-mcp-import-e5e6-{name}-{}-{}",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create script dir");
    let script = root.join("server.sh");
    std::fs::write(&script, body).expect("write script");
    let mut perms = std::fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("grant exec permission");
    (root, script)
}

#[cfg(unix)]
fn script_config(script: &std::path::Path, extra_args: &[&str]) -> UpstreamConfig {
    let mut args = vec![script.to_str().expect("utf8 path").to_string()];
    args.extend(extra_args.iter().map(|s| (*s).to_string()));
    UpstreamConfig {
        command: "/bin/sh".to_string(),
        args,
        reexport: false,
    }
}

#[cfg(unix)]
#[test]
fn omitted_params_null_lets_tools_list_succeed_on_strict_sdk_style_server() {
    // Real @modelcontextprotocol SDK servers validate the JSON-RPC
    // frame and silently drop a request whose `params` member is
    // literally `null` (as opposed to the member being absent).
    // Before the fix, `tools_list()` always sent `"params":null` — a
    // `json!` object literal always inserts the key it names — so
    // every real server ate the request and `tools/list` timed out
    // (E-5). This fake reproduces that strictness directly: it
    // refuses to answer any request line containing the substring
    // `"params":null`.
    let (root, script) = write_script(
        "params-null-dropped",
        r#"#!/bin/sh
IFS= read -r init
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake","version":"0"}}}'
IFS= read -r notif
IFS= read -r list
case "$list" in
  *'"params":null'*)
    exit 0
    ;;
esac
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"ok"}]}}'
"#,
    );
    let cfg = script_config(&script, &[]);
    let mut server = UpstreamServer::spawn_with_timeouts("strict", &cfg, short_timeouts())
        .expect("initialize must succeed");

    let result = server.tools_list();

    match result {
        Ok(list) => assert_eq!(
            list.decls.len(),
            1,
            "tools/list must succeed even on a strict server"
        ),
        Err(e) => panic!("tools/list only succeeds when `params` is omitted; got {e}"),
    }
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn sends_notifications_initialized_right_after_initialize() {
    // A server enforcing MCP's handshake order only answers
    // `tools/list` once it has seen `notifications/initialized`
    // first. If upeg's client never sends it, this fake exits
    // without responding and `tools_list()` fails — proving the
    // notification really went out, not just that some line did.
    let (root, script) = write_script(
        "notification-order",
        r#"#!/bin/sh
IFS= read -r init
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake","version":"0"}}}'
IFS= read -r maybe_notif
case "$maybe_notif" in
  *'"method":"notifications/initialized"'*) ;;
  *) exit 0 ;;
esac
IFS= read -r list
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"saw notification"}]}}'
"#,
    );
    let cfg = script_config(&script, &[]);
    let mut server = UpstreamServer::spawn_with_timeouts("ordered", &cfg, short_timeouts())
        .expect("initialize must succeed");

    let result = server
        .tools_list()
        .expect("must succeed even on a server that waits for notifications/initialized first");

    assert_eq!(result.decls.len(), 1);
    assert_eq!(result.decls[0].description, "saw notification");
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn spawned_upstream_process_inherits_self_import_marker_env_var() {
    // E-6's recursion guard only works if every spawned upstream
    // subprocess actually carries the marker — assert that end to
    // end through a real spawned process reading its own env, not by
    // introspecting the `Command` builder.
    let (root, script) = write_script(
        "env-marker",
        r#"#!/bin/sh
IFS= read -r init
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake","version":"0"}}}'
IFS= read -r notif
IFS= read -r list
marker="${UPEG_MCP_IMPORT_CHILD:-__UNSET__}"
printf '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"marker","description":"%s"}]}}\n' "$marker"
"#,
    );
    let cfg = script_config(&script, &[]);
    let mut server = UpstreamServer::spawn_with_timeouts("marked", &cfg, short_timeouts())
        .expect("initialize must succeed");

    let list = server.tools_list().expect("tools/list must succeed");

    assert_ne!(
        list.decls[0].description, "__UNSET__",
        "every MCP-import upstream spawned by upeg must carry the self-import marker env var"
    );
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn always_failing_import_is_retried_finitely() {
    // A permanently failing upstream (dead handshake, never
    // responds) must not retry forever. The fake never answers
    // anything — it just appends a marker line to a counter file and
    // exits — so every `register_server` attempt shows up as one
    // line in that file.
    let (root, script) = write_script(
        "always-fails",
        r#"#!/bin/sh
echo x >> "$1"
exit 0
"#,
    );
    let counter = root.join("attempts.log");
    let cfg = script_config(&script, &[counter.to_str().expect("utf8 path")]);

    let result = register_server("failing", &cfg);

    assert!(
        result.is_err(),
        "an unresponsive server must fail the import"
    );
    let attempts = std::fs::read_to_string(&counter)
        .map(|s| s.lines().count())
        .unwrap_or(0);
    assert_eq!(
        attempts,
        spawn_retry::MCP_IMPORT_MAX_SPAWN_ATTEMPTS as usize,
        "a permanently failing import must be attempted exactly the capped number of times (neither once nor forever)"
    );
    let _ = std::fs::remove_dir_all(root);
}
