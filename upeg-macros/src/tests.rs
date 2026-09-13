use super::*;

#[test]
fn 타입_있는_입력은_필수와_선택_필드가_있는_정적_명세를_생성한다() {
    let inputs = vec![
        syn::parse_str::<ToolInput>(r#"required input: String = "Text""#).unwrap(),
        syn::parse_str::<ToolInput>(r#"optional n: Integer = "Count""#).unwrap(),
    ];
    let tokens = build_static_input_spec_expr(&Some(inputs))
        .expect("static input spec generated")
        .to_string();
    assert!(tokens.contains("StaticInputSpec"));
    assert!(tokens.contains("name : \"input\""));
    assert!(tokens.contains("description : Some (\"Text\")"));
    assert!(tokens.contains("required : true"));
    assert!(tokens.contains("StaticInputKind :: String"));
    assert!(tokens.contains("name : \"n\""));
    assert!(tokens.contains("description : Some (\"Count\")"));
    assert!(tokens.contains("required : false"));
    assert!(tokens.contains("StaticInputKind :: Integer"));
}

#[test]
fn 타입_있는_입력은_모든_닫힌_타입을_허용한다() {
    for (ident, _) in SUPPORTED_INPUT_TYPES {
        let src = if *ident == "Options" || *ident == "MultiOptions" {
            format!("optional x: {ident}([\"a\", \"b\"])")
        } else {
            format!("optional x: {ident}")
        };
        let parsed = syn::parse_str::<ToolInput>(&src).unwrap();
        let tokens = static_input_kind_expr(&parsed.ty, parsed.params.as_ref())
            .expect("static input kind generated")
            .to_string();
        let variant = match *ident {
            "Datetime" => "DateTime",
            other => other,
        };
        assert!(
            tokens.contains(&format!("StaticInputKind :: {variant}")),
            "{ident} should map to a StaticInputKind variant, got {tokens}"
        );
    }
}

#[test]
fn 출력은_outputs_파싱과_정적_명세를_생성한다() {
    let outputs = vec![
        syn::parse_str::<ToolOutput>(r#"result: Number = "10진수""#).unwrap(),
        syn::parse_str::<ToolOutput>(r#"view: EmbeddedView("https://transform.tools/")"#).unwrap(),
    ];
    let tokens = build_static_output_spec_expr(&Some(outputs))
        .expect("static output spec generated")
        .to_string();
    assert!(tokens.contains("StaticOutputSpec"));
    assert!(tokens.contains("StaticOutputKind :: Number"));
    assert!(tokens.contains("EmbeddedView"));
    assert!(tokens.contains("\"https://transform.tools/\""));
    assert!(tokens.contains("name : \"result\""));
    assert!(tokens.contains("name : \"view\""));
}

#[test]
fn 숫자_제약은_min_max_default를_정적_구조로_방출한다() {
    let parsed =
        syn::parse_str::<ToolInput>("required port: Number(min=1, max=65535, default=8080)")
            .unwrap();
    let tokens = static_field_constraints_expr(parsed.params.as_ref()).to_string();
    assert!(tokens.contains("StaticNumberConstraints"));
    assert!(tokens.contains("min : Some (1f64)"));
    assert!(tokens.contains("max : Some (65535f64)"));
    assert!(tokens.contains("default : Some (8080f64)"));
}

#[test]
fn 문자열_제약은_regex와_default를_정적_구조로_방출한다() {
    let parsed =
        syn::parse_str::<ToolInput>(r#"required pattern: String(regex="^[a-z]+$", default="abc")"#)
            .unwrap();
    let tokens = static_field_constraints_expr(parsed.params.as_ref()).to_string();
    assert!(tokens.contains("StaticStringConstraints"));
    assert!(tokens.contains("regex : Some (\"^[a-z]+$\")"));
    assert!(tokens.contains("default : Some (\"abc\")"));
}

#[test]
fn options_choice_값은_선택지_배열로_방출된다() {
    let parsed =
        syn::parse_str::<ToolInput>(r#"required base: Options(["hex", "dec", "bin"])"#).unwrap();
    let tokens = static_input_kind_expr(&parsed.ty, parsed.params.as_ref())
        .expect("static input kind generated")
        .to_string();
    assert!(tokens.contains("StaticInputKind :: Options"));
    assert!(tokens.contains("\"hex\""));
    assert!(tokens.contains("\"dec\""));
    assert!(tokens.contains("\"bin\""));
}

#[test]
fn 소스_타이머는_밀리초로_변환된다() {
    let source: SourceArg = syn::parse_str(r#"Timer("30s")"#).unwrap();
    let tokens = build_source_expr(Some(&source)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Timer"));
    assert!(tokens.contains("interval_ms : 30000"));
}

#[test]
fn 소스_단축키는_정적_문자열로_방출된다() {
    let source: SourceArg = syn::parse_str(r#"Shortcut("Cmd+Shift+N")"#).unwrap();
    let tokens = build_source_expr(Some(&source)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Shortcut"));
    assert!(tokens.contains("\"Cmd+Shift+N\""));
}

#[test]
fn 소스_매뉴얼과_스태틱은_단순_변형이다() {
    let manual: SourceArg = syn::parse_str("Manual").unwrap();
    let static_src: SourceArg = syn::parse_str("Static").unwrap();
    assert!(matches!(manual, SourceArg::Manual));
    assert!(matches!(static_src, SourceArg::Static));
    let tokens = build_source_expr(Some(&manual)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Manual"));
}

#[test]
fn 소스_기본값은_user_input이다() {
    let tokens = build_source_expr(None).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: UserInput"));
}

#[test]
fn 소스_타이머_지속시간_파싱은_여러_접미사를_받는다() {
    use syn::LitStr;
    let make = |s: &str| LitStr::new(s, proc_macro2::Span::call_site());
    assert_eq!(parse_duration_to_ms(&make("500ms")).unwrap(), 500);
    assert_eq!(parse_duration_to_ms(&make("2s")).unwrap(), 2000);
    assert_eq!(parse_duration_to_ms(&make("1m")).unwrap(), 60_000);
    assert!(parse_duration_to_ms(&make("")).is_err());
    assert!(parse_duration_to_ms(&make("abc")).is_err());
}

#[test]
fn 소스_타이머_지속시간은_합성된_표현을_정확히_더한다() {
    // humantime crate에 위임 — "1m30s" = 90s (이전 partition 구현은
    // 130ms로 잘못 해석했음). 단위 분리·합산은 라이브러리에 맡긴다.
    use syn::LitStr;
    let make = |s: &str| LitStr::new(s, proc_macro2::Span::call_site());
    assert_eq!(parse_duration_to_ms(&make("1m30s")).unwrap(), 90_000);
    assert_eq!(parse_duration_to_ms(&make("1h")).unwrap(), 3_600_000);
    assert_eq!(
        parse_duration_to_ms(&make("2s500ms")).unwrap(),
        2_500,
        "humantime은 ms 단위의 합성도 받아들인다"
    );
}

#[test]
fn 타입_있는_입력은_알_수_없는_타입을_거부한다() {
    let parsed = syn::parse_str::<ToolInput>("optional blob: Bytes").unwrap();
    let err = match build_static_input_spec_expr(&Some(vec![parsed])) {
        Ok(_) => panic!("unknown type should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("unknown input type `Bytes`"),
        "error should name bad type, got {err}"
    );
}

#[test]
fn 타입_있는_입력은_중복_필드_이름을_거부한다() {
    let inputs = vec![
        syn::parse_str::<ToolInput>("required input: String").unwrap(),
        syn::parse_str::<ToolInput>("optional input: String").unwrap(),
    ];
    let err = match build_static_input_spec_expr(&Some(inputs)) {
        Ok(_) => panic!("duplicate input field should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("duplicate input field `input`"),
        "error should name duplicate field, got {err}"
    );
}

#[test]
fn 도구_인자는_원시_입력_schema_속성을_거부한다() {
    let err = match syn::parse_str::<ToolArgs>(
        r#"id = "text.slugify", toolkit = "text", input_schema = "{\"type\":\"object\"}""#,
    ) {
        Ok(_) => panic!("raw input_schema strings must not be accepted by #[tool]"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("unknown #[tool] argument `input_schema`"),
        "error should force typed `inputs = [...]`, got {err}"
    );
}

#[test]
fn 타입_있는_입력은_함수_시그니처와_일치해야_한다() {
    let item_fn = syn::parse_str::<ItemFn>(
        "pub fn password_generate(length: usize, include_symbols: bool) -> String { String::new() }",
    )
    .unwrap();
    let inputs = vec![
        syn::parse_str::<ToolInput>("required length: Integer").unwrap(),
        syn::parse_str::<ToolInput>("optional include_symbols: Boolean").unwrap(),
    ];

    validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect("matching input metadata and function signature");
}

#[test]
fn 파일_입력은_파일값_참조_파라미터와_일치한다() {
    let item_fn = syn::parse_str::<ItemFn>(
        "pub fn pptx_extract_images(input: &FileValue) -> Result<String, String> { Ok(String::new()) }",
    )
    .unwrap();
    let inputs =
        vec![syn::parse_str::<ToolInput>(r#"required input: File = "PPTX bytes""#).unwrap()];

    validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect("`File` input must match a `&FileValue` parameter");
}

#[test]
fn 파일_입력은_문자열_파라미터를_거부한다() {
    let item_fn = syn::parse_str::<ItemFn>(
        "pub fn takes_text(input: &str) -> Result<String, String> { Ok(String::new()) }",
    )
    .unwrap();
    let inputs = vec![syn::parse_str::<ToolInput>("required input: File").unwrap()];

    let err = validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect_err("`File` input must not bind to a `&str` parameter");
    assert!(
        err.to_string().contains("uses `File`"),
        "error should name the mismatched File input, got {err}"
    );
}

#[test]
fn 타입_있는_입력은_누락된_시그니처_필드를_거부한다() {
    let item_fn = syn::parse_str::<ItemFn>(
        "pub fn text_repeat(input: &str, n: usize) -> String { String::new() }",
    )
    .unwrap();
    let inputs = vec![syn::parse_str::<ToolInput>("required input: String").unwrap()];

    let err = validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect_err("every function parameter must be declared");
    assert!(
        err.to_string().contains("one field per function parameter"),
        "error should point at arity mismatch, got {err}"
    );
}

#[test]
fn 타입_있는_입력은_시그니처와_다른_이름을_거부한다() {
    let item_fn = syn::parse_str::<ItemFn>(
        "pub fn text_contains(input: &str, pattern: &str) -> String { String::new() }",
    )
    .unwrap();
    let inputs = vec![
        syn::parse_str::<ToolInput>("required input: String").unwrap(),
        syn::parse_str::<ToolInput>("required needle: String").unwrap(),
    ];

    let err = validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect_err("input names must match parameter names");
    assert!(
        err.to_string().contains("function parameter is `pattern`"),
        "error should name the mismatched parameter, got {err}"
    );
}

#[test]
fn 타입_있는_입력은_시그니처와_다른_타입을_거부한다() {
    let item_fn =
        syn::parse_str::<ItemFn>("pub fn random_hex_bytes(n: usize) -> String { String::new() }")
            .unwrap();
    let inputs = vec![syn::parse_str::<ToolInput>("required n: String").unwrap()];

    let err = validate_inputs_match_signature(&Some(inputs), &item_fn)
        .expect_err("integer parameter must not be declared as string input");
    assert!(
        err.to_string().contains("compatible with Integer"),
        "error should name expected type family, got {err}"
    );
}

#[test]
fn 오타난_pin으로_도구_매크로_인자를_파싱해도_파싱_자체는_성공한다() {
    // 매크로 인자 파싱(`syn::parse_str::<ToolArgs>`)은 ident 문법만 확인하고,
    // variant 값 검증은 `lib.rs`의 `tool()`에서 `validate_enum_ident`로 별도
    // 수행한다. 여기서는 파싱 단계와 검증 단계가 분리돼 있음을 확인한다.
    // (`validate_enum_ident`/`ALLOWED_PIN_KINDS`는 `upeg-tool-grammar`로
    // 이동했다 — 그 자체 커버리지는 그 크레이트의 테스트가 담당한다.)
    let args: ToolArgs = syn::parse_str(
        r#"id = "text.slugify", toolkit = "text", pegboard_units = U1, pin = Inlin"#,
    )
    .unwrap();
    assert_eq!(args.pin.to_string(), "Inlin");
    assert!(validate_enum_ident(&args.pin, "pin", ALLOWED_PIN_KINDS).is_err());
}
