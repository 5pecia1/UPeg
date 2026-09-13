use super::common::parse;
use crate::*;

fn write_tmp_toml(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("upeg_cli_validate_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, v21_single_tool_toml(content)).unwrap();
    path
}

// ─── wasm-plugin feature gate  ───────────────────────

#[cfg(feature = "wasm-plugin")]
#[test]
fn wasm_로드_하위명령은_테스트_플러그인을_로드한다() {
    // The fixture .wasm lives in upeg-wasm/tests/fixtures/. Use a    // path-relative-to-workspace lookup.
    let path = std::path::Path::new("../upeg-wasm/tests/fixtures/test_plugin.wasm");
    if !path.exists() {
        // Workspace test runner sometimes uses a different cwd; bail
        // gracefully so this test isn't a flake-source.
        eprintln!("skipping: fixture missing at {}", path.display());
        return;
    }
    let out = run(parse(&["upeg", "wasm", "load", path.to_str().unwrap()]))
        .expect("wasm load should succeed for the fixture");
    assert!(out.contains("test.wasm.echo"));
    assert!(out.contains("test.wasm.shout"));

    // Tools must now be reachable via dispatch_tool.
    let r = run(parse(&[
        "upeg",
        "call",
        "test.wasm.echo",
        "-a",
        "input=cli-route",
    ]))
    .expect("call after load");
    assert_eq!(r, "echoed: cli-route\n");
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn wasm_템플릿은_플러그인_로드에_필요한_토큰을_내보낸다() {
    let out = run(parse(&["upeg", "wasm", "template"])).unwrap();
    // Required tokens — if the plugin contract changes (the
    // `export` field, the manifest JSON shape, etc.), this asserts
    // the template stays in sync rather than rotting silently.
    for tok in [
        "extism_pdk",
        "use upeg_plugin_api::{",
        "PluginInputSpec",
        "PluginManifest",
        "PluginToolDecl",
        "#[plugin_fn]",
        "pub fn manifest",
        "PluginManifest::new(\"myplugin\")",
        "PluginToolDecl::new",
        "\"myplugin.greet\"",
        "\"myplugin_greet\"",
        "manifest.to_json()",
        "upeg-plugin-api = \"0.1\"",
        "wasm32-unknown-unknown",
        "[workspace]",
    ] {
        assert!(out.contains(tok), "wasm template missing `{tok}`:\n{out}");
    }
    assert!(
        !out.contains("\"toolkits\": ["),
        "wasm template must emit `tools`, not retired `toolkits`:\n{out}"
    );
    assert!(
        !out.contains("let m = r#"),
        "wasm template should build its manifest from upeg's typed DTOs, not raw JSON:\n{out}"
    );
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn wasm_템플릿_출력은_파이프가능한_러스트_소스이다() {
    // Should at least lex as Rust syntax (rustc would parse it).
    // We don't shell out to rustc here — just assert it isn't empty
    // and starts with the canonical `//!` doc-comment.
    let out = run(parse(&["upeg", "wasm", "template"])).unwrap();
    assert!(out.starts_with("//!"));
    assert!(
        out.len() > 200,
        "expected substantive content, got {} bytes",
        out.len()
    );
}

#[cfg(feature = "wasm-plugin")]
#[test]
fn wasm_로드_알수없는_경로는_깨끗한_오류를_반환한다() {
    let r = run(parse(&[
        "upeg",
        "wasm",
        "load",
        "/no/such/file/upeg-iter47.wasm",
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("wasm"), "got: {msg}");
            assert!(msg.contains("io") || msg.contains("No such"), "got: {msg}");
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

// ─── --resolve-chain  ─────────────────────────────

#[test]
fn 체인_해석은_내장의_체인을_허용한다() {
    let path = write_tmp_toml(
        "valid_resolve_builtins.toml",
        r#"id = "iter54.upper_then_md5"
toolkit = "iter54"
connections = [{ from = "uppercase", to = "hash" }]
steps = [
  { id = "uppercase", tool = "text.uppercase" },
  { id = "hash", tool = "hash.md5" },
]"#,
    );
    let out = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]))
    .expect("validate ok");
    assert!(out.contains("chain: 2 node(s), 1 connection(s)"));
    assert!(out.contains("chain resolved: all 2 node(s) available"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_해석은_같은_manifest_형제를_허용한다() {
    let path = write_tmp_toml(
        "valid_resolve_same_manifest_sibling.toml",
        r#"id = "iter260"

[[tools]]
id = "chain"
pegboard_units = "U1"
connections = [{ from = "echo", to = "hash" }]
steps = [
  { id = "echo", tool = "iter260.echo" },
  { id = "hash", tool = "hash.md5" },
]

[[tools]]
id = "echo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["{input}"]
"#,
    );
    let out = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]))
    .expect("same-manifest sibling chain node should validate");
    assert!(out.contains("ok: toolkit iter260 (2 tool(s))"));
    assert!(out.contains("tool: iter260.chain"));
    assert!(out.contains("tool: iter260.echo"));
    assert!(out.contains("chain: 2 node(s), 1 connection(s)"));
    assert!(out.contains("[iter260.echo, hash.md5]"));
    assert!(out.contains("chain resolved: all 2 node(s) available"));
    assert!(
        upeg_runtime::toolbox_tool("iter260.echo").is_none(),
        "validate must resolve same-file siblings without registering them globally"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_해석은_자신_참조를_거부한다() {
    let path = write_tmp_toml(
        "self_ref.toml",
        r#"id = "iter54.self"
toolkit = "iter54"
connections = [{ from = "self", to = "uppercase" }]
steps = [
  { id = "self", tool = "iter54.self" },
  { id = "uppercase", tool = "text.uppercase" },
]"#,
    );
    let r = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("recurse"), "got: {msg}");
            assert!(msg.contains("iter54.self"));
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_해석은_단계_ids를_잘라낸다() {
    // parallel to chain_dispatcher_for trim. Step ids must be    // trimmed before the self-ref check; otherwise a step like
    // `"iter240.cli_self "` (trailing space) would bypass the check
    // (compared un-trimmed to the tool's id) and surface as
    // "unknown tool id `iter240.cli_self `" instead of catching the
    // actual self-recursion. Pin both halves: padded self-ref still
    // triggers the recursion error, padded existing tool id resolves
    // cleanly.
    let path = write_tmp_toml(
        "iter240_padded_self.toml",
        r#"id = "iter240.cli_self"
toolkit = "iter240"
connections = [{ from = "self", to = "uppercase" }]
steps = [
  { id = "self", tool = "iter240.cli_self " },
  { id = "uppercase", tool = "text.uppercase" },
]"#,
    );
    let r = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("recurse"),
                "padded self-ref must trigger the recursion error post-trim; got: {msg}"
            );
        }
        other => panic!("expected self-ref ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);

    // Padded reference to a real built-in: must resolve, not appear
    // in the missing-step list.
    let path = write_tmp_toml(
        "iter240_padded_step.toml",
        r#"id = "iter240.cli_padded"
toolkit = "iter240"
connections = [{ from = "uppercase", to = "hash" }]
steps = [
  { id = "uppercase", tool = "text.uppercase " },
  { id = "hash", tool = " hash.md5" },
]"#,
    );
    let out = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]))
    .expect("padded existing-tool steps must validate cleanly post-trim");
    assert!(
        out.contains("chain resolved"),
        "padded but resolvable steps must succeed; got: {out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_해석은_누락된_단계를_거부한다() {
    let path = write_tmp_toml(
        "missing_step.toml",
        r#"id = "iter54.broken"
toolkit = "iter54"
connections = [{ from = "uppercase", to = "missing" }]
steps = [
  { id = "uppercase", tool = "text.uppercase" },
  { id = "missing", tool = "no.such.tool.zzz" },
]"#,
    );
    let r = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "--resolve-chain",
        path.to_str().unwrap(),
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("no.such.tool.zzz"),
                "missing id should appear: {msg}"
            );
            assert!(msg.contains("unknown"), "got: {msg}");
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_해석은_존재_확인_없이_건너뛴다() {
    // Same broken chain as the previous test, but without --resolve-chain
    // the validator only sanity-checks the SHAPE; missing step ids pass.
    let path = write_tmp_toml(
        "shape_only.toml",
        r#"id = "iter54.shape_only"
toolkit = "iter54"
steps = [{ tool = "no.such.tool.also_zzz" }]"#,
    );
    let out = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]))
        .expect("shape-only validate must accept missing step");
    assert!(out.starts_with("ok:"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn 검증은_id를_가린_내장을_거부한다() {
    // a TOML whose `id` matches a link-time built-in's id
    // used to register cleanly but then `toolbox_tools()` iterates
    // inventory first and runtime second — so `tool show` returned
    // the built-in's meta while `dispatch_tool` ran the TOML's
    // dispatcher. The user saw built-in metadata but unexpected
    // behaviour. Now: refuse at load time. `num.hex_to_decimal`
    // is a stable inventory entry from upeg-tools.
    let path = write_tmp_toml(
        "shadow_builtin.toml",
        r#"id = "num.hex_to_decimal"
toolkit = "num"
description = "tries to shadow the built-in""#,
    );
    let r = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("shadows a built-in"),
                "error must explain the cause; got `{msg}`"
            );
            assert!(
                msg.contains("num.hex_to_decimal"),
                "error must echo the colliding id; got `{msg}`"
            );
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 패딩된_id로_가려진_내장도_검증에서_거부된다() {
    // a padded TOML id (e.g., `id = " num.hex_to_decimal "`)
    // must be rejected before registration. With v2.1 Toolkit-local tool
    // ids, the validator no longer normalizes this legacy full-id shape into
    // a built-in collision; the canonical-id error is the boundary check.
    let path = write_tmp_toml(
        "shadow_padded.toml",
        r#"id = " num.hex_to_decimal "
toolkit = "convert""#,
    );
    let r = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(
                msg.contains("must be canonical and unpadded"),
                "padded id must still be detected; got `{msg}`"
            );
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 인벤토리_id_검사는_내장을_감지하고_알수없는_id를_거부한다() {
    // The predicate underpins three loader-side collision checks
    // (loader, wasm, mcp_import). Pin both halves: existing built-in
    // returns true, non-colliding id returns false. `num.hex_to_decimal`
    // is stable in upeg-tools' inventory.
    assert!(
        upeg_runtime::toolbox_has_id("num.hex_to_decimal"),
        "known built-in must be detected by toolbox_has_id"
    );
    assert!(
        !upeg_runtime::toolbox_has_id("not.a.real.tool.iter249.xyz"),
        "non-existent id must NOT match"
    );
    // Empty string is a valid input — must not match anything.
    assert!(
        !upeg_runtime::toolbox_has_id(""),
        "empty id must not match any inventory entry"
    );
}

#[test]
fn 고유한_id는_검증을_거쳐_로드된다() {
    // Sanity: a non-colliding id passes the new check.
    let path = write_tmp_toml(
        "unique_id.toml",
        r#"id = "iter249.my_custom_tool"
	toolkit = "iter249"
	invoker = "External"
	command = "echo""#,
    );
    let out = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]))
        .expect("non-colliding id must validate");
    assert!(out.starts_with("ok:"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn 검증은_툴박스를_오염시키지_않는다() {
    // After validate runs against a TOML with id `validate.no_register`,
    // the toolbox must NOT contain it — validate is a dry-run.
    let path = write_tmp_toml(
        "no_register.toml",
        r#"id = "validate.no_register_iter40"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).unwrap();
    assert!(
        upeg_runtime::toolbox_tool("validate.no_register_iter40").is_none(),
        "validate must not register tools",
    );
    let _ = std::fs::remove_file(path);
}

// ─── Surface gating  ─────────────────────────────────

#[test]
fn cli_표면의_도구_목록은_도구가_아닌_것을_걸러낸다() {
    let id = "test.iter41.cli_excluded";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::try_from(&serde_json::json!({
            "type": "object",
            "properties": { "n": { "type": "integer" } },
            "additionalProperties": false,
        }))
        .expect("test input spec should import"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Mcp], // mcp-only
        boards: &[],
    });

    let listed = format_tool_list_filtered(None, None, None, None, false).expect("tool list");
    assert!(
        !listed.contains(id),
        "tool with surfaces=[mcp] must not appear in CLI tool list:\n{listed}",
    );
}

#[test]
fn cli_표면은_도구가_아닌_호출을_거부한다() {
    let id = "test.iter41.cli_refused";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Mcp],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |_| Ok("would-have-run".into()));

    let r = run(parse(&["upeg", "call", id]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("not on surface `cli`"), "got: {msg}");
            assert!(
                msg.contains("mcp"),
                "expected current surfaces in error: {msg}"
            );
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

// ─── New built-in tools  ───────────────────────────

#[test]
fn 텍스트_소문자_하위명령을_검증한다() {
    let out = run(parse(&["upeg", "text", "lowercase", "Hello, World"])).unwrap();
    assert_eq!(out, "hello, world\n");
}

#[test]
fn 텍스트_대문자_하위명령을_검증한다() {
    let out = run(parse(&["upeg", "text", "uppercase", "Hello, World"])).unwrap();
    assert_eq!(out, "HELLO, WORLD\n");
}

#[test]
fn 해시_sha256_하위명령은_알려진_벡터와_일치한다() {
    let out = run(parse(&["upeg", "hash", "sha256", "abc"])).unwrap();
    assert_eq!(
        out,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n"
    );
}

#[test]
fn 시간_유닉스_현재시각_하위명령은_2020년_이후의_초를_반환한다() {
    let out = run(parse(&["upeg", "time", "epoch-now"])).unwrap();
    let n: u64 = out.trim().parse().expect("expected integer seconds");
    assert!(n > 1_577_836_800, "expected post-2020 epoch, got {n}");
}

#[test]
fn 새_도구는_도구_목록에_나타난다() {
    // Surface-listing must include the four new tools after their
    // #[upeg::tool] annotations are linked into this binary.
    let out = run(parse(&["upeg", "tool", "list"])).unwrap();
    for expected in [
        "text.lowercase",
        "text.uppercase",
        "hash.sha256",
        "time.epoch_now",
    ] {
        assert!(out.contains(expected), "missing `{expected}` in:\n{out}");
    }
}

// ─── text.split + text.join ───────────────────

#[test]
fn 텍스트_분할_결합_왕복은_cli를_통해_동작한다() {
    let split_out = run(parse(&["upeg", "text", "split", "a,b,c", ","])).unwrap();
    assert_eq!(split_out, "[\"a\",\"b\",\"c\"]\n");
    let join_out = run(parse(&["upeg", "text", "join", r#"["a","b","c"]"#, ","])).unwrap();
    assert_eq!(join_out, "a,b,c\n");
}

#[test]
fn 텍스트_분할_기본_구분자는_공백이다() {
    let out = run(parse(&["upeg", "text", "split", "  hello   world  "])).unwrap();
    assert_eq!(out, "[\"hello\",\"world\"]\n");
}

#[test]
fn 텍스트_결합_기본_구분자는_공백이다() {
    let out = run(parse(&["upeg", "text", "join", r#"["a","b","c"]"#])).unwrap();
    assert_eq!(out, "a b c\n", "default delim should be space");
}

#[test]
fn 텍스트_결합은_유효하지_않은_입력에_도구_실패를_반환한다() {
    let r = run(parse(&["upeg", "text", "join", "not json", ","]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

// ─── --dry-run ────────────────────────────────

#[test]
fn dry_run은_dispatch_없이_해결된_인자를_출력한다() {
    let out = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
        "--dry-run",
    ]))
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("--dry-run must emit valid JSON: {e}\n{out}"));
    assert_eq!(v["input"], "0xff");
}

#[test]
fn dry_run은_위치_인자와_인자_플래그를_올바른_우선순위로_결합한다() {
    // -a flags win over positional JSON. Verify dry-run reflects    // that precedence.
    let out = run(parse(&[
        "upeg",
        "call",
        "x",
        r#"{"keep": "ignored"}"#, // positional
        "-a",
        "win=yes", // -a flag
        "--dry-run",
    ]))
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("win").is_some(), "win should be present from -a");
    assert!(
        v.get("keep").is_none(),
        "positional ignored when -a present"
    );
}

#[test]
fn dry_run은_표면_게이트를_건너뛴다() {
    // A tool declaring only `surfaces = ["mcp"]` would normally be    // refused via the CLI surface gate. --dry-run should still print
    // the args — the gate only applies to actual dispatch.
    let id = "test.iter73.dry_run_mcp_only";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::new(vec![upeg_core::InputFieldSpec {
            name: upeg_core::InputName::new("n").expect("valid input name"),
            label: None,
            description: None,
            required: true,
            kind: upeg_core::InputKind::Integer,
            constraints: upeg_core::FieldConstraints::default(),
        }])
        .expect("valid InputSpec"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Mcp],
        boards: &[],
    });
    let out = run(parse(&["upeg", "call", id, "-a", "n=42", "--dry-run"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["n"], 42);
}

// ─── hash.sha1 + hash.sha512 ──────────────────

#[test]
fn 해시_sha1_하위명령은_알려진_벡터와_일치한다() {
    let out = run(parse(&["upeg", "hash", "sha1", "abc"])).unwrap();
    assert_eq!(out, "a9993e364706816aba3e25717850c26c9cd0d89d\n");
}

#[test]
fn 해시_sha512_하위명령은_알려진_벡터와_일치한다() {
    let out = run(parse(&["upeg", "hash", "sha512", "abc"])).unwrap();
    assert_eq!(out.trim_end_matches('\n').len(), 128);
    assert!(out.starts_with("ddaf35a193617aba"));
}

#[test]
fn 해시_도구군은_hash_태그_도구_목록에_나타난다() {
    let listed = run(parse(&["upeg", "tool", "list", "--tag", "hash"])).unwrap();
    for expected in ["hash.md5", "hash.sha1", "hash.sha256", "hash.sha512"] {
        assert!(
            listed.contains(expected),
            "expected `{expected}` in --toolkit=hash listing:\n{listed}"
        );
    }
}

// ─── stdin args sentinel ─────────────────────────

#[test]
fn 호출의_대시_인자는_표준입력_센티널로_파싱된다() {
    // `upeg call X -` keeps `-` in the parsed args field. The actual
    // stdin read happens inside `run()`; we can't unit-test that path
    // without mocking the global stdin handle, but we can confirm
    // the sentinel survives clap parsing intact.
    let cli = parse(&["upeg", "call", "num.hex_to_decimal", "-"]);
    match cli.command {
        Some(Command::Call { args, .. }) => assert_eq!(args, "-"),
        other => panic!("expected Call with args=`-`, got {other:?}"),
    }
}

// ─── tool show --json ────────────────────────────

#[test]
fn 도구_표시_json은_v1_도구_형태를_내보낸다() {
    // Cover every field the `to_json_object` helper emits so that
    // additions like `outputSchema` / `embedUrl` / `selectorBindings` cannot silently
    // drop here. Same shape pinned by upeg-core's
    // `to_json_object_includes_all_iter128_iter93_fields_iter141`
    // (the source-of-truth contract test) and MCP's
    // `tools_list_each_entry_has_full_meta_iter161`.
    let out = run(parse(&[
        "upeg",
        "tool",
        "show",
        "num.hex_to_decimal",
        "--json",
    ]))
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("--json must emit valid JSON: {e}\n{out}"));
    for required in [
        "id",
        "toolkit",
        "tool",
        "tags",
        "description",
        "inputSchema",
        "outputSchema",
        "pin",
        "pegboardUnits",
        "pegboardSpan",
        "invoker",
        "surfaces",
        "boards",
        "embedUrl",
        "selectorBindings",
    ] {
        assert!(
            v.get(required).is_some(),
            "json show missing `{required}`: {v}"
        );
    }
    assert_eq!(v["id"], "num.hex_to_decimal");
    assert_eq!(v["toolkit"], "num");
    assert_eq!(v["invoker"], "function");
    // selectorBindings is always an array (empty when none registered).
    assert!(
        v["selectorBindings"].is_array(),
        "selectorBindings must always be an array; got {v}"
    );
}

#[test]
fn 도구_표시_json_알수없는_id는_알수없는_도구_오류를_반환한다() {
    let r = run(parse(&[
        "upeg",
        "tool",
        "show",
        "no.such.tool.iter63",
        "--json",
    ]));
    assert_eq!(r, Err(CliError::UnknownTool("no.such.tool.iter63".into())));
}

#[test]
fn json_플래그_없이_호출하면_도구_표시는_기존_텍스트_출력을_유지한다() {
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).unwrap(); // Tabular form preserves the original contract.
    assert!(out.contains("id            num.hex_to_decimal"));
    assert!(out.contains("pin   Inline"));
}

#[test]
fn 도구_표시는_입력들_섹션을_포함한다() {
    // text-form `tool show` must surface the input schema
    // fields so CLI users can discover a tool's args without falling
    // back to `--json`. hex_to_decimal has a single required string field
    // `input` with a description.
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).unwrap();
    // Header line with field count.
    assert!(
        out.contains("inputs        1 field(s)"),
        "tool show must surface the inputs header with field count; got:\n{out}"
    );
    // Indented field row: name (type, required) — description.
    // Pin substring tolerant to description copy-edits.
    assert!(out.contains("input"), "field name must appear; got:\n{out}");
    assert!(
        out.contains("(string, required)"),
        "field type + required marker must appear; got:\n{out}"
    );
}

#[test]
fn schema가_비어_있으면_도구_표시는_입력_없음을_알린다() {
    // Iter 180: tools with no typed inputs (e.g., id.uuid_v7) get    // "(none)" — same convention as boards. Pin so a future
    // edit that drops the placeholder for empty schemas is loud.
    let out = run(parse(&["upeg", "tool", "show", "id.uuid_v7"])).unwrap();
    assert!(
        out.contains("inputs        (none)"),
        "tool show must show inputs `(none)` for zero-arg tools; got:\n{out}"
    );
}

#[test]
fn 도구_표시는_description을_포함한다() {
    // text-form `tool show` must surface the tool's    // description so users see what it does before running it.
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"])).unwrap();
    assert!(
        out.contains("description   "),
        "tool show must have a description row; got:\n{out}"
    );
    // hex_to_decimal's description starts with "Parse a hex string" — pin
    // a substring stable enough to survive copy edits but specific
    // enough to prove the row carries the actual content (not the
    // "(no description)" placeholder).
    assert!(
        out.contains("Parse a hex string"),
        "tool show description row must show the tool's actual text; got:\n{out}"
    );
}

#[test]
fn 비어_있으면_도구_표시는_description이_없는_자리표시자를_사용한다() {
    // Match the TUI Detail view's convention: empty description shows
    // "(no description)" so the row never appears blank. Register a
    // throwaway runtime tool with empty description to exercise the
    // placeholder branch — built-in tools all have descriptions, so
    // there's no production tool we can use here.
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id: "test.iter122.no_desc",
        toolkit: "test",
        local_id: "iter122.no_desc",
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    let out = run(parse(&["upeg", "tool", "show", "test.iter122.no_desc"])).unwrap();
    assert!(
        out.contains("description   (no description)"),
        "empty description must render as `(no description)` placeholder; got:\n{out}"
    );
}

// ─── iso_now + json_minify ─────────────────────────

#[test]
fn 시간_iso_현재시각_하위명령은_iso_8601_형태를_반환한다() {
    let out = run(parse(&["upeg", "time", "iso-now"])).unwrap();
    let s = out.trim_end_matches('\n');
    assert_eq!(s.len(), 20, "expected `YYYY-MM-DDTHH:MM:SSZ`, got {s:?}");
    assert!(s.ends_with('Z'));
    assert!(s.contains('T'));
    // Spot-check: year >= 2025.
    let y: u32 = s[..4].parse().unwrap();
    assert!(y >= 2025);
}

#[test]
fn 인코딩_json_압축_하위명령이_동작한다() {
    let out = run(parse(&[
        "upeg",
        "convert",
        "json-minify",
        "{\"a\": 1, \"b\": [2, 3]}",
    ]))
    .unwrap();
    // serde_json serializes maps in declaration order for input parsing
    // (it's `Value`'s default), so the order survives the round-trip.
    assert_eq!(out, "{\"a\":1,\"b\":[2,3]}\n");
}

#[test]
fn 시간_iso와_json_축소_도구는_목록과_dispatch_모두에_나타난다() {
    let listed = run(parse(&["upeg", "tool", "list"])).unwrap();
    for expected in ["time.iso_now", "convert.json_minify"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
    // `upeg call` parity for json_minify.
    let direct = run(parse(&["upeg", "convert", "json-minify", r#"{"x":  1}"#])).unwrap();
    let viacall = run(parse(&[
        "upeg",
        "call",
        "convert.json_minify",
        "-a",
        r#"input={"x":  1}"#,
    ]))
    .unwrap();
    assert_eq!(direct, viacall);
}

// ─── --quiet global flag ──────────────────────────

#[test]
fn 조용한_플래그는_상단_수준에서_파싱된다() {
    let cli = parse(&["upeg", "--quiet", "tool", "list"]);
    assert!(cli.quiet);
}

#[test]
fn 하위명령_뒤의_조용한_플래그도_파싱된다() {
    // global=true means clap accepts the flag at any position.
    let cli = parse(&["upeg", "tool", "list", "--quiet"]);
    assert!(cli.quiet);
}

#[test]
fn 짧은_큐_플래그도_동작한다() {
    let cli = parse(&["upeg", "-q", "tool", "list"]);
    assert!(cli.quiet);
}

#[test]
fn 조용한_플래그의_기본값은_거짓이다() {
    let cli = parse(&["upeg", "tool", "list"]);
    assert!(!cli.quiet);
}

// ─── text.trim + security.bytes_generate ──────────────────

#[test]
fn 텍스트_잘라냄_하위명령을_검증한다() {
    let out = run(parse(&["upeg", "text", "trim", "  hello  "])).unwrap();
    assert_eq!(out, "hello\n");
}

#[test]
fn 랜덤_hex_바이트_기본_하위명령이_동작한다() {
    let out = run(parse(&["upeg", "security", "bytes-generate"])).unwrap();
    let s = out.trim_end_matches('\n');
    assert_eq!(s.len(), 32, "default n=16 → 32 hex chars");
    assert!(
        s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn 랜덤_hex_바이트_사용자지정_길이_하위명령이_동작한다() {
    let out = run(parse(&["upeg", "security", "bytes-generate", "8"])).unwrap();
    assert_eq!(out.trim_end_matches('\n').len(), 16);
}

#[test]
fn 랜덤_hex_바이트_이_너무_크면_도구_실패를_반환한다() {
    let r = run(parse(&["upeg", "security", "bytes-generate", "2000"]));
    match r {
        Err(CliError::ToolFailed(msg)) => assert!(msg.contains("too large")),
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

#[test]
fn 텍스트_다듬기와_랜덤_hex_도구는_도구_목록에_나타난다() {
    let listed = run(parse(&["upeg", "tool", "list"])).unwrap();
    for expected in ["text.trim", "security.bytes_generate"] {
        assert!(
            listed.contains(expected),
            "missing `{expected}` in:\n{listed}"
        );
    }
}
