use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Number, Value};
use similar::{Algorithm, DiffOp, capture_diff_slices};

const MAX_CHANGES: usize = 5_000;
const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Change {
    Added {
        path: String,
        right: Value,
    },
    Removed {
        path: String,
        left: Value,
    },
    ValueChanged {
        path: String,
        left: Value,
        right: Value,
    },
    TypeChanged {
        path: String,
        left: Value,
        right: Value,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Equal,
    Different,
}

#[derive(Serialize)]
struct Report {
    status: Status,
    changes: Vec<Change>,
}

pub(super) fn compare(left: &Value, right: &Value) -> Result<String, String> {
    let mut changes = Vec::new();
    walk(left, right, "", &mut changes)?;
    let report = Report {
        status: if changes.is_empty() {
            Status::Equal
        } else {
            Status::Different
        },
        changes,
    };
    let output = serde_json::to_string(&report)
        .map_err(|e| format!("failed to serialize value diff: {e}"))?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err("value diff output exceeds 2 MiB; narrow the input".into());
    }
    Ok(output)
}

fn walk(left: &Value, right: &Value, path: &str, changes: &mut Vec<Change>) -> Result<(), String> {
    if equivalent(left, right) {
        return Ok(());
    }
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let child = pointer(path, key);
                match (a.get(key), b.get(key)) {
                    (Some(value), Some(other)) => walk(value, other, &child, changes)?,
                    (Some(value), None) => {
                        push(
                            changes,
                            Change::Removed {
                                path: child,
                                left: value.clone(),
                            },
                        )?;
                    }
                    (None, Some(value)) => {
                        push(
                            changes,
                            Change::Added {
                                path: child,
                                right: value.clone(),
                            },
                        )?;
                    }
                    (None, None) => {}
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => compare_arrays(a, b, path, changes)?,
        _ => {
            let path = path.to_owned();
            let left = left.clone();
            let right = right.clone();
            let change = if kind(&left) == kind(&right) {
                Change::ValueChanged { path, left, right }
            } else {
                Change::TypeChanged { path, left, right }
            };
            push(changes, change)?;
        }
    }
    Ok(())
}

fn compare_arrays(
    a: &[Value],
    b: &[Value],
    path: &str,
    changes: &mut Vec<Change>,
) -> Result<(), String> {
    if a.len().saturating_mul(b.len()) > 1_000_000 {
        return Err("array alignment exceeds the 1000000-pair work limit; narrow the input".into());
    }
    let left_tokens: Vec<String> = a.iter().map(canonical).collect();
    let right_tokens: Vec<String> = b.iter().map(canonical).collect();
    for op in capture_diff_slices(Algorithm::Myers, &left_tokens, &right_tokens) {
        match op {
            DiffOp::Equal { .. } => {}
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for (offset, value) in a[old_index..old_index + old_len].iter().enumerate() {
                    push(
                        changes,
                        Change::Removed {
                            path: format!("{path}/{}", old_index + offset),
                            left: value.clone(),
                        },
                    )?;
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for (offset, value) in b[new_index..new_index + new_len].iter().enumerate() {
                    push(
                        changes,
                        Change::Added {
                            path: format!("{path}/{}", new_index + offset),
                            right: value.clone(),
                        },
                    )?;
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let paired = old_len.min(new_len);
                for offset in 0..paired {
                    walk(
                        &a[old_index + offset],
                        &b[new_index + offset],
                        &format!("{path}/{}", new_index + offset),
                        changes,
                    )?;
                }
                for (offset, value) in a[old_index + paired..old_index + old_len]
                    .iter()
                    .enumerate()
                {
                    push(
                        changes,
                        Change::Removed {
                            path: format!("{path}/{}", old_index + paired + offset),
                            left: value.clone(),
                        },
                    )?;
                }
                for (offset, value) in b[new_index + paired..new_index + new_len]
                    .iter()
                    .enumerate()
                {
                    push(
                        changes,
                        Change::Added {
                            path: format!("{path}/{}", new_index + paired + offset),
                            right: value.clone(),
                        },
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn push(changes: &mut Vec<Change>, change: Change) -> Result<(), String> {
    if changes.len() == MAX_CHANGES {
        return Err("value diff exceeds 5000 changes; narrow the input".into());
    }
    changes.push(change);
    Ok(())
}

fn pointer(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => normalize_number(x) == normalize_number(y),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, value)| y.get(key).is_some_and(|v| equivalent(value, v)))
        }
        _ => a == b,
    }
}

fn canonical(value: &Value) -> String {
    match value {
        Value::Number(n) => format!("n:{}", normalize_number(n)),
        Value::Array(items) => format!(
            "a:[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            format!(
                "o:{{{}}}",
                keys.into_iter()
                    .map(|k| format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical(&map[k])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        _ => value.to_string(),
    }
}

// Decimal normalization compares integers and finite JSON floats without converting
// large integers to f64. serde_json has already parsed floating input by this point.
fn normalize_number(number: &Number) -> String {
    let repr = number.to_string();
    let (base, exponent) = repr.split_once(['e', 'E']).unwrap_or((&repr, "0"));
    let exp = exponent.parse::<i64>().unwrap_or(0);
    let negative = base.starts_with('-');
    let unsigned = base.trim_start_matches('-');
    let fractional = unsigned
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len()) as i64;
    let mut digits: String = unsigned.chars().filter(|c| *c != '.').collect();
    let without_leading = digits.trim_start_matches('0').to_string();
    if without_leading.is_empty() {
        return "0".into();
    }
    digits = without_leading;
    let trailing = digits.len() - digits.trim_end_matches('0').len();
    digits.truncate(digits.len() - trailing);
    format!(
        "{}{}e{}",
        if negative { "-" } else { "" },
        digits,
        exp - fractional + trailing as i64
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn normalized_numbers_and_object_order() {
        let a: Value = serde_json::from_str(r#"{"b":1.0,"a":9007199254740993}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":9007199254740993,"b":1}"#).unwrap();
        assert!(equivalent(&a, &b));
        let c: Value = serde_json::from_str("9007199254740992.0").unwrap();
        assert!(!equivalent(&a["a"], &c));
        let adjacent: Value = serde_json::from_str("9007199254740992").unwrap();
        assert!(!equivalent(&a["a"], &adjacent));
    }
    #[test]
    fn missing_and_null_are_distinct_and_paths_escape() {
        let a = json!({"a/b~":null});
        let b = json!({});
        let output: Value = serde_json::from_str(&compare(&a, &b).unwrap()).unwrap();
        assert_eq!(output["changes"][0]["kind"], "removed");
        assert_eq!(output["changes"][0]["path"], "/a~1b~0");
    }
    #[test]
    fn array_alignment_keeps_stable_elements() {
        let a = json!([1, 2, 3]);
        let b = json!([1, 9, 2, 3]);
        let output: Value = serde_json::from_str(&compare(&a, &b).unwrap()).unwrap();
        assert_eq!(output["changes"].as_array().unwrap().len(), 1);
        assert_eq!(output["changes"][0]["kind"], "added");
    }
    #[test]
    fn distinguishes_type_and_value_changes() {
        let a = json!({"x":1,"y":1});
        let b = json!({"x":"1","y":2});
        let output: Value = serde_json::from_str(&compare(&a, &b).unwrap()).unwrap();
        assert_eq!(output["changes"][0]["kind"], "type_changed");
        assert_eq!(output["changes"][1]["kind"], "value_changed");
    }
}
