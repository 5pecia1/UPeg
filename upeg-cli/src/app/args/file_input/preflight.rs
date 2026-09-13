use std::fmt;

use upeg_core::{
    MAX_FILE_INPUT_COUNT, MAX_FILE_INPUT_METADATA_BYTES, MAX_FILE_INPUT_NODES,
    MAX_FILE_INPUT_RAW_BYTES,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FilePreflight {
    file_count: u64,
    node_count: u64,
    metadata_bytes: u64,
    raw_bytes: u64,
}

impl FilePreflight {
    pub(super) fn for_file(
        name: &str,
        mime: Option<&str>,
        raw_bytes: u64,
    ) -> Result<Self, FilePreflightError> {
        Self::default().add_node(name, mime, raw_bytes, FileNodeKind::File)
    }

    pub(super) fn for_directory(name: &str) -> Result<Self, FilePreflightError> {
        Self::default().add_node(name, None, 0, FileNodeKind::Directory)
    }

    pub(super) fn add_file(
        self,
        name: &str,
        mime: Option<&str>,
        raw_bytes: u64,
    ) -> Result<Self, FilePreflightError> {
        self.add_node(name, mime, raw_bytes, FileNodeKind::File)
    }

    fn add_node(
        self,
        name: &str,
        mime: Option<&str>,
        raw_bytes: u64,
        node_kind: FileNodeKind,
    ) -> Result<Self, FilePreflightError> {
        let file_count = match node_kind {
            FileNodeKind::File => checked_total(
                self.file_count,
                1,
                FileResource::FileCount,
                u64::from(MAX_FILE_INPUT_COUNT),
            )?,
            FileNodeKind::Directory => self.file_count,
        };
        let node_count = checked_total(
            self.node_count,
            1,
            FileResource::NodeCount,
            MAX_FILE_INPUT_NODES,
        )?;
        let name_bytes = u64::try_from(name.len())
            .map_err(|_| FilePreflightError::overflow(FileResource::MetadataBytes))?;
        let mime_bytes = u64::try_from(mime.map_or(0, str::len))
            .map_err(|_| FilePreflightError::overflow(FileResource::MetadataBytes))?;
        let added_metadata = name_bytes
            .checked_add(mime_bytes)
            .ok_or_else(|| FilePreflightError::overflow(FileResource::MetadataBytes))?;
        let metadata_bytes = checked_total(
            self.metadata_bytes,
            added_metadata,
            FileResource::MetadataBytes,
            MAX_FILE_INPUT_METADATA_BYTES,
        )?;
        let raw_bytes = checked_total(
            self.raw_bytes,
            raw_bytes,
            FileResource::RawBytes,
            MAX_FILE_INPUT_RAW_BYTES,
        )?;
        Ok(Self {
            file_count,
            node_count,
            metadata_bytes,
            raw_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum FileNodeKind {
    File,
    Directory,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum FileResource {
    FileCount,
    NodeCount,
    MetadataBytes,
    RawBytes,
}

impl fmt::Display for FileResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FileCount => "file count",
            Self::NodeCount => "node count",
            Self::MetadataBytes => "metadata bytes",
            Self::RawBytes => "raw bytes",
        })
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub(super) enum FilePreflightError {
    #[error("{resource} exceeds the fixed {max}-unit limit (actual {actual})")]
    LimitExceeded {
        resource: FileResource,
        max: u64,
        actual: u64,
    },
    #[error("{resource} overflowed while computing preflight totals")]
    ArithmeticOverflow { resource: FileResource },
}

impl FilePreflightError {
    const fn overflow(resource: FileResource) -> Self {
        Self::ArithmeticOverflow { resource }
    }
}

fn checked_total(
    current: u64,
    added: u64,
    resource: FileResource,
    max: u64,
) -> Result<u64, FilePreflightError> {
    let actual = current
        .checked_add(added)
        .ok_or_else(|| FilePreflightError::overflow(resource))?;
    if actual > max {
        return Err(FilePreflightError::LimitExceeded {
            resource,
            max,
            actual,
        });
    }
    Ok(actual)
}
