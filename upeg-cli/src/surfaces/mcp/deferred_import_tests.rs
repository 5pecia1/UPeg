//! In-process `upeg mcp` lane: conditional + deferred MCP-import load.
//!
//! Two contracts:
//!   - every declaration on the default `reexport` policy → nothing is
//!     loaded at all (imported tools would land on
//!     `ALL_SURFACES_EXCEPT_MCP`, so this surface pays a spawn it can
//!     never use);
//!   - one declaration opting in → the load runs off the request path
//!     and, once `initialize` has been answered, the client is told to
//!     re-read `tools/list`.

use super::*;
use std::sync::{Arc, Mutex};
use upeg_sources::mcp_import::McpReexport;

/// Writer that a test can read back while the loader thread still owns
/// its half — `Vec<u8>` alone can't be shared across the thread
/// boundary the notification is written from.
#[derive(Clone, Default)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    fn contents(&self) -> String {
        let guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        String::from_utf8(guard.clone()).expect("utf-8")
    }
}

impl std::io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn without_reexport_opt_in_imports_are_not_loaded_at_all() {
    let out = SharedBuffer::default();
    let gate = Arc::new(InitializeGate::default());

    let loader = spawn_deferred_import_load(McpReexport::Blocked, Arc::clone(&gate), out.clone());

    assert!(
        loader.is_none(),
        "the loader thread must not even spawn under the Blocked policy"
    );
    gate.open();
    assert!(
        out.contents().is_empty(),
        "no notification may go out either"
    );
}

/// How long the deferred loader gets to finish before the test declares a
/// regression. Generous next to the import layer's own bound (three attempts
/// of a 5 s `initialize` + 5 s `tools/list`), because the point is to fail
/// with a diagnosis instead of hanging a CI run forever.
#[cfg(unix)]
const LOADER_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Hard ceiling on the fake upstream's life. The registration that owns it is
/// leaked on purpose (`retain_for_process_lifetime`), so nothing in-process
/// will ever kill it; this is what keeps a panicking test from leaving a
/// subprocess behind.
#[cfg(unix)]
const FAKE_UPSTREAM_LIFETIME: std::time::Duration = std::time::Duration::from_secs(120);

/// Join `handle`, panicking rather than blocking forever once `timeout`
/// passes. `JoinHandle` has no timed join, so the wait is moved onto a
/// courier thread and observed through a channel.
#[cfg(unix)]
fn join_within(handle: std::thread::JoinHandle<()>, timeout: std::time::Duration) {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("deferred-import-join".to_string())
        .spawn(move || {
            let _ = done_tx.send(handle.join());
        })
        .expect("join courier thread");
    use std::sync::mpsc::RecvTimeoutError;
    match done_rx.recv_timeout(timeout) {
        Ok(Ok(())) => {}
        Ok(Err(payload)) => std::panic::resume_unwind(payload),
        Err(RecvTimeoutError::Timeout) => {
            panic!("loader thread did not finish within {timeout:?}")
        }
        Err(RecvTimeoutError::Disconnected) => {
            panic!("join courier thread vanished without sending a result")
        }
    }
}

/// With a `reexport = true` declaration the load runs in the
/// background, and `tools/list_changed` goes out only after
/// `initialize` has been answered. A fast fake upstream server
/// actually drives the load.
#[cfg(unix)]
#[test]
fn reexport_declaration_loads_then_notifies_list_changed_after_initialize() {
    use std::os::unix::fs::PermissionsExt;

    let _home_guard = crate::test_support::pegboard_home_test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let namespace = format!("upeg_mcp_deferred_{}", std::process::id());
    let root = std::env::temp_dir().join(&namespace);
    let imports = root.join("mcp-imports");
    let script = root.join("server.sh");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&imports).expect("scratch import dir");
    // Two fixture guarantees beyond answering the handshake:
    //   - `exec 2>/dev/null` drops the inherited stderr immediately.
    //     `spawn_child` uses `Stdio::inherit()` for stderr, and this
    //     child is deliberately leaked for the process lifetime
    //     (`retain_for_process_lifetime`) — holding the test binary's
    //     stderr open would keep `cargo test` waiting for EOF long
    //     after the suite finished.
    //   - the watchdog bounds the child's life. Its read loop already
    //     ends at stdin EOF, but a test that panics mid-run can leave
    //     the pipe owned by a leaked registration; nothing should
    //     outlive the suite by more than this.
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
exec 2>/dev/null
(sleep {watchdog}; kill $$) &
IFS= read -r line
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{}}}}'
IFS= read -r line
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"tools":[{{"name":"echo","description":"fixture","inputSchema":{{"type":"object","properties":{{}}}}}}]}}}}'
while IFS= read -r line; do :; done
"#,
            watchdog = FAKE_UPSTREAM_LIFETIME.as_secs(),
        ),
    )
    .expect("fake upstream script");
    let mut perms = std::fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("script is executable");
    std::fs::write(
        imports.join(format!("{namespace}.toml")),
        format!(
            "command = {:?}\nreexport = true\n",
            script.to_str().expect("utf-8 script path")
        ),
    )
    .expect("declaration file");

    let out = SharedBuffer::default();
    let gate = Arc::new(InitializeGate::default());
    // The env override has to outlive the loader thread: the thread
    // reads `RuntimeSourceConfig::from_env()` itself, so restoring the
    // variable at spawn time would race it back to the real directory.
    with_mcp_import_dir(&imports, || {
        let loader =
            spawn_deferred_import_load(McpReexport::OptedIn, Arc::clone(&gate), out.clone())
                .expect("opt-in policy must spawn the loader thread");

        // No notification may go out while the gate is closed — the
        // loader waits for the `initialize` answer.
        assert!(
            out.contents().is_empty(),
            "no notification may go out before initialize"
        );
        gate.open();
        join_within(loader, LOADER_JOIN_TIMEOUT);
    });

    assert_eq!(
        out.contents().trim(),
        json!({ "jsonrpc": JSON_RPC_VERSION, "method": METHOD_TOOLS_LIST_CHANGED }).to_string(),
        "once the load finishes, a single tools/list_changed notification line must go out"
    );
    let imported = format!("{namespace}.echo");
    assert!(
        upeg_runtime::toolbox_tool(&imported).is_some(),
        "the background load must actually register {imported}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Point `UPEG_MCP_IMPORTS_DIR` at `dir` for the duration of `f`.
/// Caller must hold [`crate::test_support::pegboard_home_test_lock`] —
/// this mutates process-global env.
#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "std::env::set_var/remove_var are unsafe since edition 2024; the caller holds the \
              pegboard-home test lock for the whole closure, so no other test observes the swap"
)]
fn with_mcp_import_dir<R>(dir: &std::path::Path, f: impl FnOnce() -> R) -> R {
    let key = upeg_core::paths::env::MCP_IMPORTS_DIR;
    let previous = std::env::var_os(key);
    unsafe {
        std::env::set_var(key, dir);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    match previous {
        Some(value) => unsafe {
            std::env::set_var(key, value);
        },
        None => unsafe {
            std::env::remove_var(key);
        },
    }
    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
