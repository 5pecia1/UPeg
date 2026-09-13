//! `cli_call` 모듈의 짝 — `upeg tool validate` 회귀 테스트만 모은다.
//! 워크스페이스 1000-LoC 파일 크기 예산 때문에 분리했다.

use super::*;

fn write_tmp_toml(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("upeg_cli_validate_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, v21_single_tool_toml(content)).unwrap();
    path
}

#[test]
fn 검증은_최소_유효한_toml을_허용한다() {
    let path = write_tmp_toml(
        "valid_minimal.toml",
        r#"id = "validate.minimal"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    let out = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]))
        .expect("validate should accept a valid TOML");
    assert!(out.starts_with("ok: toolkit validate"));
    assert!(out.contains("tool: validate.minimal"));
    assert!(out.contains("invoker=external"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn 체인_도구는_체인_단계를_되돌려준다() {
    let path = write_tmp_toml(
        "valid_chain.toml",
        r#"id = "validate.chain"
toolkit = "validate"
connections = [{ from = "alpha", to = "beta" }]
steps = [
  { id = "alpha", tool = "step.alpha" },
  { id = "beta", tool = "step.beta" },
]"#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(out.contains("chain: 2 node(s), 1 connection(s)"));
    assert!(out.contains("step.alpha"));
    assert!(out.contains("step.beta"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn description이_설정되면_validate는_그것을_표시한다() {
    // validate must echo the parsed description so authors    // can confirm the loader understood it. Empty description stays
    // suppressed (test below pins that branch).
    let path = write_tmp_toml(
        "valid_with_desc.toml",
        r#"id = "validate.with_desc"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	description = "iter 143 test fixture""#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        out.contains("description: iter 143 test fixture"),
        "validate must echo description; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 비어_있으면_검증은_description을_생략한다() {
    // Compactness: don't print "description: " with nothing after it    // for tools that don't declare one.
    let path = write_tmp_toml(
        "valid_no_desc.toml",
        r#"id = "validate.no_desc"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        !out.contains("description:"),
        "empty description must not surface a `description:` line; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 부분집합인_표면도_표시된다() {
    // surfaces shown only when narrower than ALL_SURFACES
    // (the default). A typo'd surface that silently defaulted to "all"
    // would be visible here.
    let path = write_tmp_toml(
        "valid_subset_surfaces.toml",
        r#"id = "validate.subset"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	surfaces = ["mcp", "http"]"#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        out.contains("surfaces: [mcp, http]"),
        "subset surfaces must be listed; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 기본이_전체이면_검증은_표면을_생략한다() {
    // ALL_SURFACES (the implicit default) is the common case — keep
    // the line out so the validate output stays scannable.
    let path = write_tmp_toml(
        "valid_default_surfaces.toml",
        r#"id = "validate.default_surfaces"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        !out.contains("surfaces:"),
        "default-all surfaces must not surface a `surfaces:` line; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 보드가_설정되면_validate는_그것을_표시한다() {
    // Pinned boards default empty; show only when non-empty so the
    // author can confirm the parse landed.
    let path = write_tmp_toml(
        "valid_pinned.toml",
        r#"id = "validate.pinned"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	boards = ["dev", "personal"]"#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        out.contains("boards: [dev, personal]"),
        "pinned boards must be listed; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn validate는_embed_도구에_대해_embed_url을_되돌려준다() {
    // An Embed-tool author validates their TOML and wants to    // confirm `embed_url` parsed correctly without loading the
    // desktop UI. Validate output must mention the URL.
    let path = write_tmp_toml(
        "valid_embed.toml",
        r#"id = "validate.embed"
toolkit = "validate"
pin = "Embed"
invoker = "Embed"
embed_url = "https://example.org/test""#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        out.contains("embed_url: https://example.org/test"),
        "validate must echo the embed_url so authors can confirm it parsed; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn validate는_embed_도구에_대해_선택자_바인딩을_되돌려준다() {
    // same for selector_bindings — an embed tool's binding    // table should be visible at validate time. Field names + count
    // are enough; selectors stay in the TOML.
    let path = write_tmp_toml(
        "valid_embed_bindings.toml",
        // Use double-`#` raw-string fence so the CSS selector
        // `#result` doesn't close the string early — same trick
        // used in upeg-loader's bindings tests.
        r##"id = "validate"

[[tools]]
id = "embed_bindings"
pin = "Embed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "https://example.org/"

[[tools.controlled_embed.bindings]]
role = "input"
field = "input"
selector = ".query-box"

[[tools.controlled_embed.bindings]]
role = "output"
field = "output"
selector = "#result""##,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        out.contains("controlled_embed.bindings: 2 → [input, output]"),
        "validate must echo binding count + field list; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 검증은_embed가_아닌_도구의_embed_줄을_생략한다() {
    // regular non-Embed tools shouldn't gain noise
    // — validate should keep its compact output for tools that don't
    // declare these fields.
    let path = write_tmp_toml(
        "valid_function.toml",
        r#"id = "validate.func"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(
        !out.contains("embed_url"),
        "non-embed tool must not surface an empty embed_url line; got:\n{out}"
    );
    assert!(
        !out.contains("selector_bindings"),
        "non-embed tool must not surface an empty bindings line; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 외부_도구는_명령을_되돌려준다() {
    let path = write_tmp_toml(
        "valid_ext.toml",
        r#"id = "validate.ext"
toolkit = "validate"
invoker = "External"
command = "echo"
args_template = ["hi"]"#,
    );
    let out =
        run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()])).expect("validate ok");
    assert!(out.contains("invoker=external"));
    assert!(out.contains("command: `echo`"));
    assert!(out.contains("\"hi\""));
    let _ = std::fs::remove_file(path);
}

#[test]
fn 검증은_알수없는_pin_종류를_거부한다() {
    let path = write_tmp_toml(
        "bad_pin.toml",
        r#"id = "validate.bad"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	pin = "Mauve""#,
    );
    let r = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]));
    match r {
        Err(CliError::ToolFailed(msg)) => assert!(msg.contains("pin")),
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 검증은_빈_체인을_거부한다() {
    let path = write_tmp_toml(
        "empty_chain.toml",
        r#"id = "validate.empty_chain"
toolkit = "validate"
steps = []"#,
    );
    let r = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]));
    match r {
        Err(CliError::ToolFailed(msg)) => assert!(msg.contains("chain")),
        other => panic!("expected ToolFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn 필드가_없는_진단은_전체_원인을_포괄한다() {
    // Iter 211: With typed inputs, an empty `inputs = []` is intentional
    // and doesn't need the old "zero-field causes" diagnostic.
    // The inputs row is simply omitted when empty (no inputs = no row).
    let path = write_tmp_toml(
        "iter211_empty_schema.toml",
        r#"id = "validate.iter211.empty_schema"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	inputs = []"#,
    );
    let out = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]))
        .expect("empty inputs must validate");
    // Empty inputs are intentional - no diagnostic row needed.
    // The formatter omits the inputs row when empty.
    assert!(
        !out.contains("inputs:"),
        "empty inputs intentionally omit the inputs row; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn 입력_schema_필드도_되돌려진다() {
    // Parity with the `format_tool_show` inputs section. An author    // validating a TOML should see the schema parsed without also
    // running `tool show`. Pin that the section appears for tools
    // with a schema and is suppressed for tools without one.
    let path = write_tmp_toml(
        "iter209_with_schema.toml",
        r#"id = "validate.iter209.with_schema"
	toolkit = "validate"
	invoker = "External"
	command = "echo"
	inputs = [{ name = "input", type = "string" }, { name = "n", type = "integer" }]"#,
    );
    let out = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]))
        .expect("schema-bearing TOML must validate");
    assert!(
        out.contains("inputs:"),
        "validate output must include the inputs row when schema declared; got:\n{out}"
    );
    assert!(
        out.contains("2 field(s)"),
        "validate must report the field count; got:\n{out}"
    );
    // serde_json::Map is BTreeMap (no preserve_order feature in
    // Cargo.toml) so the field order is deterministic alphabetical.
    // A defensive `|| [n, input]` implied non-determinism that
    // doesn't actually exist; pin the alphabetical contract so a
    // future Cargo.toml feature flip (enabling preserve_order) is
    // a loud diff that forces the ordering question to be answered
    // explicitly.
    assert!(
        out.contains("[input, n]"),
        "BTreeMap-backed properties yield alphabetical order; got:\n{out}"
    );
    let _ = std::fs::remove_file(path);

    // No-schema tool: inputs row suppressed entirely (terse output for
    // plain function tools, matching the design for other optional
    // sections).
    let path2 = write_tmp_toml(
        "iter209_no_schema.toml",
        r#"id = "validate.iter209.no_schema"
	toolkit = "validate"
	invoker = "External"
	command = "echo""#,
    );
    let out2 = run(parse(&[
        "upeg",
        "tool",
        "validate",
        path2.to_str().unwrap(),
    ]))
    .expect("schemaless TOML must validate");
    assert!(
        !out2.contains("inputs:"),
        "validate must suppress the inputs row when no schema declared; got:\n{out2}"
    );
    let _ = std::fs::remove_file(path2);
}

#[test]
fn 검증은_빈_필드_오류를_거부한다() {
    // End-to-end coverage of every empty-field LoadError variant
    // (Empty{Id,Toolkit,ChainStep,Board,BindingField,Invoker,
    // PinKind,InSurfaces}) flowing through `upeg tool validate`.
    // The upeg-loader unit tests cover each variant in isolation;
    // this test verifies the user-facing message is propagated
    // correctly through `format_tool_validate` → `CliError::tool_failed`.
    // Each case must produce a ToolFailed with a discriminating
    // substring so a future merge that drops one error variant in
    // upeg-loader fails this test, not just the loader test.
    let cases = [
        // (filename, toml, expected substring in error message)
        (
            "iter208_empty_id.toml",
            r#"id = ""
toolkit = "validate""#,
            "`id`",
        ),
        (
            "iter208_empty_toolkit.toml",
            r#"id = "validate.iter208"
toolkit = "  ""#,
            "`toolkit`",
        ),
        (
            "iter208_empty_chain_step.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
connections = [{ from = "hash", to = "empty" }]
steps = [
  { id = "hash", tool = "hash.md5" },
  { id = "empty", tool = "" },
]"#,
            "steps[1]",
        ),
        (
            "iter208_empty_board.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
boards = ["dev", ""]"#,
            "boards[1]",
        ),
        (
            "iter208_empty_binding_field.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
controlled_embed = { bindings = [{ field = "", selector = ".q" }] }"#,
            "bindings[0].field",
        ),
        // EmptyInvoker / EmptyPinKind:
        (
            "iter208_empty_invoker.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
invoker = """#,
            "`invoker`",
        ),
        (
            "iter208_empty_pin.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
pin = "  ""#,
            "`pin`",
        ),
        // EmptyInSurfaces:
        (
            "iter208_empty_surface.toml",
            r#"id = "validate.iter208"
toolkit = "validate"
surfaces = ["cli", ""]"#,
            "surfaces[1]",
        ),
    ];

    for (name, toml, expected) in cases {
        let path = write_tmp_toml(name, toml);
        let r = run(parse(&["upeg", "tool", "validate", path.to_str().unwrap()]));
        match r {
            Err(CliError::ToolFailed(msg)) => {
                assert!(
                    msg.contains(expected),
                    "case `{name}`: expected error to contain `{expected}`; got `{msg}`"
                );
            }
            other => panic!("case `{name}`: expected ToolFailed, got {other:?}"),
        }
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn 누락된_파일의_검증은_깨끗한_오류를_반환한다() {
    let r = run(parse(&[
        "upeg",
        "tool",
        "validate",
        "/no/such/upeg/test/file.toml",
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => assert!(msg.contains("read")),
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}
