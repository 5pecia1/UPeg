use std::ffi::OsStr;
use std::io;
use std::path::Path;

#[cfg(unix)]
use cap_fs_ext::OpenOptionsSyncExt as _;
use cap_fs_ext::{DirExt as _, FollowSymlinks, MetadataExt as _, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata, OpenOptions};

const CHANGED_WHILE_OPENING: &str = "file changed while being opened";

pub(super) enum OpenedInput {
    File(OpenedFile),
    Directory(Dir),
    Symlink,
    Other,
}

pub(super) struct OpenedFile {
    pub(super) file: cap_std::fs::File,
    pub(super) metadata: Metadata,
}

pub(super) fn open_input_nofollow(path: &Path) -> io::Result<OpenedInput> {
    let Some(name) = path.file_name() else {
        return Dir::open_ambient_dir(path, ambient_authority()).map(OpenedInput::Directory);
    };
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())?;
    let inspected = parent.symlink_metadata(name)?;
    if inspected.is_symlink() {
        return Ok(OpenedInput::Symlink);
    }
    if inspected.is_file() {
        return open_file_nofollow(&parent, name, &inspected).map(OpenedInput::File);
    }
    if inspected.is_dir() {
        return open_directory_entry_nofollow(&parent, name, &inspected)
            .map(OpenedInput::Directory);
    }
    Ok(OpenedInput::Other)
}

#[cfg(test)]
pub(super) fn open_directory_nofollow(path: &Path) -> io::Result<Dir> {
    match open_input_nofollow(path)? {
        OpenedInput::Directory(directory) => Ok(directory),
        OpenedInput::File(_) | OpenedInput::Symlink | OpenedInput::Other => {
            Err(io::Error::other("path is not a directory"))
        }
    }
}

pub(super) fn open_child_file_nofollow(
    directory: &Dir,
    name: &OsStr,
    inspected: &Metadata,
) -> io::Result<OpenedFile> {
    open_file_nofollow(directory, name, inspected)
}

fn open_file_nofollow(
    directory: &Dir,
    name: &OsStr,
    inspected: &Metadata,
) -> io::Result<OpenedFile> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.nonblock(true);
    let file = directory.open_with(name, &options)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(io::Error::other(CHANGED_WHILE_OPENING));
    }
    let current = directory.symlink_metadata(name)?;
    validate_identity(inspected, &opened, &current)?;
    #[cfg(unix)]
    clear_nonblocking(&file)?;
    Ok(OpenedFile {
        file,
        metadata: opened,
    })
}

#[cfg(unix)]
fn clear_nonblocking(file: &cap_std::fs::File) -> io::Result<()> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    let flags = fcntl_getfl(file)?;
    fcntl_setfl(file, flags & !OFlags::NONBLOCK)?;
    Ok(())
}

fn open_directory_entry_nofollow(
    parent: &Dir,
    name: &OsStr,
    inspected: &Metadata,
) -> io::Result<Dir> {
    let directory = parent.open_dir_nofollow(name)?;
    let opened = directory.dir_metadata()?;
    let current = parent.symlink_metadata(name)?;
    validate_identity(inspected, &opened, &current)?;
    Ok(directory)
}

fn validate_identity(
    inspected: &Metadata,
    opened: &Metadata,
    current: &Metadata,
) -> io::Result<()> {
    let inspected = (inspected.dev(), inspected.ino());
    let opened = (opened.dev(), opened.ino());
    let current = (current.dev(), current.ino());
    if inspected == opened && opened == current {
        Ok(())
    } else {
        Err(io::Error::other(CHANGED_WHILE_OPENING))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::process::Command;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;

    const FIFO_CREATOR: &str = "mkfifo";
    const OPEN_RESULT_DEADLINE: Duration = Duration::from_millis(250);
    const CLEANUP_RESULT_DEADLINE: Duration = Duration::from_secs(1);

    #[test]
    fn open_does_not_block_when_the_inspected_regular_file_is_swapped_for_a_fifo() {
        let temp = tempdir().expect("must create the test directory");
        let path = temp.path().join("input.png");
        let name = path
            .file_name()
            .expect("the test file must have a name")
            .to_os_string();
        fs::write(&path, b"regular").expect("must write the regular file to inspect");
        let directory = open_directory_nofollow(temp.path()).expect("must open the test directory");
        let inspected = directory
            .symlink_metadata(&name)
            .expect("must inspect the regular file's metadata");
        fs::remove_file(&path).expect("must remove the inspected regular file");
        let fifo_created = Command::new(FIFO_CREATOR)
            .arg(&path)
            .status()
            .expect("must run the FIFO creation command");
        assert!(fifo_created.success(), "must create the FIFO");
        let (started_sender, started_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let opener = thread::spawn(move || {
            started_sender
                .send(())
                .expect("must report the file open starting");
            let result = open_child_file_nofollow(&directory, &name, &inspected).map(drop);
            result_sender
                .send(result)
                .expect("must send the file open result");
        });
        started_receiver
            .recv()
            .expect("must receive the file open start");

        match result_receiver.recv_timeout(OPEN_RESULT_DEADLINE) {
            Ok(result) => {
                opener.join().expect("the file open thread must exit");
                assert!(result.is_err(), "the FIFO swap must be rejected");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let writer = fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .expect("must clean up the waiting FIFO reader");
                drop(writer);
                result_receiver
                    .recv_timeout(CLEANUP_RESULT_DEADLINE)
                    .expect("FIFO reader cleanup must finish within the deadline")
                    .expect_err("the FIFO swap must be rejected");
                opener.join().expect("the file open thread must exit");
                panic!("blocked while opening a file swapped for a FIFO");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                opener.join().expect("the file open thread must exit");
                panic!("the file open result channel disconnected");
            }
        }
    }
}
