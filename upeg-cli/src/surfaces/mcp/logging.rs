//! MCP's `logging` capability: the server→client frames a running tool's
//! output travels on, the severity gate in front of them, and the one
//! method a client uses to move that gate.
//!
//! The capability is real only where there is somewhere to *put* an
//! unsolicited frame, so what a lane holds decides what it may promise:
//!
//!   * The stdio in-process lane owns a long-lived stdout it can write
//!     between responses, so it holds an [`McpLogWriter`] over a
//!     [`LineSink`] and advertises `logging`.
//!   * The HTTP `/mcp` lane can open a `text/event-stream` body per
//!     request and holds a long-lived one for server-initiated frames,
//!     so it advertises `logging` too. A *single* plain-JSON request on
//!     that lane opened no stream, and gets [`McpLogWriter::discarding`]
//!     — the client declared it did not want frames, which is a choice,
//!     not a broken promise.
//!   * The stdio proxy lane runs nothing itself (the host does), so it
//!     holds no writer at all, advertises no capability, and answers
//!     `logging/setLevel` with `Method not found` — the honest answer
//!     for a capability it never claimed.
//!
//! The severity floor is split out as [`LogLevelGate`] precisely because
//! those lanes disagree about its lifetime: stdio owns one floor for the
//! whole process-long session, while HTTP hands each request the floor
//! belonging to its `Mcp-Session-Id`. Sink and floor are separate values
//! so a lane can vary one without the other.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::{Value, json};

use super::{FIELD_METHOD, FIELD_PARAMS, JSON_RPC_VERSION, invalid_params};

/// Server→client notification: one log record. This surface uses it to
/// stream a running tool's output while `tools/call` is still in flight.
pub(super) const METHOD_NOTIFICATIONS_MESSAGE: &str = "notifications/message";
/// Client→server request: raise or lower this session's severity floor.
pub(super) const METHOD_LOGGING_SET_LEVEL: &str = "logging/setLevel";

/// `logger` name on every progress frame, so a client can route or mute
/// tool output separately from anything else upeg might ever log.
const LOG_LOGGER_TOOL_OUTPUT: &str = "upeg.tool";
/// `logger` name on frames the transport emits about itself — today the
/// one marker that accounts for notifications it had to drop.
const LOG_LOGGER_TRANSPORT: &str = "upeg.transport";

/// Fields of a log frame's `params` object.
const PARAMS_LEVEL: &str = "level";
const PARAMS_LOGGER: &str = "logger";
const PARAMS_DATA: &str = "data";
/// Fields of a progress frame's `params.data` object.
const LOG_DATA_TOOL: &str = "tool";
const LOG_DATA_STREAM: &str = "stream";
const LOG_DATA_SEQ: &str = "seq";
const LOG_DATA_TEXT: &str = "text";
/// Field of a drop marker's `params.data` object: how many frames were lost.
const LOG_DATA_DROPPED: &str = "dropped";

/// Separator between severity names in the `-32602` message.
const LEVEL_LIST_SEPARATOR: &str = ", ";

/// The MCP log severities, lowest to highest.
///
/// The set and its order are fixed by the spec (it adopts syslog's, RFC
/// 5424) — a client is entitled to send any of the eight and to have an
/// unknown ninth refused. Ordering is the whole point of the type:
/// `logging/setLevel` sets a *floor*, so the gate is one `>=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl LogLevel {
    /// Every severity, lowest first — the one list the validator, the
    /// error message and the rank round-trip all read.
    const ALL: &'static [Self] = &[
        Self::Debug,
        Self::Info,
        Self::Notice,
        Self::Warning,
        Self::Error,
        Self::Critical,
        Self::Alert,
        Self::Emergency,
    ];

    /// Severity a running tool's output is reported at. Progress is
    /// ordinary information, not a warning.
    pub(super) const PROGRESS: Self = Self::Info;

    /// Severity floor a session starts at, so a client that never calls
    /// `logging/setLevel` sees exactly what it saw before the method
    /// existed.
    pub(super) const DEFAULT: Self = Self::Info;

    /// Severity a lost-frames marker is reported at. Losing a client's
    /// log records is a fault the client should hear about even after it
    /// muted ordinary progress, so it sits above [`Self::PROGRESS`].
    const DROP_NOTICE: Self = Self::Warning;

    /// The name this severity carries on the wire.
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
            Self::Alert => "alert",
            Self::Emergency => "emergency",
        }
    }

    /// Parse a client-supplied severity name, or `None` when it is not
    /// one of the eight.
    fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|level| level.wire_name() == value)
    }

    /// Every accepted name, for the `-32602` message — the client is
    /// told the whole list rather than left guessing.
    fn wire_names() -> String {
        Self::ALL
            .iter()
            .map(|level| level.wire_name())
            .collect::<Vec<_>>()
            .join(LEVEL_LIST_SEPARATOR)
    }

    /// Position in [`Self::ALL`]: how the level rides in an
    /// [`AtomicU8`] without a lock on the reader threads' path.
    const fn rank(self) -> u8 {
        self as u8
    }

    /// Inverse of [`Self::rank`]. An out-of-range rank cannot happen —
    /// only `rank()` ever writes the cell — so it falls back to the
    /// default rather than panicking on a reader thread.
    fn from_rank(rank: u8) -> Self {
        Self::ALL
            .get(rank as usize)
            .copied()
            .unwrap_or(Self::DEFAULT)
    }
}

/// Where a lane puts a server-originated JSON-RPC frame.
///
/// One method, taking a whole frame rather than bytes, because the two
/// implementations frame it differently: [`LineSink`] writes one JSON
/// line to a `Write`, and the HTTP lane wraps the same value in an SSE
/// `message` event. Neither returns an error — a client that hung up
/// mid-run must not take the running tool down with it, so a sink that
/// cannot deliver drops the frame and says nothing.
pub(crate) trait McpNotificationSink: Send + Sync {
    fn emit(&self, frame: &Value);
}

/// One frame per line on a shared writer — the stdio lane's stdout.
///
/// Shared (`Arc<Mutex<…>>`) rather than borrowed because the frames are
/// written by the invoker's reader threads while the request thread is
/// still blocked inside the tool. In `serve` this and the response
/// writer both point at the process `Stdout`, whose `Write` impl takes
/// the lock per call — so a progress frame can never land inside a
/// response line.
struct LineSink<W> {
    writer: Arc<std::sync::Mutex<W>>,
}

impl<W: std::io::Write + Send> McpNotificationSink for LineSink<W> {
    fn emit(&self, frame: &Value) {
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let _ = writeln!(writer, "{frame}");
        let _ = writer.flush();
    }
}

/// The sink for a request that opened no stream to carry frames.
///
/// Not a missing capability — the lane holding this one can push, this
/// *request* just declined to take a body it could push on (a plain-JSON
/// `POST /mcp`). Making that explicit keeps `Option<&McpLogWriter>`
/// meaning exactly one thing: whether the lane can push at all.
struct DiscardSink;

impl McpNotificationSink for DiscardSink {
    fn emit(&self, _frame: &Value) {}
}

/// This session's severity floor, shared with every frame producer.
///
/// Its own value rather than a field of [`McpLogWriter`] because the two
/// have different lifetimes on the HTTP lane: one floor per
/// `Mcp-Session-Id` outlives the per-request sink that reads it.
///
/// Atomic rather than a lock: the invoker's reader threads test it on
/// every chunk and only `logging/setLevel` ever writes it.
#[derive(Clone)]
pub(crate) struct LogLevelGate(Arc<AtomicU8>);

impl Default for LogLevelGate {
    fn default() -> Self {
        Self(Arc::new(AtomicU8::new(LogLevel::DEFAULT.rank())))
    }
}

impl LogLevelGate {
    /// Move the floor. Every holder of this gate — including the reader
    /// threads of a dispatch already running — sees the new value.
    fn set(&self, level: LogLevel) {
        self.0.store(level.rank(), Ordering::Relaxed);
    }

    /// Whether a frame of this severity is above the floor.
    fn admits(&self, level: LogLevel) -> bool {
        level >= LogLevel::from_rank(self.0.load(Ordering::Relaxed))
    }
}

/// The stream server-originated log frames are written to while a
/// `tools/call` is still running, plus the severity floor in front of
/// them.
#[derive(Clone)]
pub(crate) struct McpLogWriter {
    sink: Arc<dyn McpNotificationSink>,
    level: LogLevelGate,
}

impl McpLogWriter {
    /// Wrap a writer the caller keeps its own handle to (`Stdout` in
    /// `serve`, a byte buffer in tests), with a floor of its own.
    pub(crate) fn new<W: std::io::Write + Send + 'static>(
        writer: Arc<std::sync::Mutex<W>>,
    ) -> Self {
        Self::with_level(Arc::new(LineSink { writer }), LogLevelGate::default())
    }

    /// Wrap a sink whose floor is owned elsewhere — the HTTP lane's
    /// per-session gate, which outlives this one request's sink.
    pub(crate) fn with_level(sink: Arc<dyn McpNotificationSink>, level: LogLevelGate) -> Self {
        Self { sink, level }
    }

    /// A writer for a request that can answer `logging/setLevel` for its
    /// session but has nowhere to put a frame this time. See
    /// [`DiscardSink`].
    pub(crate) fn discarding(level: LogLevelGate) -> Self {
        Self::with_level(Arc::new(DiscardSink), level)
    }

    /// A progress sink that turns each chunk into one
    /// `notifications/message` frame for `tool_id`, unless the session
    /// asked not to see [`LogLevel::PROGRESS`].
    pub(super) fn progress_sink(&self, tool_id: &str) -> upeg_runtime::SharedProgressSink {
        let sink = Arc::clone(&self.sink);
        let level = self.level.clone();
        let tool_id = tool_id.to_string();
        Arc::new(move |event: upeg_runtime::ProgressEvent| {
            if !level.admits(LogLevel::PROGRESS) {
                return;
            }
            sink.emit(&progress_frame(&tool_id, &event));
        })
    }
}

/// One `notifications/message` frame carrying a chunk of a running
/// tool's output.
fn progress_frame(tool_id: &str, event: &upeg_runtime::ProgressEvent) -> Value {
    log_frame(
        LogLevel::PROGRESS,
        LOG_LOGGER_TOOL_OUTPUT,
        json!({
            LOG_DATA_TOOL: tool_id,
            LOG_DATA_STREAM: event.stream.wire_name(),
            LOG_DATA_SEQ: event.seq,
            LOG_DATA_TEXT: event.chunk,
        }),
    )
}

/// The honest marker for frames a transport could not hold.
///
/// A `notifications/message` like the frames it accounts for, but under
/// [`LOG_LOGGER_TRANSPORT`]: the loss is the transport's, not the tool's,
/// and a client muting tool output still wants to hear that it lost
/// some. Counted in frames rather than bytes because a frame is one log
/// record — "you missed 12 records" is actionable where "you missed 4 KB
/// of JSON envelope" is not.
pub(crate) fn dropped_notifications_frame(frames: usize) -> Value {
    log_frame(
        LogLevel::DROP_NOTICE,
        LOG_LOGGER_TRANSPORT,
        json!({ LOG_DATA_DROPPED: frames }),
    )
}

/// The one place a `notifications/message` frame is shaped.
fn log_frame(level: LogLevel, logger: &str, data: Value) -> Value {
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        FIELD_METHOD: METHOD_NOTIFICATIONS_MESSAGE,
        FIELD_PARAMS: {
            PARAMS_LEVEL: level.wire_name(),
            PARAMS_LOGGER: logger,
            PARAMS_DATA: data,
        },
    })
}

/// `logging/setLevel`: the client picks the severity floor for the rest
/// of the session.
///
/// An unknown name is `-32602 Invalid params` rather than a silent
/// clamp — a client that asked for `"verbose"` wants to know its
/// vocabulary is wrong, not to quietly keep the old floor.
pub(super) fn set_level_result(params: Option<&Value>, log: &McpLogWriter) -> Result<Value, Value> {
    let requested = params
        .and_then(|params| params.get(PARAMS_LEVEL))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("missing or non-string `params.level`"))?;
    let level = LogLevel::from_wire(requested.trim()).ok_or_else(|| {
        invalid_params(&format!(
            "unknown log level `{}`; expected one of {}",
            crate::display_id(requested),
            LogLevel::wire_names()
        ))
    })?;
    log.level.set(level);
    // MCP defines no result payload; an empty object is the shape every
    // other "acknowledged" response on this surface uses.
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::LogLevel;

    #[test]
    fn 심각도는_낮은_것부터_높은_것으로_정렬된다() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Emergency);
    }

    #[test]
    fn 이름은_순위를_왕복한다() {
        for level in LogLevel::ALL.iter().copied() {
            assert_eq!(LogLevel::from_rank(level.rank()), level);
            assert_eq!(LogLevel::from_wire(level.wire_name()), Some(level));
        }
    }

    #[test]
    fn 목록에_없는_이름은_거절된다() {
        assert_eq!(LogLevel::from_wire("verbose"), None);
        assert_eq!(LogLevel::from_wire("INFO"), None);
    }

    #[test]
    fn 진행_출력은_info_심각도로_나간다() {
        assert_eq!(LogLevel::PROGRESS, LogLevel::Info);
        assert_eq!(LogLevel::DEFAULT, LogLevel::Info);
    }

    #[test]
    fn 허용값_안내는_여덟_이름을_모두_담는다() {
        let names = LogLevel::wire_names();
        for level in LogLevel::ALL.iter().copied() {
            assert!(names.contains(level.wire_name()), "{names}");
        }
    }
}
