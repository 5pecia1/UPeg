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
fn number_constraints_reject_below_the_minimum() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(0));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { actual, min: Some(1.0), max: Some(65535.0), .. })
        if (actual - 0.0).abs() < f64::EPSILON
    ));
}

#[test]
fn number_constraints_reject_above_the_maximum() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(70000));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { .. })
    ));
}

#[test]
fn number_constraints_accept_in_range_values() {
    let spec = InputSpec::new(vec![numeric_field("port", Some(1.0), Some(65535.0))]).expect("spec");
    let args = args_with("port", serde_json::json!(8080));
    assert!(spec.validate_json_args(&args).is_ok());
}

#[test]
fn integer_constraints_enforce_the_range_too() {
    let spec = InputSpec::new(vec![integer_field("retries", Some(0.0), Some(5.0))]).expect("spec");
    let args = args_with("retries", serde_json::json!(10));
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::OutOfRange { .. })
    ));
}

#[test]
fn string_regex_constraint_rejects_mismatches() {
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
fn string_regex_constraint_passes_on_match() {
    let spec =
        InputSpec::new(vec![string_field_with_regex("slug", "^[a-z][a-z0-9-]*$")]).expect("spec");
    let ok = args_with("slug", serde_json::json!("hello-world"));
    assert!(spec.validate_json_args(&ok).is_ok());
}

#[test]
fn file_input_accepts_only_structured_objects() {
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
fn file_input_rejects_an_empty_object() {
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
fn file_input_rejects_a_missing_content_kind() {
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
fn file_input_draft_serializes_to_json() {
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
fn absent_file_input_draft_is_omitted_from_args() {
    // When an optional File field is empty, args_from_form_state must omit
    // the key instead of falling back to an error (P1 #3 regression guard).
    let f = field("upload", false, InputKind::File(FileInputPolicy::default()));
    let spec = InputSpec::new(vec![f]).expect("spec");
    let state = spec.initial_form_state();
    let json = spec
        .args_from_form_state(&state)
        .expect("optional missing File must not error");
    assert!(json.get("upload").is_none());
}

// ─── When an ECMA-262 pattern does not compile in the regex crate ────
//
// MCP servers export ECMA-262 `pattern`s. Look-around and backreferences
// are legal there but unsupported by the `regex` crate. Previously they
// surfaced as PatternMismatch at validation time, producing a field no
// user input could ever satisfy.

const UNCOMPILABLE_PATTERN: &str = "(?<=a)b";

#[test]
fn uncompilable_pattern_is_rejected_at_spec_construction() {
    let error = InputFieldSpec::with_constraints(
        input_name("token"),
        None,
        None,
        true,
        InputKind::String,
        FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: Some(UNCOMPILABLE_PATTERN.to_string()),
                placeholder: None,
                default: None,
            }),
        },
    )
    .expect_err("look-behind does not compile in the regex crate");

    let InputSpecError::UncompilablePattern {
        name,
        pattern,
        detail,
    } = error
    else {
        panic!("expected UncompilablePattern: {error:?}");
    };
    assert_eq!(name.as_str(), "token");
    assert_eq!(pattern, UNCOMPILABLE_PATTERN);
    assert!(!detail.is_empty(), "must explain why it does not compile");
}

#[test]
fn json_schema_with_an_uncompilable_pattern_is_rejected_on_import() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "token": { "type": "string", "pattern": UNCOMPILABLE_PATTERN } }
    });

    let error = InputSpec::try_from(&schema).expect_err("import must be rejected");

    assert!(
        matches!(
            error,
            InputAdapterError::Spec(InputSpecError::UncompilablePattern { .. })
        ),
        "must show the declaration is at fault, not the value: {error:?}"
    );
}
