#![cfg(all(feature = "controlled-embed", not(target_arch = "wasm32")))]
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

//! E2E for the Controlled Embed example and wait contract — split out of
//! `examples_validate`. Every test here requires the `controlled-embed`
//! feature and a real system browser (headless Chrome/Chromium/Edge), so
//! cfg-gating the whole file reads better than repeating the same `#[cfg]`
//! per item and keeps the per-file budget (1000 lines) too.

use std::path::PathBuf;
use std::sync::Arc;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde_json::json;
use upeg_cli::{Outcome, dispatch_tool};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `upeg-cli/`; walk up one to the repo root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli has a parent (the workspace root)")
        .to_path_buf()
}

const WAIT_TIMEOUT_BINDING_SELECTOR: &str = "#result";
const WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
const WAIT_TIMEOUT_MS: u64 = 7;
const WAIT_SEMANTICS_FIXTURE_RELATIVE_PATH: &str =
    "upeg-cli/tests/fixtures/controlled_embed_wait_semantics.html";
const WAIT_SEMANTICS_HIDDEN_SELECTORS: &str = "#hidden, #hidden-vis, #zero-opacity, #zero-size";
const WAIT_SEMANTICS_MULTI_SELECTOR: &str = ".multi";
const WAIT_SEMANTICS_VISIBLE_SELECTOR: &str = "#visible";
const WAIT_SEMANTICS_TIMEOUT_MS: u64 = 0;
const INVALID_CSS_SELECTOR: &str = "[";
const CONTROLLED_EMBED_EXECUTION_FAILED_CODE: &str = "controlled_embed_execution_failed";
fn controlled_embed_backend_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
struct RestoreNoopControlledEmbedBackend;
impl Drop for RestoreNoopControlledEmbedBackend {
    fn drop(&mut self) {
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
        ));
    }
}
struct WaitTimeoutFixtureBackend;
impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitTimeoutFixtureBackend {
    fn run(
        &self,
        _request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        Err(
            upeg_runtime::controlled_embed::ControlledEmbedError::WaitTimeout {
                role: upeg_core::BindingRole::Output,
                selector: WAIT_TIMEOUT_BINDING_SELECTOR.into(),
                for_selector: WAIT_TIMEOUT_FOR_SELECTOR.into(),
                condition: upeg_core::BindingWaitCondition::Visible,
                timeout_ms: WAIT_TIMEOUT_MS,
            },
        )
    }
}
fn spawn_enter_navigation_fixture_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
    listener
        .set_nonblocking(true)
        .expect("set fixture server nonblocking");
    let addr = listener.local_addr().expect("fixture server local addr");
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0_u8; 2048];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let body = if request.starts_with("GET /done ") {
                        r#"<!doctype html><title>done</title><div id="out">navigated</div>"#
                    } else {
                        r#"<!doctype html><title>start</title><input id="q"><script>document.getElementById("q").addEventListener("keydown",function(e){if(e.key==="Enter"){location.href="/done";}});</script>"#
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    format!("http://{addr}/")
}
/// Browser-dependent E2E guard. Headless controlled-embed tests drive a real
/// Chrome/Chromium/Edge; on hosts without one (bare CI, minimal containers)
/// they skip gracefully with a clear stderr note instead of panicking. Returns
/// `true` when the caller should `return` early.
fn controlled_embed_browser_missing(test_name: &str) -> bool {
    if upeg_runtime::controlled_embed::discover_system_browser().is_ok() {
        return false;
    }
    eprintln!(
        "SKIP {test_name}: no Chrome/Chromium/Edge browser available on this host; \
         headless controlled-embed E2E requires a system browser"
    );
    true
}
fn install_headless_controlled_embed_backend() -> RestoreNoopControlledEmbedBackend {
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        upeg_runtime::controlled_embed_headless::HeadlessControlledEmbedBackend::new(),
    ));
    RestoreNoopControlledEmbedBackend
}
fn controlled_embed_wait_semantics_url() -> String {
    let fixture_path = workspace_root().join(WAIT_SEMANTICS_FIXTURE_RELATIVE_PATH);
    assert!(
        fixture_path.is_file(),
        "wait semantics fixture must exist at {}",
        fixture_path.display()
    );
    format!("file://{}", fixture_path.display())
}
fn prepare_controlled_embed_test_dir(name: &str) -> PathBuf {
    let dir = workspace_root().join("target/test-tmp").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create controlled embed test toolkit dir");
    dir
}
fn write_wait_semantics_toolkit(
    dir: &std::path::Path,
    toolkit_id: &str,
    tool_id: &str,
    output_selector: &str,
    wait_for_selector: &str,
    condition: &str,
    timeout_ms: u64,
) -> String {
    let embed_url = controlled_embed_wait_semantics_url();
    std::fs::write(
        dir.join(format!("{tool_id}.toml")),
        format!(
            r#"id = "{toolkit_id}"

[[tools]]
id = "{tool_id}"
description = "Controlled Embed wait selector semantics fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "{embed_url}"
surfaces = ["cli"]
primary_output_id = "result"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "{WAIT_SEMANTICS_VISIBLE_SELECTOR}"

[[tools.controlled_embed.bindings]]
role = "output"
field = "result"
selector = "{output_selector}"
wait = {{ for_selector = "{wait_for_selector}", condition = "{condition}", timeout_ms = {timeout_ms}, settle_ms = 0 }}

[[tools.outputs]]
name = "result"
type = "string"
"#
        ),
    )
    .expect("write wait semantics toolkit");
    format!("{toolkit_id}.{tool_id}")
}
fn load_controlled_embed_test_dir(dir: &std::path::Path) {
    let outcome = upeg_loader::load_and_register_dir_verbose(dir);
    assert!(
        outcome.failed.is_empty(),
        "controlled embed fixture must load: {:?}",
        outcome.failed
    );
}
fn assert_controlled_embed_primary_text(tool_id: &str, expected: &str) {
    match dispatch_tool(tool_id, &json!({})) {
        Outcome::Success(success) => assert_eq!(
            upeg_runtime::tool_success_primary_text(&success),
            expected,
            "expected controlled embed primary output for {tool_id}"
        ),
        other => panic!("controlled embed dispatch should succeed for {tool_id}: {other:?}"),
    }
}
fn controlled_embed_failure_message(tool_id: &str, expected_code: &str) -> String {
    match dispatch_tool(tool_id, &json!({})) {
        Outcome::Failure(failure) => {
            assert_eq!(failure.error.code, expected_code);
            failure.error.message
        }
        other => panic!("controlled embed dispatch should fail for {tool_id}: {other:?}"),
    }
}
#[test]
fn controlled_embed_reads_output_after_enter_navigation() {
    if controlled_embed_browser_missing("controlled_embed_reads_output_after_enter_navigation") {
        return;
    }
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        upeg_runtime::controlled_embed_headless::HeadlessControlledEmbedBackend::new(),
    ));

    let dir = workspace_root().join("target/test-tmp/controlled-embed-navigation");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test toolkit dir");

    let source_url = spawn_enter_navigation_fixture_server();

    std::fs::write(
        dir.join("enter-navigation.toml"),
        format!(
            r##"id = "nav"

[[tools]]
id = "enter"
description = "Enter navigation fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "{source_url}"
surfaces = ["cli"]
primary_output_id = "result"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "#q"
action = "enter"

[[tools.controlled_embed.bindings]]
role = "output"
field = "result"
selector = "#out"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "result"
type = "string"
"##
        ),
    )
    .expect("write test toolkit");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "navigation fixture must load: {:?}",
        outcome.failed
    );

    match dispatch_tool("nav.enter", &json!({"query": "upeg"})) {
        Outcome::Success(success) => assert_eq!(
            upeg_runtime::tool_success_primary_text(&success),
            "navigated",
            "expected navigation primary output"
        ),
        other => panic!("navigation dispatch failed: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn controlled_embed_delivers_the_wait_timeout_error_code() {
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = RestoreNoopControlledEmbedBackend;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        WaitTimeoutFixtureBackend,
    ));

    let dir = workspace_root().join("target/test-tmp/controlled-embed-wait-timeout");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test toolkit dir");

    std::fs::write(
        dir.join("wait-timeout.toml"),
        r##"id = "wait"

[[tools]]
id = "timeout"
description = "Wait timeout fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "data:text/html,<div id='fixture'></div>"
surfaces = ["cli"]
primary_output_id = "result"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#query"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "#go"

[[tools.controlled_embed.bindings]]
role = "output"
field = "result"
selector = "#result"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "result"
type = "string"
"##,
    )
    .expect("write test toolkit");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "wait-timeout fixture must load: {:?}",
        outcome.failed
    );

    match dispatch_tool("wait.timeout", &json!({"query": "upeg"})) {
        Outcome::Failure(failure) => {
            assert_eq!(failure.error.code, "wait-timeout");
            assert!(
                failure
                    .error
                    .message
                    .contains(WAIT_TIMEOUT_BINDING_SELECTOR)
            );
            assert!(failure.error.message.contains(WAIT_TIMEOUT_FOR_SELECTOR));
            assert!(failure.error.message.contains(&WAIT_TIMEOUT_MS.to_string()));
        }
        other => panic!("wait timeout dispatch should fail canonically: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn controlled_embed_distinguishes_exists_from_visible_by_real_dom_visibility() {
    if controlled_embed_browser_missing(
        "controlled_embed_distinguishes_exists_from_visible_by_real_dom_visibility",
    ) {
        return;
    }
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = install_headless_controlled_embed_backend();

    let dir = prepare_controlled_embed_test_dir("controlled-embed-wait-semantics-hidden");
    let exists_tool_id = write_wait_semantics_toolkit(
        &dir,
        "wait_semantics_hidden_exists",
        "hidden_exists",
        "#hidden",
        WAIT_SEMANTICS_HIDDEN_SELECTORS,
        "exists",
        WAIT_SEMANTICS_TIMEOUT_MS,
    );
    let visible_tool_id = write_wait_semantics_toolkit(
        &dir,
        "wait_semantics_hidden_visible",
        "hidden_visible",
        WAIT_SEMANTICS_VISIBLE_SELECTOR,
        WAIT_SEMANTICS_HIDDEN_SELECTORS,
        "visible",
        WAIT_SEMANTICS_TIMEOUT_MS,
    );
    load_controlled_embed_test_dir(&dir);

    assert_controlled_embed_primary_text(&exists_tool_id, "ready");
    let message = controlled_embed_failure_message(
        &visible_tool_id,
        upeg_runtime::controlled_embed::CONTROLLED_EMBED_WAIT_TIMEOUT_CODE,
    );
    assert!(message.contains(WAIT_SEMANTICS_HIDDEN_SELECTORS));
    assert!(message.contains(&WAIT_SEMANTICS_TIMEOUT_MS.to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn controlled_embed_visible_succeeds_when_only_one_of_many_matches_is_shown() {
    if controlled_embed_browser_missing(
        "controlled_embed_visible_succeeds_when_only_one_of_many_matches_is_shown",
    ) {
        return;
    }
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = install_headless_controlled_embed_backend();

    let dir = prepare_controlled_embed_test_dir("controlled-embed-wait-semantics-multi");
    let tool_id = write_wait_semantics_toolkit(
        &dir,
        "wait_semantics_multi_visible",
        "one_visible",
        WAIT_SEMANTICS_VISIBLE_SELECTOR,
        WAIT_SEMANTICS_MULTI_SELECTOR,
        "visible",
        WAIT_SEMANTICS_TIMEOUT_MS,
    );
    load_controlled_embed_test_dir(&dir);

    assert_controlled_embed_primary_text(&tool_id, "ready");

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn a_bad_css_selector_in_controlled_embed_wait_options_propagates_as_a_clear_error() {
    if controlled_embed_browser_missing(
        "a_bad_css_selector_in_controlled_embed_wait_options_propagates_as_a_clear_error",
    ) {
        return;
    }
    let _guard = controlled_embed_backend_test_lock().lock().unwrap();
    let _restore = install_headless_controlled_embed_backend();

    let dir = prepare_controlled_embed_test_dir("controlled-embed-wait-semantics-invalid");
    let tool_id = write_wait_semantics_toolkit(
        &dir,
        "wait_semantics_invalid_selector",
        "invalid_selector",
        WAIT_SEMANTICS_VISIBLE_SELECTOR,
        INVALID_CSS_SELECTOR,
        "exists",
        WAIT_SEMANTICS_TIMEOUT_MS,
    );
    load_controlled_embed_test_dir(&dir);

    let message =
        controlled_embed_failure_message(&tool_id, CONTROLLED_EMBED_EXECUTION_FAILED_CODE);
    assert!(message.contains("wait:"));
    assert!(message.contains(INVALID_CSS_SELECTOR));

    let _ = std::fs::remove_dir_all(&dir);
}
