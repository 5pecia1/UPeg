use super::common::parse;
use crate::*;

fn register_cli_multi_output_tool(id: &'static str) {
    use upeg_core::{Invoker, OutputEntry, OutputKind, OutputValue, PegboardUnits, PinKind};

    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test multi output tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    upeg_runtime::register_runtime_dispatcher(id, |_| {
        upeg_core::ToolResult::Success(
            upeg_core::ToolSuccess::new(
                Some("summary".to_string()),
                vec![
                    OutputEntry {
                        id: "summary".to_string(),
                        label: Some("Summary".to_string()),
                        kind: OutputKind::String,
                        value: OutputValue::String("done".to_string()),
                    },
                    OutputEntry {
                        id: "count".to_string(),
                        label: None,
                        kind: OutputKind::Integer,
                        value: OutputValue::Integer(7),
                    },
                ],
            )
            .expect("test result must be canonical"),
        )
    });
}

const CONTROLLED_EMBED_CLI_TOOL_ID: &str = "cefixture.run";
const CONTROLLED_EMBED_CLI_OUTPUT: &str = "첫 줄\n둘째 줄 🙂";
const CONTROLLED_EMBED_WAIT_TIMEOUT_CODE: &str =
    upeg_runtime::controlled_embed::CONTROLLED_EMBED_WAIT_TIMEOUT_CODE;
const CONTROLLED_EMBED_WAIT_TIMEOUT_SELECTOR: &str = "#summary";
const CONTROLLED_EMBED_WAIT_TIMEOUT_FOR_SELECTOR: &str = "#ready";
const CONTROLLED_EMBED_WAIT_TIMEOUT_MS: u64 = 125;

type ControlledEmbedCliInputs = std::sync::Arc<std::sync::Mutex<Vec<Vec<(String, String)>>>>;

struct RecordingControlledEmbedBackend {
    seen_inputs: ControlledEmbedCliInputs,
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
                selector: CONTROLLED_EMBED_WAIT_TIMEOUT_SELECTOR.into(),
                for_selector: CONTROLLED_EMBED_WAIT_TIMEOUT_FOR_SELECTOR.into(),
                condition: upeg_core::BindingWaitCondition::Visible,
                timeout_ms: CONTROLLED_EMBED_WAIT_TIMEOUT_MS,
            },
        )
    }
}

impl upeg_runtime::controlled_embed::ControlledEmbedBackend for RecordingControlledEmbedBackend {
    fn run(
        &self,
        request: upeg_runtime::controlled_embed::ControlledEmbedRequest<'_>,
    ) -> Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    > {
        let inputs = request
            .inputs
            .iter()
            .map(|(field, value)| ((*field).to_string(), (*value).to_string()))
            .collect();
        self.seen_inputs.lock().unwrap().push(inputs);

        Ok(upeg_runtime::controlled_embed::ControlledEmbedResponse {
            outputs: vec![(
                "summary".to_string(),
                CONTROLLED_EMBED_CLI_OUTPUT.to_string(),
            )],
        })
    }
}

fn install_recording_controlled_embed_backend() -> ControlledEmbedCliInputs {
    let seen_inputs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    upeg_runtime::controlled_embed::set_controlled_embed_backend(std::sync::Arc::new(
        RecordingControlledEmbedBackend {
            seen_inputs: std::sync::Arc::clone(&seen_inputs),
        },
    ));
    seen_inputs
}

fn install_wait_timeout_controlled_embed_backend() {
    upeg_runtime::controlled_embed::set_controlled_embed_backend(std::sync::Arc::new(
        WaitTimeoutControlledEmbedBackend,
    ));
}

fn load_controlled_embed_cli_manifest_fixture() {
    let workspace_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli must live under workspace root");
    let dir = workspace_dir.join("target/test-tmp/cli-controlled-embed-call");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("controlled embed CLI fixture dir");
    std::fs::write(
        dir.join("cefixture.toml"),
        r##"id = "cefixture"

[[tools]]
id = "run"
description = "Controlled Embed fixture"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.test/tool"
surfaces = ["cli", "tui", "http", "mcp"]
primary_output_id = "summary"

[[tools.inputs]]
name = "query"
type = "string"
required = true

[[tools.outputs]]
name = "summary"
type = "string"
label = "Summary"

[[tools.controlled_embed.bindings]]
role = "input"
field = "query"
selector = "#q"

[[tools.controlled_embed.bindings]]
role = "trigger"
field = ""
selector = "button"

[[tools.controlled_embed.bindings]]
role = "output"
field = "summary"
selector = "#summary"
"##,
    )
    .expect("write controlled embed CLI fixture");

    let outcome = upeg_loader::load_and_register_dir_verbose(&dir);
    assert_eq!(
        outcome.failed.len(),
        0,
        "controlled embed CLI fixture load failed: {:?}",
        outcome.failed
    );
    assert_eq!(outcome.loaded, vec![CONTROLLED_EMBED_CLI_TOOL_ID]);
}

#[test]
fn 파싱_kv_인자는_빈_키를_거부한다() {
    assert!(parse_kv_arg("=value").is_err());
}

#[test]
fn kv_인자_파싱은_값에_등호가_있어도_나머지를_그대로_유지한다() {
    // base64 padding, k=v=foo style — split on FIRST `=` only.
    let (k, v) = parse_kv_arg("input=Zm9v=").unwrap();
    assert_eq!(k, "input");
    assert_eq!(v, serde_json::Value::String("Zm9v=".into()));
}

#[test]
fn 인자_플래그가_있는_호출은_로컬로_dispatch한다() {
    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
    ]))
    .expect("call -a");
    assert_eq!(out, "255\n");
}

#[test]
fn call은_dash_표기_tool_id를_underscore로_정규화한다() {
    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex-to-decimal",
        "-a",
        "input=0xff",
    ]))
    .expect("call with dashes in tool_id");
    assert_eq!(out, "255\n");
}

#[test]
fn 기본_호출은_기본_output_값만_표준출력으로_반환한다() {
    let id = "test.cli_call.default_primary_only";
    register_cli_multi_output_tool(id);

    let out = run(parse(&["upeg", "call", id])).expect("call default output mode");

    assert_eq!(out, "done\n");
}

#[test]
fn json_호출은_정식_성공_envelope를_표준출력으로_반환한다() {
    let id = "test.cli_call.json_success";
    register_cli_multi_output_tool(id);

    let out = run(parse(&["upeg", "call", id, "--json"])).expect("call --json");
    let value: serde_json::Value = serde_json::from_str(&out).expect("canonical JSON");

    assert_eq!(value["ok"], true);
    assert_eq!(value["primary_output_id"], "summary");
    assert_eq!(value["outputs"][0]["id"], "summary");
    assert_eq!(value["outputs"][0]["value"], "done");
    assert_eq!(value["outputs"][1]["id"], "count");
    assert_eq!(value["outputs"][1]["value"], 7);
}

#[test]
fn 필드_호출은_요청한_output_값만_반환한다() {
    let id = "test.cli_call.field_count";
    register_cli_multi_output_tool(id);

    let out = run(parse(&["upeg", "call", id, "--field", "count"])).expect("call --field");

    assert_eq!(out, "7\n");
}

#[test]
fn pretty_호출은_라벨이_붙은_행들을_반환한다() {
    let id = "test.cli_call.pretty_rows";
    register_cli_multi_output_tool(id);

    let out = run(parse(&["upeg", "call", id, "--pretty"])).expect("call --pretty");

    assert_eq!(out, "Summary: done\ncount: 7\n");
}

#[test]
fn controlled_embed_cli_json은_정식_success_envelope를_반환한다() {
    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    let seen_inputs = install_recording_controlled_embed_backend();
    load_controlled_embed_cli_manifest_fixture();

    let out = run(parse(&[
        "upeg",
        "call",
        CONTROLLED_EMBED_CLI_TOOL_ID,
        "-a",
        "query=upeg",
        "--json",
    ]))
    .expect("controlled embed call --json");
    let value: serde_json::Value = serde_json::from_str(&out).expect("controlled embed JSON");

    assert_eq!(value["ok"], true);
    assert_eq!(value["primary_output_id"], "summary");
    assert_eq!(value["outputs"][0]["id"], "summary");
    assert_eq!(value["outputs"][0]["value"], CONTROLLED_EMBED_CLI_OUTPUT);
    assert!(
        seen_inputs
            .lock()
            .unwrap()
            .iter()
            .any(|inputs| inputs.contains(&("query".to_string(), "upeg".to_string()))),
        "fake backend should receive CLI -a query input"
    );
}

#[test]
fn controlled_embed_cli_pretty는_라벨과_줄바꿈_값을_보존한다() {
    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    install_recording_controlled_embed_backend();
    load_controlled_embed_cli_manifest_fixture();

    let out = run(parse(&[
        "upeg",
        "call",
        CONTROLLED_EMBED_CLI_TOOL_ID,
        "-a",
        "query=upeg",
        "--pretty",
    ]))
    .expect("controlled embed call --pretty");

    assert!(
        out.contains("Summary:"),
        "pretty output must include label: {out}"
    );
    assert!(
        out.contains("첫 줄"),
        "pretty output must include first line: {out}"
    );
    assert!(
        out.contains("둘째 줄 🙂"),
        "pretty output must include second line: {out}"
    );
}

#[test]
fn controlled_embed_cli_json은_backend_unavailable_code를_반환한다() {
    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(std::sync::Arc::new(
        upeg_runtime::controlled_embed::NoopControlledEmbedBackend,
    ));
    load_controlled_embed_cli_manifest_fixture();

    let result = run(parse(&[
        "upeg",
        "call",
        CONTROLLED_EMBED_CLI_TOOL_ID,
        "-a",
        "query=upeg",
        "--json",
    ]));

    match result {
        Err(CliError::StdoutFailure { stdout }) => {
            let value: serde_json::Value = serde_json::from_str(&stdout).expect("failure JSON");
            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "controlled_embed_unavailable");
        }
        other => panic!("expected controlled embed stdout failure JSON, got {other:?}"),
    }
}

#[test]
fn controlled_embed_cli_json은_wait_timeout_failure_envelope를_반환한다() {
    let _guard = crate::test_support::controlled_embed_backend_test_lock()
        .lock()
        .unwrap();
    let _restore = crate::test_support::RestoreNoopControlledEmbedBackend;
    install_wait_timeout_controlled_embed_backend();
    load_controlled_embed_cli_manifest_fixture();

    let result = run(parse(&[
        "upeg",
        "call",
        CONTROLLED_EMBED_CLI_TOOL_ID,
        "-a",
        "query=upeg",
        "--json",
    ]));

    match result {
        Err(CliError::StdoutFailure { stdout }) => {
            let value: serde_json::Value = serde_json::from_str(&stdout).expect("failure JSON");
            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], CONTROLLED_EMBED_WAIT_TIMEOUT_CODE);
            let message = value["error"]["message"]
                .as_str()
                .expect("wait timeout message");
            assert!(
                message.contains(CONTROLLED_EMBED_WAIT_TIMEOUT_SELECTOR),
                "message must include the timed-out binding selector: {message}"
            );
            assert!(
                message.contains(CONTROLLED_EMBED_WAIT_TIMEOUT_FOR_SELECTOR),
                "message must include the waited selector: {message}"
            );
            assert!(
                message.contains(CONTROLLED_EMBED_WAIT_TIMEOUT_CODE),
                "message must include the canonical wait-timeout code: {message}"
            );
        }
        other => panic!("expected controlled embed wait-timeout JSON failure, got {other:?}"),
    }
}

#[test]
fn json_호출의_도구_오류는_표준출력_failure_envelope로_반환된다() {
    let result = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xZZ",
        "--json",
    ]));

    match result {
        Err(CliError::StdoutFailure { stdout }) => {
            let value: serde_json::Value = serde_json::from_str(&stdout).expect("failure JSON");
            assert_eq!(value["ok"], false);
            assert_eq!(value["primary_output_id"], serde_json::Value::Null);
            assert_eq!(value["outputs"].as_array().map(Vec::len), Some(0));
            assert!(
                value["error"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("invalid hex")
            );
        }
        other => panic!("expected stdout failure JSON, got {other:?}"),
    }
}

#[test]
fn 알수없는_도구의_json_호출은_정식_오류를_표준출력으로_반환한다() {
    let result = run(parse(&["upeg", "call", "no.such.tool", "--json"]));

    match result {
        Err(CliError::StdoutFailure { stdout }) => {
            let value: serde_json::Value = serde_json::from_str(&stdout).expect("failure JSON");
            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "unknown_tool");
            assert!(
                value["error"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("unknown tool")
            );
        }
        other => panic!("expected stdout failure JSON, got {other:?}"),
    }
}

#[test]
fn 인자_플래그는_위치인자_json이_있는_호출에서도_덮어쓴다() {
    // Positional `{}` ignored when `-a` present — otherwise `{}` would
    // dispatch with empty args and the tool would error.
    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "{}",
        "-a",
        "input=0xff",
    ]))
    .expect("call -a wins over positional");
    assert_eq!(out, "255\n");
}

#[test]
fn 여러_인자_플래그가_있는_호출은_객체를_빌드한다() {
    // text.regex_match wants two fields; chained `-a` should compose them.
    let out = run(parse(&[
        "upeg",
        "call",
        "text.regex_match",
        "-a",
        "pattern=foo",
        "-a",
        "input=foobarfoo",
    ]))
    .expect("call -a -a");
    assert_eq!(out, "[\"foo\",\"foo\"]\n");
}

#[test]
fn 인자_플래그가_없는_호출은_위치인자_json으로_대체된다() {
    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        r#"{"input":"0xff"}"#,
    ]))
    .expect("call positional");
    assert_eq!(out, "255\n");
}

#[test]
fn 호출은_객체가_아닌_위치인자_인자를_거부한다() {
    // parallel to MCP/HTTP non-object args. Shape-check at the
    // boundary so the user sees a clear "args must be a JSON object"
    // instead of a confusing tool error pointing at the wrong place
    // (e.g., `[1,2,3]` would otherwise reach dispatch_tool, then
    // `args.get("input")` → None → "" → tool errors with "empty
    // input").
    for bad_args in [
        "[1,2,3]",    // array
        r#""hello""#, // string
        "42",         // number
        "true",       // boolean
    ] {
        let r = run(parse(&["upeg", "call", "num.hex_to_decimal", bad_args]));
        match r {
            Err(CliError::ToolFailed(msg)) => {
                assert!(
                    msg.contains("args must be a JSON object"),
                    "non-object args `{bad_args}` must trigger the iter-258 shape check; got `{msg}`"
                );
            }
            other => panic!("non-object args `{bad_args}` must be rejected; got {other:?}"),
        }
    }
}

#[test]
fn 키_이퀄_값을_위치인자로_넘기면_인자_플래그를_알려준다() {
    // `-a`를 빠뜨린 호출은 의도가 명확하다. serde_json의 "expected
    // value at line 1 column 1"만 돌려주면 JSON을 고치라는 말로
    // 읽히므로, 쓰려던 명령을 플래그와 함께 그대로 되돌려준다.
    let r = run(parse(&["upeg", "call", "num.hex_to_decimal", "input=0xff"]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("`-a input=0xff`"),
                "dropped `-a` must be quoted back with the flag; got `{msg}`"
            );
        }
        other => panic!("`input=0xff` as positional args must be rejected; got {other:?}"),
    }
}

#[test]
fn 잘못된_json은_더_쉬운_인자_형식과_기대_입력을_알려준다() {
    // 원시 JSON 파싱 실패는 호출자가 손으로 JSON을 쓰다 틀린
    // 순간이다. 이때가 `-a key=value`가 있다는 사실과 이 Tool이
    // 무엇을 받는지 알려줄 유일한 지점이다.
    let r = run(parse(&["upeg", "call", "num.hex_to_decimal", "0xff"]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("-a key=value"),
                "raw-JSON failure must name the `-a` form; got `{msg}`"
            );
            assert!(
                msg.contains("avoids JSON syntax") && msg.contains("shell quoting still applies"),
                "the simpler form must distinguish JSON syntax from shell quoting; got `{msg}`"
            );
            assert!(
                msg.contains("expected inputs: `input`"),
                "raw-JSON failure must list the target's declared inputs; got `{msg}`"
            );
        }
        other => panic!("`0xff` as positional args must be rejected; got {other:?}"),
    }
}

#[test]
fn 미등록_툴의_잘못된_json은_기대_입력을_지어내지_않는다() {
    // InputSpec이 없으면 알려줄 입력 목록도 없다. 없는 스펙을
    // 추측해 출력하지 않는지 고정한다.
    let r = run(parse(&["upeg", "call", "no.such_tool", "0xff"]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                !msg.contains("expected inputs"),
                "unregistered tool has no spec to report; got `{msg}`"
            );
        }
        other => panic!("`0xff` as positional args must be rejected; got {other:?}"),
    }
}

#[test]
fn 인자가_없는_호출은_기본값으로_빈_객체를_가진다() {
    // Pin the interaction between clap's `default_value = "{}"`
    // on the `args` positional and the JSON-object shape check.
    // `upeg call <zero-arg-tool>` with no positional/stdin args
    // must successfully dispatch: the default `{}` parses to an
    // empty Object, passes the shape check, and reaches the
    // dispatcher — verifying the default isn't accidentally a
    // non-object literal that the gate would reject.
    let out = run(parse(&["upeg", "call", "id.uuid_v7"]))
        .expect("no-args call must succeed for zero-arg tool");
    assert_eq!(
        out.trim_end().len(),
        36,
        "uuid_v7 must produce a 36-char canonical form even when called with no positional args"
    );
}

#[test]
fn 호출은_객체_또는_널_위치인자_인자를_허용한다() {
    // zero-arg tools (uuid_v7) work with `{}` or `null`
    // positional args; arg-taking tools work with `{"key": ...}`.
    // All three shapes pass the JSON-object shape check.
    let out = run(parse(&["upeg", "call", "id.uuid_v7", "{}"]))
        .expect("empty object args must work for zero-arg tool");
    assert_eq!(
        out.trim_end().len(),
        36,
        "uuid_v7 must produce a 36-char canonical form"
    );

    let out = run(parse(&["upeg", "call", "id.uuid_v7", "null"]))
        .expect("null args must work for zero-arg tool");
    assert_eq!(out.trim_end().len(), 36);

    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        r#"{"input":"0x10"}"#,
    ]))
    .expect("object args must dispatch normally");
    assert_eq!(out, "16\n");
}

// ─── `upeg http` subcommand parsing (PRD §5.5) ─────────────────

#[test]
fn http_그대로의는_액션이_없는_있음을_파싱한다() {
    let cli = parse(&["upeg", "http"]);
    match cli.command {
        Some(Command::Http { action, daemon, .. }) => {
            assert!(action.is_none(), "no subcommand → action is None");
            assert!(!daemon, "no --daemon flag → false");
        }
        other => panic!("expected Command::Http, got {other:?}"),
    }
}

#[test]
fn http_데몬_플래그는_파싱한다() {
    let cli = parse(&["upeg", "http", "--daemon"]);
    match cli.command {
        Some(Command::Http { daemon, .. }) => assert!(daemon),
        other => panic!("expected Command::Http, got {other:?}"),
    }
}

#[test]
fn http_url_와_토큰_플래그들은_파싱한다() {
    let cli = parse(&[
        "upeg",
        "http",
        "--addr",
        "127.0.0.1:7173",
        "--token",
        "secret",
    ]);
    match cli.command {
        Some(Command::Http { addr, token, .. }) => {
            assert_eq!(addr.as_deref(), Some("127.0.0.1:7173"));
            assert_eq!(token.as_deref(), Some("secret"));
        }
        other => panic!("expected Command::Http, got {other:?}"),
    }
}

#[test]
fn http_상태_하위명령은_json_플래그_있음을_파싱한다() {
    let cli = parse(&["upeg", "http", "status", "--json"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Status { json, .. }),
            ..
        }) => assert!(json),
        other => panic!("expected Http::Status {{ json: true }}, got {other:?}"),
    }
}

#[test]
fn http_상태_하위명령은_페어링_플래그_있음을_파싱한다() {
    let cli = parse(&["upeg", "http", "status", "--pairing"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Status { pairing, .. }),
            ..
        }) => assert!(pairing),
        other => panic!("expected Http::Status {{ pairing: true }}, got {other:?}"),
    }
}

#[test]
fn http_상태_하위명령은_페어링_플래그_기본값_거짓이다() {
    let cli = parse(&["upeg", "http", "status"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Status { pairing, .. }),
            ..
        }) => assert!(!pairing),
        other => panic!("expected Http::Status {{ pairing: false }}, got {other:?}"),
    }
}

#[test]
fn http_중지_하위명령은_강제_플래그_있음을_파싱한다() {
    let cli = parse(&["upeg", "http", "stop", "--force"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Stop { force }),
            ..
        }) => assert!(force),
        other => panic!("expected Http::Stop {{ force: true }}, got {other:?}"),
    }
}

#[test]
fn 호스트_시작_하위명령은_데몬_플래그를_파싱한다() {
    let cli = parse(&["upeg", "host", "start", "--daemon"]);
    match cli.command {
        Some(Command::Host {
            action: HostAction::Start { daemon, .. },
        }) => assert!(daemon),
        other => panic!("expected Host::Start {{ daemon: true }}, got {other:?}"),
    }
}

#[test]
fn 삭제된_service_하위명령은_더이상_내장_명령이_아니다() {
    // service/source control plane은 삭제되었다. host 프로세스 자체가
    // 명시적 활성화이므로 desired-state 명령이 존재하면 안 된다.
    // 남은 것은 동적 도구 라우팅(External)뿐 — 즉 `service`라는 이름의
    // 내장 하위명령은 사라졌다.
    let cli = parse(&["upeg", "service", "enable", "rest-api"]);
    match cli.command {
        Some(Command::External(argv)) => assert_eq!(argv[0], "service"),
        other => panic!("expected External passthrough, got {other:?}"),
    }
}

#[test]
fn 삭제된_source_하위명령은_더이상_내장_명령이_아니다() {
    let cli = parse(&["upeg", "source", "load", "mcp-imports"]);
    match cli.command {
        Some(Command::External(argv)) => assert_eq!(argv[0], "source"),
        other => panic!("expected External passthrough, got {other:?}"),
    }
}

#[test]
fn http_재시작_하위명령은_상단_수준_데몬_있음을_파싱한다() {
    // The `--daemon` flag is parsed at the top level of `Http`, not    // inside the Restart subcommand — the iter-pre-tray design
    // intentionally shares startup flags between bare-start and
    // restart so the same binding/token apply to both paths.
    let cli = parse(&["upeg", "http", "--daemon", "restart"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Restart { force }),
            daemon,
            ..
        }) => {
            assert!(!force);
            assert!(daemon, "top-level --daemon must propagate into restart");
        }
        other => panic!("expected Http::Restart, got {other:?}"),
    }
}

#[test]
fn http는_하위명령_기본값_로_100_줄들을_로그에_기록한다() {
    let cli = parse(&["upeg", "http", "logs"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Logs { lines }),
            ..
        }) => assert_eq!(lines, 100),
        other => panic!("expected Http::Logs, got {other:?}"),
    }
}

#[test]
fn http_logs_하위명령은_명시한_줄_개수를_그대로_기록한다() {
    let cli = parse(&["upeg", "http", "logs", "--lines", "42"]);
    match cli.command {
        Some(Command::Http {
            action: Some(HttpAction::Logs { lines }),
            ..
        }) => assert_eq!(lines, 42),
        other => panic!("expected Http::Logs {{ lines: 42 }}, got {other:?}"),
    }
}

#[test]
fn 호출은_데몬_변경_없이_플래그를_허용한다() {
    // PRD §11 removed `--daemon` from `upeg call`. clap's `parse_from`
    // calls `process::exit` on error which we can't catch from a
    // unit test; use `try_parse_from` so the error is returned.
    use clap::Parser;
    let result = Cli::try_parse_from(["upeg", "call", "x", "--daemon"]);
    assert!(
        result.is_err(),
        "`upeg call --daemon` must fail to parse after legacy removal"
    );
}

mod toolkit_validate;
mod validate;
