//! Semantic JSON/YAML value comparison and observational schema tools.

use serde_json::Value;
use upeg_core::tool;

mod schema;
mod value_diff;
mod yaml;

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 20_000;
const MAX_DEPTH: usize = 64;

struct ValueLimits {
    bytes: usize,
    nodes: usize,
    depth: usize,
}

const DOCUMENT_LIMITS: ValueLimits = ValueLimits {
    bytes: MAX_INPUT_BYTES,
    nodes: MAX_NODES,
    depth: MAX_DEPTH,
};
// Schema keywords and observation maps expand the source tree. Reserve room
// below serde_json's 128-container limit for the surrounding Tool/Chain wire.
const SCHEMA_LIMITS: ValueLimits = ValueLimits {
    bytes: 2 * MAX_INPUT_BYTES,
    nodes: 6 * MAX_NODES,
    depth: 96,
};

/// Compare structured values. Object key order is ignored; array order matters.
#[tool(
    id = "data.value_diff",
    display_label = "Value diff",
    toolkit = "data",
    description = "Compare JSON or YAML values structurally, with JSON Pointer paths and aligned array changes.",
    inputs = [
        required left: String = "First JSON or YAML document",
        required right: String = "Second JSON or YAML document",
        optional left_format: Options(["json", "yaml"]) = "First document format (default: json)",
        optional right_format: Options(["json", "yaml"]) = "Second document format (default: json)",
    ],
    outputs = [result: Json = "Equality status and typed changes"],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn data_value_diff(
    left: &str,
    right: &str,
    left_format: &str,
    right_format: &str,
) -> Result<String, String> {
    let left = parse_document(left, left_format, "left")?;
    let right = parse_document(right, right_format, "right")?;
    value_diff::compare(&left, &right)
}

/// Infer the observed JSON-compatible shape of one document or many samples.
#[tool(
    id = "data.schema_generate",
    display_label = "Generate schema",
    toolkit = "data",
    description = "Infer an observational JSON-compatible schema from a JSON or YAML document or a nonempty array of samples.",
    inputs = [
        required input: String = "JSON or YAML document",
        optional format: Options(["json", "yaml"]) = "Document format (default: json)",
        optional sample_mode: Options(["document", "samples"]) = "Treat input as one document or as an array of samples (default: document)",
    ],
    outputs = [result: Json = "Versioned schema and observation counts"],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn data_schema_generate(
    input: &str,
    format: &str,
    sample_mode: &str,
) -> Result<String, String> {
    let format = selected(format, "json", &["json", "yaml"], "format")?;
    let mode = selected(
        sample_mode,
        "document",
        &["document", "samples"],
        "sample_mode",
    )?;
    let value = parse_document(input, format, "input")?;
    match mode {
        "document" => schema::generate(&[value], mode, format),
        "samples" => {
            let samples = value.as_array().ok_or_else(|| {
                "samples mode requires a nonempty array of document values".to_string()
            })?;
            if samples.is_empty() {
                return Err("samples mode requires a nonempty array of document values".into());
            }
            schema::generate(samples, mode, format)
        }
        _ => Err(format!("unsupported sample_mode `{mode}`")),
    }
}

/// Compare inferred schemas or versioned generation envelopes.
#[tool(
    id = "data.schema_diff",
    display_label = "Schema diff",
    toolkit = "data",
    description = "Compare supported inferred schemas or generation envelopes, reporting observed changes and unknown item types.",
    inputs = [
        required left: Json = "First inferred schema or generation envelope",
        required right: Json = "Second inferred schema or generation envelope",
    ],
    outputs = [result: Json = "Equal, different, or inconclusive status with typed findings"],
    pin = Launcher,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn data_schema_diff(left: &Value, right: &Value) -> Result<String, String> {
    validate_schema(left, "left schema")?;
    validate_schema(right, "right schema")?;
    schema::compare(left, right)
}

fn selected<'a>(
    given: &'a str,
    default: &'a str,
    allowed: &[&str],
    name: &str,
) -> Result<&'a str, String> {
    let value = if given.is_empty() { default } else { given };
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "unsupported {name} `{value}`; expected {}",
            allowed.join(" or ")
        ))
    }
}

fn parse_document(input: &str, format: &str, side: &str) -> Result<Value, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!("{side} input exceeds 1 MiB"));
    }
    let format = selected(format, "json", &["json", "yaml"], "format")?;
    let value = match format {
        "json" => serde_json::from_str(input).map_err(|e| format!("invalid {side} JSON: {e}"))?,
        "yaml" => yaml::parse(input).map_err(|e| format!("invalid {side} YAML: {e}"))?,
        _ => return Err(format!("unsupported format `{format}`")),
    };
    validate_value(&value, side, &DOCUMENT_LIMITS)?;
    Ok(value)
}

pub(super) fn validate_schema(value: &Value, side: &str) -> Result<(), String> {
    validate_value(value, side, &SCHEMA_LIMITS)
}

fn validate_value(value: &Value, side: &str, limits: &ValueLimits) -> Result<(), String> {
    let mut remaining = limits.nodes;
    let mut remaining_bytes = limits.bytes;
    let mut stack = vec![(value, 0_usize)];
    while let Some((value, depth)) = stack.pop() {
        if remaining == 0 {
            return Err(format!("{side} input exceeds {} value nodes", limits.nodes));
        }
        remaining -= 1;
        if depth > limits.depth {
            return Err(format!(
                "{side} input exceeds {} levels of nesting",
                limits.depth
            ));
        }
        let own_bytes = match value {
            Value::String(text) => text.len(),
            Value::Number(number) => number.to_string().len(),
            Value::Object(map) => map.keys().map(String::len).sum(),
            _ => 1,
        };
        remaining_bytes = remaining_bytes.checked_sub(own_bytes).ok_or_else(|| {
            format!(
                "{side} input exceeds {} MiB of value content",
                limits.bytes / MAX_INPUT_BYTES
            )
        })?;
        match value {
            Value::Number(number) if number.is_f64() => {
                if number
                    .as_f64()
                    .is_some_and(|n| n.fract() == 0.0 && n.abs() >= 9_007_199_254_740_992.0)
                {
                    return Err(format!(
                        "{side} contains an integral floating number beyond exact comparison range (2^53)"
                    ));
                }
            }
            Value::Array(items) => {
                if items.len() > 2_048 {
                    return Err(format!("{side} array exceeds 2048 elements"));
                }
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(map) => stack.extend(map.values().map(|item| (item, depth + 1))),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_tools_accept_yaml_and_samples() {
        let diff: Value =
            serde_json::from_str(&data_value_diff("a: 1", "{\"a\":1}", "yaml", "json").unwrap())
                .unwrap();
        assert_eq!(diff["status"], "equal");
        let schema: Value = serde_json::from_str(
            &data_schema_generate("[{\"a\":1},{\"a\":2.0}]", "json", "samples").unwrap(),
        )
        .unwrap();
        assert_eq!(schema["observations"]["sample_count"], 2);
        assert_eq!(schema["schema"]["properties"]["a"]["type"], "number");
    }
    #[test]
    fn refuses_integer_values_rounded_by_json_float_parsing() {
        assert!(
            data_value_diff(
                "18446744073709551617",
                "18446744073709551618",
                "json",
                "json"
            )
            .unwrap_err()
            .contains("exact comparison range")
        );
    }
}
