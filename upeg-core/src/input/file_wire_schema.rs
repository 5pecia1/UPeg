use serde_json::{Map, Value};

use super::json_value_kind;

pub(crate) const UPEG_FILE_WIRE_SCHEMA_KEY: &str = "x-upeg-file-wire";

const VERSION_KEY: &str = "version";
const BYTES_ENCODING_KEY: &str = "bytesEncoding";
const LEGACY_NUMERIC_ARRAYS_KEY: &str = "legacyNumericArrays";
const RECURSIVE_DIRECTORIES_KEY: &str = "recursiveDirectories";
const DOCUMENTATION_KEY: &str = "documentation";
const FILE_WIRE_KEYS: &[&str] = &[
    VERSION_KEY,
    BYTES_ENCODING_KEY,
    LEGACY_NUMERIC_ARRAYS_KEY,
    RECURSIVE_DIRECTORIES_KEY,
    DOCUMENTATION_KEY,
];

const SUPPORTED_VERSION: u64 = 1;
const SUPPORTED_BYTES_ENCODING: &str = "base64-rfc4648-padded";
const SUPPORTED_LEGACY_NUMERIC_ARRAYS: bool = false;
const SUPPORTED_RECURSIVE_DIRECTORIES: bool = true;
const PUBLIC_DOCUMENTATION: &str = "README.md#file-input-wire";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileWireContract;

impl FileWireContract {
    pub(crate) fn schema_value() -> Value {
        Value::Object(Map::from_iter([
            (
                VERSION_KEY.to_string(),
                Value::Number(SUPPORTED_VERSION.into()),
            ),
            (
                BYTES_ENCODING_KEY.to_string(),
                Value::String(SUPPORTED_BYTES_ENCODING.to_string()),
            ),
            (
                LEGACY_NUMERIC_ARRAYS_KEY.to_string(),
                Value::Bool(SUPPORTED_LEGACY_NUMERIC_ARRAYS),
            ),
            (
                RECURSIVE_DIRECTORIES_KEY.to_string(),
                Value::Bool(SUPPORTED_RECURSIVE_DIRECTORIES),
            ),
            (
                DOCUMENTATION_KEY.to_string(),
                Value::String(PUBLIC_DOCUMENTATION.to_string()),
            ),
        ]))
    }

    pub(super) fn from_schema(value: Option<&Value>) -> Result<Option<Self>, FileWireSchemaError> {
        let Some(value) = value else {
            return Ok(None);
        };
        let object = value.as_object().ok_or(FileWireSchemaError::NotObject {
            kind: json_value_kind(value),
        })?;
        if let Some(key) = object
            .keys()
            .find(|key| !FILE_WIRE_KEYS.contains(&key.as_str()))
        {
            return Err(FileWireSchemaError::UnknownKey { key: key.clone() });
        }

        require_exact(object, VERSION_KEY, Value::Number(SUPPORTED_VERSION.into()))?;
        require_exact(
            object,
            BYTES_ENCODING_KEY,
            Value::String(SUPPORTED_BYTES_ENCODING.to_string()),
        )?;
        require_exact(
            object,
            LEGACY_NUMERIC_ARRAYS_KEY,
            Value::Bool(SUPPORTED_LEGACY_NUMERIC_ARRAYS),
        )?;
        require_exact(
            object,
            RECURSIVE_DIRECTORIES_KEY,
            Value::Bool(SUPPORTED_RECURSIVE_DIRECTORIES),
        )?;
        require_exact(
            object,
            DOCUMENTATION_KEY,
            Value::String(PUBLIC_DOCUMENTATION.to_string()),
        )?;
        Ok(Some(Self))
    }
}

fn require_exact(
    object: &Map<String, Value>,
    key: &'static str,
    expected: Value,
) -> Result<(), FileWireSchemaError> {
    let actual = object
        .get(key)
        .ok_or(FileWireSchemaError::MissingKey { key })?;
    if actual == &expected {
        return Ok(());
    }
    Err(FileWireSchemaError::UnsupportedValue { key })
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum FileWireSchemaError {
    #[error("file wire contract must be an object, got {kind}")]
    NotObject { kind: &'static str },
    #[error("file wire contract contains unknown key `{key}`")]
    UnknownKey { key: String },
    #[error("file wire contract is missing required key `{key}`")]
    MissingKey { key: &'static str },
    #[error("file wire contract key `{key}` has an unsupported value")]
    UnsupportedValue { key: &'static str },
}
