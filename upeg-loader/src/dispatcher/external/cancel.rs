use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
use super::error::OutputStream;

pub(super) struct DrainControl {
    cancelled: AtomicBool,
    #[cfg(windows)]
    handles: Mutex<[Option<isize>; 2]>,
}

impl DrainControl {
    pub(super) const fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            #[cfg(windows)]
            handles: Mutex::new([None, None]),
        }
    }

    pub(super) fn cancel(&self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            let handles = self
                .handles
                .lock()
                .map_err(|_| io::Error::other("drain handle registry is poisoned"))?;
            for handle in handles.iter().flatten().copied() {
                cancel_windows_read(handle)?;
            }
        }
        Ok(())
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    #[cfg(windows)]
    pub(super) fn register<R>(
        &self,
        reader: R,
        stream: OutputStream,
    ) -> io::Result<ReaderRegistration<'_, R>>
    where
        R: std::os::windows::io::AsRawHandle,
    {
        let handle = reader.as_raw_handle() as isize;
        let mut handles = self
            .handles
            .lock()
            .map_err(|_| io::Error::other("drain handle registry is poisoned"))?;
        let index = stream_index(stream);
        handles[index] = Some(handle);
        if self.is_cancelled() {
            if let Err(source) = cancel_windows_read(handle) {
                handles[index] = None;
                return Err(source);
            }
        }
        drop(handles);
        Ok(ReaderRegistration {
            control: self,
            stream,
            reader,
        })
    }
}

#[cfg(windows)]
pub(super) struct ReaderRegistration<'a, R> {
    control: &'a DrainControl,
    stream: OutputStream,
    reader: R,
}

#[cfg(windows)]
impl<R: io::Read> io::Read for ReaderRegistration<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }
}

#[cfg(windows)]
impl<R> Drop for ReaderRegistration<'_, R> {
    fn drop(&mut self) {
        if let Ok(mut handles) = self.control.handles.lock() {
            handles[stream_index(self.stream)] = None;
        }
    }
}

#[cfg(windows)]
const fn stream_index(stream: OutputStream) -> usize {
    match stream {
        OutputStream::Stdout => 0,
        OutputStream::Stderr => 1,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn cancel_windows_read(handle: isize) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, HANDLE};
    use windows_sys::Win32::System::IO::CancelIoEx;

    // SAFETY: `ReaderRegistration` owns ChildStdout/ChildStderr and the
    // registry mutex prevents its HANDLE from being cleared while used here.
    let cancelled = unsafe { CancelIoEx(handle as HANDLE, std::ptr::null()) };
    if cancelled != 0 {
        return Ok(());
    }
    let source = io::Error::last_os_error();
    if source.raw_os_error() == i32::try_from(ERROR_NOT_FOUND).ok() {
        return Ok(());
    }
    Err(source)
}
