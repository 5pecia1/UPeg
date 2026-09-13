use super::file_budget::{MAX_FILE_VALUE_METADATA_BYTES, MAX_FILE_VALUE_NODES};
use super::file_validation::FileInputValueError;
use super::{InputName, InputValueError};

// `MAX_FILE_INPUT_COUNT` (max recursive regular-file count) and
// `MAX_FILE_INPUT_RAW_BYTES` (max aggregate decoded bytes) are declared once
// in the canonical `file_budget` module; re-exported here under their
// established names so call sites keep compiling.
pub use super::file_budget::{MAX_FILE_INPUT_COUNT, MAX_FILE_INPUT_RAW_BYTES};

/// Maximum number of FileValue nodes accepted by Core input validation.
///
/// Aliases the canonical [`MAX_FILE_VALUE_NODES`] budget — input and output
/// walk the same tree shape and must agree on the same cap.
pub const MAX_FILE_INPUT_NODES: u64 = MAX_FILE_VALUE_NODES;

/// Maximum aggregate UTF-8 bytes of FileValue names and optional MIME values.
///
/// Aliases the canonical [`MAX_FILE_VALUE_METADATA_BYTES`] budget.
pub const MAX_FILE_INPUT_METADATA_BYTES: u64 = MAX_FILE_VALUE_METADATA_BYTES;

#[derive(Clone, Copy)]
pub(super) struct FileResourceSummary {
    node_count: u64,
    metadata_bytes: u64,
}

impl FileResourceSummary {
    pub(super) fn for_node(
        field_name: &InputName,
        file_name: &str,
        mime: Option<&str>,
    ) -> Result<Self, InputValueError> {
        let name_bytes =
            u64::try_from(file_name.len()).map_err(|_| metadata_error(field_name, u64::MAX))?;
        let mime_bytes = u64::try_from(mime.map_or(0, str::len))
            .map_err(|_| metadata_error(field_name, u64::MAX))?;
        let metadata_bytes = name_bytes
            .checked_add(mime_bytes)
            .ok_or_else(|| metadata_error(field_name, u64::MAX))?;

        Ok(Self {
            node_count: 1,
            metadata_bytes,
        })
    }

    pub(super) fn checked_add(
        self,
        field_name: &InputName,
        child: Self,
    ) -> Result<Self, InputValueError> {
        let node_count = self
            .node_count
            .checked_add(child.node_count)
            .ok_or_else(|| {
                InputValueError::from(FileInputValueError::NodeCountExceeded {
                    name: field_name.clone(),
                    max: MAX_FILE_INPUT_NODES,
                    actual: u64::MAX,
                })
            })?;
        let metadata_bytes = self
            .metadata_bytes
            .checked_add(child.metadata_bytes)
            .ok_or_else(|| metadata_error(field_name, u64::MAX))?;

        Ok(Self {
            node_count,
            metadata_bytes,
        })
    }

    pub(super) fn enforce(self, field_name: &InputName) -> Result<(), InputValueError> {
        if self.node_count > MAX_FILE_INPUT_NODES {
            return Err(FileInputValueError::NodeCountExceeded {
                name: field_name.clone(),
                max: MAX_FILE_INPUT_NODES,
                actual: self.node_count,
            }
            .into());
        }
        if self.metadata_bytes > MAX_FILE_INPUT_METADATA_BYTES {
            return Err(metadata_error(field_name, self.metadata_bytes));
        }
        Ok(())
    }
}

fn metadata_error(field_name: &InputName, actual: u64) -> InputValueError {
    FileInputValueError::MetadataTooLarge {
        name: field_name.clone(),
        max: MAX_FILE_INPUT_METADATA_BYTES,
        actual,
    }
    .into()
}
