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

// ─── #[tool] expansion — token level ──────────────────────────

#[test]
fn tool_decl_fn_emits_export_symbol_and_pegboard_units() {
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
fn file_policy_emits_losslessly_into_plugin_input_field() {
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
    .expect("parse the File policy grammar");
    let item_fn: ItemFn =
        syn::parse_str("pub fn inspect(input: serde_json::Value) -> String { input.to_string() }")
            .expect("parse the tool function");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Inspect", None)
        .expect("generate the decl fn with the policy")
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
fn file_without_policy_omits_plugin_file_policy() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "files.inspect", toolkit = "files", pegboard_units = U1, inputs = [
            required input: File
        ]"#,
    )
    .expect("parse the policy-free File grammar");
    let item_fn: ItemFn =
        syn::parse_str("pub fn inspect(input: serde_json::Value) -> String { input.to_string() }")
            .expect("parse the tool function");

    let tokens = build_tool_decl_fn(&args, &item_fn, "Inspect", None)
        .expect("generate the decl fn without a policy")
        .to_string();

    assert!(tokens.contains("PluginInputKind :: File"));
    assert!(!tokens.contains("with_file_policy"));
    assert!(!tokens.contains("PluginFileInputPolicy"));
}

#[test]
fn export_symbol_name_follows_fixed_prefix() {
    assert_eq!(EXPORT_FN_PREFIX, "__upeg_export_");
    assert_eq!(DECL_FN_PREFIX, "__upeg_tool_decl_");
    let fn_ident: syn::Ident = syn::parse_str("greet_hello").unwrap();
    assert_eq!(export_symbol_name(&fn_ident), "__upeg_export_greet_hello");
}

#[test]
fn export_wrapper_calls_with_required_string_param_as_ref() {
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
fn export_wrapper_passes_option_param_through() {
    let item_fn: ItemFn =
        syn::parse_str("pub fn maybe_double(n: Option<i32>) -> Option<i32> { n.map(|x| x * 2) }")
            .unwrap();
    let params = collect_guest_params(&item_fn).expect("params classified");

    let tokens = build_export_wrapper(&item_fn, &params).to_string();

    assert!(tokens.contains("Option < i32 >"));
    assert!(tokens.contains("maybe_double (n)"));
}

#[test]
fn export_wrapper_passes_option_string_param_via_as_deref() {
    let item_fn: ItemFn = syn::parse_str(
        "pub fn maybe_shout(text: Option<&str>) -> String { text.unwrap_or(\"\").to_string() }",
    )
    .unwrap();
    let params = collect_guest_params(&item_fn).expect("params classified");

    let tokens = build_export_wrapper(&item_fn, &params).to_string();

    assert!(tokens.contains("maybe_shout (text . as_deref ())"));
}

// ─── upeg_plugin! expansion — token level ────────────────────

#[test]
fn plugin_macro_emits_manifest_export_and_calls_each_tool_decl() {
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
fn plugin_macro_includes_tags_and_description() {
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
fn plugin_macro_rejects_empty_tool_list() {
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
fn plugin_macro_rejects_duplicate_tool_names() {
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
fn plugin_macro_rejects_missing_toolkit() {
    let err = expect_err(
        syn::parse_str::<PluginArgs>("tools: [greet_hello]"),
        "missing toolkit must be rejected",
    );
    assert!(err.to_string().contains("toolkit"));
}

// ─── Forbidden keys — #[tool] ───────────────────────────────

#[test]
fn forbidden_invoker_key_is_compile_error() {
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
fn forbidden_boards_key_is_compile_error() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(
            r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, boards = ["dev"]"#,
        ),
        "boards must be rejected on the guest macro",
    );
    assert!(err.to_string().contains("boards` is not allowed"));
}

#[test]
fn forbidden_source_key_is_compile_error() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(
            r#"id = "greet.hello", toolkit = "greet", pegboard_units = U1, source = Manual"#,
        ),
        "source must be rejected on the guest macro",
    );
    assert!(err.to_string().contains("source` is not allowed"));
}

#[test]
fn missing_pegboard_units_is_compile_error() {
    let err = expect_err(
        syn::parse_str::<ToolArgs>(r#"id = "greet.hello", toolkit = "greet""#),
        "missing pegboard_units must be rejected",
    );
    assert!(err.to_string().contains("pegboard_units"));
}

#[test]
fn unknown_pegboard_units_value_is_rejected_at_validation() {
    // Parsing itself (`syn::parse_str::<ToolArgs>`) only checks ident
    // grammar; the value is validated by `validate_enum_ident` inside
    // `tool()` in `lib.rs` (same parse/validate split as upeg-macros —
    // `upeg_tool_grammar` covers the validation logic itself).
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

// ─── Signature ⇄ inputs validation ──────────────────────────

#[test]
fn required_input_requires_non_option_param() {
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
fn optional_input_requires_option_param() {
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
fn signature_validation_rejects_arity_mismatch() {
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
fn signature_validation_rejects_name_mismatch() {
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
fn signature_validation_rejects_type_family_mismatch() {
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
fn signature_validation_returns_classified_params_on_match() {
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

// ─── Tool identity / display label reuse check (grammar wiring) ─

#[test]
fn tool_identity_validation_reused_from_grammar_crate() {
    let id: LitStr = syn::parse_str("\"foo.bar\"").unwrap();
    let toolkit: LitStr = syn::parse_str("\"baz\"").unwrap();
    let err = validate_tool_identity(&id, &toolkit)
        .expect_err("id must start with its owning toolkit prefix");
    assert!(err.to_string().contains("Toolkit prefix"));
}

#[test]
fn display_label_slug_conversion_reused_from_grammar_crate() {
    assert_eq!(display_label_from_slug("greet.hello"), "Hello");
    assert!(rustdoc_first_line(&syn::parse_str("pub fn f() {}").unwrap()).is_none());
}

// ─── Inline constraints unsupported in the plugin input contract ─

#[test]
fn numeric_inline_constraints_are_rejected_on_plugin_input() {
    let args: ToolArgs = syn::parse_str(
        r#"id = "num.port", toolkit = "num", pegboard_units = U1, inputs = [ required port: Integer(min=1, max=65535) ]"#,
    )
    .unwrap();
    let item_fn: ItemFn = syn::parse_str("pub fn port(port: i32) -> i32 { port }").unwrap();

    let err = build_tool_decl_fn(&args, &item_fn, "Port", None)
        .expect_err("inline numeric constraints aren't representable in PluginInputKind");
    assert!(err.to_string().contains("inline constraints"));
}
