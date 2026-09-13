use super::*;
use syn::ItemFn;

/// `Result::expect_err` requires the `Ok` type to implement `Debug`, which
/// `syn`'s AST types (and our thin wrappers around them) don't without the
/// `extra-traits` feature this workspace doesn't enable. Mirrors the
/// `match`-based error extraction `upeg-macros`' own tests.rs already uses
/// for the same reason.
fn expect_err<T>(result: syn::Result<T>, panic_message: &str) -> syn::Error {
    match result {
        Ok(_) => panic!("{panic_message}"),
        Err(err) => err,
    }
}

// ─── #[tool] 확장 — 토큰 레벨 ─────────────────────────────────────

#[test]
fn 도구_선언_함수는_export_심볼과_pegboard_유닛을_방출한다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, inputs = [ required name: String = "Person to greet" ]"#,
    )
    .unwrap();
    let item_fn: ItemFn =
        syn::parse_str("pub fn greet_hello(name: &str) -> String { format!(\"Hello, {name}!\") }")
            .unwrap();

    let decl_fn = build_tool_decl_fn(&args, &item_fn, "Greet Hello", Some("Greet a person"))
        .expect("decl fn generated");
    let tokens = decl_fn.to_string();

    assert!(tokens.contains("__upeg_tool_decl_greet_hello"));
    assert!(tokens.contains("PluginToolDecl :: new"));
    assert!(tokens.contains("\"greet\""));
    assert!(tokens.contains("\"greet.hello\""));
    assert!(tokens.contains("\"__upeg_export_greet_hello\""));
    assert!(tokens.contains("with_pegboard_units (\"U1\")"));
    assert!(tokens.contains("with_display_label (\"Greet Hello\")"));
    assert!(tokens.contains("with_description (\"Greet a person\")"));
    assert!(tokens.contains("PluginInputField :: required (\"name\""));
    assert!(tokens.contains("PluginInputKind :: String"));
}

#[test]
fn file_정책은_plugin_input_field에_손실없이_방출된다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.inspect", toolkit = "files", pegboard_units = U1, inputs = [
            required input: File(
                max_count = 3,
                extensions = ["png", "tar.gz"],
                max_file_bytes = 5_000_000,
                max_total_bytes = 10_000_000
            )
        ]"#,
    )
    .expect("File 정책 문법을 파싱한다");
    let item_fn: ItemFn =
        syn::parse_str("pub fn inspect(input: serde_json::Value) -> String { input.to_string() }")
            .expect("도구 함수를 파싱한다");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Inspect", None)
        .expect("정책이 있는 선언 함수를 생성한다")
        .to_string();

    assert!(tokens.contains("PluginInputKind :: File"));
    assert!(tokens.contains("with_file_policy"));
    assert!(tokens.contains("PluginFileInputPolicy"));
    assert!(tokens.contains('3'));
    assert!(tokens.contains("\"png\""));
    assert!(tokens.contains("\"tar.gz\""));
    assert!(tokens.contains("5000000"));
    assert!(tokens.contains("10000000"));
}

#[test]
fn 정책이_없는_file은_plugin_file_정책을_생략한다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.inspect", toolkit = "files", pegboard_units = U1, inputs = [
            required input: File
        ]"#,
    )
    .expect("정책 없는 File 문법을 파싱한다");
    let item_fn: ItemFn =
        syn::parse_str("pub fn inspect(input: serde_json::Value) -> String { input.to_string() }")
            .expect("도구 함수를 파싱한다");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Inspect", None)
        .expect("정책 없는 선언 함수를 생성한다")
        .to_string();

    assert!(tokens.contains("PluginInputKind :: File"));
    assert!(!tokens.contains("with_file_policy"));
    assert!(!tokens.contains("PluginFileInputPolicy"));
}

#[test]
fn export_심볼_이름은_고정_접두사를_따른다() {
    assert_eq!(EXPORT_FN_PREFIX, "__upeg_export_");
    assert_eq!(DECL_FN_PREFIX, "__upeg_tool_decl_");
    let fn_ident: syn::Ident = syn::parse_str("greet_hello").unwrap();
    assert_eq!(export_symbol_name(&fn_ident), "__upeg_export_greet_hello");
}

#[test]
fn export_래퍼는_필수_문자열_파라미터를_참조로_바꿔서_호출한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn greet_hello(name: &str) -> String { format!(\"Hello, {name}!\") }")
            .unwrap();
    let params = collect_guest_params(&item_fn).expect("params classified");

    let tokens = build_export_wrapper(&item_fn, &params).to_string();

    assert!(tokens.contains("extism_pdk :: plugin_fn"));
    assert!(tokens.contains("__upeg_export_greet_hello"));
    assert!(tokens.contains("FromPluginArg"));
    assert!(tokens.contains("\"name\""));
    assert!(tokens.contains("greet_hello (& name)"));
    assert!(tokens.contains("ToPluginOutput :: to_plugin_output"));
}

#[test]
fn export_래퍼는_option_파라미터를_그대로_전달한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn maybe_double(n: Option<i32>) -> Option<i32> { n.map(|x| x * 2) }")
            .unwrap();
    let params = collect_guest_params(&item_fn).expect("params classified");

    let tokens = build_export_wrapper(&item_fn, &params).to_string();

    assert!(tokens.contains("Option < i32 >"));
    assert!(tokens.contains("maybe_double (n)"));
}

#[test]
fn export_래퍼는_option_문자열_파라미터를_as_deref로_전달한다() {
    let item_fn: ItemFn = syn::parse_str(
        "pub fn maybe_shout(text: Option<&str>) -> String { text.unwrap_or(\"\").to_string() }",
    )
    .unwrap();
    let params = collect_guest_params(&item_fn).expect("params classified");

    let tokens = build_export_wrapper(&item_fn, &params).to_string();

    assert!(tokens.contains("maybe_shout (text . as_deref ())"));
}

// ─── upeg_plugin! 확장 — 토큰 레벨 ────────────────────────────────

#[test]
fn 플러그인_매크로는_manifest_export를_방출하고_각_도구_선언을_호출한다() {
    let args: PluginArgs =
        syn::parse_str(r#"toolkit: "greet", tools: [greet_hello, greet_bye]"#).unwrap();

    let tokens = build_plugin_manifest(&args).to_string();

    assert!(tokens.contains("extism_pdk :: plugin_fn"));
    assert!(tokens.contains("pub fn manifest"));
    assert!(tokens.contains("PluginManifest :: new (\"greet\")"));
    assert!(tokens.contains("with_tool (__upeg_tool_decl_greet_hello ())"));
    assert!(tokens.contains("with_tool (__upeg_tool_decl_greet_bye ())"));
}

#[test]
fn 플러그인_매크로는_태그와_설명을_포함한다() {
    let args: PluginArgs = syn::parse_str(
        r#"toolkit: "greet", tags: ["demo", "wasm"], description: "Say hi.", tools: [greet_hello]"#,
    )
    .unwrap();

    let tokens = build_plugin_manifest(&args).to_string();

    assert!(tokens.contains("with_tags"));
    assert!(tokens.contains("\"demo\""));
    assert!(tokens.contains("\"wasm\""));
    assert!(tokens.contains("with_description (\"Say hi.\")"));
}

#[test]
fn 플러그인_매크로는_빈_도구_목록을_거부한다() {
    let err = expect_err(
        syn::parse_str::<PluginArgs>(r#"toolkit: "greet", tools: []"#),
        "empty tools list must be rejected",
    );
    assert!(
        err.to_string().contains("at least one"),
        "error should explain the empty list, got {err}"
    );
}

#[test]
fn 플러그인_매크로는_중복된_도구_이름을_거부한다() {
    let err = expect_err(
        syn::parse_str::<PluginArgs>(r#"toolkit: "greet", tools: [greet_hello, greet_hello]"#),
        "duplicate tool idents must be rejected",
    );
    assert!(
        err.to_string().contains("duplicate tool `greet_hello`"),
        "error should name the duplicate, got {err}"
    );
}

#[test]
fn 플러그인_매크로는_toolkit_누락을_거부한다() {
    let err = expect_err(
        syn::parse_str::<PluginArgs>("tools: [greet_hello]"),
        "missing toolkit must be rejected",
    );
    assert!(err.to_string().contains("toolkit"));
}

// ─── 금지된 키 — #[tool] ────────────────────────────────────────

#[test]
fn 금지된_invoker_키는_컴파일_에러다() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(
            r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, invoker = Function"#,
        ),
        "invoker must be rejected on the guest macro",
    );
    assert!(
        err.to_string().contains("invoker` is not allowed"),
        "error should name the forbidden key, got {err}"
    );
    assert!(err.to_string().contains("Wasm"));
}

#[test]
fn 금지된_boards_키는_컴파일_에러다() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(
            r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, boards = ["dev"]"#,
        ),
        "boards must be rejected on the guest macro",
    );
    assert!(err.to_string().contains("boards` is not allowed"));
}

#[test]
fn 금지된_source_키는_컴파일_에러다() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(
            r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, source = Manual"#,
        ),
        "source must be rejected on the guest macro",
    );
    assert!(err.to_string().contains("source` is not allowed"));
}

#[test]
fn pegboard_units_누락은_컴파일_에러다() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(r#"id = "greet.hello", toolkit = "greet""#),
        "missing pegboard_units must be rejected",
    );
    assert!(err.to_string().contains("pegboard_units"));
}

#[test]
fn 알수없는_pegboard_units_값은_검증에서_거부된다() {
    // 파싱 자체(`syn::parse_str::<ToolArgs>`)는 ident 문법만 확인하고,
    // 값 검증은 `lib.rs`의 `tool()`에서 `validate_enum_ident`로 수행한다
    // (upeg-macros와 동일한 파싱/검증 분리 구조 — `upeg_tool_grammar`가
    // 그 검증 로직 자체의 커버리지를 담당한다).
    let args: ToolArgs =
        syn::parse_str(r#"id = "greet.hello", toolkit = "greet", pegboard_units = U3"#).unwrap();
    assert_eq!(args.pegboard_units.to_string(), "U3");
    assert!(
        validate_enum_ident(
            &args.pegboard_units,
            "pegboard_units",
            ALLOWED_PEGBOARD_UNITS,
        )
        .is_err()
    );
}

// ─── 시그니처 ⇄ inputs 검증 ──────────────────────────────────────

#[test]
fn 필수_입력은_option이_아닌_파라미터를_요구한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn greet_hello(name: Option<String>) -> String { String::new() }")
            .unwrap();
    let inputs = vec![syn::parse_str("required name: String").unwrap()];

    let err = expect_err(
        validate_guest_inputs_match_signature(&Some(inputs), &item_fn),
        "required input paired with an Option param must be rejected",
    );
    assert!(
        err.to_string().contains("is `required`"),
        "error should explain the required/Option mismatch, got {err}"
    );
}

#[test]
fn 선택_입력은_option_파라미터를_요구한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn greet_hello(name: String) -> String { String::new() }").unwrap();
    let inputs = vec![syn::parse_str("optional name: String").unwrap()];

    let err = expect_err(
        validate_guest_inputs_match_signature(&Some(inputs), &item_fn),
        "optional input paired with a non-Option param must be rejected",
    );
    assert!(
        err.to_string().contains("is `optional`"),
        "error should explain the optional/Option mismatch, got {err}"
    );
}

#[test]
fn 시그니처_검증은_개수가_일치하지_않으면_거부한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn text_repeat(input: &str, n: usize) -> String { String::new() }")
            .unwrap();
    let inputs = vec![syn::parse_str("required input: String").unwrap()];

    let err = expect_err(
        validate_guest_inputs_match_signature(&Some(inputs), &item_fn),
        "every function parameter must be declared",
    );
    assert!(err.to_string().contains("one field per function parameter"));
}

#[test]
fn 시그니처_검증은_이름이_다르면_거부한다() {
    let item_fn: ItemFn = syn::parse_str(
        "pub fn text_contains(input: &str, pattern: &str) -> String { String::new() }",
    )
    .unwrap();
    let inputs = vec![
        syn::parse_str("required input: String").unwrap(),
        syn::parse_str("required needle: String").unwrap(),
    ];

    let err = expect_err(
        validate_guest_inputs_match_signature(&Some(inputs), &item_fn),
        "input names must match parameter names",
    );
    assert!(err.to_string().contains("function parameter is `pattern`"));
}

#[test]
fn 시그니처_검증은_타입_계열이_다르면_거부한다() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn random_hex_bytes(n: usize) -> String { String::new() }").unwrap();
    let inputs = vec![syn::parse_str("required n: String").unwrap()];

    let err = expect_err(
        validate_guest_inputs_match_signature(&Some(inputs), &item_fn),
        "integer parameter must not be declared as string input",
    );
    assert!(err.to_string().contains("compatible with Integer"));
}

#[test]
fn 시그니처_검증은_일치하면_분류된_파라미터를_반환한다() {
    let item_fn: ItemFn = syn::parse_str(
        "pub fn greet_hello(name: &str, shout: Option<bool>) -> String { String::new() }",
    )
    .unwrap();
    let inputs = vec![
        syn::parse_str("required name: String").unwrap(),
        syn::parse_str("optional shout: Boolean").unwrap(),
    ];

    let params = validate_guest_inputs_match_signature(&Some(inputs), &item_fn)
        .expect("matching input metadata and function signature");
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name.to_string(), "name");
    assert_eq!(params[1].name.to_string(), "shout");
}

// ─── 도구 식별자 / 표시 라벨 재사용 확인 (grammar 배선) ────────────

#[test]
fn 도구_식별자_검증은_grammar_크레이트에서_재사용된다() {
    let id: LitStr = syn::parse_str("\"foo.bar\"").unwrap();
    let toolkit: LitStr = syn::parse_str("\"baz\"").unwrap();
    let err = validate_tool_identity(&id, &toolkit)
        .expect_err("id must start with its owning toolkit prefix");
    assert!(err.to_string().contains("Toolkit prefix"));
}

#[test]
fn 표시_라벨_슬러그_변환은_grammar_크레이트에서_재사용된다() {
    assert_eq!(display_label_from_slug("greet.hello"), "Hello");
    assert!(rustdoc_first_line(&syn::parse_str("pub fn f() {}").unwrap()).is_none());
}

// ─── 인라인 제약은 플러그인 입력 계약에서 지원하지 않는다 ──────────

#[test]
fn 숫자_인라인_제약은_플러그인_입력에서_거부된다() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "num.port", toolkit = "num", pegboard_units = U1, inputs = [ required port: Integer(min=1, max=65535) ]"#,
    )
    .unwrap();
    let item_fn: ItemFn = syn::parse_str("pub fn port(port: i32) -> i32 { port }").unwrap();

    let err = build_tool_decl_fn(&args, &item_fn, "Port", None)
        .expect_err("inline numeric constraints aren't representable in PluginInputKind");
    assert!(err.to_string().contains("inline constraints"));
}
