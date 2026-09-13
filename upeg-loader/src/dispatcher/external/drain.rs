use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::cancel::DrainControl;
use super::error::{CaptureLimit, ExternalProcessError, OutputStream};
use super::progress::ProgressForwarder;

const READ_BUFFER_BYTES: usize = 8 * 1024;
const NONBLOCKING_READ_RETRY_INTERVAL: Duration = Duration::from_millis(10);

pub(super) struct CaptureBudget {
    remaining: AtomicU64,
}

impl CaptureBudget {
    pub(super) const fn new(limit: CaptureLimit) -> Self {
        Self {
            remaining: AtomicU64::new(limit.bytes()),
        }
    }

    fn reserve(&self, bytes: u64) -> bool {
        self.remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(bytes)
            })
            .is_ok()
    }
}

pub(super) enum DrainEvent {
    Complete(OutputStream),
    Failed(OutputStream, ExternalProcessError),
}

#[cfg(unix)]
pub(super) trait DrainReader: Read + std::os::fd::AsFd + Send {}
#[cfg(unix)]
impl<T: Read + std::os::fd::AsFd + Send> DrainReader for T {}

#[cfg(windows)]
pub(super) trait DrainReader: Read + std::os::windows::io::AsRawHandle + Send {}
#[cfg(windows)]
impl<T: Read + std::os::windows::io::AsRawHandle + Send> DrainReader for T {}

#[cfg(not(any(unix, windows)))]
pub(super) trait DrainReader: Read + Send {}
#[cfg(not(any(unix, windows)))]
impl<T: Read + Send> DrainReader for T {}

pub(super) fn spawn<R: DrainReader + 'static>(
    reader: R,
    stream: OutputStream,
    limit: CaptureLimit,
    budget: Arc<CaptureBudget>,
    control: Arc<DrainControl>,
    events: mpsc::Sender<DrainEvent>,
    forwarder: ProgressForwarder,
) -> JoinHandle<Option<Vec<u8>>> {
    thread::spawn(
        move || match read_bounded(reader, stream, limit, &budget, &control, forwarder) {
            Ok(bytes) => {
                let _ = events.send(DrainEvent::Complete(stream));
                Some(bytes)
            }
            Err(error) => {
                let _ = events.send(DrainEvent::Failed(stream, error));
                None
            }
        },
    )
}

#[cfg(not(windows))]
fn read_bounded(
    mut reader: impl DrainReader,
    stream: OutputStream,
    limit: CaptureLimit,
    budget: &CaptureBudget,
    control: &DrainControl,
    mut forwarder: ProgressForwarder,
) -> Result<Vec<u8>, ExternalProcessError> {
    configure_reader(&reader).map_err(|source| ExternalProcessError::Read { stream, source })?;
    let result = read_loop(&mut reader, stream, limit, budget, control, &mut forwarder);
    // Flush the unterminated tail on every exit path — a command whose
    // last line lacks a newline, and a run that ended in cancellation or
    // an error, both still owe the consumer what they already wrote.
    forwarder.finish();
    result
}

#[cfg(windows)]
fn read_bounded(
    reader: impl DrainReader,
    stream: OutputStream,
    limit: CaptureLimit,
    budget: &CaptureBudget,
    control: &DrainControl,
    mut forwarder: ProgressForwarder,
) -> Result<Vec<u8>, ExternalProcessError> {
    configure_reader(&reader).map_err(|source| ExternalProcessError::Read { stream, source })?;
    let mut reader = control
        .register(reader, stream)
        .map_err(|source| ExternalProcessError::DrainCancel { source })?;
    let result = read_loop(&mut reader, stream, limit, budget, control, &mut forwarder);
    forwarder.finish();
    result
}

fn read_loop(
    reader: &mut impl Read,
    stream: OutputStream,
    limit: CaptureLimit,
    budget: &CaptureBudget,
    control: &DrainControl,
    forwarder: &mut ProgressForwarder,
) -> Result<Vec<u8>, ExternalProcessError> {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                if control.is_cancelled() {
                    return Ok(captured);
                }
                thread::sleep(NONBLOCKING_READ_RETRY_INTERVAL);
                continue;
            }
            Err(source) if cancelled_read(control, &source) => return Ok(captured),
            Err(source) => return Err(ExternalProcessError::Read { stream, source }),
        };
        if read == 0 {
            return Ok(captured);
        }
        let read = u64::try_from(read).map_err(|_| stream_limit_error(stream, limit))?;
        let captured_bytes = u64::try_from(captured.len())
            .ok()
            .and_then(|current| current.checked_add(read))
            .ok_or_else(|| stream_limit_error(stream, limit))?;
        if captured_bytes > limit.bytes() {
            return Err(stream_limit_error(stream, limit));
        }
        if !budget.reserve(read) {
            return Err(ExternalProcessError::AggregateLimitExceeded { max: limit.bytes() });
        }
        let read = usize::try_from(read).map_err(|_| stream_limit_error(stream, limit))?;
        captured
            .try_reserve(read)
            .map_err(|error| ExternalProcessError::Allocation {
                stream,
                detail: error.to_string(),
            })?;
        captured.extend_from_slice(&buffer[..read]);
        forwarder.push(&buffer[..read]);
    }
}

#[cfg(unix)]
fn configure_reader(reader: &impl DrainReader) -> std::io::Result<()> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    let flags = fcntl_getfl(reader)?;
    fcntl_setfl(reader, flags | OFlags::NONBLOCK)?;
    Ok(())
}

#[cfg(not(unix))]
fn configure_reader(_reader: &impl DrainReader) -> std::io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn cancelled_read(control: &DrainControl, source: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::ERROR_OPERATION_ABORTED;

    control.is_cancelled() && source.raw_os_error() == i32::try_from(ERROR_OPERATION_ABORTED).ok()
}

#[cfg(not(windows))]
fn cancelled_read(_control: &DrainControl, _source: &std::io::Error) -> bool {
    false
}

fn stream_limit_error(stream: OutputStream, limit: CaptureLimit) -> ExternalProcessError {
    ExternalProcessError::StreamLimitExceeded {
        stream,
        max: limit.bytes(),
    }
}

pub(super) fn join(
    handle: JoinHandle<Option<Vec<u8>>>,
    stream: OutputStream,
) -> Result<Option<Vec<u8>>, ExternalProcessError> {
    handle
        .join()
        .map_err(|_| ExternalProcessError::DrainThreadStopped { stream })
}
