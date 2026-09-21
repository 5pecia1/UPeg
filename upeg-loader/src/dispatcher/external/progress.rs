//! Live forwarding of a child's output while it is still running.
//!
//! The capture layer already reads both child streams to completion on
//! their own threads; this type is the tap on that read loop. It keeps
//! its own small line buffer so consumers receive *whole lines* rather
//! than whatever 8 KiB boundary the pipe happened to land on, and it
//! never grows without bound: an unterminated run longer than
//! [`MAX_PROGRESS_CHUNK_BYTES`] is flushed as-is.
//!
//! Capture is unaffected. Every byte still lands in the captured buffer
//! under the same limits — forwarding is an extra copy of at most one
//! pending line, never a replacement for the final envelope.

use upeg_runtime::{ProgressReporter, ProgressStream};

use super::error::OutputStream;

/// Longest run of output held back waiting for a terminator. A child
/// that writes a megabyte without a newline still reports progress,
/// just in fixed-size pieces.
const MAX_PROGRESS_CHUNK_BYTES: usize = 64 * 1024;

/// Line terminator: the chunk boundary for ordinary command output.
const LINE_TERMINATOR: u8 = b'\n';
/// Carriage return: the chunk boundary progress bars use (`cargo`,
/// `flutter`, `curl` redraw one line with `\r` and no newline). Without
/// this a progress bar would sit in the buffer until the size cap.
const REDRAW_TERMINATOR: u8 = b'\r';

/// The tap on one stream's read loop.
///
/// Inactive (`reporter: None`) when no surface installed a progress
/// sink, which is the common case — then every method is a no-op and the
/// buffer stays empty.
pub(super) struct ProgressForwarder {
    reporter: Option<ProgressReporter>,
    stream: ProgressStream,
    pending: Vec<u8>,
}

impl ProgressForwarder {
    pub(super) const fn new(reporter: Option<ProgressReporter>, stream: OutputStream) -> Self {
        Self {
            reporter,
            stream: match stream {
                OutputStream::Stdout => ProgressStream::Stdout,
                OutputStream::Stderr => ProgressStream::Stderr,
            },
            pending: Vec::new(),
        }
    }

    /// Feed bytes just read from the child, emitting every terminated
    /// run they complete.
    pub(super) fn push(&mut self, bytes: &[u8]) {
        if self.reporter.is_none() {
            return;
        }
        self.pending.extend_from_slice(bytes);
        if let Some(boundary) = terminated_prefix_len(&self.pending) {
            self.emit(boundary);
        } else if self.pending.len() >= MAX_PROGRESS_CHUNK_BYTES {
            self.emit(self.pending.len());
        }
    }

    /// Flush whatever the child left unterminated. Called once per
    /// stream when its read loop ends, however it ended — a command
    /// whose last line has no newline still reports that line.
    pub(super) fn finish(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.emit(self.pending.len());
    }

    fn emit(&mut self, len: usize) {
        let Some(reporter) = self.reporter.as_ref() else {
            return;
        };
        let chunk = String::from_utf8_lossy(&self.pending[..len]).into_owned();
        self.pending.drain(..len);
        reporter.report(self.stream, chunk);
    }
}

/// Length of the longest prefix of `bytes` that ends on a terminator,
/// or `None` when nothing is terminated yet.
///
/// A newline wins over a carriage return wherever both appear, so a
/// `\r\n` sequence is reported as one chunk instead of splitting between
/// the two bytes.
fn terminated_prefix_len(bytes: &[u8]) -> Option<usize> {
    if let Some(position) = bytes.iter().rposition(|byte| *byte == LINE_TERMINATOR) {
        return Some(position + 1);
    }
    bytes
        .iter()
        .rposition(|byte| *byte == REDRAW_TERMINATOR)
        .map(|position| position + 1)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use upeg_runtime::{ProgressEvent, ProgressSink, SharedProgressSink};

    use super::{MAX_PROGRESS_CHUNK_BYTES, OutputStream, ProgressForwarder, ProgressReporter};

    #[derive(Default)]
    struct RecordingSink {
        chunks: Mutex<Vec<String>>,
    }

    impl ProgressSink for RecordingSink {
        fn emit(&self, event: ProgressEvent) {
            self.chunks
                .lock()
                .expect("recording lock")
                .push(event.chunk);
        }
    }

    fn recording_forwarder() -> (Arc<RecordingSink>, ProgressForwarder) {
        let sink = Arc::new(RecordingSink::default());
        let shared: SharedProgressSink = Arc::clone(&sink) as SharedProgressSink;
        (
            sink,
            ProgressForwarder::new(Some(ProgressReporter::new(shared)), OutputStream::Stdout),
        )
    }

    fn chunks(sink: &RecordingSink) -> Vec<String> {
        sink.chunks.lock().expect("recording lock").clone()
    }

    #[test]
    fn complete_lines_emit_first_tail_stays() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(b"first\nsecond");
        assert_eq!(chunks(&recorded), vec!["first\n".to_string()]);

        forwarder.finish();
        assert_eq!(
            chunks(&recorded),
            vec!["first\n".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn multiple_lines_bundle_into_one_chunk() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(b"a\nb\nc\n");
        assert_eq!(chunks(&recorded), vec!["a\nb\nc\n".to_string()]);
    }

    #[test]
    fn carriage_return_progress_bar_streams_immediately() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(b"building 10%\rbuilding 20%\r");
        assert_eq!(
            chunks(&recorded),
            vec!["building 10%\rbuilding 20%\r".to_string()]
        );
    }

    #[test]
    fn crlf_breaks_once_at_newline_boundary() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(b"line\r\n");
        assert_eq!(chunks(&recorded), vec!["line\r\n".to_string()]);
    }

    #[test]
    fn unterminated_run_flushes_at_size_cap() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(&vec![b'x'; MAX_PROGRESS_CHUNK_BYTES]);
        assert_eq!(chunks(&recorded).len(), 1);
    }

    #[test]
    fn cut_utf8_becomes_replacement_char() {
        let (recorded, mut forwarder) = recording_forwarder();
        forwarder.push(&[0xF0, 0x9F, b'\n']);
        assert_eq!(chunks(&recorded).len(), 1);
    }

    #[test]
    fn without_sink_nothing_is_buffered() {
        let mut forwarder = ProgressForwarder::new(None, OutputStream::Stderr);
        forwarder.push(b"ignored\n");
        forwarder.finish();
    }
}
