use super::parse_single_tool as parse_fixture_tool;
use crate::LoadError;
use upeg_core::{DEFAULT_FILE_MAX_COUNT, InputKind, MAX_FILE_INPUT_COUNT};

#[test]
fn toml_loader_converts_file_policy_into_core_contract() {
    const EXPECTED_MAX_COUNT: u32 = 7;
    const EXPECTED_MAX_FILE_BYTES: u64 = 1_024;
    const EXPECTED_MAX_TOTAL_BYTES: u64 = 4_096;

    let manifest = format!(
        r#"id = "files.upload"
toolkit = "files"
invoker = "External"
command = "echo"
inputs = [{{ name = "attachment", type = "file", extensions = [".PNG", "Tar.GZ"], max_count = {EXPECTED_MAX_COUNT}, max_file_bytes = {EXPECTED_MAX_FILE_BYTES}, max_total_bytes = {EXPECTED_MAX_TOTAL_BYTES} }}]"#
    );

    let meta = parse_fixture_tool(&manifest).expect("should parse an input with a File policy");
    let policy = match &meta.input_spec.fields[0].kind {
        InputKind::File(policy) => policy,
        other => panic!("expected a File input: {other:?}"),
    };

    assert_eq!(policy.max_count(), EXPECTED_MAX_COUNT);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(EXPECTED_MAX_FILE_BYTES));
    assert_eq!(policy.max_total_bytes(), Some(EXPECTED_MAX_TOTAL_BYTES));
}

#[test]
fn toml_loader_applies_defaults_for_omitted_file_policy() {
    let manifest = r#"id = "files.upload"
toolkit = "files"
invoker = "External"
command = "echo"
inputs = [{ name = "attachment", type = "file" }]"#;

    let meta = parse_fixture_tool(manifest).expect("should parse a File input with omitted policy");
    let policy = match &meta.input_spec.fields[0].kind {
        InputKind::File(policy) => policy,
        other => panic!("expected a File input: {other:?}"),
    };

    assert_eq!(policy.max_count(), DEFAULT_FILE_MAX_COUNT);
    assert_eq!(policy.extensions().len(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn toml_loader_rejects_file_policy_on_non_file_input() {
    for (key, value) in [
        ("extensions", r#"["png"]"#),
        ("max_count", "2"),
        ("max_file_bytes", "1024"),
        ("max_total_bytes", "4096"),
    ] {
        let manifest = format!(
            r#"id = "files.invalid"
toolkit = "files"
invoker = "External"
command = "echo"
inputs = [{{ name = "text", type = "string", {key} = {value} }}]"#
        );

        match parse_fixture_tool(&manifest) {
            Err(LoadError::UnexpectedInputFilePolicy { name, kind, field }) => {
                assert_eq!(name, "text");
                assert_eq!(kind, "string");
                assert_eq!(field, key);
            }
            other => {
                panic!("should explicitly reject the {key} policy on a non-File input: {other:?}")
            }
        }
    }
}

#[test]
fn toolkit_schema_exposes_file_policy_fields_with_exact_types() {
    let schema = crate::toolkit_schema_value().expect("should build the toolkit schema");
    let properties = schema
        .pointer("/$defs/InputFieldToml/properties")
        .and_then(serde_json::Value::as_object)
        .expect("InputFieldToml properties must exist");

    let extensions = properties
        .get("extensions")
        .expect("extensions schema must exist");
    assert!(schema_accepts_type(extensions, "array"));
    assert_eq!(
        extensions
            .pointer("/items/type")
            .and_then(|value| value.as_str()),
        Some("string")
    );

    for field in ["max_count", "max_file_bytes", "max_total_bytes"] {
        let field_schema = properties
            .get(field)
            .unwrap_or_else(|| panic!("{field} schema must exist"));
        assert!(
            schema_accepts_type(field_schema, "integer"),
            "{field} must be integer: {field_schema}"
        );
    }
}

#[test]
fn toolkit_schema_exposes_file_max_count_minimum_of_one() {
    let schema = crate::toolkit_schema_value().expect("should build the toolkit schema");
    let max_count = schema
        .pointer("/$defs/InputFieldToml/properties/max_count")
        .expect("max_count schema must exist");

    assert_eq!(
        max_count.get("minimum").and_then(serde_json::Value::as_u64),
        Some(1),
        "max_count schema must not allow 0: {max_count}"
    );
}

#[test]
fn toml_loader_converges_with_core_at_file_max_count_upper_bound() {
    for (max_count, is_valid) in [
        (MAX_FILE_INPUT_COUNT, true),
        (MAX_FILE_INPUT_COUNT + 1, false),
    ] {
        let manifest = format!(
            r#"id = "files.upload"
toolkit = "files"
invoker = "External"
command = "echo"
inputs = [{{ name = "attachment", type = "file", max_count = {max_count} }}]"#
        );

        let result = parse_fixture_tool(&manifest);

        assert_eq!(
            result.is_ok(),
            is_valid,
            "core convergence for max_count={max_count} should differ: {result:?}"
        );
    }
}

#[test]
fn toolkit_schema_exposes_file_max_count_maximum_matching_core() {
    // Given
    let schema = crate::toolkit_schema_value().expect("should build the toolkit schema");
    let max_count = schema
        .pointer("/$defs/InputFieldToml/properties/max_count")
        .expect("max_count schema must exist");

    // When
    let maximum = max_count.get("maximum").and_then(serde_json::Value::as_u64);

    // Then
    assert_eq!(maximum, Some(u64::from(MAX_FILE_INPUT_COUNT)));
}

fn schema_accepts_type(schema: &serde_json::Value, expected: &str) -> bool {
    schema.get("type").is_some_and(|kind| match kind {
        serde_json::Value::String(kind) => kind == expected,
        serde_json::Value::Array(kinds) => kinds.iter().any(|kind| kind.as_str() == Some(expected)),
        _ => false,
    }) || schema
        .get("anyOf")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|variants| {
            variants
                .iter()
                .any(|variant| schema_accepts_type(variant, expected))
        })
}
