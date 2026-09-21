use super::*;
use syn::{Ident, LitStr};

// ─── validate_tool_identity ─────────────────────────────────────────

#[test]
fn tool_identity_rejects_empty_local_name() {
    let id = LitStr::new("text.", proc_macro2::Span::call_site());
    let toolkit = LitStr::new("text", proc_macro2::Span::call_site());
    let err = validate_tool_identity(&id, &toolkit).expect_err("empty local id rejected");
    assert!(
        err.to_string().contains("non-empty local tool name"),
        "error should identify the missing local id, got {err}"
    );
}

#[test]
fn tool_identity_accepts_dotted_toolkit_and_local_name() {
    let id = LitStr::new(
        "text.extra.admin.tools.list",
        proc_macro2::Span::call_site(),
    );
    let toolkit = LitStr::new("text.extra", proc_macro2::Span::call_site());
    let local = validate_tool_identity(&id, &toolkit)
        .expect("dotted toolkit and dotted local name should be accepted");
    assert_eq!(local, "admin.tools.list");
}

#[test]
fn tool_identity_rejects_padded_dot_components() {
    let id = LitStr::new(
        "text.extra.admin. tools.list",
        proc_macro2::Span::call_site(),
    );
    let toolkit = LitStr::new("text.extra", proc_macro2::Span::call_site());
    let err = validate_tool_identity(&id, &toolkit).expect_err("padded local component");
    assert!(
        err.to_string().contains("unpadded dot components"),
        "error should identify padded dot component, got {err}"
    );

    let id = LitStr::new(
        "text. extra.admin.tools.list",
        proc_macro2::Span::call_site(),
    );
    let toolkit = LitStr::new("text. extra", proc_macro2::Span::call_site());
    let err = validate_tool_identity(&id, &toolkit).expect_err("padded toolkit component");
    assert!(
        err.to_string().contains("unpadded dot components"),
        "error should identify padded toolkit component, got {err}"
    );
}

// ─── validate_enum_ident / ALLOWED_* ────────────────────────────────

#[test]
fn bad_pin_produces_friendly_error() {
    let ident: Ident = syn::parse_str("Inlin").unwrap();
    let err = validate_enum_ident(&ident, "pin", ALLOWED_PIN_KINDS)
        .expect_err("typo'd pin variant must be rejected");
    assert_eq!(
        err.to_string(),
        "unknown pin `Inlin`; expected one of: Inline, Launcher, Live, Action, Embed, ControlledEmbed, Chain, Llm"
    );
}

#[test]
fn bad_pegboard_units_produces_friendly_error() {
    let ident: Ident = syn::parse_str("U3").unwrap();
    let err = validate_enum_ident(&ident, "pegboard_units", ALLOWED_PEGBOARD_UNITS)
        .expect_err("unknown pegboard_units variant must be rejected");
    assert_eq!(
        err.to_string(),
        "unknown pegboard_units `U3`; expected one of: U1, U2, U2T"
    );
}

#[test]
fn bad_invoker_produces_friendly_error() {
    let ident: Ident = syn::parse_str("Funtion").unwrap();
    let err = validate_enum_ident(&ident, "invoker", ALLOWED_INVOKERS)
        .expect_err("typo'd invoker variant must be rejected");
    assert_eq!(
        err.to_string(),
        "unknown invoker `Funtion`; expected one of: Function, External, Http, Static, Embed, Chain, Llm, Wasm"
    );
}

#[test]
fn bad_surfaces_element_produces_friendly_error() {
    let ident: Ident = syn::parse_str("Dsktop").unwrap();
    let err = validate_enum_ident(&ident, "surfaces", ALLOWED_SURFACES)
        .expect_err("typo'd surfaces element must be rejected");
    assert_eq!(
        err.to_string(),
        "unknown surfaces `Dsktop`; expected one of: Cli, Tui, Desktop, Pwa, Ext, Mcp, Http"
    );
}

#[test]
fn allowed_enum_idents_all_pass() {
    for allowed in [
        ALLOWED_PIN_KINDS,
        ALLOWED_PEGBOARD_UNITS,
        ALLOWED_INVOKERS,
        ALLOWED_SURFACES,
    ] {
        for &variant in allowed {
            let ident: Ident = syn::parse_str(variant).unwrap();
            validate_enum_ident(&ident, "field", allowed)
                .unwrap_or_else(|err| panic!("`{variant}` should be accepted, got {err}"));
        }
    }
}

// ─── input_type_label / SUPPORTED_INPUT_TYPES ───────────────────────

#[test]
fn input_type_label_maps_all_supported_types() {
    for (ident_name, label) in SUPPORTED_INPUT_TYPES {
        let ident: Ident = syn::parse_str(ident_name).unwrap();
        assert_eq!(
            input_type_label(&ident).unwrap(),
            *label,
            "{ident_name} should map to label `{label}`"
        );
    }
}

#[test]
fn input_type_label_rejects_unknown_type() {
    let ident: Ident = syn::parse_str("Bytes").unwrap();
    let err = input_type_label(&ident).expect_err("unknown type should be rejected");
    assert!(
        err.to_string().contains("unknown input type `Bytes`"),
        "error should name bad type, got {err}"
    );
}

// ─── ToolInput / ToolOutput / KindParams parsing ─────────────────────

#[test]
fn required_input_field_parses_name_type_and_description() {
    let parsed = syn::parse_str::<ToolInput>(r#"required input: String = "Text""#).unwrap();
    assert!(matches!(parsed.requirement, InputRequirement::Required));
    assert_eq!(parsed.name.to_string(), "input");
    assert_eq!(parsed.ty.to_string(), "String");
    assert_eq!(
        parsed.description.map(|lit| lit.value()),
        Some("Text".to_string())
    );
    assert!(parsed.params.is_none());
}

#[test]
fn output_field_parses_without_default_constraints() {
    let parsed = syn::parse_str::<ToolOutput>(r#"result: Number = "10진수""#).unwrap();
    assert_eq!(parsed.name.to_string(), "result");
    assert_eq!(parsed.ty.to_string(), "Number");
    assert_eq!(
        parsed.description.map(|lit| lit.value()),
        Some("10진수".to_string())
    );
}

#[test]
fn numeric_inline_params_parse_min_max_default() {
    let parsed =
        syn::parse_str::<ToolInput>("required port: Number(min=1, max=65535, default=8080)")
            .unwrap();
    match parsed.params {
        Some(KindParams::Numeric { min, max, default }) => {
            assert_eq!(min, Some(1.0));
            assert_eq!(max, Some(65535.0));
            assert_eq!(default, Some(8080.0));
        }
        _ => panic!("expected numeric inline params"),
    }
}

#[test]
fn string_inline_params_parse_regex_and_default() {
    let parsed =
        syn::parse_str::<ToolInput>(r#"required pattern: String(regex="^[a-z]+$", default="abc")"#)
            .unwrap();
    match parsed.params {
        Some(KindParams::StringConstraints { regex, default, .. }) => {
            assert_eq!(regex.map(|lit| lit.value()), Some("^[a-z]+$".to_string()));
            assert_eq!(default.map(|lit| lit.value()), Some("abc".to_string()));
        }
        _ => panic!("expected string inline params"),
    }
}

#[test]
fn choices_inline_params_parse_choice_array() {
    let parsed =
        syn::parse_str::<ToolInput>(r#"required base: Options(["hex", "dec", "bin"])"#).unwrap();
    match parsed.params {
        Some(KindParams::Choices(choices)) => {
            let values: Vec<String> = choices.into_iter().map(|lit| lit.value()).collect();
            assert_eq!(values, vec!["hex", "dec", "bin"]);
        }
        _ => panic!("expected choices inline params"),
    }
}

#[test]
fn embedded_view_output_parses_url() {
    let parsed =
        syn::parse_str::<ToolOutput>(r#"view: EmbeddedView("https://transform.tools/")"#).unwrap();
    match parsed.params {
        Some(KindParams::EmbedUrl(url)) => {
            assert_eq!(url.value(), "https://transform.tools/");
        }
        _ => panic!("expected embed url params"),
    }
}
