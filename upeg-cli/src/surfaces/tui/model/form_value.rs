//! Typed draft → [`InputValue`] conversion for the TUI form.
//!
//! Split out of `model.rs` so the View/State machine file stays about
//! *state*: everything here is a pure function from one field's
//! [`InputFieldSpec`] plus its [`DraftInputValue`] to the canonical
//! value the dispatch envelope carries, with no knowledge of the TUI's
//! screens.

use upeg_core::{ChoiceSpec, DraftInputValue, InputFieldSpec, InputKind, InputValue};

pub(super) fn input_value_from_tui_draft(
    spec: &InputFieldSpec,
    draft: &DraftInputValue,
) -> Result<Option<InputValue>, String> {
    match (&spec.kind, draft) {
        (InputKind::String, DraftInputValue::Text(value)) => {
            Ok(Some(InputValue::String(value.clone())))
        }
        (InputKind::Markdown, DraftInputValue::Text(value)) => {
            Ok(Some(InputValue::Markdown(value.clone())))
        }
        (InputKind::DateTime, DraftInputValue::Text(value)) => {
            Ok(Some(InputValue::DateTime(value.clone())))
        }
        (InputKind::FilePath, DraftInputValue::Text(value)) => {
            Ok(Some(InputValue::FilePath(value.clone())))
        }
        (InputKind::Url, DraftInputValue::Text(value)) => Ok(Some(InputValue::Url(value.clone()))),
        (InputKind::Number, DraftInputValue::Text(value)) => number_from_text(spec, value),
        (InputKind::Integer, DraftInputValue::Text(value)) => integer_from_text(spec, value),
        (InputKind::Boolean, DraftInputValue::Boolean(value)) => {
            Ok(Some(InputValue::Boolean(*value)))
        }
        (InputKind::Options(choices), DraftInputValue::Options(value)) => {
            let Some(value) = value else { return Ok(None) };
            validate_choice(spec, choices, value)?;
            Ok(Some(InputValue::Options(value.clone())))
        }
        (InputKind::MultiOptions(choices), DraftInputValue::Text(value)) => {
            let values = comma_tokens(value);
            if values.is_empty() {
                return Ok(None);
            }
            for value in &values {
                validate_choice(spec, choices, value)?;
            }
            Ok(Some(InputValue::MultiOptions(values)))
        }
        (InputKind::Json, DraftInputValue::Text(value)) => json_from_text(spec, value),
        // TUI surface has no file picker yet, so the only valid file draft
        // is `None` (untouched optional field). Programmatic callers that
        // build a draft via API can also pass a populated `FileValue`.
        (InputKind::File(_), DraftInputValue::File(None)) => Ok(None),
        (InputKind::File(_), DraftInputValue::File(Some(file))) => {
            Ok(Some(InputValue::File(file.clone())))
        }
        _ => Err(format!(
            "input `{}` expected {} draft",
            spec.name,
            spec.kind.label()
        )),
    }
}

fn number_from_text(spec: &InputFieldSpec, value: &str) -> Result<Option<InputValue>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let number = trimmed
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .ok_or_else(|| format!("input `{}` contains invalid number `{value}`", spec.name))?;
    Ok(Some(InputValue::Number(number)))
}

fn integer_from_text(spec: &InputFieldSpec, value: &str) -> Result<Option<InputValue>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = trimmed
        .parse::<i64>()
        .map_err(|_| format!("input `{}` contains invalid integer `{value}`", spec.name))?;
    Ok(Some(InputValue::Integer(value)))
}

fn json_from_text(spec: &InputFieldSpec, value: &str) -> Result<Option<InputValue>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(InputValue::Json)
        .map(Some)
        .map_err(|err| format!("input `{}` contains invalid json: {err}", spec.name))
}

fn validate_choice(spec: &InputFieldSpec, choices: &ChoiceSpec, value: &str) -> Result<(), String> {
    if choices.contains(value) {
        Ok(())
    } else {
        Err(format!(
            "input `{}` must be one of {:?}; got `{value}`",
            spec.name,
            choices.allowed_values()
        ))
    }
}

pub(super) fn cycle_choice(
    current: Option<&str>,
    choices: &ChoiceSpec,
    reverse: bool,
) -> Option<String> {
    if choices.options.is_empty() {
        return None;
    }
    let current = current.and_then(|value| {
        choices
            .options
            .iter()
            .position(|option| option.value == value)
    });
    let next = match (current, reverse) {
        (Some(0) | None, true) => choices.options.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) => (index + 1) % choices.options.len(),
        (None, false) => 0,
    };
    Some(choices.options[next].value.clone())
}

pub(super) fn cycle_multi_options_text(value: &mut String, choices: &ChoiceSpec, reverse: bool) {
    let mut values = comma_tokens(value);
    let next = cycle_choice(values.last().map(String::as_str), choices, reverse);
    let Some(next) = next else {
        return;
    };
    if values.is_empty() {
        values.push(next);
    } else if let Some(last) = values.last_mut() {
        *last = next;
    }
    *value = values.join(", ");
}

pub(super) fn comma_tokens(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
