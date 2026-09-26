//! Streaming attach: an attached surface that watches its run.
//!
//! [`super::dispatch_tool`] posts to `/v1/tools/{id}` and waits for the
//! one envelope at the end. For a ten-minute `just verify` that means an
//! attached TUI shows a spinner over an empty pane and its `Esc` reaches
//! nobody — the live half of the call (`upeg_runtime::progress` and
//! `upeg_runtime::cancel`) is installed on the caller's thread and the
//! buffered route has no way to feed it.
//!
//! So an attached surface uses the host's NDJSON route instead
//! (`surfaces::http::stream`), which is the same shape a standalone
//! dispatch already produces:
//!
//!   * every `chunk` line becomes one [`upeg_runtime::ProgressEvent`] on
//!     the ambient sink, so the surface's live tail fills exactly as it
//!     would locally;
//!   * the ambient [`CancellationToken`] is polled between reads, and
//!     tripping it hangs up — the host cancels a streamed call when its
//!     body is dropped, so hanging up *is* the cancel wire;
//!   * the terminal `result` line carries the one canonical envelope,
//!     which is still the contract.
//!
//! A host that predates the route answers `404`. That is reported as
//! [`StreamedDispatch::RouteUnavailable`] rather than a failure, so the
//! caller can fall back to the buffered route and say so honestly
//! instead of pretending the run produced no output.

use std::io::ErrorKind;
use std::time::Duration;

use serde_json::Value;
use upeg_core::Surface;
use upeg_runtime::{
    CancellationToken, PROGRESS_STREAM_STDERR, PROGRESS_STREAM_STDOUT, ProgressReporter,
    ProgressStream, active_cancellation,
};

use super::discovery::DiscoveredHost;
use super::http_transport::{OpenStream, RequestBudget, open_stream_discovered};
use crate::domain::execution::dispatch::{Outcome, dispatch_failure};

/// NDJSON line fields, mirrored from `surfaces::http::stream`. Spelled
/// here rather than imported because the host module's constants are
/// private to the surface that writes them; the wire is the contract.
const EVENT_FIELD: &str = "event";
const EVENT_CHUNK: &str = "chunk";
const EVENT_RESULT: &str = "result";
const EVENT_DROPPED: &str = "dropped";
const CHUNK_STREAM_FIELD: &str = "stream";
const CHUNK_DATA_FIELD: &str = "data";
const RESULT_FIELD: &str = "result";
const DROPPED_BYTES_FIELD: &str = "bytes";

/// Path segment appended to a tool call to ask for the streaming twin.
const STREAM_PATH_SUFFIX: &str = "/stream";

/// NDJSON line terminator.
const LINE_TERMINATOR: u8 = b'\n';

/// How long one read waits before the loop looks at the cancellation
/// token again.
///
/// Not a timeout on the run: a silent ten-minute build is a tool doing
/// work, and there is no byte to wait for. It is only how often `Esc`
/// gets a chance to be honoured, so it is short enough to feel immediate
/// and long enough not to spin.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Read buffer size. Matches the host's own chunk cadence closely enough
/// that a chatty build costs one read per line or two.
const STREAM_READ_CHUNK_BYTES: usize = 8 * 1024;

/// How long one NDJSON line may be before the client gives up.
///
/// The body is streamed rather than buffered, so this bounds the *one*
/// line being assembled, not the run. A host that never sends a
/// terminator would otherwise grow this buffer without limit.
const MAX_STREAM_LINE_BYTES: usize = 8 * 1024 * 1024;

/// `error.code` of the envelope synthesized when the surface hung up on
/// its own run. Same spelling the `External` invoker answers a locally
/// cancelled run with (`upeg_loader`'s `CANCELLED_ERROR_CODE`), so a
/// cancelled run reads the same attached as standalone.
const CANCELLED_ERROR_CODE: &str = "cancelled";

/// `error.code` for a stream that ended without its terminal line.
const TRUNCATED_STREAM_ERROR_CODE: &str = "attach_error";

/// What a streaming dispatch attempt produced.
#[derive(Debug)]
pub(crate) enum StreamedDispatch {
    /// The host streamed the run. Chunks already reached the ambient
    /// progress sink; this is the one envelope at the end.
    Streamed(Outcome),
    /// The host does not know the streaming route — an older host. The
    /// caller falls back to the buffered route.
    RouteUnavailable,
}

/// The two ambient halves of a live call, captured once per dispatch.
///
/// Captured rather than looked up per line because both are thread-local
/// scopes: the reads happen on this thread, but naming the capture makes
/// the dependency visible and lets a test hand in an explicit pair.
pub(crate) struct LiveCall {
    reporter: Option<ProgressReporter>,
    cancel: Option<CancellationToken>,
}

impl LiveCall {
    /// Whatever the caller installed around this dispatch. Both halves
    /// are optional by the runtime's own contract — a surface that wants
    /// neither pays nothing.
    pub(crate) fn ambient() -> Self {
        Self {
            reporter: ProgressReporter::capture(),
            cancel: active_cancellation(),
        }
    }

    /// An explicit pair, for callers that hold their own scopes.
    #[cfg(test)]
    pub(crate) fn new(
        reporter: Option<ProgressReporter>,
        cancel: Option<CancellationToken>,
    ) -> Self {
        Self { reporter, cancel }
    }

    fn report(&self, stream: ProgressStream, chunk: String) {
        if let Some(reporter) = &self.reporter {
            reporter.report(stream, chunk);
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }
}

/// Streaming twin of [`super::dispatch_tool`].
pub(crate) fn dispatch_tool_streamed(
    server: &DiscoveredHost,
    tool_id: &str,
    args: &Value,
    origin: Surface,
    live: &LiveCall,
) -> std::io::Result<StreamedDispatch> {
    let url = format!(
        "{}/v1/tools/{}{STREAM_PATH_SUFFIX}",
        server.endpoint.trim_end_matches('/'),
        tool_id
    );
    dispatch_streamed_via_url(server, &url, args, origin, live)
}

/// Streaming twin of [`super::dispatch_tool_on_board`].
pub(crate) fn dispatch_tool_on_board_streamed(
    server: &DiscoveredHost,
    board: &str,
    tool_id: &str,
    args: &Value,
    origin: Surface,
    live: &LiveCall,
) -> std::io::Result<StreamedDispatch> {
    let url = format!(
        "{}/v1/boards/{}/tools/{}{STREAM_PATH_SUFFIX}",
        server.endpoint.trim_end_matches('/'),
        board,
        tool_id
    );
    dispatch_streamed_via_url(server, &url, args, origin, live)
}

fn dispatch_streamed_via_url(
    server: &DiscoveredHost,
    url: &str,
    args: &Value,
    origin: Surface,
    live: &LiveCall,
) -> std::io::Result<StreamedDispatch> {
    let body = if args.is_null() {
        String::new()
    } else {
        args.to_string()
    };
    let open = open_stream_discovered(
        "POST",
        url,
        server,
        &[(crate::surfaces::http::ORIGIN_SURFACE_HEADER, origin.label())],
        &body,
        RequestBudget::DISPATCH,
    )?;
    consume_stream(open, live)
}

/// Read the NDJSON body to its terminal line, feeding `live` as it goes.
fn consume_stream(open: OpenStream, live: &LiveCall) -> std::io::Result<StreamedDispatch> {
    match open.status {
        // The route itself is missing (an older host) or the tool is not
        // callable here. Both answer 404 and both are answered the same
        // way: retry on the buffered route, which knows the difference
        // and reports it as `Outcome::NotFound`.
        404 => return Ok(StreamedDispatch::RouteUnavailable),
        401 => {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "attach: host paused or token rejected",
            ));
        }
        status if !(200..300).contains(&status) => {
            return Ok(StreamedDispatch::Streamed(Outcome::Failure(
                dispatch_failure(
                    "host_error",
                    format!("host returned HTTP {status} for the streaming route"),
                ),
            )));
        }
        _ => {}
    }
    read_ndjson_lines(open, live).map(StreamedDispatch::Streamed)
}

fn read_ndjson_lines(mut open: OpenStream, live: &LiveCall) -> std::io::Result<Outcome> {
    let mut pending: Vec<u8> = std::mem::take(&mut open.prefix);
    let mut buffer = [0_u8; STREAM_READ_CHUNK_BYTES];
    loop {
        while let Some(end) = pending.iter().position(|byte| *byte == LINE_TERMINATOR) {
            let line: Vec<u8> = pending.drain(..=end).collect();
            if let Some(outcome) = apply_line(&line, live) {
                return Ok(outcome);
            }
        }
        if live.is_cancelled() {
            // Hanging up *is* the cancel wire: the host's body owns this
            // call's token and cancels when it is dropped.
            open.abort();
            return Ok(cancelled_outcome());
        }
        if pending.len() > MAX_STREAM_LINE_BYTES {
            open.abort();
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "attach: streamed line exceeds the configured limit",
            ));
        }
        let read = match open.read(&mut buffer, CANCEL_POLL_INTERVAL) {
            Ok(read) => read,
            // Nothing to read yet. A tool that has printed nothing for a
            // minute is still running.
            Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if read == 0 {
            // End of body with no `result` line: the contract says the
            // last line is always the result, so this is a dropped
            // connection, not a silent success.
            return Ok(Outcome::Failure(dispatch_failure(
                TRUNCATED_STREAM_ERROR_CODE,
                "attach: stream ended before its result line",
            )));
        }
        pending
            .try_reserve(read)
            .map_err(|error| std::io::Error::other(format!("attach stream allocation: {error}")))?;
        pending.extend_from_slice(&buffer[..read]);
    }
}

/// Apply one NDJSON line. `Some` only for the terminal `result` line.
fn apply_line(line: &[u8], live: &LiveCall) -> Option<Outcome> {
    let value: Value = serde_json::from_slice(line).ok()?;
    match value.get(EVENT_FIELD).and_then(Value::as_str) {
        Some(EVENT_CHUNK) => {
            let data = value.get(CHUNK_DATA_FIELD).and_then(Value::as_str)?;
            let stream = value
                .get(CHUNK_STREAM_FIELD)
                .and_then(Value::as_str)
                .and_then(progress_stream_from_wire)
                .unwrap_or(ProgressStream::Stdout);
            live.report(stream, data.to_string());
            None
        }
        Some(EVENT_DROPPED) => {
            // The host is telling us what it could not hold. Surfacing
            // it in the tail keeps the gap visible instead of leaving a
            // silently incomplete transcript.
            let bytes = value.get(DROPPED_BYTES_FIELD).and_then(Value::as_u64)?;
            live.report(
                ProgressStream::Stderr,
                format!("… host dropped {bytes} bytes of output\n"),
            );
            None
        }
        Some(EVENT_RESULT) => {
            let envelope = value.get(RESULT_FIELD)?;
            Some(
                super::outcome_from_canonical_envelope(envelope).unwrap_or_else(|error| {
                    Outcome::Failure(dispatch_failure(
                        TRUNCATED_STREAM_ERROR_CODE,
                        format!("attach: unreadable result line: {error}"),
                    ))
                }),
            )
        }
        _ => None,
    }
}

/// Wire name → typed stream. The inverse of
/// [`ProgressStream::wire_name`], spelled over the runtime's exported
/// constants so the two directions cannot drift.
fn progress_stream_from_wire(name: &str) -> Option<ProgressStream> {
    match name {
        PROGRESS_STREAM_STDOUT => Some(ProgressStream::Stdout),
        PROGRESS_STREAM_STDERR => Some(ProgressStream::Stderr),
        _ => None,
    }
}

/// The envelope a surface that hung up on its own run renders.
///
/// Synthesized rather than read off the wire: the body is gone by the
/// time the host answers, which is the whole point of hanging up.
fn cancelled_outcome() -> Outcome {
    Outcome::Failure(dispatch_failure(
        CANCELLED_ERROR_CODE,
        "cancelled by the attached surface",
    ))
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
