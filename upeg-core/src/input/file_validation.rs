use serde_json::Value;

use super::file_budget::MAX_FILE_NESTING_DEPTH;
use super::file_resource_limits::{FileResourceSummary, MAX_FILE_INPUT_RAW_BYTES};
use super::file_structure::{FileNodeContent, parse_file_node};
use super::{FileInputPolicy, InputFieldSpec, InputName, InputValueError};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileInputValueError {
    #[error("input `{name}` contains invalid file structure: {detail}")]
    InvalidStructure { name: InputName, detail: String },
    #[error("input `{name}` contains empty directory `{directory_name}`")]
    EmptyDirectory {
        name: InputName,
        directory_name: String,
    },
    #[error("input `{name}` exceeds the maximum file nesting depth of {max}")]
    NestingTooDeep { name: InputName, max: usize },
    #[error("input `{name}` contains {actual} files, exceeding the maximum of {max}")]
    CountExceeded {
        name: InputName,
        max: u32,
        actual: u64,
    },
    #[error("input `{name}` contains {actual} file nodes, exceeding the maximum of {max}")]
    NodeCountExceeded {
        name: InputName,
        max: u64,
        actual: u64,
    },
    #[error("input `{name}` has {actual} UTF-8 metadata bytes, exceeding the maximum of {max}")]
    MetadataTooLarge {
        name: InputName,
        max: u64,
        actual: u64,
    },
    #[error("input `{name}` rejects extension of file `{file_name}`; allowed: {allowed:?}")]
    ExtensionRejected {
        name: InputName,
        file_name: String,
        allowed: Vec<String>,
    },
    #[error("input `{name}` file `{file_name}` is {actual} bytes, exceeding {max}")]
    FileTooLarge {
        name: InputName,
        file_name: String,
        max: u64,
        actual: u64,
    },
    #[error("input `{name}` totals {actual} bytes, exceeding {max}")]
    TotalTooLarge {
        name: InputName,
        max: u64,
        actual: u64,
    },
}

#[derive(Clone, Copy)]
struct FileSummary {
    file_count: u64,
    total_bytes: u64,
    resources: FileResourceSummary,
}

pub(super) fn validate_file_value(
    field: &InputFieldSpec,
    policy: &FileInputPolicy,
    value: &Value,
) -> Result<(), InputValueError> {
    validate_node(&field.name, policy, value, 1)?;
    Ok(())
}

fn validate_node(
    field_name: &InputName,
    policy: &FileInputPolicy,
    value: &Value,
    depth: usize,
) -> Result<FileSummary, InputValueError> {
    if depth > MAX_FILE_NESTING_DEPTH {
        return Err(FileInputValueError::NestingTooDeep {
            name: field_name.clone(),
            max: MAX_FILE_NESTING_DEPTH,
        }
        .into());
    }
    let node = parse_file_node(field_name, value)?;
    let resources = FileResourceSummary::for_node(field_name, node.name, node.mime)?;
    let summary = match node.content {
        FileNodeContent::Bytes(byte_count) => {
            validate_bytes_policy(field_name, policy, node.name, byte_count, resources)?
        }
        FileNodeContent::Directory(entries) => entries.iter().try_fold(
            FileSummary {
                file_count: 0,
                total_bytes: 0,
                resources,
            },
            |summary, entry| {
                merge_summary(field_name, policy, summary, entry, depth.saturating_add(1))
            },
        )?,
    };
    enforce_aggregate_limits(field_name, policy, summary)?;
    Ok(summary)
}

fn validate_bytes_policy(
    field_name: &InputName,
    policy: &FileInputPolicy,
    file_name: &str,
    byte_count: usize,
    resources: FileResourceSummary,
) -> Result<FileSummary, InputValueError> {
    if !policy.accepts_extension(file_name) {
        return Err(FileInputValueError::ExtensionRejected {
            name: field_name.clone(),
            file_name: file_name.to_string(),
            allowed: policy.extensions().map(str::to_string).collect(),
        }
        .into());
    }
    let actual = u64::try_from(byte_count).map_err(|_| FileInputValueError::FileTooLarge {
        name: field_name.clone(),
        file_name: file_name.to_string(),
        max: policy.max_file_bytes().unwrap_or(u64::MAX),
        actual: u64::MAX,
    })?;
    if let Some(max) = policy.max_file_bytes()
        && actual > max
    {
        return Err(FileInputValueError::FileTooLarge {
            name: field_name.clone(),
            file_name: file_name.to_string(),
            max,
            actual,
        }
        .into());
    }
    Ok(FileSummary {
        file_count: 1,
        total_bytes: actual,
        resources,
    })
}

fn merge_summary(
    field_name: &InputName,
    policy: &FileInputPolicy,
    summary: FileSummary,
    entry: &Value,
    depth: usize,
) -> Result<FileSummary, InputValueError> {
    let child = validate_node(field_name, policy, entry, depth)?;
    let file_count = summary
        .file_count
        .checked_add(child.file_count)
        .ok_or_else(|| {
            InputValueError::from(FileInputValueError::CountExceeded {
                name: field_name.clone(),
                max: policy.max_count(),
                actual: u64::MAX,
            })
        })?;
    let total_bytes = summary
        .total_bytes
        .checked_add(child.total_bytes)
        .ok_or_else(|| {
            InputValueError::from(FileInputValueError::TotalTooLarge {
                name: field_name.clone(),
                max: aggregate_raw_input_limit(policy),
                actual: u64::MAX,
            })
        })?;
    let resources = summary.resources.checked_add(field_name, child.resources)?;
    let merged = FileSummary {
        file_count,
        total_bytes,
        resources,
    };
    enforce_aggregate_limits(field_name, policy, merged)?;
    Ok(merged)
}

fn enforce_aggregate_limits(
    field_name: &InputName,
    policy: &FileInputPolicy,
    summary: FileSummary,
) -> Result<(), InputValueError> {
    if summary.file_count > u64::from(policy.max_count()) {
        return Err(FileInputValueError::CountExceeded {
            name: field_name.clone(),
            max: policy.max_count(),
            actual: summary.file_count,
        }
        .into());
    }
    summary.resources.enforce(field_name)?;
    let max = aggregate_raw_input_limit(policy);
    if summary.total_bytes > max {
        return Err(FileInputValueError::TotalTooLarge {
            name: field_name.clone(),
            max,
            actual: summary.total_bytes,
        }
        .into());
    }
    Ok(())
}

fn aggregate_raw_input_limit(policy: &FileInputPolicy) -> u64 {
    policy
        .max_total_bytes()
        .map_or(MAX_FILE_INPUT_RAW_BYTES, |max| {
            max.min(MAX_FILE_INPUT_RAW_BYTES)
        })
}
