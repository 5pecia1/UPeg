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
    std::fs::create_dir_all(&root).expect("스크립트 디렉터리 생성");
    let script = root.join("server.sh");
    std::fs::write(&script, body).expect("스크립트 작성");
    let mut perms = std::fs::metadata(&script)
        .expect("스크립트 메타데이터")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("실행 권한 부여");
    (root, script)
}

#[cfg(unix)]
fn script_config(script: &std::path::Path, extra_args: &[&str]) -> UpstreamConfig {
    let mut args = vec![script.to_str().expect("utf8 경로").to_string()];
    args.extend(extra_args.iter().map(|s| (*s).to_string()));
    UpstreamConfig {
        command: "/bin/sh".to_string(),
        args,
        reexport: false,
    }
}

#[cfg(unix)]
#[test]
fn params_null은_생략되어_엄격한_sdk_스타일_서버에서도_tools_list가_성공한다() {
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
        .expect("initialize는 성공해야 한다");

    let result = server.tools_list();

    match result {
        Ok(list) => assert_eq!(
            list.decls.len(),
            1,
            "엄격한 서버라도 tools/list가 성공해야 한다"
        ),
        Err(e) => panic!("`params`가 생략되어야 tools/list가 성공한다; got {e}"),
    }
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn initialize_직후_notifications_initialized_알림을_전송한다() {
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
        .expect("initialize는 성공해야 한다");

    let result = server
        .tools_list()
        .expect("notifications/initialized를 먼저 봐야 응답하는 서버에서도 성공해야 한다");

    assert_eq!(result.decls.len(), 1);
    assert_eq!(result.decls[0].description, "saw notification");
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn 스폰된_업스트림_프로세스는_자기임포트_마커_환경변수를_물려받는다() {
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
        .expect("initialize는 성공해야 한다");

    let list = server.tools_list().expect("tools/list는 성공해야 한다");

    assert_ne!(
        list.decls[0].description, "__UNSET__",
        "upeg이 spawn하는 모든 MCP-import 업스트림은 자기임포트 마커 환경변수를 가져야 한다"
    );
    drop(server);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn 항상_실패하는_임포트는_유한_횟수만_재시도된다() {
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
    let cfg = script_config(&script, &[counter.to_str().expect("utf8 경로")]);

    let result = register_server("failing", &cfg);

    assert!(result.is_err(), "응답 없는 서버는 임포트가 실패해야 한다");
    let attempts = std::fs::read_to_string(&counter)
        .map(|s| s.lines().count())
        .unwrap_or(0);
    assert_eq!(
        attempts,
        spawn_retry::MCP_IMPORT_MAX_SPAWN_ATTEMPTS as usize,
        "영구적으로 실패하는 임포트는 정확히 상한 횟수만큼만 시도되어야 한다 (1회도, 무한도 아님)"
    );
    let _ = std::fs::remove_dir_all(root);
}
