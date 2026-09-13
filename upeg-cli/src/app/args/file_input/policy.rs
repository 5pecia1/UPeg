use std::io::Read as _;

use upeg_core::{FileInputPolicy, InputFieldSpec, MAX_FILE_INPUT_RAW_BYTES};

use super::identity::{FileIdentitySnapshot, validate_file_identity};
use super::{DirectFile, unreadable_file};
use crate::CliError;

pub(super) fn validate_extension(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    name: &str,
) -> Result<(), CliError> {
    if policy.extensions().next().is_none() {
        return Ok(());
    }
    let lower_name = name.to_ascii_lowercase();
    if policy.extensions().any(|extension| {
        lower_name
            .strip_suffix(extension)
            .is_some_and(|prefix| prefix.ends_with('.'))
    }) {
        return Ok(());
    }
    Err(CliError::tool_failed(format!(
        "file input `{}` rejects `{name}` by extension policy",
        field.name
    )))
}

pub(super) fn validate_file_size(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    file: &DirectFile,
) -> Result<(), CliError> {
    if policy
        .max_file_bytes()
        .is_some_and(|max_file_bytes| file.size > max_file_bytes)
    {
        return Err(CliError::tool_failed(format!(
            "file input `{}` file `{}` exceeds max_file_bytes",
            field.name, file.name
        )));
    }
    if file.size > MAX_FILE_INPUT_RAW_BYTES {
        return Err(CliError::tool_failed(format!(
            "file input `{}` file `{}` exceeds the {MAX_FILE_INPUT_RAW_BYTES}-byte CLI limit",
            field.name, file.name
        )));
    }
    Ok(())
}

pub(super) fn validate_total_size(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    total: u64,
) -> Result<(), CliError> {
    if policy
        .max_total_bytes()
        .is_some_and(|max_total_bytes| total > max_total_bytes)
    {
        return Err(CliError::tool_failed(format!(
            "file input `{}` exceeds max_total_bytes",
            field.name
        )));
    }
    if total > MAX_FILE_INPUT_RAW_BYTES {
        return Err(CliError::tool_failed(format!(
            "file input `{}` exceeds the {MAX_FILE_INPUT_RAW_BYTES}-byte total CLI limit",
            field.name
        )));
    }
    Ok(())
}

pub(super) fn effective_file_limit(policy: &FileInputPolicy) -> u64 {
    policy
        .max_file_bytes()
        .map_or(MAX_FILE_INPUT_RAW_BYTES, |limit| {
            limit.min(MAX_FILE_INPUT_RAW_BYTES)
        })
}

pub(super) fn effective_total_limit(policy: &FileInputPolicy) -> u64 {
    policy
        .max_total_bytes()
        .map_or(MAX_FILE_INPUT_RAW_BYTES, |limit| {
            limit.min(MAX_FILE_INPUT_RAW_BYTES)
        })
}

pub(super) fn read_capped(
    field: &InputFieldSpec,
    direct_file: &DirectFile,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, CliError> {
    let opened = direct_file
        .file
        .metadata()
        .map_err(|error| unreadable_file(field, &direct_file.path, error))?;
    validate_file_identity(
        field,
        &direct_file.path,
        FileIdentitySnapshot {
            inspected: &direct_file.metadata,
            opened: &opened,
            current: &opened,
        },
    )?;
    let mut bytes = Vec::new();
    (&direct_file.file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable_file(field, &direct_file.path, error))?;
    if u64::try_from(bytes.len()).is_ok_and(|len| len <= max_bytes) {
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

pub(super) fn effective_size_error(field: &InputFieldSpec, name: &str, limit: u64) -> CliError {
    CliError::tool_failed(format!(
        "file input `{}` file `{name}` exceeds the effective {limit}-byte limit",
        field.name
    ))
}
