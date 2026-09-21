//! JSON Schema projection of [`super::StringConstraints`].
//!
//! Sibling of [`super::numeric_constraint_schema`]: `regex` maps to the
//! standard `pattern` keyword and `default` to the standard `default`
//! keyword, so external consumers (MCP `tools/list`, HTTP `/v1/tools`,
//! the generated interface inventory) get real JSON Schema rather than a
//! upeg-private extension.
//!
//! `placeholder` is deliberately NOT projected: JSON Schema has no
//! placeholder keyword and a placeholder is presentation-only, so it
//! stays a UI-surface concern (same call as choice option labels). A
//! spec that declares only a placeholder therefore round-trips through
//! JSON Schema without it.
//!
//! The two keywords have *different* kind sets, because the grammar
//! does:
//!
//!   * `pattern` (`regex=…`) belongs to [`InputKind::String`] alone —
//!     the one type whose macro/TOML grammar accepts it. `pattern` on
//!     any other kind is a declaration error rather than something to
//!     silently drop.
//!   * `default` belongs to every text-shaped kind, because the loader
//!     lowers a TOML `default = "…"` into `StringConstraints` for
//!     `string`, `options`, `markdown`, `json`, `datetime`, `file_path`,
//!     and `url` alike (see the loader's `DefaultSlot::Text`). Emitting
//!     it for `String` only silently dropped the declared default of six
//!     kinds on the way to `tools/list`, `/v1/tools`, and the interface
//!     inventory — a schema consumer could not see a default the tool
//!     itself uses.

use serde_json::{Map, Value};

use super::{FieldConstraints, InputAdapterError, InputKind, StringConstraints};

const PATTERN_SCHEMA_KEY: &str = "pattern";
const DEFAULT_SCHEMA_KEY: &str = "default";

pub(super) fn write_string_constraints(
    schema: &mut Map<String, Value>,
    kind: &InputKind,
    constraints: &FieldConstraints,
) {
    let Some(string) = constraints.string.as_ref() else {
        return;
    };
    if accepts_pattern(kind) {
        insert_text(schema, PATTERN_SCHEMA_KEY, string.regex.as_deref());
    }
    if accepts_text_default(kind) {
        insert_text(schema, DEFAULT_SCHEMA_KEY, string.default.as_deref());
    }
}

pub(super) fn string_constraints_from_schema(
    property: &str,
    kind: &InputKind,
    schema: &Map<String, Value>,
) -> Result<Option<StringConstraints>, InputAdapterError> {
    if !accepts_pattern(kind) && schema.contains_key(PATTERN_SCHEMA_KEY) {
        return Err(
            InputAdapterError::JsonSchemaStringConstraintUnsupportedKind {
                property: property.to_string(),
                keyword: PATTERN_SCHEMA_KEY,
                kind: kind.label(),
            },
        );
    }

    let regex = if accepts_pattern(kind) {
        optional_text(property, schema, PATTERN_SCHEMA_KEY)?
    } else {
        None
    };
    let default = if accepts_text_default(kind) {
        optional_text(property, schema, DEFAULT_SCHEMA_KEY)?
    } else {
        None
    };
    if regex.is_none() && default.is_none() {
        return Ok(None);
    }
    Ok(Some(StringConstraints {
        regex,
        // Presentation-only, never carried by JSON Schema (see module doc).
        placeholder: None,
        default,
    }))
}

/// Only `String` declares `regex=…`, so only `String` projects
/// `pattern`.
fn accepts_pattern(kind: &InputKind) -> bool {
    match kind {
        InputKind::String => true,
        InputKind::Number
        | InputKind::Integer
        | InputKind::Boolean
        | InputKind::Options(_)
        | InputKind::MultiOptions(_)
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url
        | InputKind::File(_) => false,
    }
}

/// Every kind the loader lowers a textual `default` into. Kept
/// exhaustive so adding an input kind forces a decision here rather
/// than silently dropping its default.
fn accepts_text_default(kind: &InputKind) -> bool {
    match kind {
        InputKind::String
        | InputKind::Options(_)
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url => true,
        InputKind::Number
        | InputKind::Integer
        | InputKind::Boolean
        | InputKind::MultiOptions(_)
        | InputKind::File(_) => false,
    }
}

fn insert_text(schema: &mut Map<String, Value>, keyword: &str, text: Option<&str>) {
    if let Some(text) = text {
        schema.insert(keyword.to_string(), Value::String(text.to_string()));
    }
}

fn optional_text(
    property: &str,
    schema: &Map<String, Value>,
    keyword: &'static str,
) -> Result<Option<String>, InputAdapterError> {
    let Some(value) = schema.get(keyword) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|text| Some(text.to_string()))
        .ok_or_else(|| InputAdapterError::JsonSchemaStringConstraintNotString {
            property: property.to_string(),
            keyword,
            kind: json_value_kind(value),
        })
}

fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::{
        ChoiceOption, ChoiceSpec, InputFieldSpec, InputName, InputSpec, StringConstraints,
    };
    use super::*;

    fn string_field(name: &str, constraints: StringConstraints) -> InputFieldSpec {
        InputFieldSpec::with_constraints(
            InputName::new(name).expect("test input name should be valid"),
            None,
            None,
            false,
            InputKind::String,
            FieldConstraints {
                number: None,
                string: Some(constraints),
            },
        )
        .expect("test field should be valid")
    }

    fn spec_of(field: InputFieldSpec) -> InputSpec {
        InputSpec::new(vec![field]).expect("test spec should be valid")
    }

    #[test]
    fn string_constraints_export_pattern_and_default_to_json_schema() {
        let spec = spec_of(string_field(
            "slug",
            StringConstraints {
                regex: Some("^[a-z-]+$".to_string()),
                placeholder: None,
                default: Some("hello-world".to_string()),
            },
        ));

        let schema = spec.to_json_schema_value();

        assert_eq!(schema["properties"]["slug"]["type"], json!("string"));
        assert_eq!(schema["properties"]["slug"]["pattern"], json!("^[a-z-]+$"));
        assert_eq!(
            schema["properties"]["slug"]["default"],
            json!("hello-world")
        );
        assert_eq!(
            InputSpec::try_from(&schema).expect("exported schema should import"),
            spec,
        );
    }

    #[test]
    fn placeholder_is_ui_only_and_not_emitted_into_json_schema() {
        let spec = spec_of(string_field(
            "slug",
            StringConstraints {
                regex: None,
                placeholder: Some("my-post-title".to_string()),
                default: None,
            },
        ));

        let schema = spec.to_json_schema_value();

        assert!(schema["properties"]["slug"].get("pattern").is_none());
        assert!(schema["properties"]["slug"].get("default").is_none());
        let imported = InputSpec::try_from(&schema).expect("exported schema should import");
        assert!(
            imported.fields[0].constraints.string.is_none(),
            "placeholder-only constraints must not survive the JSON Schema round-trip",
        );
    }

    #[test]
    fn unconstrained_string_field_exports_no_pattern() {
        let spec = InputSpec::new(vec![
            InputFieldSpec::new(
                InputName::new("plain").expect("test input name should be valid"),
                None,
                None,
                false,
                InputKind::String,
            )
            .expect("test field should be valid"),
        ])
        .expect("test spec should be valid");

        let schema = spec.to_json_schema_value();

        assert!(schema["properties"]["plain"].get("pattern").is_none());
        assert_eq!(
            InputSpec::try_from(&schema).expect("exported schema should import"),
            spec,
        );
    }

    #[test]
    fn non_string_pattern_is_rejected_on_import() {
        let schema = json!({
            "type": "object",
            "properties": { "slug": { "type": "string", "pattern": 7 } }
        });

        assert!(matches!(
            InputSpec::try_from(&schema),
            Err(InputAdapterError::JsonSchemaStringConstraintNotString {
                property,
                keyword: "pattern",
                kind: "number",
            }) if property == "slug"
        ));
    }

    #[test]
    fn pattern_on_a_non_string_kind_is_rejected_on_import() {
        let schema = json!({
            "type": "object",
            "properties": { "flag": { "type": "boolean", "pattern": "^x$" } }
        });

        assert!(matches!(
            InputSpec::try_from(&schema),
            Err(InputAdapterError::JsonSchemaStringConstraintUnsupportedKind {
                property,
                keyword: "pattern",
                kind: "boolean",
            }) if property == "flag"
        ));
    }

    fn text_field(name: &str, kind: InputKind, default: &str) -> InputFieldSpec {
        InputFieldSpec::with_constraints(
            InputName::new(name).expect("test input name should be valid"),
            None,
            None,
            false,
            kind,
            FieldConstraints {
                number: None,
                string: Some(StringConstraints {
                    regex: None,
                    placeholder: None,
                    default: Some(default.to_string()),
                }),
            },
        )
        .expect("test field should be valid")
    }

    fn options_kind() -> InputKind {
        InputKind::Options(
            ChoiceSpec::new(vec![
                ChoiceOption::new("fast", None, None).expect("test choice should be valid"),
                ChoiceOption::new("safe", None, None).expect("test choice should be valid"),
            ])
            .expect("test choice spec should be valid"),
        )
    }

    #[test]
    fn file_path_default_round_trips_through_json_schema() {
        let spec = spec_of(text_field(
            "manifest_path",
            InputKind::FilePath,
            "Cargo.toml",
        ));

        let schema = spec.to_json_schema_value();

        assert_eq!(
            schema["properties"]["manifest_path"]["default"],
            json!("Cargo.toml"),
            "the loader lowers a file_path default into StringConstraints, so the schema must show it",
        );
        assert_eq!(
            InputSpec::try_from(&schema).expect("exported schema should import"),
            spec,
        );
    }

    #[test]
    fn options_default_round_trips_through_json_schema() {
        let spec = spec_of(text_field("mode", options_kind(), "fast"));

        let schema = spec.to_json_schema_value();

        assert_eq!(
            schema["properties"]["mode"]["enum"],
            json!(["fast", "safe"])
        );
        assert_eq!(schema["properties"]["mode"]["default"], json!("fast"));
        assert_eq!(
            InputSpec::try_from(&schema).expect("exported schema should import"),
            spec,
        );
    }

    #[test]
    fn a_default_on_a_non_string_kind_does_not_become_string_constraints() {
        let field = InputFieldSpec::new(
            InputName::new("flag").expect("test input name should be valid"),
            None,
            None,
            false,
            InputKind::Boolean,
        )
        .expect("test field should be valid");
        let schema = json!({
            "type": "object",
            "properties": { "flag": { "type": "boolean", "default": true } }
        });

        let imported = InputSpec::try_from(&schema).expect("boolean default should import");

        assert_eq!(imported, spec_of(field));
    }

    #[test]
    fn pattern_is_still_rejected_on_non_textual_kinds() {
        let schema = json!({
            "type": "object",
            "properties": { "path": { "type": "string", "x-upeg-kind": "file_path", "pattern": "^/" } }
        });

        assert!(matches!(
            InputSpec::try_from(&schema),
            Err(InputAdapterError::JsonSchemaStringConstraintUnsupportedKind {
                property,
                keyword: "pattern",
                kind: "file_path",
            }) if property == "path"
        ));
    }
}
