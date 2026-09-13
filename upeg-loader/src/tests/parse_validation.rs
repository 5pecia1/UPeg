use super::{parse_single_tool as parse_fixture_tool, single_tool_toml_str};
use crate::dispatcher::external_dispatcher_for;
use crate::{LoadError, ToolToml, load_and_register_dir_verbose};

mod controlled_embed;
mod external_pty;
use upeg_core::Invoker;

fn call_dispatcher<F>(f: &F, value: serde_json::Value) -> Result<String, String>
where
    F: for<'a> Fn(upeg_runtime::DispatchArgs<'a>) -> upeg_core::ToolResult,
{
    upeg_runtime::tool_result_text(f(
        upeg_runtime::DispatchArgs::parse(&value).expect("test args must be an object")
    ))
}

#[test]
fn 파싱은_명령_없는_외부_호출자를_거부한다() {
    // Iter 242: `invoker = "External"` requires a non-empty `command`
    // field. Pre-iter-242 the loader registered such tools with no
    // dispatcher — the failure surfaced as the cryptic "dispatch
    // not implemented" only when the user finally tried to call.
    // Three sub-cases: command field absent, present-but-empty,
    // present-but-whitespace-only. All three must surface the
    // dedicated error variant.
    let cases = [
        // command field omitted entirely
        r#"id = "y.x"
               toolkit = "y"
               invoker = "External""#,
        // command field present but empty
        r#"id = "y.x"
               toolkit = "y"
               invoker = "External"
               command = """#,
        // command field present but whitespace-only
        r#"id = "y.x"
               toolkit = "y"
               invoker = "External"
               command = "   ""#,
    ];
    for s in cases {
        match parse_fixture_tool(s) {
            Err(LoadError::ExternalRequiresCommand) => {}
            other => panic!(
                "External-with-no-command must surface ExternalRequiresCommand, got {other:?}\nfor TOML:\n{s}"
            ),
        }
    }
}

#[test]
fn 외부_검증_파싱은_공백이_붙은_호출자를_거부한다() {
    // Runtime manifests no longer preserve trim-forgiveness for enum
    // fields. A padded invoker is an unknown invoker, not External.
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External ""#;
    match parse_fixture_tool(s) {
        Err(LoadError::UnknownInvoker(invoker)) => assert_eq!(invoker, "External "),
        other => panic!("padded `External ` invoker must be rejected, got {other:?}"),
    }
}

#[test]
fn 공백이_붙은_명령이_있는_외부_파싱은_로드된다() {
    // Iter 242: a valid External + padded command must load.
    // The trim happens at external_dispatcher_for so the spawned
    // process gets the trimmed binary name (otherwise it'd fail
    // with "executable not found" for `git ` with a trailing space).
    let s = r#"id = "iter242.padded"
                   toolkit = "iter242"
                   invoker = "External"
                   command = "  echo  ""#;
    let m = parse_fixture_tool(s).expect("padded command must load");
    assert_eq!(m.invoker, Invoker::External);
    // Build the dispatcher and confirm it'd spawn the trimmed
    // binary. We don't need to actually run it; the trim is
    // applied at build time, so we can pin it via the closure
    // execution path.
    let parsed = toml::from_str::<ToolToml>(s).unwrap();
    let f = external_dispatcher_for(&parsed, None).expect("External + command builds dispatcher");
    // Run with empty args (echo prints nothing). Either spawn
    // succeeds (dispatcher exists) or it fails with a real
    // error — the failure mode we're guarding against is
    // "executable not found because of trailing space", which
    // would surface as `spawn `echo  `: ...` with the literal
    // padded name in the error.
    match call_dispatcher(&f, serde_json::json!({})) {
        Ok(out) => assert_eq!(
            out.trim_end(),
            "",
            "echo with no args produces empty output"
        ),
        Err(e) => panic!(
            "padded command must trim before spawn — error mentioning literal padded name suggests trim missed: {e}"
        ),
    }
}

#[test]
fn 선언형_loader는_헤드리스_호출자에_어댑터_필드를_요구한다() {
    for (invoker, field) in [("Http", "url"), ("Llm", "prompt"), ("Wasm", "wasm_path")] {
        let s = format!(
            r#"id = "future.{invoker}"
toolkit = "future"
invoker = "{invoker}""#
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::MissingInvokerField {
                invoker: got_invoker,
                field: got_field,
            }) => {
                assert_eq!(got_invoker, invoker);
                assert_eq!(got_field, field);
            }
            other => panic!("expected missing-field error for {invoker}, got {other:?}"),
        }
    }
}

#[test]
fn 파싱은_퇴역한_카테고리_필드를_거부한다() {
    let toolkit_category = r#"
id = "retired"
category = "old"

[[tools]]
id = "echo"
"#;
    match crate::parse_toolkit_full(toolkit_category) {
        Err(LoadError::RetiredField {
            field, replacement, ..
        }) => {
            assert_eq!(field, "category");
            assert_eq!(replacement, "tags");
        }
        other => panic!("retired toolkit category must be rejected, got {other:?}"),
    }

    let tool_category = r#"id = "retired.echo"
toolkit = "retired"
category = "old""#;
    match parse_fixture_tool(tool_category) {
        Err(LoadError::RetiredField {
            field, replacement, ..
        }) => {
            assert_eq!(field, "category");
            assert_eq!(replacement, "tags");
        }
        other => panic!("retired tool category must be rejected, got {other:?}"),
    }
}

#[test]
fn 파싱은_알수없는_도구킷_수준_필드를_거부한다() {
    let raw = r#"
id = "strict"
surprise_toolkit_field = true

[[tools]]
id = "echo"
pegboard_units = "U1"
"#;

    match crate::parse_toolkit_full(raw) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("unknown field"), "{message}");
            assert!(message.contains("surprise_toolkit_field"), "{message}");
        }
        other => panic!("unknown toolkit field must be rejected as TOML, got {other:?}"),
    }
}

#[test]
fn 파싱은_알수없는_도구_항목_필드를_거부한다() {
    let raw = r#"
id = "strict"

[[tools]]
id = "echo"
pegboard_units = "U1"
unexpected_tool_field = "nope"
"#;

    match crate::parse_toolkit_full(raw) {
        Err(LoadError::Toml(error)) => {
            let message = error.to_string();
            assert!(message.contains("unknown field"), "{message}");
            assert!(message.contains("unexpected_tool_field"), "{message}");
        }
        other => panic!("unknown tool field must be rejected as TOML, got {other:?}"),
    }
}

#[test]
fn 파싱은_트리거_소스와_자격증명_schema_참조를_받아들인다() {
    let bad_trigger = r#"id = "auto.echo"
toolkit = "auto"
triggers = [{ source = "mouse" }]"#;
    match parse_fixture_tool(bad_trigger) {
        Err(LoadError::UnknownTriggerSource(source)) => assert_eq!(source, "mouse"),
        other => panic!("unknown trigger source must be rejected, got {other:?}"),
    }

    let bad_keychain = r#"id = "cred.echo"
toolkit = "cred"
credentials = [{ name = "api", type = "api_key", store = "keychain", keychain_service = "upeg" }]"#;
    match parse_fixture_tool(bad_keychain) {
        Err(LoadError::EmptyCredentialField { field, .. }) => {
            assert_eq!(field, "keychain_account");
        }
        other => panic!("incomplete keychain reference must be rejected, got {other:?}"),
    }

    let ok = r#"id = "cred.echo"
toolkit = "cred"
invoker = "External"
command = "echo"
credentials = [{ name = "api", type = "api_key", store = "keychain", keychain_service = "upeg", keychain_account = "api" }]
triggers = [{ source = "schedule", condition = "now" }]"#;
    let meta = parse_fixture_tool(ok).expect("typed credential + trigger source should parse");
    assert_eq!(meta.id, "cred.echo");
}

#[test]
fn 파싱은_리터럴_자격증명_비밀값_필드를_거부한다() {
    for forbidden in ["value", "secret_value", "literal_secret"] {
        let raw = format!(
            r#"id = "cred.literal"
toolkit = "cred"
credentials = [{{ name = "api", {forbidden} = "do-not-store-me" }}]"#
        );
        match parse_fixture_tool(&raw) {
            Err(LoadError::SecretField { position, field }) => {
                assert_eq!(position, 0);
                assert_eq!(field, forbidden);
            }
            other => {
                panic!("literal credential field `{forbidden}` must be rejected, got {other:?}")
            }
        }
    }
    let msg = format!(
        "{}",
        LoadError::SecretField {
            position: 0,
            field: "value".to_string(),
        }
    );
    assert!(msg.contains("forbidden"), "{msg}");
    assert!(msg.contains("env or OS keychain"), "{msg}");
}

#[test]
fn 설정되면_선언형_loader는_prd_헤드리스_호출자를_허용한다() {
    let cases = [
        (
            r#"id = "future.http"
toolkit = "future"
invoker = "Http"
url = "mock://echo/http-ok""#,
            Invoker::Http,
        ),
        (
            r#"id = "future.llm"
toolkit = "future"
invoker = "Llm"
prompt = "Summarize {{input}}""#,
            Invoker::Llm,
        ),
        (
            r#"id = "future.wasm"
toolkit = "future"
invoker = "Wasm"
wasm_path = "/tmp/plugin.wasm""#,
            Invoker::Wasm,
        ),
    ];
    for (toml, expected) in cases {
        let meta = parse_fixture_tool(toml).expect("configured PRD invoker should parse");
        assert_eq!(meta.invoker, expected);
    }
}

#[test]
fn 도구킷_manifest에서_체인_필드는_호출자_기본값을_체인으로_둔다() {
    let meta = parse_fixture_tool(
        r#"id = "chain.default_invoker"
toolkit = "chain"
steps = [{ tool = "text.uppercase" }]"#,
    )
    .expect("chain field should imply Chain invoker");
    assert_eq!(meta.invoker, Invoker::Chain);
}

#[test]
fn 명시적_체인_호출자는_체인_단계를_요구한다() {
    match parse_fixture_tool(
        r#"id = "chain.missing_steps"
toolkit = "chain"
invoker = "Chain""#,
    ) {
        Err(LoadError::EmptyChain) => {}
        other => panic!("explicit Chain invoker without steps must fail fast, got {other:?}"),
    }
}

#[test]
fn 파싱은_중복된_체인_노드_id를_거부한다() {
    match parse_fixture_tool(
        r#"id = "chain.duplicate_nodes"
toolkit = "chain"
invoker = "Chain"
steps = [
  { id = "normalize", tool = "text.uppercase" },
  { id = " normalize ", tool = "text.lowercase" },
]"#,
    ) {
        Err(LoadError::DuplicateChainStepId { position, id }) => {
            assert_eq!(position, 1);
            assert_eq!(id, "normalize");
        }
        other => panic!("duplicate normalized chain node ids must fail fast, got {other:?}"),
    }
}

#[test]
fn 파싱은_알수없는_체인_연결_끝점을_거부한다() {
    match parse_fixture_tool(
        r#"id = "chain.bad_connection"
toolkit = "chain"
invoker = "Chain"
connections = [{ from = "start", to = "missing" }]
steps = [{ id = "start", tool = "text.uppercase" }]"#,
    ) {
        Err(LoadError::UnknownChainConnectionStep { position, step }) => {
            assert_eq!(position, 0);
            assert_eq!(step, "missing");
        }
        other => panic!("unknown chain connection endpoint must fail fast, got {other:?}"),
    }
}

#[test]
fn 파싱은_빈_체인_연결_끝점을_거부한다() {
    match parse_fixture_tool(
        r#"id = "chain.empty_connection"
toolkit = "chain"
invoker = "Chain"
connections = [{ from = "start", to = " " }]
steps = [{ id = "start", tool = "text.uppercase" }]"#,
    ) {
        Err(LoadError::EmptyChainConnection { position, field }) => {
            assert_eq!(position, 0);
            assert_eq!(field, "to");
        }
        other => panic!("empty chain connection endpoint must fail fast, got {other:?}"),
    }
}

#[test]
fn 파싱은_체인_연결_순환을_거부한다() {
    match parse_fixture_tool(
        r#"id = "chain.cycle"
toolkit = "chain"
invoker = "Chain"
connections = [
  { from = "a", to = "b" },
  { from = "b", to = "a" },
]
steps = [
  { id = "a", tool = "text.uppercase" },
  { id = "b", tool = "text.lowercase" },
]"#,
    ) {
        Err(LoadError::ChainConnectionCycle) => {}
        other => panic!("cyclic chain connections must fail fast, got {other:?}"),
    }
}

#[test]
fn 파싱은_퇴역한_체인_의존_필드를_거부한다() {
    assert!(matches!(
        parse_fixture_tool(
            r#"id = "chain.retired_depends"
toolkit = "chain"
invoker = "Chain"
steps = [
  { id = "uppercase", tool = "text.uppercase" },
  { id = "lowercase", tool = "text.lowercase", depends = ["uppercase"] },
]"#
        ),
        Err(LoadError::Toml(_))
    ));
}

#[test]
fn 파싱은_빈_고정_보드를_거부한다() {
    // Iter 197: pre-iter-197 the loader accepted `boards = [""]`
    // → silent dead data (boards.rs::slugify guarantees non-empty
    // board keys, so the empty entry never matched anything).
    // Pin three sub-cases: first-position, mid-position, whitespace-only.
    for (toml_value, expected_pos) in [
        (r#"["", "dev"]"#, 0),
        (r#"["dev", "", "trading"]"#, 1),
        (r#"["dev", "   "]"#, 1),
    ] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   boards = {toml_value}"#,
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::EmptyBoard { position }) => {
                assert_eq!(
                    position, expected_pos,
                    "boards {toml_value}: expected position {expected_pos}, got {position}"
                );
            }
            other => {
                panic!("boards {toml_value}: expected EmptyPinnedBoard, got {other:?}")
            }
        }
    }
}

#[test]
fn 파싱은_빈_선택자_바인딩_필드나_선택자를_거부한다() {
    // Iter 197: empty field → unmatchable typed input name;
    // empty selector → querySelector("") that targets nothing
    // (or throws). Reject both at the loader boundary so the
    // user sees the typo before the desktop UI silently misbehaves.
    let cases = [
        (r#"[{ field = "", selector = ".q" }]"#, 0, "field"),
        (r#"[{ field = "input", selector = "" }]"#, 0, "selector"),
        (
            r#"[{ field = "input", selector = ".q" }, { field = "", selector = ".r" }]"#,
            1,
            "field",
        ),
        (r#"[{ field = "input", selector = "  " }]"#, 0, "selector"),
    ];
    for (toml_value, expected_pos, expected_missing) in cases {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   controlled_embed = {{ bindings = {toml_value} }}"#,
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::EmptySelectorBinding { position, missing }) => {
                assert_eq!(
                    position, expected_pos,
                    "case {toml_value}: position {expected_pos}, got {position}"
                );
                assert_eq!(
                    missing, expected_missing,
                    "case {toml_value}: missing `{expected_missing}`, got `{missing}`"
                );
            }
            other => panic!("case {toml_value}: expected EmptySelectorBinding, got {other:?}"),
        }
    }
}

#[test]
fn 파싱은_빈_트리거_소스를_거부한다() {
    let s = r#"id = "trigger.empty"
toolkit = "trigger"
triggers = [{ source = "   " }]"#;
    match parse_fixture_tool(s) {
        Err(LoadError::EmptyTriggerSource { position }) => assert_eq!(position, 0),
        other => panic!("empty trigger source must fail fast, got {other:?}"),
    }
}

#[test]
fn 빈_고정_보드와_선택자_메시지는_필드_이름을_밝힌다() {
    let pinned_msg = format!("{}", LoadError::EmptyBoard { position: 2 });
    assert!(pinned_msg.contains("boards[2]"));
    assert!(pinned_msg.contains("non-empty"));

    let sel_msg = format!(
        "{}",
        LoadError::EmptySelectorBinding {
            position: 0,
            missing: "field"
        }
    );
    assert!(sel_msg.contains("controlled_embed.bindings[0].field"));
    assert!(sel_msg.contains("is empty"));
}

#[test]
fn 빈_체인_단계_메시지는_위치를_밝힌다() {
    // The error message must include the offending position so
    // users can pinpoint which step entry to fix in a long chain.
    let err = LoadError::EmptyChainStep { position: 3 };
    let msg = format!("{err}");
    assert!(
        msg.contains("steps[3]"),
        "error must name the position; got `{msg}`"
    );
    assert!(
        msg.contains("non-empty"),
        "error should hint at the contract; got `{msg}`"
    );
}

#[test]
fn embed_url_필드는_도구_toml_구조체로_파싱된다() {
    // Pure-shape test: the deserializer accepts `embed_url` and
    // surfaces it on `ToolToml`. No registry interaction.
    let s = r#"id = "x.embed.cfg"
            toolkit = "x"
            pin = "Embed"
            invoker = "Embed"
            embed_url = "https://example.com/iter87""#;
    let parsed = toml::from_str::<ToolToml>(s).expect("parse");
    assert_eq!(
        parsed.embed_url.as_deref(),
        Some("https://example.com/iter87")
    );
}

#[test]
fn 누락된_embed_url_필드는_없음이다() {
    let s = r#"id = "y.x"
            toolkit = "y""#;
    let parsed = toml::from_str::<ToolToml>(s).expect("parse");
    assert!(parsed.embed_url.is_none());
}

#[test]
fn 파싱은_빈_embed_url을_거부한다() {
    // Iter 244: present-but-empty embed_url defeats the desktop
    // UI's "no URL configured" empty-state check. Pre-iter-244
    // the URL registered as Some("") and the iframe got `src=""`.
    // Three sub-cases parallel to iter-205's empty-invoker tests:
    // empty literal, all-spaces, mixed whitespace.
    for raw in [r#""""#, r#""   ""#, r#""\t\n""#] {
        let s = format!(
            r#"id = "y.x"
                   toolkit = "y"
                   embed_url = {raw}"#,
        );
        match parse_fixture_tool(&s) {
            Err(LoadError::EmptyEmbedUrl) => {}
            other => panic!("embed_url={raw}: expected EmptyEmbedUrl, got {other:?}"),
        }
    }
}

#[test]
fn 파싱은_공백이_붙은_embed_url을_잘라낸다() {
    // Iter 244: surrounding whitespace on a real URL must be
    // trimmed before storage so the iframe src matches what the
    // author wrote. End-to-end via load_and_register_dir_verbose
    // since trim happens at the register site, not parse.
    let dir = std::env::temp_dir().join("upeg_loader_iter244_padded_url");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("padded.toml"),
        single_tool_toml_str(
            r#"id = "iter244.padded_url"
toolkit = "iter244"
pin = "Embed"
invoker = "Static"
embed_url = "  https://example.com/path  ""#,
        ),
    )
    .unwrap();
    let outcome = load_and_register_dir_verbose(&dir);
    assert!(
        outcome.failed.is_empty(),
        "padded but valid URL must load: {:?}",
        outcome.failed
    );
    let url = upeg_runtime::embed_url_for("iter244.padded_url").expect("url must register");
    assert_eq!(
        url, "https://example.com/path",
        "embed_url must be trimmed before storage; got {url:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 남은_로드_오류_메시지를_고정한다() {
    // Iter 261: completes the message-format pin coverage started
    // by iter 244 / iter 256 / iter 260. Four LoadError variants
    // were left without unit pins on their Display wording —
    // EmptyInvoker, EmptyPinKind, EmptyInSurfaces, and
    // InvalidInputSpec. Each variant's message has actionable
    // content that future contributors might inadvertently strip
    // (e.g., the "omit the field" hint, the valid-set listing,
    // or the input validation detail). Pin the load-bearing pieces so
    // drift fails loudly.

    // EmptyInvoker: must name the field, explain the omit-vs-empty
    // distinction (omitting is allowed only for steps→Chain), and tell
    // the user how to fix.
    let msg = format!("{}", LoadError::EmptyInvoker);
    assert!(
        msg.contains("`invoker`"),
        "EmptyInvoker message must name the field; got `{msg}`"
    );
    assert!(
        msg.contains("omit"),
        "EmptyInvoker must mention the omission boundary; got `{msg}`"
    );
    assert!(
        msg.contains("Chain"),
        "EmptyInvoker must name the only inferred invoker; got `{msg}`"
    );

    let msg = format!("{}", LoadError::MissingInvoker);
    assert!(
        msg.contains("`invoker`"),
        "MissingInvoker message must name the field; got `{msg}`"
    );
    assert!(
        msg.contains("no implicit `Function`"),
        "MissingInvoker must explicitly reject the old default; got `{msg}`"
    );

    // EmptyPinKind: parallel structure to EmptyInvoker.
    let msg = format!("{}", LoadError::EmptyPinKind);
    assert!(
        msg.contains("`pin`"),
        "EmptyPinKind message must name the field; got `{msg}`"
    );
    assert!(
        msg.contains("omit"),
        "EmptyPinKind must hint that omitting defaults; got `{msg}`"
    );
    assert!(
        msg.contains("Inline"),
        "EmptyPinKind must name the default pin kind; got `{msg}`"
    );

    // EmptyInSurfaces: must include the position and the valid
    // surface set so the author can fix the typo without grepping.
    let msg = format!("{}", LoadError::EmptyInSurfaces { position: 2 });
    assert!(
        msg.contains("`surfaces[2]`"),
        "EmptyInSurfaces must include the offending array index; got `{msg}`"
    );
    assert!(
        msg.contains("cli/tui/desktop/pwa/ext/mcp/http"),
        "EmptyInSurfaces must list the valid surface set; got `{msg}`"
    );

    // InvalidInputSpec: must name `inputs` and echo the validation detail.
    let msg = format!(
        "{}",
        LoadError::InvalidInputSpec {
            detail: "input name ` value` must not have leading or trailing whitespace".into(),
        }
    );
    assert!(
        msg.contains("`inputs`"),
        "InvalidInputSpec must name the field; got `{msg}`"
    );
    assert!(
        msg.contains("leading or trailing whitespace"),
        "InvalidInputSpec must echo the validation detail verbatim; got `{msg}`"
    );
}

#[test]
fn 알수없는_열거형_메시지는_유효한_집합을_나열한다() {
    // Iter 261: parallel to upeg-wasm iter-136 tests
    // (`unknown_pin_message_lists_valid_set_iter136` and
    // `unknown_surface_message_lists_valid_set_iter136`). Pre-iter-261
    // the loader's Unknown* Display messages weren't unit-pinned —
    // a future drift that strips the valid-set listing would let
    // tool authors guess at the valid options. Pin all three
    // `Unknown*` variants together so the parity contract is
    // discoverable from one test.
    let pin_msg = format!("{}", LoadError::UnknownPinKind("Mauve".into()));
    assert!(
        pin_msg.contains("`Mauve`"),
        "UnknownPinKind must echo the user's input; got `{pin_msg}`"
    );
    assert!(
        pin_msg.contains("Inline/Launcher/Live/Action/Embed"),
        "UnknownPinKind must list the valid set; got `{pin_msg}`"
    );

    let invoker_msg = format!("{}", LoadError::UnknownInvoker("Cooked".into()));
    assert!(
        invoker_msg.contains("`Cooked`"),
        "UnknownInvoker must echo the user's input; got `{invoker_msg}`"
    );
    assert!(
        invoker_msg.contains("Function/External/Http/Embed"),
        "UnknownInvoker must list the valid set; got `{invoker_msg}`"
    );

    let surface_msg = format!("{}", LoadError::UnknownSurface("fax".into()));
    assert!(
        surface_msg.contains("`fax`"),
        "UnknownSurface must echo the user's input; got `{surface_msg}`"
    );
    assert!(
        surface_msg.contains("cli/tui/desktop/pwa/ext/mcp/http"),
        "UnknownSurface must list the valid set; got `{surface_msg}`"
    );
}

#[test]
fn 외부_호출자_명령_요구_메시지는_명확하다() {
    // Iter 260 (parity with iter 244 / iter 256 message-format pins):
    // pre-iter-260 the `ExternalRequiresCommand` Display message
    // wasn't pinned by a unit test. A future drift in the user-
    // facing wording — say someone removes the `command = "git"`
    // example or the field-name backticks — would ship without
    // tripping any in-crate test. Pin the load-bearing pieces
    // here.
    let msg = format!("{}", LoadError::ExternalRequiresCommand);
    assert!(
        msg.contains("invoker"),
        "error must mention the offending invoker context; got `{msg}`"
    );
    assert!(
        msg.contains("External"),
        "error must echo the invoker value; got `{msg}`"
    );
    assert!(
        msg.contains("`command`"),
        "error must name the missing field with backticks; got `{msg}`"
    );
    assert!(
        msg.contains("non-empty"),
        "error must explain the constraint (non-empty); got `{msg}`"
    );
    // Concrete example helps tool authors fix without grepping
    // upstream docs.
    assert!(
        msg.contains("git"),
        "error should give a concrete command example like `git`; got `{msg}`"
    );
}

#[test]
fn 빈_embed_url_메시지는_명확하다() {
    let msg = format!("{}", LoadError::EmptyEmbedUrl);
    assert!(
        msg.contains("`embed_url`"),
        "error must name the field; got `{msg}`"
    );
    assert!(
        msg.contains("omit"),
        "error should hint that omitting is the empty-state path; got `{msg}`"
    );
}

#[test]
fn 중첩_선택자_바인딩_필드는_도구_toml_구조체로_파싱된다() {
    let s = r##"id = "x.embed.cfg"
            toolkit = "x"
            pin = "Embed"
            invoker = "Embed"
            embed_url = "https://example.com/iter92"
            controlled_embed = { bindings = [
              { field = "input", selector = ".search-box" },
              { field = "output", selector = "#result" },
            ] }"##;
    let parsed = toml::from_str::<ToolToml>(s).expect("parse");
    let bindings = parsed
        .controlled_embed
        .expect("controlled_embed present")
        .bindings
        .expect("bindings present");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].field, "input");
    assert_eq!(bindings[0].selector, ".search-box");
    assert_eq!(bindings[1].field, "output");
    assert_eq!(bindings[1].selector, "#result");
}

#[test]
fn 누락된_중첩_선택자_바인딩_필드는_없음이다() {
    let s = r#"id = "y.x"
            toolkit = "y""#;
    let parsed = toml::from_str::<ToolToml>(s).expect("parse");
    assert!(
        parsed
            .controlled_embed
            .and_then(|controlled_embed| controlled_embed.bindings)
            .is_none()
    );
}

/// The committed trigger example is the loader's end-to-end proof that every
/// runtime-serviced source parses from real, user-copyable TOML.
const TRIGGERS_DEMO_TOML: &str = include_str!("../../../examples/tools/triggers-demo.toml");

#[test]
fn 트리거_예제_toolkit은_런타임이_처리하는_모든_소스를_적재한다() {
    let (toolkit, tools) =
        crate::parse_toolkit_full(TRIGGERS_DEMO_TOML).expect("트리거 예제는 적재되어야 한다");
    assert_eq!(toolkit.id, "trigger_demo");

    let declared: std::collections::BTreeSet<(String, Option<String>)> = tools
        .iter()
        .flat_map(|(_, tool)| tool.triggers.iter().flatten())
        .map(|trigger| (trigger.source.clone(), trigger.condition.clone()))
        .collect();
    let expected: std::collections::BTreeSet<(String, Option<String>)> = [
        ("schedule", Some("now")),
        ("schedule", Some("every:30s")),
        ("file", Some("/tmp/upeg-trigger-demo/drop.txt")),
        ("directory", Some("/tmp/upeg-trigger-demo/inbox")),
        ("clipboard", None),
        ("webhook", None),
    ]
    .into_iter()
    .map(|(source, condition)| (source.to_string(), condition.map(str::to_string)))
    .collect();
    assert_eq!(declared, expected);
}

#[test]
fn 파싱은_해석할_수_없는_schedule_주기를_거부한다() {
    // `every:soon` used to sail through the loader and then fire on every
    // one-second watch poll. It now fails at load time.
    let s = r#"id = "trigger.badschedule"
toolkit = "trigger"
triggers = [{ source = "schedule", condition = "every:soon" }]"#;
    match parse_fixture_tool(s) {
        Err(LoadError::InvalidScheduleCondition { position, error }) => {
            assert_eq!(position, 0);
            let message = error.to_string();
            assert!(message.contains("every:soon"), "{message}");
        }
        other => panic!("unreadable schedule interval must be rejected, got {other:?}"),
    }
}

#[test]
fn 파싱은_now도_every도_아닌_schedule_조건을_거부한다() {
    // Cron expressions were never implemented; they must not load.
    let s = r#"id = "trigger.cron"
toolkit = "trigger"
triggers = [{ source = "schedule", condition = "0 * * * *" }]"#;
    match parse_fixture_tool(s) {
        Err(LoadError::InvalidScheduleCondition { error, .. }) => {
            let message = error.to_string();
            assert!(message.contains("every:<duration>"), "{message}");
        }
        other => panic!("unsupported schedule condition must be rejected, got {other:?}"),
    }
}

#[test]
fn 알_수_없는_트리거_소스_메시지는_남아있는_여섯_소스를_밝힌다() {
    let message = LoadError::UnknownTriggerSource("typing".to_string()).to_string();
    assert!(
        message.contains("unknown trigger source `typing`"),
        "{message}"
    );
    for source in upeg_runtime::TRIGGER_SOURCES {
        assert!(
            message.contains(source),
            "{message}는 {source}를 나열해야 한다"
        );
    }
    // The retired `typing` source must not be advertised as a valid choice.
    assert!(
        !message.contains("/typing"),
        "{message}는 은퇴한 typing을 유효한 소스로 나열하면 안 된다"
    );
}
