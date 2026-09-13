use std::fs::Metadata;
use std::path::Path;

use upeg_core::InputFieldSpec;

use crate::CliError;

pub(super) struct FileIdentitySnapshot<'a> {
    pub(super) inspected: &'a Metadata,
    pub(super) opened: &'a Metadata,
    pub(super) current: &'a Metadata,
}

pub(super) fn validate_file_identity(
    field: &InputFieldSpec,
    path: &Path,
    snapshot: FileIdentitySnapshot<'_>,
) -> Result<(), CliError> {
    let inspected =
        file_identity(snapshot.inspected).ok_or_else(|| unsupported_identity_error(field, path))?;
    let opened =
        file_identity(snapshot.opened).ok_or_else(|| unsupported_identity_error(field, path))?;
    let current =
        file_identity(snapshot.current).ok_or_else(|| unsupported_identity_error(field, path))?;
    if inspected == opened && opened == current {
        return Ok(());
    }
    Err(changed_file_error(field, path))
}

fn changed_file_error(field: &InputFieldSpec, path: &Path) -> CliError {
    CliError::tool_failed(format!(
        "file input `{}` at `{}` changed while being read",
        field.name,
        path.display()
    ))
}

fn unsupported_identity_error(field: &InputFieldSpec, path: &Path) -> CliError {
    CliError::tool_failed(format!(
        "file input `{}` at `{}` cannot be read safely because file identity is unavailable",
        field.name,
        path.display()
    ))
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn file_identity(metadata: &Metadata) -> Option<FileIdentity> {
    use std::os::unix::fs::MetadataExt as _;

    Some(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    volume_serial_number: u32,
    file_index: u64,
}

#[cfg(windows)]
fn file_identity(metadata: &Metadata) -> Option<FileIdentity> {
    use std::os::windows::fs::MetadataExt as _;

    Some(FileIdentity {
        volume_serial_number: metadata.volume_serial_number()?,
        file_index: metadata.file_index()?,
    })
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity;

#[cfg(not(any(unix, windows)))]
fn file_identity(_metadata: &Metadata) -> Option<FileIdentity> {
    None
}
