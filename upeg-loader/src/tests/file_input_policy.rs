use super::parse_single_tool as parse_fixture_tool;
use crate::LoadError;
use upeg_core::{DEFAULT_FILE_MAX_COUNT, InputKind, MAX_FILE_INPUT_COUNT};

#[test]
fn toml_loader는_file_정책을_코어_계약으로_변환한다() {
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

    let meta = parse_fixture_tool(&manifest).expect("File 정책이 있는 입력을 파싱해야 한다");
    let policy = match &meta.input_spec.fields[0].kind {
        InputKind::File(policy) => policy,
        other => panic!("File 입력이어야 한다: {other:?}"),
    };

    assert_eq!(policy.max_count(), EXPECTED_MAX_COUNT);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(EXPECTED_MAX_FILE_BYTES));
    assert_eq!(policy.max_total_bytes(), Some(EXPECTED_MAX_TOTAL_BYTES));
}

#[test]
fn toml_loader는_생략된_file_정책에_기본값을_적용한다() {
    let manifest = r#"id = "files.upload"
toolkit = "files"
invoker = "External"
command = "echo"
inputs = [{ name = "attachment", type = "file" }]"#;

    let meta = parse_fixture_tool(manifest).expect("정책이 생략된 File 입력을 파싱해야 한다");
    let policy = match &meta.input_spec.fields[0].kind {
        InputKind::File(policy) => policy,
        other => panic!("File 입력이어야 한다: {other:?}"),
    };

    assert_eq!(policy.max_count(), DEFAULT_FILE_MAX_COUNT);
    assert_eq!(policy.extensions().len(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn toml_loader는_file이_아닌_입력의_file_정책을_거부한다() {
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
            other => panic!("비-File 입력의 {key} 정책을 명시적으로 거부해야 한다: {other:?}"),
        }
    }
}

#[test]
fn 도구킷_schema는_file_정책_필드를_정확한_타입으로_노출한다() {
    let schema = crate::toolkit_schema_value().expect("도구킷 schema를 생성해야 한다");
    let properties = schema
        .pointer("/$defs/InputFieldToml/properties")
        .and_then(serde_json::Value::as_object)
        .expect("InputFieldToml properties가 있어야 한다");

    let extensions = properties
        .get("extensions")
        .expect("extensions schema가 있어야 한다");
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
            .unwrap_or_else(|| panic!("{field} schema가 있어야 한다"));
        assert!(
            schema_accepts_type(field_schema, "integer"),
            "{field}는 integer여야 한다: {field_schema}"
        );
    }
}

#[test]
fn 도구킷_schema는_file_max_count의_최솟값을_1로_노출한다() {
    let schema = crate::toolkit_schema_value().expect("도구킷 schema를 생성해야 한다");
    let max_count = schema
        .pointer("/$defs/InputFieldToml/properties/max_count")
        .expect("max_count schema가 있어야 한다");

    assert_eq!(
        max_count.get("minimum").and_then(serde_json::Value::as_u64),
        Some(1),
        "max_count schema는 0을 허용하면 안 된다: {max_count}"
    );
}

#[test]
fn toml_loader는_file_max_count의_코어_상한에서_수렴한다() {
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
            "max_count={max_count}의 Core 수렴 결과가 달라야 한다: {result:?}"
        );
    }
}

#[test]
fn 도구킷_schema는_file_max_count의_최대값을_코어와_같이_노출한다() {
    // Given
    let schema = crate::toolkit_schema_value().expect("도구킷 schema를 생성해야 한다");
    let max_count = schema
        .pointer("/$defs/InputFieldToml/properties/max_count")
        .expect("max_count schema가 있어야 한다");

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
