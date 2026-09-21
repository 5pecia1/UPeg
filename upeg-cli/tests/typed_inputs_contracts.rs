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

//! End-to-end contract coverage for typed Tool inputs.
//!
//! `InputSpec` is the canonical model. JSON Schema only appears as the
//! OpenAPI/MCP/HTTP boundary representation generated from that typed spec.

use axum::body::{Body, to_bytes};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use upeg_cli::{Action, Key, Outcome, State, View, dispatch_tool, handle_key, handle_mcp_message};
use upeg_core::{DraftInputValue, InputKind, InputSpec, InputValueError, Surface};
use upeg_loader::{LoadError, parse_toolkit_full};
use upeg_runtime::{tool_json_entries_for_surface, toolbox_tool};

const FIELD_NAMES: &[&str] = &[
    "text", "ratio", "count", "enabled", "mode", "flags", "body", "payload", "when", "path", "site",
];

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFixture {
    dir: PathBuf,
    home: PathBuf,
    toolkit: String,
    tool_id: String,
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

const fn upeg_bin() -> &'static str {
    env!("CARGO_BIN_EXE_upeg")
}

fn unique_toolkit(label: &str) -> String {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("typed_contracts_{label}_{}_{}", std::process::id(), n)
}

fn write_fixture_toolkit(label: &str) -> TempFixture {
    let toolkit = unique_toolkit(label);
    let tool_id = format!("{toolkit}.all");
    let root = std::env::temp_dir().join(&toolkit);
    let dir = root.join("toolkits");
    let home = root.join("home");
    std::fs::create_dir_all(&dir).expect("create fixture toolkit dir");
    std::fs::create_dir_all(&home).expect("create fixture home dir");
    std::fs::write(dir.join("typed.toml"), canonical_toolkit_toml(&toolkit))
        .expect("write typed toolkit fixture");
    TempFixture {
        dir,
        home,
        toolkit,
        tool_id,
    }
}

fn canonical_toolkit_toml(toolkit: &str) -> String {
    format!(
        r#"id = "{toolkit}"
tags = ["contract"]
description = "Typed input contract fixture"

[[tools]]
id = "all"
display_label = "All Inputs"
description = "Every current InputKind"
pegboard_units = "U2"
pin = "Inline"
invoker = "External"
command = "printf"
args_template = ["typed-ok"]
surfaces = ["cli", "tui", "desktop", "pwa", "ext", "mcp", "http"]
inputs = [
  {{ name = "text", type = "string", label = "Text", description = "Plain text", required = true }},
  {{ name = "ratio", type = "number", label = "Ratio", description = "Floating point", required = true }},
  {{ name = "count", type = "integer", label = "Count", description = "Whole number", required = true }},
  {{ name = "enabled", type = "boolean", label = "Enabled", description = "Feature flag", required = true }},
  {{ name = "mode", type = "options", label = "Mode", description = "Single choice", required = true, options = [
    {{ value = "fast", label = "Fast", description = "Fast path" }},
    {{ value = "safe", label = "Safe", description = "Safe path" }},
  ] }},
  {{ name = "flags", type = "multi_options", label = "Flags", description = "Multiple choices", required = true, options = [
    {{ value = "dry", label = "Dry run" }},
    {{ value = "verbose", label = "Verbose" }},
  ] }},
  {{ name = "body", type = "markdown", label = "Body", description = "Markdown text", required = true }},
  {{ name = "payload", type = "json", label = "Payload", description = "JSON value", required = true }},
  {{ name = "when", type = "datetime", label = "When", description = "ISO timestamp", required = true }},
  {{ name = "path", type = "file_path", label = "Path", description = "Filesystem path", required = true }},
  {{ name = "site", type = "url", label = "Site", description = "URL", required = true }},
]
"#,
    )
}

fn minimal_toolkit_toml(toolkit: &str, tool_body: &str) -> String {
    format!(
        r#"id = "{toolkit}"

[[tools]]
id = "bad"
pegboard_units = "U1"
invoker = "External"
command = "printf"
{tool_body}
"#,
    )
}

fn parse_single_tool(toml: &str) -> upeg_core::ToolMeta {
    let (_toolkit, mut tools) = parse_toolkit_full(toml).expect("fixture parses");
    assert_eq!(tools.len(), 1, "fixture should declare one tool");
    tools.remove(0).0
}

fn field_names(spec: &InputSpec) -> Vec<&str> {
    spec.fields
        .iter()
        .map(|field| field.name.as_str())
        .collect()
}

fn schema_property_order(schema: &Value) -> Vec<String> {
    schema["properties"]
        .as_object()
        .expect("schema properties object")
        .keys()
        .cloned()
        .collect()
}

fn schema_required_order(schema: &Value) -> Vec<String> {
    schema["required"]
        .as_array()
        .expect("schema required array")
        .iter()
        .map(|name| name.as_str().expect("required string").to_string())
        .collect()
}

fn assert_schema_preserves_field_order(schema: &Value) {
    let expected: Vec<String> = FIELD_NAMES.iter().map(ToString::to_string).collect();
    assert_eq!(schema_property_order(schema), expected);
    assert_eq!(schema_required_order(schema), expected);
}

fn assert_canonical_input_spec(spec: &InputSpec) {
    assert_eq!(field_names(spec), FIELD_NAMES.to_vec());
    assert_eq!(spec.fields[0].label.as_deref(), Some("Text"));
    assert_eq!(spec.fields[0].description.as_deref(), Some("Plain text"));
    assert!(spec.fields.iter().all(|field| field.required));

    assert!(matches!(spec.fields[0].kind, InputKind::String));
    assert!(matches!(spec.fields[1].kind, InputKind::Number));
    assert!(matches!(spec.fields[2].kind, InputKind::Integer));
    assert!(matches!(spec.fields[3].kind, InputKind::Boolean));
    match &spec.fields[4].kind {
        InputKind::Options(choices) => {
            assert_eq!(
                choices.allowed_values(),
                vec!["fast".to_string(), "safe".to_string()]
            );
            assert_eq!(choices.options[0].label.as_deref(), Some("Fast"));
            assert_eq!(choices.options[0].description.as_deref(), Some("Fast path"));
        }
        other => panic!("expected options field, got {other:?}"),
    }
    match &spec.fields[5].kind {
        InputKind::MultiOptions(choices) => {
            assert_eq!(
                choices.allowed_values(),
                vec!["dry".to_string(), "verbose".to_string()]
            );
        }
        other => panic!("expected multi_options field, got {other:?}"),
    }
    assert!(matches!(spec.fields[6].kind, InputKind::Markdown));
    assert!(matches!(spec.fields[7].kind, InputKind::Json));
    assert!(matches!(spec.fields[8].kind, InputKind::DateTime));
    assert!(matches!(spec.fields[9].kind, InputKind::FilePath));
    assert!(matches!(spec.fields[10].kind, InputKind::Url));

    assert_schema_preserves_field_order(&spec.to_json_schema_value());
}

fn canonical_args() -> Value {
    json!({
        "text": "hello",
        "ratio": 1.5,
        "count": 7,
        "enabled": true,
        "mode": "fast",
        "flags": ["dry", "verbose"],
        "body": "# Heading",
        "payload": { "ok": true },
        "when": "2026-05-20T12:34:56Z",
        "path": "/tmp/upeg-contract.txt",
        "site": "https://example.com/upeg",
    })
}

fn canonical_args_with(name: &str, value: Value) -> Value {
    let mut args = canonical_args();
    args.as_object_mut()
        .expect("canonical args object")
        .insert(name.to_string(), value);
    args
}

/// `-a name=value` text form of [`canonical_args`], one pair per field,
/// in declaration order. Used to build `-a`-path fixtures that isolate
/// a single bad/extra field the same way [`canonical_args_with`]
/// isolates one on the raw-JSON path (C-4: both paths must be tested
/// the same way to prove they now report the same error).
fn canonical_named_arg_pairs() -> Vec<(&'static str, String)> {
    vec![
        ("text", "hello".to_string()),
        ("ratio", "1.5".to_string()),
        ("count", "7".to_string()),
        ("enabled", "true".to_string()),
        ("mode", "fast".to_string()),
        ("flags", "dry,verbose".to_string()),
        ("body", "# Heading".to_string()),
        ("payload", r#"{"ok":true}"#.to_string()),
        ("when", "2026-05-20T12:34:56Z".to_string()),
        ("path", "/tmp/upeg-contract.txt".to_string()),
        ("site", "https://example.com/upeg".to_string()),
    ]
}

/// Flat `["-a", "field=value", ...]` argv for every canonical field,
/// with `name`'s value replaced by `value`.
fn named_args_with_override(name: &str, value: &str) -> Vec<String> {
    canonical_named_arg_pairs()
        .into_iter()
        .flat_map(|(field, default)| {
            let value = if field == name {
                value.to_string()
            } else {
                default
            };
            vec!["-a".to_string(), format!("{field}={value}")]
        })
        .collect()
}

/// Flat `["-a", "field=value", ...]` argv for every canonical field,
/// plus one extra `name=value` pair the target's `InputSpec` never
/// declared.
fn named_args_with_extra(name: &str, value: &str) -> Vec<String> {
    let mut argv: Vec<String> = canonical_named_arg_pairs()
        .into_iter()
        .flat_map(|(field, default)| vec!["-a".to_string(), format!("{field}={default}")])
        .collect();
    argv.push("-a".to_string());
    argv.push(format!("{name}={value}"));
    argv
}

fn as_str_argv(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

fn assert_input_value_error(error: InputValueError, expected_name: &str) -> InputValueError {
    match &error {
        InputValueError::MissingRequired { name }
        | InputValueError::TypeMismatch { name, .. }
        | InputValueError::InvalidChoice { name, .. }
        | InputValueError::InvalidMultiChoice { name, .. }
        | InputValueError::OutOfRange { name, .. }
        | InputValueError::PatternMismatch { name, .. }
        | InputValueError::UncompilablePattern { name, .. } => {
            assert_eq!(name.as_str(), expected_name);
        }
        InputValueError::File(error) => panic!("expected scalar input error, got {error}"),
    }
    error
}

fn load_runtime_fixture(fixture: &TempFixture) -> &'static upeg_core::ToolMeta {
    let outcome = upeg_loader::load_and_register_dir_verbose(&fixture.dir);
    assert_eq!(
        outcome.failed.len(),
        0,
        "fixture load failed: {:?}",
        outcome.failed
    );
    assert_eq!(outcome.loaded, vec![fixture.tool_id.as_str()]);
    toolbox_tool(&fixture.tool_id).expect("runtime fixture registered")
}

fn find_tool<'a>(tools: &'a [Value], id: &str) -> &'a Value {
    tools
        .iter()
        .find(|tool| tool["name"] == id)
        .unwrap_or_else(|| panic!("missing {id} in tools: {tools:?}"))
}

async fn response_body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX)
        .await
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("response body json")
}

fn run_upeg_with_fixture(fixture: &TempFixture, args: &[&str]) -> Output {
    Command::new(upeg_bin())
        .args(args)
        .env("UPEG_TOOLKITS_DIR", &fixture.dir)
        .env("UPEG_MCP_IMPORTS_DIR", fixture.dir.join("no-mcp"))
        .env("UPEG_WASM_DIR", fixture.dir.join("no-wasm"))
        .env("HOME", &fixture.home)
        // The repo root carries a dogfood `upeg.toml`; without this the
        // spawned binary would auto-detect it and register `dev.*`.
        .env(
            upeg_sources::project::PROJECT_MANIFEST_PATH_ENV,
            upeg_sources::project::PROJECT_MANIFEST_OVERRIDE_OFF,
        )
        .output()
        .unwrap_or_else(|err| panic!("run upeg {args:?}: {err}"))
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_failure_contains(output: &Output, context: &str, expected: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "{context} stderr should contain `{expected}`, got:\n{stderr}"
    );
}

/// The failed run's stderr, trimmed. Used to compare the `-a` and
/// raw-JSON paths' error text for byte-for-byte equality (C-4: ONE
/// validator, ONE message, regardless of which arg syntax hit it).
fn failure_stderr(output: &Output, context: &str) -> String {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

#[test]
fn the_loader_parses_toml_inputs_into_an_exact_input_spec() {
    let toolkit = unique_toolkit("loader");
    let meta = parse_single_tool(&canonical_toolkit_toml(&toolkit));

    assert_eq!(meta.id, format!("{toolkit}.all"));
    assert_eq!(meta.input_spec.fields.len(), 11);
    assert_canonical_input_spec(&meta.input_spec);
}

#[test]
fn the_loader_rejects_raw_input_schema_unknown_types_and_invalid_choices() {
    let raw_schema = minimal_toolkit_toml(
        &unique_toolkit("raw_schema"),
        r#"input_schema = "{\"type\":\"object\",\"properties\":{}}""#,
    );
    match parse_toolkit_full(&raw_schema) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("input_schema"), "{message}");
            assert!(message.contains("inputs"), "{message}");
        }
        other => panic!("raw input_schema must be rejected, got {other:?}"),
    }

    let unknown_type = minimal_toolkit_toml(
        &unique_toolkit("unknown_type"),
        r#"inputs = [{ name = "blob", type = "bytes" }]"#,
    );
    match parse_toolkit_full(&unknown_type) {
        Err(LoadError::UnknownInputType { name, kind, .. }) => {
            assert_eq!(name, "blob");
            assert_eq!(kind, "bytes");
        }
        other => panic!("invalid type string must be rejected, got {other:?}"),
    }

    let missing_options = minimal_toolkit_toml(
        &unique_toolkit("missing_options"),
        r#"inputs = [{ name = "mode", type = "options" }]"#,
    );
    match parse_toolkit_full(&missing_options) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("choice inputs"), "{detail}");
        }
        other => panic!("choice without options must be rejected, got {other:?}"),
    }

    let duplicate_multi = minimal_toolkit_toml(
        &unique_toolkit("duplicate_multi"),
        r#"inputs = [{ name = "flags", type = "multi_options", options = [{ value = "dry" }, { value = "dry" }] }]"#,
    );
    match parse_toolkit_full(&duplicate_multi) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("appears more than once"), "{detail}");
        }
        other => panic!("invalid multi_options choices must be rejected, got {other:?}"),
    }
}

#[tokio::test]
async fn the_runtime_exports_the_derived_input_spec_to_openapi_mcp_and_desktop() {
    let fixture = write_fixture_toolkit("protocols");
    let meta = load_runtime_fixture(&fixture);
    assert_canonical_input_spec(&meta.input_spec);

    let valid_args = canonical_args();
    assert_eq!(
        meta.input_spec
            .validate_json_args(valid_args.as_object().expect("valid args object")),
        Ok(())
    );

    let missing = json!({});
    let missing_error = meta
        .input_spec
        .validate_json_args(missing.as_object().expect("missing args object"))
        .expect_err("missing required field must fail");
    assert!(matches!(
        assert_input_value_error(missing_error, "text"),
        InputValueError::MissingRequired { .. }
    ));

    let bad_type = canonical_args_with("count", json!("seven"));
    let bad_type_error = meta
        .input_spec
        .validate_json_args(bad_type.as_object().expect("bad type args object"))
        .expect_err("bad integer type must fail");
    assert!(matches!(
        assert_input_value_error(bad_type_error.clone(), "count"),
        InputValueError::TypeMismatch {
            expected: "integer",
            actual: "string",
            ..
        }
    ));
    match dispatch_tool(&fixture.tool_id, &bad_type) {
        Outcome::Failure(failure) => assert_eq!(failure.error.message, bad_type_error.to_string()),
        other => panic!("dispatch should surface InputSpec validation error, got {other:?}"),
    }

    let bad_choice = canonical_args_with("mode", json!("turbo"));
    let choice_error = meta
        .input_spec
        .validate_json_args(bad_choice.as_object().expect("bad choice args object"))
        .expect_err("bad option must fail");
    assert!(matches!(
        assert_input_value_error(choice_error, "mode"),
        InputValueError::InvalidChoice { .. }
    ));

    let bad_multi = canonical_args_with("flags", json!(["dry", "loud"]));
    let multi_error = meta
        .input_spec
        .validate_json_args(bad_multi.as_object().expect("bad multi args object"))
        .expect_err("bad multi option must fail");
    assert!(matches!(
        assert_input_value_error(multi_error, "flags"),
        InputValueError::InvalidMultiChoice { .. }
    ));

    let expected_schema = meta.input_spec.to_json_schema_value();
    assert_schema_preserves_field_order(&expected_schema);

    let openapi_response = upeg_cli::http_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/openapi.json")
                .body(Body::empty())
                .expect("openapi request"),
        )
        .await
        .expect("openapi response");
    assert_eq!(openapi_response.status(), StatusCode::OK);
    let openapi = response_body_to_value(openapi_response.into_body()).await;
    let openapi_schema = &openapi["paths"][format!("/v1/tools/{}", fixture.tool_id)]["post"]["requestBody"]
        ["content"]["application/json"]["schema"];
    assert_eq!(openapi_schema, &expected_schema);
    assert_schema_preserves_field_order(openapi_schema);

    let mcp = handle_mcp_message(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    }))
    .expect("mcp tools/list response");
    let mcp_tools = mcp["result"]["tools"].as_array().expect("mcp tools array");
    let mcp_tool = find_tool(mcp_tools, &fixture.tool_id);
    assert_eq!(&mcp_tool["inputSchema"], &expected_schema);
    assert_schema_preserves_field_order(&mcp_tool["inputSchema"]);

    let desktop_tools = tool_json_entries_for_surface(Surface::Desktop, "name");
    let desktop_tool = find_tool(&desktop_tools, &fixture.tool_id);
    assert_eq!(&desktop_tool["inputSchema"], &expected_schema);
    assert_schema_preserves_field_order(&desktop_tool["inputSchema"]);
}

#[test]
fn the_cli_validates_valid_and_invalid_argument_cases_against_the_input_spec() {
    let fixture = write_fixture_toolkit("cli");

    let valid_dynamic = run_upeg_with_fixture(
        &fixture,
        &[
            "--quiet",
            &fixture.toolkit,
            "all",
            "hello",
            "1.5",
            "7",
            "true",
            "fast",
            "dry,verbose",
            "# Heading",
            r#"{"ok":true}"#,
            "2026-05-20T12:34:56Z",
            "/tmp/upeg-contract.txt",
            "https://example.com/upeg",
        ],
    );
    assert_success(&valid_dynamic, "dynamic positional typed invocation");
    assert_eq!(String::from_utf8_lossy(&valid_dynamic.stdout), "typed-ok\n");

    let valid_named = run_upeg_with_fixture(
        &fixture,
        &[
            "--quiet",
            "call",
            &fixture.tool_id,
            "--dry-run",
            "-a",
            "text=hello",
            "-a",
            "ratio=1.5",
            "-a",
            "count=7",
            "-a",
            "enabled=true",
            "-a",
            "mode=fast",
            "-a",
            "flags=dry,verbose",
            "-a",
            "body=# Heading",
            "-a",
            r#"payload={"ok":true}"#,
            "-a",
            "when=2026-05-20T12:34:56Z",
            "-a",
            "path=/tmp/upeg-contract.txt",
            "-a",
            "site=https://example.com/upeg",
        ],
    );
    assert_success(&valid_named, "named typed dry-run invocation");
    let dry_run_args: Value = serde_json::from_slice(&valid_named.stdout).expect("dry-run json");
    assert_eq!(dry_run_args["count"], 7);
    assert_eq!(dry_run_args["enabled"], true);
    assert_eq!(dry_run_args["flags"], json!(["dry", "verbose"]));
    assert_eq!(dry_run_args["payload"], json!({ "ok": true }));

    // C-4: `-a` no longer coerces/validates on its own — a bad or
    // invalid-choice field is inserted as-is and reported by the SAME
    // shared validator the raw-JSON path uses, so both argv forms below
    // isolate ONE bad field against an otherwise-full-and-valid set of
    // `-a` pairs, exactly like `canonical_args_with` already does for
    // the raw-JSON path.
    let mut bad_named_type_argv = vec![
        "--quiet".to_string(),
        "call".to_string(),
        fixture.tool_id.clone(),
        "--dry-run".to_string(),
    ];
    bad_named_type_argv.extend(named_args_with_override("count", "seven"));
    let bad_named_type = run_upeg_with_fixture(&fixture, &as_str_argv(&bad_named_type_argv));
    assert_failure_contains(
        &bad_named_type,
        "named invalid integer",
        "input `count` expected integer, got string",
    );

    let mut bad_named_choice_argv = vec![
        "--quiet".to_string(),
        "call".to_string(),
        fixture.tool_id.clone(),
        "--dry-run".to_string(),
    ];
    bad_named_choice_argv.extend(named_args_with_override("flags", "dry,loud"));
    let bad_named_choice = run_upeg_with_fixture(&fixture, &as_str_argv(&bad_named_choice_argv));
    assert_failure_contains(
        &bad_named_choice,
        "named invalid multi-option",
        "invalid option `loud`",
    );

    let bad_raw_json = canonical_args_with("count", json!("seven")).to_string();
    let bad_raw = run_upeg_with_fixture(
        &fixture,
        &[
            "--quiet",
            "call",
            &fixture.tool_id,
            "--dry-run",
            &bad_raw_json,
        ],
    );
    assert_failure_contains(
        &bad_raw,
        "raw JSON invalid integer",
        "input `count` expected integer, got string",
    );

    // C-4's actual point: the `-a` and raw-JSON paths must produce the
    // literal SAME error text for the same mistake, not just similar
    // wording.
    assert_eq!(
        failure_stderr(&bad_named_type, "named invalid integer"),
        failure_stderr(&bad_raw, "raw JSON invalid integer"),
        "`-a count=seven` and raw JSON `{{\"count\":\"seven\"}}` must report the identical error"
    );

    // Unknown-key rejection must ALSO agree between the two paths — the
    // raw-JSON path used to accept an unrecognized key silently.
    let mut unknown_named_argv = vec![
        "--quiet".to_string(),
        "call".to_string(),
        fixture.tool_id.clone(),
        "--dry-run".to_string(),
    ];
    unknown_named_argv.extend(named_args_with_extra("bogus", "1"));
    let unknown_named = run_upeg_with_fixture(&fixture, &as_str_argv(&unknown_named_argv));
    assert_failure_contains(&unknown_named, "named unknown key", "unknown input `bogus`");

    let unknown_raw_json = canonical_args_with("bogus", json!(1)).to_string();
    let unknown_raw = run_upeg_with_fixture(
        &fixture,
        &[
            "--quiet",
            "call",
            &fixture.tool_id,
            "--dry-run",
            &unknown_raw_json,
        ],
    );
    assert_failure_contains(
        &unknown_raw,
        "raw JSON unknown key",
        "unknown input `bogus`",
    );
    assert_eq!(
        failure_stderr(&unknown_named, "named unknown key"),
        failure_stderr(&unknown_raw, "raw JSON unknown key"),
        "`-a bogus=1` and raw JSON `{{...,\"bogus\":1}}` must report the identical error"
    );
}

#[test]
fn the_tui_form_surface_follows_the_driving_canonical_input_spec() {
    // Desktop generic-form branch coverage now lives in the Flutter
    // widget test `flutter_app/test/widgets/generic_form_test.dart`.
    // The TUI half stays here so the Rust workspace still pins typed
    // input coverage end-to-end.
    let fixture = write_fixture_toolkit("ui");
    let meta = load_runtime_fixture(&fixture);
    let tools = [meta];

    let mut state = State::default();
    // Enter always runs/confirms now; `o` opens the manifest (Detail),
    // then F1 runs from there — same two-hop path as before, just
    // keyed on `o` instead of the overloaded old Enter-opens-Detail.
    assert_eq!(handle_key(&mut state, Key::Char('o'), &tools), Action::None);
    assert_eq!(handle_key(&mut state, Key::F(1), &tools), Action::None);

    match &state.view {
        View::Form { tool_id, form } => {
            assert_eq!(*tool_id, fixture.tool_id.as_str());
            assert_eq!(field_names(&form.input_spec), FIELD_NAMES.to_vec());
            assert_eq!(
                form.fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                FIELD_NAMES.to_vec()
            );
            assert!(matches!(
                form.fields[3].draft,
                DraftInputValue::Boolean(false)
            ));
            assert!(matches!(
                form.fields[4].draft,
                DraftInputValue::Options(None)
            ));
            assert!(matches!(form.fields[5].draft, DraftInputValue::Text(_)));
        }
        other => panic!("expected typed TUI form view, got {other:?}"),
    }
}

/// Inherited from the retired Dioxus surface's toolbox-visibility tests:
/// the full typed-form path — edit form state, coerce to args, dispatch —
/// must round-trip an empty optional-looking integer field to its declared
/// default rather than sending a literal `null` the dispatcher rejects
/// (iter 108 regression).
#[test]
fn an_empty_form_field_round_trips_to_its_default_on_the_dispatch_path() {
    let tool = toolbox_tool("text.repeat").expect("text.repeat must be a registered built-in");
    let mut state = tool.input_spec.initial_form_state();
    for field in &mut state.fields {
        field.draft = match field.name.as_str() {
            "input" => DraftInputValue::Text("foo".to_string()),
            "count" => DraftInputValue::Text(String::new()), // the field the user left blank
            _ => field.draft.clone(),
        };
    }
    let args = tool
        .input_spec
        .args_from_form_state(&state)
        .expect("typed form state must convert");
    assert!(
        args.get("count").is_none(),
        "args coercion must omit the empty integer field (iter 108)"
    );

    match dispatch_tool("text.repeat", &args) {
        Outcome::Success(success) => {
            assert_eq!(
                upeg_runtime::tool_success_primary_text(&success),
                "foo",
                "blank `count` form field must dispatch text.repeat with its default count=1"
            );
        }
        other => panic!("expected text.repeat to dispatch successfully, got {other:?}"),
    }
}
