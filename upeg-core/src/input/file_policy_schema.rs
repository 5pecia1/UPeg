use serde_json::{Map, Value};

use super::{FileInputPolicy, FileInputPolicyError, FileInputPolicyParams, json_value_kind};

pub(super) const UPEG_FILE_POLICY_SCHEMA_KEY: &str = "x-upeg-file-policy";

const MAX_COUNT_KEY: &str = "maxCount";
const EXTENSIONS_KEY: &str = "extensions";
const MAX_FILE_BYTES_KEY: &str = "maxFileBytes";
const MAX_TOTAL_BYTES_KEY: &str = "maxTotalBytes";
const POLICY_KEYS: &[&str] = &[
    MAX_COUNT_KEY,
    EXTENSIONS_KEY,
    MAX_FILE_BYTES_KEY,
    MAX_TOTAL_BYTES_KEY,
];

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FilePolicySchemaError {
    #[error("file policy must be an object, got {kind}")]
    NotObject { kind: &'static str },
    #[error("file policy contains unknown key `{key}`")]
    UnknownKey { key: String },
    #[error("file policy is missing required key `{key}`")]
    MissingKey { key: &'static str },
    #[error("file policy key `{key}` expected {expected}, got {actual}")]
    TypeMismatch {
        key: &'static str,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("file policy key `{key}` is outside its supported range")]
    InvalidValue { key: &'static str },
    #[error(transparent)]
    Policy(#[from] FileInputPolicyError),
}

pub(super) fn file_policy_schema_value(policy: &FileInputPolicy) -> Value {
    let mut schema = Map::from_iter([
        (
            MAX_COUNT_KEY.to_string(),
            Value::Number(policy.max_count().into()),
        ),
        (
            EXTENSIONS_KEY.to_string(),
            Value::Array(
                policy
                    .extensions()
                    .map(|extension| Value::String(extension.to_string()))
                    .collect(),
            ),
        ),
    ]);
    if let Some(max_file_bytes) = policy.max_file_bytes() {
        schema.insert(
            MAX_FILE_BYTES_KEY.to_string(),
            Value::Number(max_file_bytes.into()),
        );
    }
    if let Some(max_total_bytes) = policy.max_total_bytes() {
        schema.insert(
            MAX_TOTAL_BYTES_KEY.to_string(),
            Value::Number(max_total_bytes.into()),
        );
    }
    Value::Object(schema)
}

pub(super) fn file_policy_from_schema(
    value: Option<&Value>,
) -> Result<FileInputPolicy, FilePolicySchemaError> {
    let Some(value) = value else {
        return Ok(FileInputPolicy::default());
    };
    let object = value.as_object().ok_or(FilePolicySchemaError::NotObject {
        kind: json_value_kind(value),
    })?;
    if let Some(key) = object
        .keys()
        .find(|key| !POLICY_KEYS.contains(&key.as_str()))
    {
        return Err(FilePolicySchemaError::UnknownKey { key: key.clone() });
    }

    let max_count_value = required_value(object, MAX_COUNT_KEY)?;
    let max_count_u64 = max_count_value
        .as_u64()
        .ok_or(FilePolicySchemaError::TypeMismatch {
            key: MAX_COUNT_KEY,
            expected: "unsigned integer",
            actual: json_value_kind(max_count_value),
        })?;
    let max_count = u32::try_from(max_count_u64)
        .map_err(|_| FilePolicySchemaError::InvalidValue { key: MAX_COUNT_KEY })?;

    let extensions_value = required_value(object, EXTENSIONS_KEY)?;
    let extensions_array =
        extensions_value
            .as_array()
            .ok_or(FilePolicySchemaError::TypeMismatch {
                key: EXTENSIONS_KEY,
                expected: "string array",
                actual: json_value_kind(extensions_value),
            })?;
    let extensions = extensions_array
        .iter()
        .map(|extension| {
            extension
                .as_str()
                .map(str::to_string)
                .ok_or(FilePolicySchemaError::TypeMismatch {
                    key: EXTENSIONS_KEY,
                    expected: "string array",
                    actual: json_value_kind(extension),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    FileInputPolicy::try_from(FileInputPolicyParams {
        max_count,
        extensions,
        max_file_bytes: optional_u64(object, MAX_FILE_BYTES_KEY)?,
        max_total_bytes: optional_u64(object, MAX_TOTAL_BYTES_KEY)?,
    })
    .map_err(FilePolicySchemaError::from)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    key: &'static str,
) -> Result<&'a Value, FilePolicySchemaError> {
    object
        .get(key)
        .ok_or(FilePolicySchemaError::MissingKey { key })
}

fn optional_u64(
    object: &Map<String, Value>,
    key: &'static str,
) -> Result<Option<u64>, FilePolicySchemaError> {
    object
        .get(key)
        .map(|value| {
            value.as_u64().ok_or(FilePolicySchemaError::TypeMismatch {
                key,
                expected: "unsigned integer",
                actual: json_value_kind(value),
            })
        })
        .transpose()
}
