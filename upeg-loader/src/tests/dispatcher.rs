use super::{
    call_dispatcher, call_dispatcher_result, controlled_embed_backend_test_lock,
    runtime_success_text, single_tool_toml_str, tool_success,
};
use crate::dispatcher::{external_dispatcher_for, http_dispatcher_for, llm_dispatcher_for};
use crate::{ToolToml, load_and_register_dir};
use std::sync::{Arc, Mutex};
use upeg_core::{OutputKind, OutputValue};

struct RestoreNoopControlledEmbedBackend;

impl Drop for RestoreNoopControlledEmbedBackend {
    fn drop(&mut self) {
        upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
            upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
        ));
    }
}

const CONTROLLED_EMBED_UNAVAILABLE_ERROR_CODE: &str = "controlled_embed_unavailable";
const CONTROLLED_EMBED_EXECUTION_ERROR_CODE: &str = "controlled_embed_execution_failed";
const CONTROLLED_EMBED_WAIT_TIMEOUT_ERROR_CODE: &str = "wait-timeout";
const BACKEND_FAILED_MESSAGE: &str = "selector miss: #out";
const WAIT_TIMEOUT_BINDING_SELECTOR: &str = "#out";
const WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
const WAIT_TIMEOUT_MS: u64 = 125;
const LARGE_UNICODE_OUTPUT_ID: &str = "summary";
const LARGE_UNICODE_OUTPUT_CHUNK: &str = "첫 줄\n둘째 줄 🙂";
const LARGE_UNICODE_REPEAT_COUNT: usize = 64;

fn tool_failure(result: upeg_core::ToolResult) -> upeg_core::ToolFailure {
    match result {
        upeg_core::ToolResult::Failure(failure) => failure,
        upeg_core::ToolResult::Success(success) => {
            panic!("expected failure, got success: {success:?}")
        }
    }
}

fn large_unicode_output_text() -> String {
    LARGE_UNICODE_OUTPUT_CHUNK.repeat(LARGE_UNICODE_REPEAT_COUNT)
}

struct BackendFailedControlledEmbedBackend;

impl upeg_runtime::controlled_embed::ControlledEmbedBackend
    for BackendFailedControlledEmbedBackend
{
    fn run(
        &self,
        _request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        Err(
            upeg_runtime::controlled_embed::ControlledEmbedError::BackendFailed(
                BACKEND_FAILED_MESSAGE.into(),
            ),
        )
    }
}

struct WaitTimeoutControlledEmbedBackend;

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for WaitTimeoutControlledEmbedBackend {
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

struct LargeUnicodeOutputBackend;

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for LargeUnicodeOutputBackend {
    fn run(
        &self,
        _request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
            outputs: vec![(LARGE_UNICODE_OUTPUT_ID.into(), large_unicode_output_text())],
        })
    }
}

#[test]
fn non_external_invoker_builds_no_external_dispatcher() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            command = "echo""#,
    )
    .unwrap();
    // command set, but no External invoker — should NOT build a dispatcher.
    assert!(external_dispatcher_for(&parsed, None).is_none());
}

#[test]
fn missing_command_builds_no_external_dispatcher() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External""#,
    )
    .unwrap();
    assert!(external_dispatcher_for(&parsed, None).is_none());
}

#[test]
fn external_dispatcher_runs_echo() {
    // `echo hello` is universally available on Unix; this test is the
    // smoke check for the external-invoker plumbing.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["hello"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({})).expect("echo runs");
    assert!(out.contains("hello"), "got stdout: {out:?}");
}

#[test]
fn external_dispatcher_normalizes_stdout_into_declared_primary_output() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "printf"
            args_template = ["42"]
            primary_output_id = "answer"
            outputs = [{ name = "answer", type = "integer", label = "Answer" }]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(success.primary_output_id.as_deref(), Some("answer"));
    assert_eq!(success.outputs.len(), 1);
    assert_eq!(success.outputs[0].id, "answer");
    assert_eq!(success.outputs[0].label.as_deref(), Some("Answer"));
    assert!(matches!(success.outputs[0].kind, OutputKind::Integer));
    assert_eq!(success.outputs[0].value, OutputValue::Integer(42));
}

#[test]
fn external_dispatcher_substitutes_whole_token_placeholders() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{greeting}", "world"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"greeting": "hi"})).expect("ok");
    assert!(out.contains("hi"));
    assert!(out.contains("world"));
}

#[test]
fn external_dispatcher_substitutes_missing_key_as_empty_string() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{absent}", "after"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({})).expect("ok");
    // `echo "" after` → ` after\n` (with leading space). Just assert
    // `after` appears and nothing about the missing slot survived.
    assert!(out.contains("after"));
    assert!(!out.contains("{absent}"));
}

#[cfg(feature = "wasm")]
#[test]
fn declarative_wasm_dispatcher_loads_and_runs_module_tool() {
    let dir = std::env::temp_dir().join(format!("upeg_loader_wasm_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../upeg-wasm/tests/fixtures/test_plugin.wasm")
        .canonicalize()
        .unwrap();
    std::fs::write(
        dir.join("test.toml"),
        format!(
            r#"
id = "test"
tags = ["wasm"]

[[tools]]
id = "wasm.echo"
pegboard_units = "U1"
invoker = "Wasm"
wasm_path = "{}"
"#,
            wasm.display()
        ),
    )
    .unwrap();

    let loaded = crate::load_and_register_dir_verbose(&dir);
    assert_eq!(loaded.failed.len(), 0, "{:?}", loaded.failed);
    assert_eq!(loaded.loaded, vec!["test.wasm.echo"]);
    let out = upeg_runtime::try_runtime_dispatch(
        "test.wasm.echo",
        &serde_json::json!({ "input": "from declarative wasm" }),
    );
    let out = runtime_success_text(out);
    assert_eq!(out, "echoed: from declarative wasm");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression for the lazy `Wasm` dispatcher self-recursion hazard: a
/// toolkit TOML whose canonical tool id doesn't match any id the wasm
/// module's own manifest declares must fail cleanly, not recurse into
/// itself. Root cause: `wasm_dispatcher_for`'s lazy closure is
/// registered under the toolkit's canonical id (`mismatch.wasm.nope`
/// below) *before* the module ever loads; if the freshly-loaded module
/// registers different ids (`test.wasm.echo` / `test.wasm.shout`, from
/// the shared fixture), the registry entry for `mismatch.wasm.nope`
/// still points at this very closure, so re-dispatching by that id
/// would call the closure again — forever — absent the fix.
#[cfg(feature = "wasm")]
#[test]
fn wasm_dispatcher_with_mismatched_tool_id_errors_without_recursion() {
    let dir = std::env::temp_dir().join(format!(
        "upeg_loader_wasm_id_mismatch_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../upeg-wasm/tests/fixtures/test_plugin.wasm")
        .canonicalize()
        .unwrap();
    std::fs::write(
        dir.join("mismatch.toml"),
        format!(
            r#"
id = "mismatch"
tags = ["wasm"]

[[tools]]
id = "wasm.nope"
pegboard_units = "U1"
invoker = "Wasm"
wasm_path = "{}"
"#,
            wasm.display()
        ),
    )
    .unwrap();

    let loaded = crate::load_and_register_dir_verbose(&dir);
    assert_eq!(loaded.failed.len(), 0, "{:?}", loaded.failed);
    assert_eq!(loaded.loaded, vec!["mismatch.wasm.nope"]);

    // First dispatch triggers the lazy wasm load; the module registers
    // ids other than `mismatch.wasm.nope`, so this must return a clear
    // error instead of hanging/overflowing the stack.
    let first = upeg_runtime::try_runtime_dispatch(
        "mismatch.wasm.nope",
        &serde_json::json!({ "input": "hi" }),
    );
    let first_message = super::runtime_failure_message(first);
    assert!(
        first_message.contains("mismatch.wasm.nope"),
        "{first_message}"
    );
    assert!(first_message.contains("test.wasm.echo"), "{first_message}");
    assert!(first_message.contains("test.wasm.shout"), "{first_message}");

    // Second dispatch, after the module is already loaded: must still
    // error promptly from the cached mismatch state, not recurse into
    // the lazy closure a second time.
    let second = upeg_runtime::try_runtime_dispatch(
        "mismatch.wasm.nope",
        &serde_json::json!({ "input": "hi again" }),
    );
    let second_message = super::runtime_failure_message(second);
    assert_eq!(second_message, first_message);

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(feature = "wasm"))]
#[test]
fn wasm_dispatcher_reports_disabled_feature_in_default_build() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "Wasm"
            wasm_path = "/tmp/plugin.wasm""#,
    )
    .unwrap();
    let f = crate::dispatcher::wasm_dispatcher_for(&parsed).expect("dispatcher built");
    let err =
        call_dispatcher(&f, serde_json::json!({})).expect_err("default build should not host wasm");
    assert!(err.contains("requires the `wasm` cargo feature"), "{err}");
    assert!(err.contains("/tmp/plugin.wasm"), "{err}");
    assert!(
        err.contains("rebuild with --features wasm-plugin"),
        "stub error must point at the CLI-facing rebuild flag; got: {err}"
    );
}

mod controlled_embed;
mod external_contract;

#[test]
fn external_dispatcher_trims_whitespace_inside_braces() {
    // Iter 247: a tool author's typo `args_template = ["{ input }"]`
    // (whitespace inside braces) used to extract ` input ` as the
    // lookup key, args.get(" input ") returned None, and the
    // substitution silently became "" — the spawned command got
    // an empty arg where the user's input was supposed to land.
    // Same iter-241 trim spirit applied to the template lookup.
    for template_token in [
        r#""{ input }""#,
        r#""{ input}""#,
        r#""{input }""#,
        r#""{\tinput\n}""#,
    ] {
        let parsed = toml::from_str::<ToolToml>(&format!(
            r#"id = "y.x"
                toolkit = "y"
                invoker = "External"
                command = "echo"
                args_template = [{template_token}]"#,
        ))
        .unwrap();
        let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
        let out = call_dispatcher(&f, serde_json::json!({"input": "hello"})).expect("ok");
        assert!(
            out.contains("hello"),
            "padded `{template_token}` must trim to `input` and substitute the value; got {out:?}"
        );
    }
}

#[test]
fn external_dispatcher_handles_mixed_literal_and_padded_substitutions() {
    // Iter 257: extend iter-247 coverage. The existing iter-247
    // test only exercises single-token templates. Realistic
    // args_templates are mixed — literal flag tokens, padded and
    // unpadded substitution tokens. Pin that the per-token logic
    // composes correctly: literals pass through, padded `{ key }`
    // tokens look up the trimmed key, unpadded `{key}` tokens
    // also look up the trimmed key (parity).
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "printf"
            args_template = ["[%s|%s|%s]", "literal-flag", "{ first }", "{second}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out =
        call_dispatcher(&f, serde_json::json!({"first": "AAA", "second": "BBB"})).expect("ok");
    // printf "[%s|%s|%s]" "literal-flag" "AAA" "BBB" → [literal-flag|AAA|BBB]
    assert!(
        out.contains("[literal-flag|AAA|BBB]"),
        "mixed template (literal + padded sub + unpadded sub) must compose; got {out:?}"
    );
}

#[test]
fn external_dispatcher_stringifies_number_substitution() {
    // Iter 110: pre-iter-110, a Number arg dropped as "" because
    // `v.as_str()` returned None. Now numbers stringify to their
    // JSON form so `-a n=5` → command receives "5".
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{n}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"n": 5})).expect("ok");
    assert!(
        out.contains('5'),
        "Number(5) must substitute as `5`, not be dropped as empty; got {out:?}"
    );
}

#[test]
fn external_dispatcher_stringifies_boolean_substitution() {
    // Same omit-vs-stringify story for booleans. `true` → "true".
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{flag}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"flag": true})).expect("ok");
    assert!(
        out.contains("true"),
        "Bool(true) must substitute as `true`; got {out:?}"
    );
}

#[test]
fn external_dispatcher_substitutes_explicit_null_as_empty() {
    // Null is "no value" — treat the same as missing. Without
    // this carve-out, `null.to_string()` would emit the literal
    // string `"null"` which is rarely what callers want.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["before", "{maybe}", "after"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"maybe": null})).expect("ok");
    // `echo before "" after` → "before  after\n".
    assert!(out.contains("before"));
    assert!(out.contains("after"));
    assert!(
        !out.contains("null"),
        "explicit null must NOT substitute as the literal string `null`; got {out:?}"
    );
}

#[test]
fn external_dispatcher_substitutes_placeholder_inside_token() {
    // `{key}` substitutes anywhere inside a token, so the shapes real
    // CLIs need (`--manifest-path={path}`, `-p{crate}`) are expressible.
    // The old rule required a token to be exactly `{key}` and emitted
    // everything else verbatim, which made those flags impossible.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["prefix-{key}.txt", "{key}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"key": "value"})).expect("ok");
    assert!(
        out.contains("prefix-value.txt"),
        "embedded token must substitute; got {out:?}"
    );
    assert!(
        out.contains("value"),
        "whole-token slot must still substitute alongside; got {out:?}"
    );
}

#[test]
fn external_dispatcher_passes_escaped_braces_through_literally() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{{literal}}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({})).expect("ok");
    assert!(
        out.contains("{literal}"),
        "`{{{{`/`}}}}` must escape to literal braces; got {out:?}"
    );
}

#[test]
fn external_dispatcher_substitutes_arrays_and_objects_as_json_strings() {
    // Iter 110 routed non-string non-Null values through
    // `Value::to_string()`, which for Array/Object emits their
    // JSON form. Pin that contract — useful if the tool's
    // subprocess can parse JSON args (e.g., piping into `jq`).
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{arr}", "{obj}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(
        &f,
        serde_json::json!({
            "arr": [1, 2, 3],
            "obj": {"k": "v"},
        }),
    )
    .expect("ok");
    // serde_json's to_string emits compact JSON for Array/Object.
    assert!(
        out.contains("[1,2,3]"),
        "Array arg must substitute as compact JSON; got {out:?}"
    );
    assert!(
        out.contains("{\"k\":\"v\"}"),
        "Object arg must substitute as compact JSON; got {out:?}"
    );
}

#[test]
fn external_dispatcher_stringifies_negative_and_float_numbers() {
    // Defensive: serde_json's Number to_string covers negative,
    // zero, and decimal forms. Pin it so nobody trims this back
    // to "i64-only" without realizing.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "echo"
            args_template = ["{a}", "{b}", "{c}"]"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    let out = call_dispatcher(
        &f,
        serde_json::json!({
            "a": -7,
            "b": 0,
            "c": 0.5,
        }),
    )
    .expect("ok");
    assert!(out.contains("-7"));
    assert!(out.contains('0'));
    assert!(out.contains("0.5"));
}

#[test]
fn external_dispatcher_returns_clean_error_for_unknown_command() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "definitely_not_a_real_program_zzz_42"
            args_template = []"#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    match call_dispatcher(&f, serde_json::json!({})) {
        Err(msg) => assert!(msg.contains("spawn"), "got: {msg}"),
        Ok(_) => panic!("expected error, got success"),
    }
}

#[test]
fn external_dispatcher_returns_stderr_message_error_for_nonzero_exit() {
    // `false` exits 1 with no stdout/stderr — perfect for verifying the
    // exit-code path without depending on a particular distro.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "y.x"
            toolkit = "y"
            invoker = "External"
            command = "false""#,
    )
    .unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("dispatcher built");
    match call_dispatcher(&f, serde_json::json!({})) {
        Err(msg) => assert!(msg.contains("exited"), "got: {msg}"),
        Ok(_) => panic!("expected non-zero-exit error"),
    }
}

#[test]
fn http_dispatcher_mock_renders_body_template() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "http.echo"
            toolkit = "http"
            invoker = "Http"
            url = "mock://echo"
            body = "hello {{input.name}}""#,
    )
    .unwrap();
    let f = http_dispatcher_for(&parsed).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"input": {"name": "upeg"}})).unwrap();
    assert_eq!(out, "hello upeg");
}

#[test]
fn http_dispatcher_mock_normalizes_response_into_primary_output_type() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "http.echo"
            toolkit = "http"
            invoker = "Http"
            url = "mock://echo"
            body = "true"
            primary_output_id = "ok"
            outputs = [{ name = "ok", type = "boolean" }]"#,
    )
    .unwrap();
    let f = http_dispatcher_for(&parsed).expect("dispatcher built");
    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(success.primary_output_id.as_deref(), Some("ok"));
    assert_eq!(success.outputs[0].value, OutputValue::Boolean(true));
}

#[test]
fn http_dispatcher_rejects_newlines_in_templated_header_value() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "http.header"
            toolkit = "http"
            invoker = "Http"
            url = "mock://echo"
            headers = [{ name = "X-Test", value = "{{input}}" }]"#,
    )
    .unwrap();
    let f = http_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({"input": "ok\r\nInjected: yes"}))
        .expect_err("templated header CRLF rejected");
    assert!(err.contains("HTTP header value"), "{err}");
    assert!(err.contains("CR/LF"), "{err}");
}

#[test]
fn http_dispatcher_accepts_secure_url_via_tls_client() {
    // E8: previously https:// was pinned as "not supported" and rejected
    // before any transport ran. The Http invoker now accepts https and
    // routes it through the TLS client. Point at a closed loopback port so
    // the TLS client is actually reached (no real network) and assert the
    // failure is a transport error from that client — NOT the old early
    // rejection. This replaces the pinned https-rejection limitation test.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "http.secure"
            toolkit = "http"
            invoker = "Http"
            url = "https://127.0.0.1:1/api""#,
    )
    .unwrap();
    let f = http_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({}))
        .expect_err("closed loopback port yields a transport error");
    assert!(
        err.contains("https request failed"),
        "https must reach the TLS client, got: {err}"
    );
    assert!(
        !err.contains("not supported"),
        "https must no longer be early-rejected, got: {err}"
    );
}

#[test]
fn llm_dispatcher_echo_renders_prompt_without_network() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{input}}""#,
    )
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"input": "Rust clean architecture"})).unwrap();
    assert_eq!(out, "Summarize: Rust clean architecture");
}

#[test]
fn llm_dispatcher_echo_normalizes_prompt_into_primary_output() {
    let parsed = toml::from_str::<ToolToml>(
        r##"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "# {{input}}"
            primary_output_id = "summary"
            outputs = [{ name = "summary", type = "markdown" }]"##,
    )
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let success = tool_success(call_dispatcher_result(
        &f,
        serde_json::json!({"input": "Rust"}),
    ));

    assert_eq!(success.primary_output_id.as_deref(), Some("summary"));
    assert_eq!(success.outputs[0].id, "summary");
    assert_eq!(
        success.outputs[0].value,
        OutputValue::Markdown("# Rust".into())
    );
}

#[test]
fn llm_dispatcher_builds_for_empty_tool_provider() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{input}}"
            provider = "tool:  ""#,
    )
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({"input": "Rust"}))
        .expect_err("empty delegate rejected");
    assert!(
        err.contains("requires a non-empty delegate Tool id"),
        "{err}"
    );
}

#[test]
fn llm_dispatcher_points_to_config_path_for_unknown_provider() {
    // `openai` used to be the unconfigured case pinned here; now that
    // `openai` is a real `ProviderKind` variant, this test exercises a
    // provider string that is (and stays) unrecognized.
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{input}}"
            provider = "azure""#,
    )
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({"input": "Rust"}))
        .expect_err("provider not configured");
    assert!(
        err.contains("llm provider `azure` is not configured"),
        "{err}"
    );
    assert!(err.contains("provider = \"tool:<id>\""), "{err}");
    assert!(err.contains("provider = \"openai\""), "{err}");
}

#[test]
#[allow(
    unsafe_code,
    reason = "std::env::set_var/remove_var are unsafe since edition 2024; this test uses a \
              process-id-suffixed env var name unique to this test, so no other thread/test \
              observes a torn read of it"
)]
fn llm_dispatcher_openai_resolves_credential_and_sends_request() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let handle = std::thread::spawn(move || {
        use std::io::{BufRead as _, Read as _, Write as _};

        let (mut stream, _) = listener.accept().expect("accept");
        let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
        let mut header_text = String::new();
        let mut content_length = 0_usize;
        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read request line");
            if bytes == 0 || line == "\r\n" {
                break;
            }
            if let Some(rest) = line
                .to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::to_string)
            {
                content_length = rest.trim().parse().unwrap_or(0);
            }
            header_text.push_str(&line);
        }
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body).expect("read request body");

        let response_body =
            r#"{"choices":[{"message":{"role":"assistant","content":"Rust rocks"}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        (header_text, String::from_utf8(body).expect("utf8 body"))
    });

    let credential_env = format!("UPEG_LLM_DISPATCHER_OPENAI_KEY_{}", std::process::id());
    // SAFETY-equivalent: test-local env var name is unique per process id,
    // so parallel test runs in this binary cannot collide on it.
    unsafe {
        std::env::set_var(&credential_env, "sk-dispatcher-test");
    }

    let parsed = toml::from_str::<ToolToml>(&format!(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{{{input}}}}"
            provider = "openai"
            model = "gpt-test"
            base_url = "http://{addr}"
            credential = "openai_key"
            [[credentials]]
            name = "openai_key"
            env = "{credential_env}""#,
    ))
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let out = call_dispatcher(&f, serde_json::json!({"input": "Rust"})).expect("openai request ok");
    assert_eq!(out, "Rust rocks");

    let (headers, body) = handle.join().expect("mock server thread");
    assert!(
        headers.contains("Authorization: Bearer sk-dispatcher-test"),
        "got headers: {headers}"
    );
    assert!(body.contains("Summarize: Rust"), "got body: {body}");

    unsafe {
        std::env::remove_var(&credential_env);
    }
}

#[test]
fn llm_dispatcher_openai_errors_without_credential() {
    let parsed = toml::from_str::<ToolToml>(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{input}}"
            provider = "openai"
            model = "gpt-test""#,
    )
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({"input": "Rust"}))
        .expect_err("missing credential rejected");
    assert!(err.contains("requires `credential = "), "{err}");
}

#[test]
#[allow(
    unsafe_code,
    reason = "std::env::set_var/remove_var are unsafe since edition 2024; this test uses a \
              process-id-suffixed env var name unique to this test, so no other thread/test \
              observes a torn read of it"
)]
fn llm_dispatcher_openai_errors_on_malformed_response() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let handle = std::thread::spawn(move || {
        use std::io::{BufRead as _, Write as _};

        let (mut stream, _) = listener.accept().expect("accept");
        let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone stream"));
        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).expect("read request line");
            if bytes == 0 || line == "\r\n" {
                break;
            }
        }
        let response_body = "not json";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    let credential_env = format!(
        "UPEG_LLM_DISPATCHER_OPENAI_MALFORMED_{}",
        std::process::id()
    );
    unsafe {
        std::env::set_var(&credential_env, "sk-dispatcher-test");
    }

    let parsed = toml::from_str::<ToolToml>(&format!(
        r#"id = "ai.pattern"
            toolkit = "ai"
            invoker = "Llm"
            prompt = "Summarize: {{{{input}}}}"
            provider = "openai"
            model = "gpt-test"
            base_url = "http://{addr}"
            credential = "openai_key"
            [[credentials]]
            name = "openai_key"
            env = "{credential_env}""#,
    ))
    .unwrap();
    let f = llm_dispatcher_for(&parsed).expect("dispatcher built");
    let err = call_dispatcher(&f, serde_json::json!({"input": "Rust"}))
        .expect_err("malformed response rejected");
    assert!(err.contains("openai response is not valid JSON"), "{err}");

    handle.join().expect("mock server thread");
    unsafe {
        std::env::remove_var(&credential_env);
    }
}

#[test]
fn external_dispatcher_receives_board_project_and_board_env_context() {
    let dir = std::env::temp_dir().join("upeg_loader_external_context");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = "test.external.context_env";
    std::fs::write(
        dir.join("t.toml"),
        single_tool_toml_str(&format!(
            r#"id = "{id}"
toolkit = "test"
invoker = "External"
command = "sh"
args_template = ["-c", "printf '%s|%s|%s|%s' \"$UPEG_BOARD\" \"$UPEG_PROJECT_MANIFEST\" \"$PROFILE\" \"$UPEG_SURFACE\""]"#,
        )),
    )
    .unwrap();

    let (loaded, failed) = load_and_register_dir(&dir);
    assert_eq!((loaded, failed), (1, 0));

    let args = serde_json::json!({
        upeg_core::EXECUTION_CONTEXT_ARG: {
            "board": "dev",
            "projectManifest": "/tmp/project/upeg.toml",
            "surface": "cli",
            "boardEnv": { "PROFILE": "local" }
        }
    });
    let out = runtime_success_text(upeg_runtime::try_runtime_dispatch(id, &args));
    assert_eq!(out, "dev|/tmp/project/upeg.toml|local|cli");

    let _ = std::fs::remove_dir_all(&dir);
}
