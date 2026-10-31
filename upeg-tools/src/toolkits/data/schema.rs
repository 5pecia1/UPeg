use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

const SCHEMA_VERSION: u8 = 1;
const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHANGES: usize = 5_000;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SchemaChange {
    PropertyAdded {
        path: String,
        right: Value,
    },
    PropertyRemoved {
        path: String,
        left: Value,
    },
    TypeChanged {
        path: String,
        left: Value,
        right: Value,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Unknown {
    ItemsUnobserved { path: String, side: &'static str },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Equal,
    Different,
    Inconclusive,
}

#[derive(Serialize)]
struct DiffReport {
    status: Status,
    changes: Vec<SchemaChange>,
    unknowns: Vec<Unknown>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    Null,
    Boolean,
    Number,
    String,
    Object(BTreeMap<String, Self>),
    Array(Option<Box<Self>>),
    AnyOf(Vec<Self>),
}

impl Shape {
    fn name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::String => "string",
            Self::Object(_) => "object",
            Self::Array(_) => "array",
            Self::AnyOf(_) => "anyOf",
        }
    }

    fn infer(value: &Value, path: &str, observations: &mut Observations) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Boolean,
            Value::Number(_) => Self::Number,
            Value::String(_) => Self::String,
            Value::Object(map) => {
                *observations
                    .object_instances
                    .entry(path.to_owned())
                    .or_default() += 1;
                let mut properties = BTreeMap::new();
                for (key, value) in map {
                    let child = pointer(path, key);
                    *observations
                        .field_presence
                        .entry(child.clone())
                        .or_default() += 1;
                    properties.insert(key.clone(), Self::infer(value, &child, observations));
                }
                Self::Object(properties)
            }
            Value::Array(items) => {
                let child = format!("{path}/*");
                let elements = items
                    .iter()
                    .map(|item| Self::infer(item, &child, observations));
                Self::Array(elements.reduce(Self::merge).map(Box::new))
            }
        }
    }

    fn merge(left: Self, right: Self) -> Self {
        match (left, right) {
            (Self::Null, Self::Null) => Self::Null,
            (Self::Boolean, Self::Boolean) => Self::Boolean,
            (Self::Number, Self::Number) => Self::Number,
            (Self::String, Self::String) => Self::String,
            (Self::Object(mut a), Self::Object(b)) => {
                for (key, other) in b {
                    match a.remove(&key) {
                        Some(prior) => {
                            a.insert(key, Self::merge(prior, other));
                        }
                        None => {
                            a.insert(key, other);
                        }
                    }
                }
                Self::Object(a)
            }
            (Self::Array(a), Self::Array(b)) => Self::Array(match (a, b) {
                (Some(a), Some(b)) => Some(Box::new(Self::merge(*a, *b))),
                (a, b) => a.or(b),
            }),
            (a, b) => {
                let mut variants = Vec::new();
                for shape in <[Self; 2]>::from((a, b)) {
                    if let Self::AnyOf(inner) = shape {
                        variants.extend(inner);
                    } else {
                        variants.push(shape);
                    }
                }
                variants.sort_by_key(Self::name);
                let mut merged = Vec::new();
                for shape in variants {
                    if merged
                        .last()
                        .is_some_and(|last: &Self| last.name() == shape.name())
                    {
                        if let Some(prior) = merged.pop() {
                            merged.push(Self::merge(prior, shape));
                        } else {
                            merged.push(shape);
                        }
                    } else {
                        merged.push(shape);
                    }
                }
                match merged.as_slice() {
                    [only] => only.clone(),
                    _ => Self::AnyOf(merged),
                }
            }
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Null => json!({"type":"null"}),
            Self::Boolean => json!({"type":"boolean"}),
            Self::Number => json!({"type":"number"}),
            Self::String => json!({"type":"string"}),
            Self::Object(properties) => {
                let props: serde_json::Map<String, Value> = properties
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_json()))
                    .collect();
                json!({"type":"object","properties":props})
            }
            Self::Array(Some(items)) => json!({"type":"array","items":items.to_json()}),
            Self::Array(None) => json!({"type":"array"}),
            Self::AnyOf(shapes) => {
                json!({"anyOf":shapes.iter().map(Self::to_json).collect::<Vec<_>>()})
            }
        }
    }

    fn parse(value: &Value, path: &str) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| format!("schema at {path} must be an object"))?;
        if let Some(alts) = object.get("anyOf") {
            only_keys(object, &["anyOf"], path)?;
            let array = alts
                .as_array()
                .ok_or_else(|| format!("anyOf at {path} must be an array"))?;
            if array.len() < 2 {
                return Err(format!("anyOf at {path} must contain at least two schemas"));
            }
            let mut shapes = Vec::new();
            let mut seen_types = BTreeSet::new();
            for (index, schema) in array.iter().enumerate() {
                let branch = Self::parse(schema, &format!("{path}/anyOf/{index}"))?;
                if matches!(branch, Self::AnyOf(_)) {
                    return Err(format!(
                        "nested anyOf at {path} is unsupported; inferred unions are flat"
                    ));
                }
                if !seen_types.insert(branch.name()) {
                    return Err(format!(
                        "anyOf at {path} has duplicate type `{}`",
                        branch.name()
                    ));
                }
                shapes.push(branch);
            }
            let merged = shapes
                .into_iter()
                .reduce(Self::merge)
                .ok_or_else(|| format!("anyOf at {path} must contain schemas"))?;
            if !matches!(merged, Self::AnyOf(_)) {
                return Err(format!("anyOf at {path} has duplicate types"));
            }
            return Ok(merged);
        }
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("schema at {path} requires a single string type"))?;
        let shape = match kind {
            "null" => {
                only_keys(object, &["type"], path)?;
                Self::Null
            }
            "boolean" => {
                only_keys(object, &["type"], path)?;
                Self::Boolean
            }
            "number" => {
                only_keys(object, &["type"], path)?;
                Self::Number
            }
            "string" => {
                only_keys(object, &["type"], path)?;
                Self::String
            }
            "object" => {
                only_keys(object, &["type", "properties"], path)?;
                let properties = object
                    .get("properties")
                    .ok_or_else(|| format!("object schema at {path} requires properties"))?
                    .as_object()
                    .ok_or_else(|| format!("properties at {path} must be an object"))?;
                let mut result = BTreeMap::new();
                for (key, child) in properties {
                    result.insert(key.clone(), Self::parse(child, &pointer(path, key))?);
                }
                Self::Object(result)
            }
            "array" => {
                only_keys(object, &["type", "items"], path)?;
                Self::Array(
                    object
                        .get("items")
                        .map(|child| Self::parse(child, &format!("{path}/*")).map(Box::new))
                        .transpose()?,
                )
            }
            _ => {
                return Err(format!(
                    "unsupported schema type `{kind}` at {path}; only inferred JSON-compatible schemas are supported"
                ));
            }
        };
        Ok(shape)
    }
}

fn only_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "unsupported JSON Schema keyword `{key}` at {path}; only inferred JSON-compatible schemas are supported"
            ));
        }
    }
    Ok(())
}

fn pointer(path: &str, key: &str) -> String {
    // Schema paths use `*` for all array items. Escape literal property stars
    // separately so object properties cannot collide with that wildcard.
    format!(
        "{path}/{}",
        key.replace('~', "~0").replace('/', "~1").replace('*', "~2")
    )
}

#[derive(Default)]
struct Observations {
    object_instances: BTreeMap<String, usize>,
    field_presence: BTreeMap<String, usize>,
}

pub(super) fn generate(
    samples: &[Value],
    sample_mode: &str,
    source_format: &str,
) -> Result<String, String> {
    let mut observations = Observations::default();
    let schema = samples
        .iter()
        .map(|sample| Shape::infer(sample, "", &mut observations))
        .reduce(Shape::merge)
        .ok_or_else(|| "samples mode requires a nonempty array".to_string())?;
    let report = json!({
        "version":SCHEMA_VERSION,
        "policy":"observed_types_v1",
        "source_format":source_format,
        "sample_mode":sample_mode,
        "schema":schema.to_json(),
        "observations":{
            "sample_count":samples.len(),
            "object_instances":observations.object_instances,
            "field_presence":observations.field_presence,
        }
    });
    bound(report)
}

fn extract(input: &Value, side: &str) -> Result<Shape, String> {
    let schema = if let Some(schema) = input.get("schema") {
        let object = input
            .as_object()
            .ok_or_else(|| format!("{side} envelope must be an object"))?;
        let version = object.get("version").and_then(Value::as_u64);
        let policy = object.get("policy").and_then(Value::as_str);
        if version != Some(SCHEMA_VERSION.into()) || policy != Some("observed_types_v1") {
            return Err(format!(
                "{side} envelope requires version 1 and policy observed_types_v1"
            ));
        }
        schema
    } else {
        input
    };
    Shape::parse(schema, "")
}

pub(super) fn compare(left: &Value, right: &Value) -> Result<String, String> {
    let a = extract(left, "left")?;
    let b = extract(right, "right")?;
    let mut changes = Vec::new();
    let mut unknowns = Vec::new();
    walk(&a, &b, "", &mut changes, &mut unknowns)?;
    let status = if !changes.is_empty() {
        Status::Different
    } else if !unknowns.is_empty() {
        Status::Inconclusive
    } else {
        Status::Equal
    };
    let output = serde_json::to_string(&DiffReport {
        status,
        changes,
        unknowns,
    })
    .map_err(|e| format!("failed to serialize schema diff: {e}"))?;
    if output.len() > MAX_REPORT_BYTES {
        return Err("schema report exceeds 2 MiB; narrow the input".into());
    }
    Ok(output)
}

fn walk(
    a: &Shape,
    b: &Shape,
    path: &str,
    changes: &mut Vec<SchemaChange>,
    unknowns: &mut Vec<Unknown>,
) -> Result<(), String> {
    if changes.len() + unknowns.len() > MAX_CHANGES {
        return Err("schema diff exceeds 5000 findings; narrow the schemas".into());
    }
    match (a, b) {
        (Shape::Object(x), Shape::Object(y)) => {
            let keys: BTreeSet<_> = x.keys().chain(y.keys()).collect();
            for key in keys {
                let child = pointer(path, key);
                match (x.get(key), y.get(key)) {
                    (Some(a), Some(b)) => walk(a, b, &child, changes, unknowns)?,
                    (Some(a), None) => changes.push(SchemaChange::PropertyRemoved {
                        path: child,
                        left: a.to_json(),
                    }),
                    (None, Some(b)) => changes.push(SchemaChange::PropertyAdded {
                        path: child,
                        right: b.to_json(),
                    }),
                    (None, None) => continue,
                }
                if changes.len() + unknowns.len() > MAX_CHANGES {
                    return Err("schema diff exceeds 5000 findings; narrow the schemas".into());
                }
            }
        }
        (Shape::Array(x), Shape::Array(y)) => match (x, y) {
            (Some(a), Some(b)) => walk(a, b, &format!("{path}/*"), changes, unknowns)?,
            (None, Some(_)) | (Some(_), None) => unknowns.push(Unknown::ItemsUnobserved {
                path: format!("{path}/*"),
                side: if x.is_none() { "left" } else { "right" },
            }),
            (None, None) => unknowns.push(Unknown::ItemsUnobserved {
                path: format!("{path}/*"),
                side: "both",
            }),
        },
        (Shape::AnyOf(x), Shape::AnyOf(y)) => {
            let left: BTreeMap<_, _> = x.iter().map(|shape| (shape.name(), shape)).collect();
            let right: BTreeMap<_, _> = y.iter().map(|shape| (shape.name(), shape)).collect();
            if left.keys().ne(right.keys()) {
                changes.push(SchemaChange::TypeChanged {
                    path: path.to_owned(),
                    left: a.to_json(),
                    right: b.to_json(),
                });
            } else {
                for (kind, shape) in left {
                    walk(shape, right[kind], path, changes, unknowns)?;
                }
            }
        }
        _ if a.name() != b.name() => changes.push(SchemaChange::TypeChanged {
            path: path.to_owned(),
            left: a.to_json(),
            right: b.to_json(),
        }),
        _ => {}
    }
    if changes.len() + unknowns.len() > MAX_CHANGES {
        return Err("schema diff exceeds 5000 findings; narrow the schemas".into());
    }
    Ok(())
}

fn bound(value: Value) -> Result<String, String> {
    super::validate_schema(&value, "generated schema")?;
    let output = value.to_string();
    if output.len() > MAX_REPORT_BYTES {
        return Err("schema report exceeds 2 MiB; narrow the input".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_merge_without_claiming_required() {
        let samples = [json!({"a":1,"x":[null,2]}), json!({"b":true,"x":[]})];
        let report: Value =
            serde_json::from_str(&generate(&samples, "samples", "json").unwrap()).unwrap();
        assert_eq!(
            report["schema"]["properties"]["x"]["items"]["anyOf"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(report["schema"].get("required").is_none());
        assert_eq!(report["observations"]["field_presence"]["/a"], 1);
    }
    #[test]
    fn empty_array_evidence_is_inconclusive() {
        let a = json!({"type":"array"});
        let b = json!({"type":"array","items":{"type":"number"}});
        let report: Value = serde_json::from_str(&compare(&a, &b).unwrap()).unwrap();
        assert_eq!(report["status"], "inconclusive");
    }
    #[test]
    fn rejects_unsupported_keywords() {
        let a = json!({"type":"string","format":"date"});
        assert!(compare(&a, &a).unwrap_err().contains("format"));
    }
    #[test]
    fn generation_envelopes_ignore_observation_counts() {
        let a = json!({"version":1,"policy":"observed_types_v1","schema":{"type":"number"},"observations":{"sample_count":1}});
        let b = json!({"version":1,"policy":"observed_types_v1","schema":{"type":"number"},"observations":{"sample_count":8}});
        let report: Value = serde_json::from_str(&compare(&a, &b).unwrap()).unwrap();
        assert_eq!(report["status"], "equal");
    }
}
