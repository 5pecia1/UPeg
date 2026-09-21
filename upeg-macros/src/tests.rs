use super::*;

#[test]
fn typed_inputs_generate_static_spec_with_required_and_optional_fields() {
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
fn typed_inputs_accept_all_closed_types() {
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
fn outputs_parse_and_generate_static_spec() {
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
fn number_constraints_emit_min_max_default_into_static_struct() {
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
fn string_constraints_emit_regex_and_default_into_static_struct() {
    let parsed =
        syn::parse_str::<ToolInput>(r#"required pattern: String(regex="^[a-z]+$", default="abc")"#)
            .unwrap();
    let tokens = static_field_constraints_expr(parsed.params.as_ref()).to_string();
    assert!(tokens.contains("StaticStringConstraints"));
    assert!(tokens.contains("regex : Some (\"^[a-z]+$\")"));
    assert!(tokens.contains("default : Some (\"abc\")"));
}

#[test]
fn options_choice_values_emit_as_choice_array() {
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
fn timer_source_converts_to_milliseconds() {
    let source: SourceArg = syn::parse_str(r#"Timer("30s")"#).unwrap();
    let tokens = build_source_expr(Some(&source)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Timer"));
    assert!(tokens.contains("interval_ms : 30000"));
}

#[test]
fn shortcut_source_emits_as_static_string() {
    let source: SourceArg = syn::parse_str(r#"Shortcut("Cmd+Shift+N")"#).unwrap();
    let tokens = build_source_expr(Some(&source)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Shortcut"));
    assert!(tokens.contains("\"Cmd+Shift+N\""));
}

#[test]
fn manual_and_static_sources_are_simple_variants() {
    let manual: SourceArg = syn::parse_str("Manual").unwrap();
    let static_src: SourceArg = syn::parse_str("Static").unwrap();
    assert!(matches!(manual, SourceArg::Manual));
    assert!(matches!(static_src, SourceArg::Static));
    let tokens = build_source_expr(Some(&manual)).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: Manual"));
}

#[test]
fn default_source_is_user_input() {
    let tokens = build_source_expr(None).unwrap().to_string();
    assert!(tokens.contains("StaticSource :: UserInput"));
}

#[test]
fn timer_duration_parsing_accepts_multiple_suffixes() {
    use syn::LitStr;
    let make = |s: &str| LitStr::new(s, proc_macro2::Span::call_site());
    assert_eq!(parse_duration_to_ms(&make("500ms")).unwrap(), 500);
    assert_eq!(parse_duration_to_ms(&make("2s")).unwrap(), 2000);
    assert_eq!(parse_duration_to_ms(&make("1m")).unwrap(), 60_000);
    assert!(parse_duration_to_ms(&make("")).is_err());
    assert!(parse_duration_to_ms(&make("abc")).is_err());
}

#[test]
fn timer_duration_sums_compound_expressions_exactly() {
    // Delegated to the humantime crate — "1m30s" = 90s (the earlier
    // partition implementation misread it as 130ms). Unit splitting and
    // summation are left to the library.
    use syn::LitStr;
    let make = |s: &str| LitStr::new(s, proc_macro2::Span::call_site());
    assert_eq!(parse_duration_to_ms(&make("1m30s")).unwrap(), 90_000);
    assert_eq!(parse_duration_to_ms(&make("1h")).unwrap(), 3_600_000);
    assert_eq!(
        parse_duration_to_ms(&make("2s500ms")).unwrap(),
        2_500,
        "humantime also accepts ms-granularity compounds"
    );
}

#[test]
fn typed_inputs_reject_unknown_type() {
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
fn typed_inputs_reject_duplicate_field_names() {
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
fn tool_args_reject_raw_input_schema_attribute() {
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
fn typed_inputs_must_match_function_signature() {
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
fn file_input_matches_file_value_ref_parameter() {
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
fn file_input_rejects_string_parameter() {
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
fn typed_inputs_reject_missing_signature_field() {
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
fn typed_inputs_reject_names_differing_from_signature() {
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
fn typed_inputs_reject_types_differing_from_signature() {
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
fn tool_args_parse_succeeds_despite_misspelled_pin() {
    // Macro-argument parsing (`syn::parse_str::<ToolArgs>`) only checks
    // ident grammar; the variant value is validated separately by
    // `validate_enum_ident` inside `tool()` in `lib.rs`. This confirms the
    // parse and validate stages are separate.
    // (`validate_enum_ident`/`ALLOWED_PIN_KINDS` moved to
    // `upeg-tool-grammar` — their own coverage lives in that crate's tests.)
    let args: ToolArgs = syn::parse_str(
        r#"id = "text.slugify", toolkit = "text", pegboard_units = U1, pin = Inlin"#,
    )
    .unwrap();
    assert_eq!(args.pin.to_string(), "Inlin");
    assert!(validate_enum_ident(&args.pin, "pin", ALLOWED_PIN_KINDS).is_err());
}
