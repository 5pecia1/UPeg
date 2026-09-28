use std::path::{Path, PathBuf};

use upeg_core::{
    FileContent, FileInputPolicy, FileValue, InputFieldSpec, InputValue, MAX_FILE_INPUT_COUNT,
};

use crate::CliError;

mod capability;
mod identity;
#[cfg(test)]
mod identity_tests;
mod mime;
mod policy;
mod preflight;
#[cfg(test)]
mod root_race_tests;
mod scan;
#[cfg(test)]
mod scan_tests;

use capability::{OpenedInput, open_child_file_nofollow, open_input_nofollow};
use mime::guess_mime_from_extension;
use policy::{
    effective_file_limit, effective_size_error, effective_total_limit, read_capped,
    validate_extension, validate_file_size, validate_total_size,
};
use preflight::{FilePreflight, FilePreflightError};
use scan::bounded_directory_entries;

const CLI_FILE_PATH_SIGIL: char = '@';

struct DirectFile {
    name: String,
    path: PathBuf,
    size: u64,
    mime: Option<String>,
    metadata: cap_std::fs::Metadata,
    file: cap_std::fs::File,
}

pub(super) fn file_value_from_cli_path(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    raw: &str,
) -> Result<Option<InputValue>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Some(path_str) = trimmed.strip_prefix(CLI_FILE_PATH_SIGIL) else {
        return Err(CliError::tool_failed(format!(
            "input `{name}` is a file; pass it as `{CLI_FILE_PATH_SIGIL}<path>` (e.g. `-a {name}={CLI_FILE_PATH_SIGIL}/abs/file`)",
            name = field.name.as_str(),
        )));
    };
    let path = Path::new(path_str);
    match open_input_nofollow(path).map_err(|error| unreadable_file(field, path, error))? {
        OpenedInput::Symlink => Err(CliError::tool_failed(format!(
            "file input `{}` at `{path_str}` is a symlink",
            field.name
        ))),
        OpenedInput::File(opened) => {
            let name = direct_file_name(field, path)?;
            regular_file_value(
                field,
                policy,
                DirectFile {
                    mime: guess_mime_from_extension(path),
                    name,
                    path: path.to_path_buf(),
                    size: opened.metadata.len(),
                    metadata: opened.metadata,
                    file: opened.file,
                },
            )
            .map(Some)
        }
        OpenedInput::Directory(directory) => {
            if policy.max_count() == 1 {
                return Err(CliError::tool_failed(format!(
                    "file input `{}` has max_count=1 and does not accept a directory",
                    field.name
                )));
            }
            directory_value(field, policy, path, directory).map(Some)
        }
        OpenedInput::Other => Err(CliError::tool_failed(format!(
            "file input `{}` at `{path_str}` is not a regular file or directory",
            field.name
        ))),
    }
}

fn regular_file_value(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    file: DirectFile,
) -> Result<InputValue, CliError> {
    validate_extension(field, policy, &file.name)?;
    validate_file_size(field, policy, &file)?;
    validate_total_size(field, policy, file.size)?;
    FilePreflight::for_file(&file.name, file.mime.as_deref(), file.size)
        .map_err(|error| preflight_error(field, error))?;
    let read_limit = effective_file_limit(policy).min(effective_total_limit(policy));
    let bytes = read_capped(field, &file, read_limit)?
        .ok_or_else(|| effective_size_error(field, &file.name, read_limit))?;
    Ok(InputValue::File(FileValue {
        name: file.name,
        content: FileContent::Bytes(bytes),
        mime: file.mime,
    }))
}

fn directory_value(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    path: &Path,
    directory: cap_std::fs::Dir,
) -> Result<InputValue, CliError> {
    let directory_name = direct_file_name(field, path)?;
    let entries = directory
        .entries()
        .map_err(|error| unreadable_file(field, path, error))?;
    let entries = bounded_directory_entries(
        &field.name,
        policy.max_count().min(MAX_FILE_INPUT_COUNT),
        entries.map(|entry| entry.map_err(|error| unreadable_file(field, path, error))),
    )?;
    let mut files = entries
        .into_iter()
        .map(|entry| {
            let entry_name = entry.file_name();
            let entry_path = path.join(&entry_name);
            let metadata = directory
                .symlink_metadata(&entry_name)
                .map_err(|error| unreadable_file(field, &entry_path, error))?;
            if metadata.is_symlink() {
                return Err(CliError::tool_failed(format!(
                    "file input `{}` contains a symlink at `{}`",
                    field.name,
                    entry_path.display()
                )));
            }
            if metadata.is_dir() {
                return Err(CliError::tool_failed(format!(
                    "file input `{}` contains a nested directory at `{}`",
                    field.name,
                    entry_path.display()
                )));
            }
            if !metadata.is_file() {
                return Err(CliError::tool_failed(format!(
                    "file input `{}` contains a non-regular file at `{}`",
                    field.name,
                    entry_path.display()
                )));
            }
            let opened = open_child_file_nofollow(&directory, &entry_name, &metadata)
                .map_err(|error| unreadable_file(field, &entry_path, error))?;
            Ok(DirectFile {
                name: direct_file_name(field, &entry_path)?,
                mime: guess_mime_from_extension(&entry_path),
                path: entry_path,
                size: opened.metadata.len(),
                metadata: opened.metadata,
                file: opened.file,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if files.is_empty() {
        return Err(CliError::tool_failed(format!(
            "file input `{}` directory is empty",
            field.name
        )));
    }
    files.sort_by(|left, right| left.name.cmp(&right.name));

    let mut metadata_total = 0_u64;
    let mut preflight = FilePreflight::for_directory(&directory_name)
        .map_err(|error| preflight_error(field, error))?;
    for file in &files {
        validate_extension(field, policy, &file.name)?;
        validate_file_size(field, policy, file)?;
        metadata_total = metadata_total.checked_add(file.size).ok_or_else(|| {
            CliError::tool_failed(format!(
                "file input `{}` exceeds max_total_bytes",
                field.name
            ))
        })?;
        validate_total_size(field, policy, metadata_total)?;
        preflight = preflight
            .add_file(&file.name, file.mime.as_deref(), file.size)
            .map_err(|error| preflight_error(field, error))?;
    }

    let mut entries = Vec::with_capacity(files.len());
    let mut bytes_read = 0_u64;
    let total_limit = effective_total_limit(policy);
    for file in files {
        let remaining = total_limit.saturating_sub(bytes_read);
        let read_limit = effective_file_limit(policy).min(remaining);
        let bytes = read_capped(field, &file, read_limit)?
            .ok_or_else(|| effective_size_error(field, &file.name, read_limit))?;
        let byte_count = u64::try_from(bytes.len()).map_err(|_| {
            CliError::tool_failed(format!(
                "file input `{}` exceeds max_total_bytes",
                field.name
            ))
        })?;
        bytes_read = bytes_read.checked_add(byte_count).ok_or_else(|| {
            CliError::tool_failed(format!(
                "file input `{}` exceeds max_total_bytes",
                field.name
            ))
        })?;
        validate_total_size(field, policy, bytes_read)?;
        entries.push(FileValue {
            name: file.name,
            content: FileContent::Bytes(bytes),
            mime: file.mime,
        });
    }

    Ok(InputValue::File(FileValue {
        name: directory_name,
        content: FileContent::Directory(entries),
        mime: None,
    }))
}

fn preflight_error(field: &InputFieldSpec, error: FilePreflightError) -> CliError {
    CliError::tool_failed(format!("file input `{}` {error}", field.name))
}

fn direct_file_name(field: &InputFieldSpec, path: &Path) -> Result<String, CliError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            CliError::tool_failed(format!(
                "file input `{}` contains a non-Unicode file name at `{}`",
                field.name,
                path.display()
            ))
        })
}

fn unreadable_file(field: &InputFieldSpec, path: &Path, error: std::io::Error) -> CliError {
    CliError::tool_failed(format!(
        "could not read file input `{}` at `{}`: {error}",
        field.name,
        path.display()
    ))
}
