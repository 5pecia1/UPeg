//! JSON Schema projection of [`super::NumberConstraints`].
//!
//! `min` / `max` / `default` are stored as `f64` because `number` and
//! `integer` share one constraint struct, but they must not *serialize*
//! the same way. An `integer` input whose default came out as
//! `"default": 20.0` advertised a value its own validator refused —
//! [`super::validators::validate_integer_value`] reads `as_i64`, which
//! is `None` for a JSON float. Integer-kind bounds and defaults are
//! therefore emitted as JSON integers, and the validator additionally
//! accepts a whole-valued float so the two sides cannot disagree again.

use serde_json::{Map, Value};

use super::{FieldConstraints, InputAdapterError, InputKind, NumberConstraints};

const MINIMUM_SCHEMA_KEY: &str = "minimum";
const MAXIMUM_SCHEMA_KEY: &str = "maximum";
const DEFAULT_SCHEMA_KEY: &str = "default";

/// How a numeric constraint is written into JSON Schema.
#[derive(Clone, Copy)]
enum NumericJson {
    /// `InputKind::Integer`: emit `20`, never `20.0`.
    Integer,
    /// `InputKind::Number`: emit the `f64` as-is.
    Number,
}

pub(super) fn write_numeric_constraints(
    schema: &mut Map<String, Value>,
    kind: &InputKind,
    constraints: &FieldConstraints,
) {
    match kind {
        InputKind::Number | InputKind::Integer => {
            let Some(number) = constraints.number.as_ref() else {
                return;
            };
            let rendering = if matches!(kind, InputKind::Integer) {
                NumericJson::Integer
            } else {
                NumericJson::Number
            };
            insert_number(schema, MINIMUM_SCHEMA_KEY, number.min, rendering);
            insert_number(schema, MAXIMUM_SCHEMA_KEY, number.max, rendering);
            insert_number(schema, DEFAULT_SCHEMA_KEY, number.default, rendering);
        }
        InputKind::String
        | InputKind::Boolean
        | InputKind::Options(_)
        | InputKind::MultiOptions(_)
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url
        | InputKind::File(_) => {}
    }
}

pub(super) fn numeric_constraints_from_schema(
    property: &str,
    kind: &InputKind,
    schema: &Map<String, Value>,
) -> Result<FieldConstraints, InputAdapterError> {
    match kind {
        InputKind::Number | InputKind::Integer => Ok(FieldConstraints {
            number: Some(NumberConstraints {
                min: optional_number(property, schema, MINIMUM_SCHEMA_KEY)?,
                max: optional_number(property, schema, MAXIMUM_SCHEMA_KEY)?,
                default: optional_number(property, schema, DEFAULT_SCHEMA_KEY)?,
            })
            .filter(|constraints| {
                constraints.min.is_some()
                    || constraints.max.is_some()
                    || constraints.default.is_some()
            }),
            string: None,
        }),
        InputKind::String
        | InputKind::Boolean
        | InputKind::Options(_)
        | InputKind::MultiOptions(_)
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url
        | InputKind::File(_) => {
            let misplaced_keyword = [MINIMUM_SCHEMA_KEY, MAXIMUM_SCHEMA_KEY]
                .into_iter()
                .find(|keyword| schema.contains_key(*keyword))
                .or_else(|| {
                    schema
                        .get(DEFAULT_SCHEMA_KEY)
                        .filter(|value| value.is_number())
                        .map(|_| DEFAULT_SCHEMA_KEY)
                });
            if let Some(keyword) = misplaced_keyword {
                return Err(
                    InputAdapterError::JsonSchemaNumericConstraintUnsupportedKind {
                        property: property.to_string(),
                        keyword,
                        kind: kind.label(),
                    },
                );
            }
            Ok(FieldConstraints::default())
        }
    }
}

fn insert_number(
    schema: &mut Map<String, Value>,
    keyword: &str,
    number: Option<f64>,
    rendering: NumericJson,
) {
    if let Some(number) = number {
        schema.insert(keyword.to_string(), numeric_value(number, rendering));
    }
}

/// A non-integral value on an `integer` input is a declaration bug the
/// loader rejects, so falling back to the float here only preserves
/// whatever a hand-built spec put in rather than rounding it away.
fn numeric_value(number: f64, rendering: NumericJson) -> Value {
    match rendering {
        NumericJson::Integer => {
            super::whole_i64(number).map_or_else(|| Value::from(number), Value::from)
        }
        NumericJson::Number => Value::from(number),
    }
}

fn optional_number(
    property: &str,
    schema: &Map<String, Value>,
    keyword: &'static str,
) -> Result<Option<f64>, InputAdapterError> {
    let Some(value) = schema.get(keyword) else {
        return Ok(None);
    };
    value.as_f64().map(Some).ok_or_else(|| {
        InputAdapterError::JsonSchemaNumericConstraintNotNumber {
            property: property.to_string(),
            keyword,
            kind: json_value_kind(value),
        }
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
