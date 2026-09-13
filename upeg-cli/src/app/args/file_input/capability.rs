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
    pub(super) file: std::fs::File,
    pub(super) metadata: std::fs::Metadata,
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
    let file = file.into_std();
    let metadata = file.metadata()?;
    Ok(OpenedFile { file, metadata })
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
    fn 검사한_일반_파일이_fifo로_교체되어도_열기가_대기하지_않는다() {
        let temp = tempdir().expect("테스트 디렉터리를 만들어야 한다");
        let path = temp.path().join("input.png");
        let name = path
            .file_name()
            .expect("테스트 파일 이름이 있어야 한다")
            .to_os_string();
        fs::write(&path, b"regular").expect("검사할 일반 파일을 써야 한다");
        let directory =
            open_directory_nofollow(temp.path()).expect("테스트 디렉터리를 열어야 한다");
        let inspected = directory
            .symlink_metadata(&name)
            .expect("일반 파일 metadata를 검사해야 한다");
        fs::remove_file(&path).expect("검사한 일반 파일을 제거해야 한다");
        let fifo_created = Command::new(FIFO_CREATOR)
            .arg(&path)
            .status()
            .expect("FIFO 생성 명령을 실행해야 한다");
        assert!(fifo_created.success(), "FIFO를 만들어야 한다");
        let (started_sender, started_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();

        let opener = thread::spawn(move || {
            started_sender
                .send(())
                .expect("파일 열기 시작을 알려야 한다");
            let result = open_child_file_nofollow(&directory, &name, &inspected).map(drop);
            result_sender
                .send(result)
                .expect("파일 열기 결과를 보내야 한다");
        });
        started_receiver
            .recv()
            .expect("파일 열기 시작을 받아야 한다");

        match result_receiver.recv_timeout(OPEN_RESULT_DEADLINE) {
            Ok(result) => {
                opener.join().expect("파일 열기 thread가 종료되어야 한다");
                assert!(result.is_err(), "FIFO 교체를 거부해야 한다");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let writer = fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .expect("대기 중인 FIFO reader를 정리해야 한다");
                drop(writer);
                result_receiver
                    .recv_timeout(CLEANUP_RESULT_DEADLINE)
                    .expect("FIFO reader 정리가 제한 시간 안에 끝나야 한다")
                    .expect_err("FIFO 교체를 거부해야 한다");
                opener.join().expect("파일 열기 thread가 종료되어야 한다");
                panic!("FIFO로 교체된 파일을 여는 동안 대기했다");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                opener.join().expect("파일 열기 thread가 종료되어야 한다");
                panic!("파일 열기 결과 channel이 끊어졌다");
            }
        }
    }
}
