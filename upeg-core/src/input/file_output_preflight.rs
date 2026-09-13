//! Resource preflight for untrusted `File` outputs.
//!
//! Raw JSON is walked before `FileValue` deserialization so aggregate limits
//! are known before any base64 payload is decoded and allocated.

use serde_json::Value;

use super::file_budget::{
    MAX_FILE_NESTING_DEPTH, MAX_FILE_VALUE_METADATA_BYTES, MAX_FILE_VALUE_NODES,
};
use super::file_value::{FileContent, FileValue};

mod json;

// `MAX_FILE_OUTPUT_RAW_BYTES` is declared once in the canonical `file_budget`
// module; re-exported here under its established name so call sites keep
// compiling.
pub use super::file_budget::MAX_FILE_OUTPUT_RAW_BYTES;

/// Additional transport headroom for base64 expansion and the canonical JSON envelope.
const UNTRUSTED_OUTPUT_WIRE_OVERHEAD_BYTES: u64 = MAX_FILE_OUTPUT_RAW_BYTES / 2;
/// Maximum bytes accepted from an untrusted transport before parsing its output wire.
///
/// The 64 MiB decoded File ceiling can expand by roughly one third in base64.
/// The remaining headroom covers the canonical JSON structure and metadata.
pub const MAX_UNTRUSTED_OUTPUT_WIRE_BYTES: u64 =
    MAX_FILE_OUTPUT_RAW_BYTES + UNTRUSTED_OUTPUT_WIRE_OVERHEAD_BYTES;
/// Maximum nodes in one `FileValue` output tree.
///
/// Aliases the canonical [`MAX_FILE_VALUE_NODES`] budget — input and output
/// walk the same tree shape and must agree on the same cap.
pub const MAX_FILE_OUTPUT_NODES: u64 = MAX_FILE_VALUE_NODES;
/// Maximum aggregate UTF-8 bytes of names and MIME values in one output tree.
///
/// Aliases the canonical [`MAX_FILE_VALUE_METADATA_BYTES`] budget.
pub const MAX_FILE_OUTPUT_METADATA_BYTES: u64 = MAX_FILE_VALUE_METADATA_BYTES;
/// Maximum nodes on one root-to-leaf path, counting the root as depth one.
pub const MAX_FILE_OUTPUT_NESTING_DEPTH: usize = MAX_FILE_NESTING_DEPTH;

/// Rejection reason for an untrusted `File` output tree.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileOutputPreflightError {
    #[error("invalid file output structure: {detail}")]
    InvalidStructure { detail: String },
    #[error("file output nesting exceeds the maximum depth of {max}")]
    NestingTooDeep { max: usize },
    #[error("file output node count {actual} exceeds the maximum of {max}")]
    NodeCountExceeded { max: u64, actual: u64 },
    #[error("file output metadata bytes {actual} exceeds the maximum of {max}")]
    MetadataTooLarge { max: u64, actual: u64 },
    #[error("file output decoded bytes {actual} exceeds the maximum of {max}")]
    RawBytesTooLarge { max: u64, actual: u64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FileOutputSummary {
    nodes: u64,
    metadata_bytes: u64,
    raw_bytes: u64,
}

/// Validate an untrusted canonical `FileValue` JSON tree without decoding base64.
///
/// # Errors
/// Returns [`FileOutputPreflightError`] for malformed wire values or a resource
/// budget violation.
pub fn preflight_file_output_json(value: &Value) -> Result<(), FileOutputPreflightError> {
    let mut summary = FileOutputSummary::default();
    json::visit(value, 1, &mut summary)
}

/// Validate an already-decoded `FileValue` output tree against the same budgets.
///
/// # Errors
/// Returns [`FileOutputPreflightError`] when the tree exceeds a resource budget.
pub fn validate_file_output_tree(file: &FileValue) -> Result<(), FileOutputPreflightError> {
    let mut summary = FileOutputSummary::default();
    visit_typed_node(file, 1, &mut summary)
}

fn visit_typed_node(
    file: &FileValue,
    depth: usize,
    summary: &mut FileOutputSummary,
) -> Result<(), FileOutputPreflightError> {
    enforce_depth(depth)?;
    summary.add_node()?;
    summary.add_metadata(file.name.len())?;
    if let Some(mime) = &file.mime {
        summary.add_metadata(mime.len())?;
    }
    match &file.content {
        FileContent::Bytes(bytes) => summary.add_raw(bytes.len()),
        FileContent::Directory(entries) => {
            for entry in entries {
                let child_depth =
                    depth
                        .checked_add(1)
                        .ok_or(FileOutputPreflightError::NestingTooDeep {
                            max: MAX_FILE_OUTPUT_NESTING_DEPTH,
                        })?;
                visit_typed_node(entry, child_depth, summary)?;
            }
            Ok(())
        }
    }
}

impl FileOutputSummary {
    pub(super) fn add_node(&mut self) -> Result<(), FileOutputPreflightError> {
        self.nodes =
            self.nodes
                .checked_add(1)
                .ok_or(FileOutputPreflightError::NodeCountExceeded {
                    max: MAX_FILE_OUTPUT_NODES,
                    actual: u64::MAX,
                })?;
        if self.nodes > MAX_FILE_OUTPUT_NODES {
            return Err(FileOutputPreflightError::NodeCountExceeded {
                max: MAX_FILE_OUTPUT_NODES,
                actual: self.nodes,
            });
        }
        Ok(())
    }

    pub(super) fn add_metadata(&mut self, bytes: usize) -> Result<(), FileOutputPreflightError> {
        let bytes = u64::try_from(bytes).map_err(|_| metadata_overflow())?;
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(bytes)
            .ok_or_else(metadata_overflow)?;
        if self.metadata_bytes > MAX_FILE_OUTPUT_METADATA_BYTES {
            return Err(FileOutputPreflightError::MetadataTooLarge {
                max: MAX_FILE_OUTPUT_METADATA_BYTES,
                actual: self.metadata_bytes,
            });
        }
        Ok(())
    }

    pub(super) fn add_raw(&mut self, bytes: usize) -> Result<(), FileOutputPreflightError> {
        let bytes = u64::try_from(bytes).map_err(|_| raw_overflow())?;
        self.raw_bytes = self.raw_bytes.checked_add(bytes).ok_or_else(raw_overflow)?;
        if self.raw_bytes > MAX_FILE_OUTPUT_RAW_BYTES {
            return Err(FileOutputPreflightError::RawBytesTooLarge {
                max: MAX_FILE_OUTPUT_RAW_BYTES,
                actual: self.raw_bytes,
            });
        }
        Ok(())
    }
}

pub(super) fn enforce_depth(depth: usize) -> Result<(), FileOutputPreflightError> {
    if depth > MAX_FILE_OUTPUT_NESTING_DEPTH {
        Err(FileOutputPreflightError::NestingTooDeep {
            max: MAX_FILE_OUTPUT_NESTING_DEPTH,
        })
    } else {
        Ok(())
    }
}

fn metadata_overflow() -> FileOutputPreflightError {
    FileOutputPreflightError::MetadataTooLarge {
        max: MAX_FILE_OUTPUT_METADATA_BYTES,
        actual: u64::MAX,
    }
}

fn raw_overflow() -> FileOutputPreflightError {
    FileOutputPreflightError::RawBytesTooLarge {
        max: MAX_FILE_OUTPUT_RAW_BYTES,
        actual: u64::MAX,
    }
}

#[cfg(test)]
#[path = "file_output_preflight_tests.rs"]
mod tests;
