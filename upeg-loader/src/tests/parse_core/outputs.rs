//! Output-spec parsing tests. Previously textually included into
//! `tests/parse_core.rs` via `include!`; now a real child module. `use
//! super::*` inherits the parent module's helpers and imports
//! (`parse_fixture_tool`, `LoadError`, `OutputKind`, `OutputValue`, ...).
use super::*;

#[test]
fn missing_or_empty_outputs_are_allowed() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo""#;
    let meta = parse_fixture_tool(s).expect("omitted outputs must be accepted");
    assert!(meta.output_spec.fields.is_empty());

    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = []"#;
    let meta = parse_fixture_tool(s).expect("empty outputs must be accepted");
    assert!(meta.output_spec.fields.is_empty());
}

#[test]
fn parsing_accepts_outputs_and_builds_output_spec() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                    command = "echo"
                    primary_output_id = "result"
                    outputs = [
                     { name = "result", type = "string", label = "Result", description = "Echo result" },
                     { name = "count", type = "integer" },
                     { name = "mode", type = "options", options = [
                       { value = "fast", label = "Fast" },
                       { value = "safe", description = "Safe mode" },
                     ] },
                     { name = "view", type = "embedded_view", url = "https://example.com/view" },
                   ]"#;
    let meta = parse_fixture_tool(s).expect("typed outputs must load");
    assert_eq!(meta.output_spec.fields.len(), 4);
    assert_eq!(meta.primary_output_id, Some("result"));
    assert_eq!(meta.output_spec.fields[0].name, "result");
    assert_eq!(meta.output_spec.fields[0].label.as_deref(), Some("Result"));
    assert_eq!(
        meta.output_spec.fields[0].description.as_deref(),
        Some("Echo result")
    );
    assert_eq!(meta.output_spec.fields[1].kind.label(), "integer");
    match &meta.output_spec.fields[2].kind {
        OutputKind::Options(choices) => {
            assert_eq!(choices.allowed_values(), vec!["fast", "safe"]);
            assert_eq!(choices.options[0].label.as_deref(), Some("Fast"));
            assert_eq!(choices.options[1].description.as_deref(), Some("Safe mode"));
        }
        other => panic!("expected options output, got {other:?}"),
    }
    match &meta.output_spec.fields[3].kind {
        OutputKind::EmbeddedView { url } => assert_eq!(url, "https://example.com/view"),
        other => panic!("expected embedded_view output, got {other:?}"),
    }
}

#[test]
fn parsing_accepts_all_output_types() {
    for ty in [
        "string",
        "number",
        "integer",
        "boolean",
        "markdown",
        "json",
        "datetime",
        "file_path",
        "url",
        "file",
    ] {
        let s = format!(
            r#"id = "y.x"
               toolkit = "y"
               invoker = "External"
               command = "echo"
               primary_output_id = "value"
               outputs = [{{ name = "value", type = "{ty}" }}]"#,
        );
        let meta = parse_fixture_tool(&s).unwrap_or_else(|e| panic!("{ty}: {e}"));
        assert_eq!(meta.output_spec.fields[0].kind.label(), ty);
    }

    let embedded = r#"id = "y.x"
               toolkit = "y"
               invoker = "External"
               command = "echo"
               primary_output_id = "view"
               outputs = [{ name = "view", type = "embedded_view", url = "https://example.com" }]"#;
    let meta = parse_fixture_tool(embedded).expect("embedded_view output must load");
    assert_eq!(meta.output_spec.fields[0].kind.label(), "embedded_view");
}

#[test]
fn outputs_require_primary_output_id() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [{ name = "result", type = "string" }]"#;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("primary_output_id required"), "{detail}");
        }
        other => panic!("outputs without primary_output_id must fail, got {other:?}"),
    }
}

#[test]
fn primary_output_id_must_match_output_field_name() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   primary_output_id = "Result"
                   outputs = [{ name = "result", type = "string", label = "Result" }]"#;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("Result"), "{detail}");
        }
        other => panic!("primary_output_id must match output name, got {other:?}"),
    }
}

#[test]
fn primary_output_id_must_be_absent_without_outputs() {
    let s = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   primary_output_id = "result""#;

    match parse_fixture_tool(s) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("must be absent"), "{detail}");
        }
        other => panic!("primary_output_id without outputs must fail, got {other:?}"),
    }
}

#[test]
fn parsing_rejects_invalid_outputs() {
    let unknown_type = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [{ name = "blob", type = "bytes" }]"#;
    match parse_fixture_tool(unknown_type) {
        Err(LoadError::UnknownOutputType { name, kind, .. }) => {
            assert_eq!(name, "blob");
            assert_eq!(kind, "bytes");
        }
        other => panic!("unknown output type must fail, got {other:?}"),
    }

    let missing_choices = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [{ name = "mode", type = "options" }]"#;
    match parse_fixture_tool(missing_choices) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("choice inputs"), "{detail}");
        }
        other => panic!("options output without choices must fail, got {other:?}"),
    }

    let scalar_choices = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [{ name = "text", type = "string", options = [{ value = "x" }] }]"#;
    assert!(matches!(
        parse_fixture_tool(scalar_choices),
        Err(LoadError::UnexpectedOutputOptions { .. })
    ));

    let missing_url = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [{ name = "view", type = "embedded_view" }]"#;
    assert!(matches!(
        parse_fixture_tool(missing_url),
        Err(LoadError::MissingEmbeddedViewUrl { .. })
    ));

    let duplicate = r#"id = "y.x"
                   toolkit = "y"
                   invoker = "External"
                   command = "echo"
                   outputs = [
                     { name = "value", type = "string" },
                     { name = "value", type = "integer" },
                   ]"#;
    match parse_fixture_tool(duplicate) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("appears more than once"), "{detail}");
        }
        other => panic!("duplicate outputs must fail, got {other:?}"),
    }
}
