use serde_json::{Map, Value};

use super::{FileOutputPreflightError, FileOutputSummary, enforce_depth};
use crate::input::file_base64::canonical_base64_decoded_len;
use crate::input::file_value::{
    FILE_CONTENT_KEY_BYTES, FILE_CONTENT_KEY_ENTRIES, FILE_CONTENT_KEY_KIND,
    FILE_CONTENT_KIND_BYTES, FILE_CONTENT_KIND_DIRECTORY, FILE_KEY_CONTENT, FILE_KEY_IS_DIR,
    FILE_KEY_MIME, FILE_KEY_NAME,
};

const FILE_KEYS: &[&str] = &[
    FILE_KEY_NAME,
    FILE_KEY_IS_DIR,
    FILE_KEY_MIME,
    FILE_KEY_CONTENT,
];
const BYTES_CONTENT_KEYS: &[&str] = &[FILE_CONTENT_KEY_KIND, FILE_CONTENT_KEY_BYTES];
const DIRECTORY_CONTENT_KEYS: &[&str] = &[FILE_CONTENT_KEY_KIND, FILE_CONTENT_KEY_ENTRIES];

pub(super) fn visit(
    value: &Value,
    depth: usize,
    summary: &mut FileOutputSummary,
) -> Result<(), FileOutputPreflightError> {
    enforce_depth(depth)?;
    summary.add_node()?;
    let object = require_object(value, "file node")?;
    reject_unknown_keys(object, FILE_KEYS)?;
    let name = require_string(object, FILE_KEY_NAME)?;
    summary.add_metadata(name.len())?;
    let is_dir = require_bool(object, FILE_KEY_IS_DIR)?;
    if let Some(mime) = optional_string(object, FILE_KEY_MIME)? {
        summary.add_metadata(mime.len())?;
    }
    let content = object
        .get(FILE_KEY_CONTENT)
        .ok_or_else(|| invalid(format!("missing `{FILE_KEY_CONTENT}`")))
        .and_then(|value| require_object(value, FILE_KEY_CONTENT))?;
    match require_string(content, FILE_CONTENT_KEY_KIND)? {
        FILE_CONTENT_KIND_BYTES => visit_bytes(content, is_dir, summary),
        FILE_CONTENT_KIND_DIRECTORY => visit_directory(content, is_dir, depth, summary),
        kind => Err(invalid(format!(
            "unknown `{FILE_CONTENT_KEY_KIND}` value `{kind}`"
        ))),
    }
}

fn visit_bytes(
    content: &Map<String, Value>,
    is_dir: bool,
    summary: &mut FileOutputSummary,
) -> Result<(), FileOutputPreflightError> {
    reject_unknown_keys(content, BYTES_CONTENT_KEYS)?;
    if is_dir {
        return Err(invalid("`is_dir` is true for bytes content"));
    }
    let encoded = require_string(content, FILE_CONTENT_KEY_BYTES)?;
    let decoded_len = canonical_base64_decoded_len(encoded).map_err(|error| {
        invalid(format!(
            "`{FILE_CONTENT_KEY_BYTES}` must be canonical base64: {error}"
        ))
    })?;
    summary.add_raw(decoded_len)
}

fn visit_directory(
    content: &Map<String, Value>,
    is_dir: bool,
    depth: usize,
    summary: &mut FileOutputSummary,
) -> Result<(), FileOutputPreflightError> {
    reject_unknown_keys(content, DIRECTORY_CONTENT_KEYS)?;
    if !is_dir {
        return Err(invalid("`is_dir` is false for directory content"));
    }
    let entries = content
        .get(FILE_CONTENT_KEY_ENTRIES)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("`{FILE_CONTENT_KEY_ENTRIES}` must be an array")))?;
    for entry in entries {
        let child_depth = depth
            .checked_add(1)
            .ok_or(FileOutputPreflightError::NestingTooDeep {
                max: super::MAX_FILE_OUTPUT_NESTING_DEPTH,
            })?;
        visit(entry, child_depth, summary)?;
    }
    Ok(())
}

fn require_object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a Map<String, Value>, FileOutputPreflightError> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("`{label}` must be an object")))
}

fn require_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, FileOutputPreflightError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("`{key}` must be a string")))
}

fn require_bool(object: &Map<String, Value>, key: &str) -> Result<bool, FileOutputPreflightError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("`{key}` must be a boolean")))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, FileOutputPreflightError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| invalid(format!("`{key}` must be a string"))),
    }
}

fn reject_unknown_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), FileOutputPreflightError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(format!("unknown key `{key}`")));
    }
    Ok(())
}

fn invalid(detail: impl Into<String>) -> FileOutputPreflightError {
    FileOutputPreflightError::InvalidStructure {
        detail: detail.into(),
    }
}
