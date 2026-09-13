//! JSON value validators for each [`super::InputKind`] variant.

use super::{
    ChoiceSpec, InputFieldSpec, InputKind, InputValueError, file_validation::validate_file_value,
    json_value_kind, whole_i64,
};

pub(super) fn validate_json_value(
    field: &InputFieldSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    match &field.kind {
        InputKind::String | InputKind::Markdown | InputKind::DateTime | InputKind::Url => {
            validate_string_value(field, value)
        }
        InputKind::FilePath => validate_string_value(field, value),
        InputKind::Number => validate_number_value(field, value),
        InputKind::Integer => validate_integer_value(field, value),
        InputKind::Boolean => validate_boolean_value(field, value),
        InputKind::Options(choices) => validate_options_value(field, choices, value),
        InputKind::MultiOptions(choices) => validate_multi_options_value(field, choices, value),
        InputKind::Json => Ok(()),
        InputKind::File(policy) => validate_file_value(field, policy, value),
    }
}

fn validate_string_value(
    field: &InputFieldSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    let Some(text) = value.as_str() else {
        return Err(type_mismatch(field, value));
    };
    if let Some(constraints) = field.constraints.string.as_ref()
        && let Some(pattern) = constraints.regex.as_ref()
    {
        // Spec construction already compile-checks every pattern (see
        // `InputFieldSpec::with_constraints`), so this arm is only
        // reachable for a spec assembled by hand. It still must not
        // masquerade as PatternMismatch: blaming the caller's value for
        // an unusable declaration made the field permanently invalid
        // with no way to tell why.
        let re = regex::Regex::new(pattern).map_err(|_| InputValueError::UncompilablePattern {
            name: field.name.clone(),
            pattern: pattern.clone(),
        })?;
        if !re.is_match(text) {
            return Err(InputValueError::PatternMismatch {
                name: field.name.clone(),
                pattern: pattern.clone(),
            });
        }
    }
    Ok(())
}

fn validate_number_value(
    field: &InputFieldSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    let Some(number) = value.as_f64() else {
        return Err(type_mismatch(field, value));
    };
    check_numeric_range(field, number)
}

/// JSON has a single number type, so `20` and `20.0` denote the same
/// integer even though `serde_json` keeps them apart. Accepting only
/// `as_i64` meant a schema-declared `"default": 20.0` was rejected by
/// the very tool that declared it.
fn validate_integer_value(
    field: &InputFieldSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    let Some(integer) = value.as_f64().and_then(whole_i64) else {
        return Err(type_mismatch(field, value));
    };
    check_numeric_range(field, integer as f64)
}

fn check_numeric_range(field: &InputFieldSpec, actual: f64) -> Result<(), InputValueError> {
    let Some(constraints) = field.constraints.number.as_ref() else {
        return Ok(());
    };
    let below_min = constraints.min.is_some_and(|min| actual < min);
    let above_max = constraints.max.is_some_and(|max| actual > max);
    if below_min || above_max {
        return Err(InputValueError::OutOfRange {
            name: field.name.clone(),
            min: constraints.min,
            max: constraints.max,
            actual,
        });
    }
    Ok(())
}

fn validate_boolean_value(
    field: &InputFieldSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    if value.is_boolean() {
        Ok(())
    } else {
        Err(type_mismatch(field, value))
    }
}

fn validate_options_value(
    field: &InputFieldSpec,
    choices: &ChoiceSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    let Some(value) = value.as_str() else {
        return Err(type_mismatch(field, value));
    };
    if choices.contains(value) {
        Ok(())
    } else {
        Err(InputValueError::InvalidChoice {
            name: field.name.clone(),
            allowed_values: choices.allowed_values(),
        })
    }
}

fn validate_multi_options_value(
    field: &InputFieldSpec,
    choices: &ChoiceSpec,
    value: &serde_json::Value,
) -> Result<(), InputValueError> {
    let Some(values) = value.as_array() else {
        return Err(type_mismatch(field, value));
    };
    if values.is_empty() {
        if field.required {
            return Err(InputValueError::MissingRequired {
                name: field.name.clone(),
            });
        }
        return Ok(());
    }
    for value in values {
        let Some(selection) = value.as_str() else {
            return Err(InputValueError::TypeMismatch {
                name: field.name.clone(),
                expected: field.kind.expected_json_kind(),
                actual: json_value_kind(value),
            });
        };
        if !choices.contains(selection) {
            return Err(InputValueError::InvalidMultiChoice {
                name: field.name.clone(),
                allowed_values: choices.allowed_values(),
                invalid_value: selection.to_string(),
            });
        }
    }
    Ok(())
}

fn type_mismatch(field: &InputFieldSpec, value: &serde_json::Value) -> InputValueError {
    InputValueError::TypeMismatch {
        name: field.name.clone(),
        expected: field.kind.expected_json_kind(),
        actual: json_value_kind(value),
    }
}
