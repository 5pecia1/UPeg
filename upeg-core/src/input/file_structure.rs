use serde_json::{Map, Value};

use super::file_base64::canonical_base64_decoded_len;
use super::file_validation::FileInputValueError;
use super::file_value::{
    FILE_CONTENT_KEY_BYTES, FILE_CONTENT_KEY_ENTRIES, FILE_CONTENT_KEY_KIND,
    FILE_CONTENT_KIND_BYTES, FILE_CONTENT_KIND_DIRECTORY, FILE_KEY_CONTENT, FILE_KEY_IS_DIR,
    FILE_KEY_MIME, FILE_KEY_NAME,
};
use super::{InputName, InputValueError, json_value_kind};

const FILE_KEYS: &[&str] = &[
    FILE_KEY_NAME,
    FILE_KEY_IS_DIR,
    FILE_KEY_MIME,
    FILE_KEY_CONTENT,
];
const BYTES_CONTENT_KEYS: &[&str] = &[FILE_CONTENT_KEY_KIND, FILE_CONTENT_KEY_BYTES];
const DIRECTORY_CONTENT_KEYS: &[&str] = &[FILE_CONTENT_KEY_KIND, FILE_CONTENT_KEY_ENTRIES];

pub(super) struct FileNode<'a> {
    pub name: &'a str,
    pub mime: Option<&'a str>,
    pub content: FileNodeContent<'a>,
}

pub(super) enum FileNodeContent<'a> {
    Bytes(usize),
    Directory(&'a [Value]),
}

pub(super) fn parse_file_node<'a>(
    field_name: &InputName,
    value: &'a Value,
) -> Result<FileNode<'a>, InputValueError> {
    let object = value.as_object().ok_or_else(|| {
        invalid_structure(
            field_name,
            format!("expected object, got {}", json_value_kind(value)),
        )
    })?;
    reject_unknown_keys(field_name, object, FILE_KEYS)?;
    let file_name = required_string(field_name, object, FILE_KEY_NAME)?;
    let is_dir = required_bool(field_name, object, FILE_KEY_IS_DIR)?;
    let mime = optional_string(field_name, object, FILE_KEY_MIME)?;
    let content = object
        .get(FILE_KEY_CONTENT)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid_structure(
                field_name,
                format!("`{FILE_KEY_CONTENT}` must be an object"),
            )
        })?;
    let kind = required_string(field_name, content, FILE_CONTENT_KEY_KIND)?;
    let content = match kind {
        FILE_CONTENT_KIND_BYTES => parse_bytes_content(field_name, is_dir, content)?,
        FILE_CONTENT_KIND_DIRECTORY => {
            parse_directory_content(field_name, file_name, is_dir, content)?
        }
        _ => {
            return Err(invalid_structure(
                field_name,
                format!("unknown `{FILE_CONTENT_KEY_KIND}` value `{kind}`"),
            ));
        }
    };
    Ok(FileNode {
        name: file_name,
        mime,
        content,
    })
}

fn parse_bytes_content<'a>(
    field_name: &InputName,
    is_dir: bool,
    content: &'a Map<String, Value>,
) -> Result<FileNodeContent<'a>, InputValueError> {
    reject_unknown_keys(field_name, content, BYTES_CONTENT_KEYS)?;
    if is_dir {
        return Err(invalid_structure(
            field_name,
            "`is_dir` is true for bytes content",
        ));
    }
    let encoded = content
        .get(FILE_CONTENT_KEY_BYTES)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_structure(
                field_name,
                format!("`{FILE_CONTENT_KEY_BYTES}` must be a base64 string"),
            )
        })?;
    let byte_count = canonical_base64_decoded_len(encoded).map_err(|error| {
        invalid_structure(
            field_name,
            format!("`{FILE_CONTENT_KEY_BYTES}` must be canonical base64: {error}"),
        )
    })?;
    Ok(FileNodeContent::Bytes(byte_count))
}

fn parse_directory_content<'a>(
    field_name: &InputName,
    directory_name: &str,
    is_dir: bool,
    content: &'a Map<String, Value>,
) -> Result<FileNodeContent<'a>, InputValueError> {
    reject_unknown_keys(field_name, content, DIRECTORY_CONTENT_KEYS)?;
    if !is_dir {
        return Err(invalid_structure(
            field_name,
            "`is_dir` is false for directory content",
        ));
    }
    let entries = content
        .get(FILE_CONTENT_KEY_ENTRIES)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_structure(
                field_name,
                format!("`{FILE_CONTENT_KEY_ENTRIES}` must be an array"),
            )
        })?;
    if entries.is_empty() {
        return Err(FileInputValueError::EmptyDirectory {
            name: field_name.clone(),
            directory_name: directory_name.to_string(),
        }
        .into());
    }
    Ok(FileNodeContent::Directory(entries))
}

fn optional_string<'a>(
    field_name: &InputName,
    object: &'a Map<String, Value>,
    key: &'static str,
) -> Result<Option<&'a str>, InputValueError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| invalid_structure(field_name, format!("`{key}` must be a string"))),
    }
}

fn reject_unknown_keys(
    field_name: &InputName,
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), InputValueError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid_structure(
            field_name,
            format!("unknown key `{key}`"),
        ));
    }
    Ok(())
}

fn required_string<'a>(
    field_name: &InputName,
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, InputValueError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_structure(field_name, format!("`{key}` must be a string")))
}

fn required_bool(
    field_name: &InputName,
    object: &Map<String, Value>,
    key: &str,
) -> Result<bool, InputValueError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_structure(field_name, format!("`{key}` must be a boolean")))
}

fn invalid_structure(name: &InputName, detail: impl Into<String>) -> InputValueError {
    FileInputValueError::InvalidStructure {
        name: name.clone(),
        detail: detail.into(),
    }
    .into()
}
