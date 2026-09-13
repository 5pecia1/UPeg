//! Constraint enforcement + File-arm round-trip tests.
//!
//! Split out of `input/tests.rs` to keep that file under the 1000-line
//! budget. Helpers live here because the assertions only consume `super`'s
//! re-exports (`InputSpec`, `InputFieldSpec`, …) — none of `tests.rs`'s
//! internal fixtures.

use super::*;

fn input_name(raw: &str) -> super::InputName {
    super::InputName::new(raw).expect("test input name should be valid")
}

fn field(name: &str, required: bool, kind: super::InputKind) -> super::InputFieldSpec {
    super::InputFieldSpec::new(input_name(name), None, None, required, kind)
        .expect("test field should be valid")
}

fn numeric_field(name: &str, min: Option<f64>, max: Option<f64>) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        input_name(name),
        None,
        None,
        true,
        InputKind::Number,
        FieldConstraints {
            number: Some(NumberConstraints {
                min,
                max,
                default: None,
            }),
            string: None,
        },
    )
    .expect("numeric field should construct")
}

fn integer_field(name: &str, min: Option<f64>, max: Option<f64>) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        input_name(name),
        None,
        None,
        true,
        InputKind::Integer,
        FieldConstraints {
            number: Some(NumberConstraints {
                min,
                max,
                default: None,
            }),
            string: None,
        },
    )
    .expect("integer field should construct")
}

fn string_field_with_regex(name: &str, regex: &str) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        input_name(name),
        None,
        None,
        true,
        InputKind::String,
        FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: Some(regex.to_string()),
                placeholder: None,
                default: None,
            }),
        },
    )
    .expect("string field should construct")
}

fn args_with(key: &str, value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert(key.to_string(), value);
    m
}

#[test]
fn 숫자_제약은_최소값_미만을_거절한다() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(0));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { actual, min: Some(1.0), max: Some(65535.0), .. })
        if (actual - 0.0).abs() < f64::EPSILON
    ));
}

#[test]
fn 숫자_제약은_최대값_초과를_거절한다() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(70000));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { .. })
    ));
}

#[test]
fn 숫자_제약은_범위_내_값을_받아들인다() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(8080));
    assert!(spec.validate_json_args(&args).is_ok());
}

#[test]
fn 정수_제약도_범위를_강제한다() {
    let spec = InputSpec::new(vec![integer_field("retries", Some(0.0), Some(5.0))]).expect("spec");
    let args = args_with("retries", serde_json::json!(10));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { .. })
    ));
}

#[test]
fn 문자열_정규식_제약은_불일치를_거절한다() {
    let spec =
        InputSpec::new(vec![string_field_with_regex("slug", "^[a-z][a-z0-9-]*$")]).expect("spec");
    let bad = args_with("slug", serde_json::json!("Has Space"));
    assert!(matches!(
        spec.validate_json_args(&bad),
        Err(InputValueError::PatternMismatch { pattern, .. })
        if pattern == "^[a-z][a-z0-9-]*$"
    ));
}

#[test]
fn 문자열_정규식_제약은_일치하면_통과한다() {
    let spec =
        InputSpec::new(vec![string_field_with_regex("slug", "^[a-z][a-z0-9-]*$")]).expect("spec");
    let ok = args_with("slug", serde_json::json!("hello-world"));
    assert!(spec.validate_json_args(&ok).is_ok());
}

#[test]
fn 파일_입력은_구조화된_객체만_받는다() {
    let f = field("upload", true, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let valid = args_with(
        "upload",
        serde_json::json!({
            "name": "x.txt",
            "is_dir": false,
            "content": {"kind": "bytes", "bytes": ""},
        }),
    );
    assert!(spec.validate_json_args(&valid).is_ok());
}

#[test]
fn 파일_입력은_빈_객체를_거절한다() {
    let f = field("upload", true, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let empty = args_with("upload", serde_json::json!({}));
    assert!(matches!(
        spec.validate_json_args(&empty),
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { .. }
        ))
    ));
}

#[test]
fn 파일_입력은_content_kind_없으면_거절한다() {
    let f = field("upload", true, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let bad = args_with(
        "upload",
        serde_json::json!({
            "name": "x",
            "is_dir": false,
            "content": {"kind": "unknown"},
        }),
    );
    assert!(matches!(
        spec.validate_json_args(&bad),
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { .. }
        ))
    ));
}

#[test]
fn 파일_입력_드래프트는_json으로_직렬화된다() {
    let f = field("upload", false, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let mut state = spec.initial_form_state();
    let upload = FileValue {
        name: "hello.txt".to_string(),
        content: FileContent::Bytes(b"hi".to_vec()),
        mime: Some("text/plain".to_string()),
    };
    state.fields[0].draft = DraftInputValue::File(Some(upload));
    let json = spec
        .args_from_form_state(&state)
        .expect("file draft must serialize to json");
    assert!(json.get("upload").and_then(|v| v.get("name")).is_some());
}

#[test]
fn 파일_입력_드래프트가_없으면_args에서_누락된다() {
    // Optional File 필드가 비어있을 때 args_from_form_state가 폴백 에러로 떨어지지
    // 않고 단순히 키를 생략해야 한다 (P1 #3 회귀 방지).
    let f = field("upload", false, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let state = spec.initial_form_state();
    let json = spec
        .args_from_form_state(&state)
        .expect("optional missing File must not error");
    assert!(json.get("upload").is_none());
}

// ─── ECMA-262 pattern이 regex 크레이트에서 컴파일되지 않을 때 ────
//
// MCP 서버는 ECMA-262 `pattern`을 내보낸다. look-around와 backreference는
// 거기서는 합법이지만 `regex` 크레이트는 지원하지 않는다. 예전에는 검증
// 시점에 PatternMismatch로 보고돼서 사용자가 무엇을 입력하든 통과할 수
// 없는 필드가 만들어졌다.

const 컴파일_불가_패턴: &str = "(?<=a)b";

#[test]
fn 컴파일할_수_없는_패턴은_명세를_만들_때_거부된다() {
    let error = InputFieldSpec::with_constraints(
        input_name("token"),
        None,
        None,
        true,
        InputKind::String,
        FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: Some(컴파일_불가_패턴.to_string()),
                placeholder: None,
                default: None,
            }),
        },
    )
    .expect_err("look-behind는 regex 크레이트에서 컴파일되지 않는다");

    let InputSpecError::UncompilablePattern {
        name,
        pattern,
        detail,
    } = error
    else {
        panic!("UncompilablePattern을 기대한다: {error:?}");
    };
    assert_eq!(name.as_str(), "token");
    assert_eq!(pattern, 컴파일_불가_패턴);
    assert!(!detail.is_empty(), "왜 컴파일되지 않는지 밝혀야 한다");
}

#[test]
fn 컴파일할_수_없는_패턴을_담은_json_schema는_가져오기에서_거부된다() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "token": { "type": "string", "pattern": 컴파일_불가_패턴 } }
    });

    let error = InputSpec::try_from(&schema).expect_err("가져오기가 거부되어야 한다");

    assert!(
        matches!(
            error,
            InputAdapterError::Spec(InputSpecError::UncompilablePattern { .. })
        ),
        "값 탓이 아니라 선언 탓임을 밝혀야 한다: {error:?}"
    );
}
