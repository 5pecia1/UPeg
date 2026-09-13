//! Streaming tool calls: `POST /v1/tools/{id}/stream` and its
//! board-scoped sibling.
//!
//! The non-streaming routes answer with one canonical envelope once the
//! tool is done. For a ten-minute `just verify` wrapped as a Tool that
//! means ten minutes of nothing. These routes answer immediately with an
//! `application/x-ndjson` body — one JSON object per line — and fill it
//! in as the child writes:
//!
//! ```text
//! {"event":"chunk","stream":"stderr","seq":0,"data":"   Compiling …\n"}
//! {"event":"chunk","stream":"stdout","seq":1,"data":"ok\n"}
//! {"event":"result","result":{"ok":true,"primary_output_id":…}}
//! ```
//!
//! Three deliberate consequences of streaming:
//!
//!   * **The status code is decided before the outcome is.** Headers go
//!     out with the first byte, so a tool failure cannot become a `422`
//!     the way it does on `/v1/tools/{id}`. The final `result` line
//!     carries the same canonical failure envelope instead. Routing
//!     errors that *are* knowable up front — unknown tool, unpinned
//!     board tool — still answer with the ordinary JSON `404`.
//!   * **The last line is always a `result` line.** A consumer reads
//!     until it sees `"event":"result"`; a body that ends without one
//!     means the connection dropped, not that the tool succeeded.
//!   * **A slow consumer loses chunks, not the host's memory.** The
//!     queue between the dispatch and the socket is bounded by
//!     [`STREAM_QUEUE_BUDGET_BYTES`]; what does not fit is dropped and
//!     accounted for in a `dropped` line
//!     (`{"event":"dropped","bytes":N}`) once there is room again. See
//!     [`QueueBudget`].
//!
//! Auth, the Origin guard, the pause gate, and the board pin gate are
//! the same middleware and the same lookups the non-streaming routes
//! use — this module adds a body encoding, not a second policy.
//!
//! **A disconnected client stops the tool.** Hanging up drops the
//! response body, and the body owns a [`CancelOnDrop`] holding this
//! call's [`upeg_runtime::CancellationToken`] — the same token the
//! dispatch was installed under. Dropping it cancels, and an invoker
//! that polls the ambient token (today the `External` invoker, on every
//! tick of its wait loop) terminates its child's process group and
//! answers with a `cancelled` envelope nobody is left to read.
//!
//! The drop is the signal deliberately, rather than "the chunk queue
//! refused a line": a silent ten-minute build produces no chunk to
//! refuse, and it is exactly the run worth stopping. Cancellation after
//! the dispatch has already finished — the ordinary end of a stream,
//! where the body is dropped once its last line is read — is a no-op:
//! the token is out of scope by then and nothing polls it.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use upeg_core::{BoardKey, Surface};
use upeg_runtime::{
    CancellationToken, ExecutionContext, ProgressEvent, ProgressSink, SharedProgressSink,
    toolbox_tool, with_cancellation, with_progress_sink,
};
use upeg_sources::pegboard;

use super::cancel_on_drop::CancelOnDrop;
use super::{
    HttpState, not_found_response, origin_surface_from_headers, parse_tool_call_body,
    principal_from_headers,
};
use crate::app;
use crate::domain::execution::dispatch::Outcome;
use crate::infrastructure::notify;

/// Media type of the streaming response body: newline-delimited JSON.
const NDJSON_CONTENT_TYPE: &str = "application/x-ndjson";

/// Discriminator field every line carries.
const EVENT_FIELD: &str = "event";
/// `event` value for one incremental output chunk.
const EVENT_CHUNK: &str = "chunk";
/// `event` value for the single terminal line.
const EVENT_RESULT: &str = "result";
/// `event` value for the marker that accounts for chunks the host had
/// to drop because the consumer was not reading fast enough.
const EVENT_DROPPED: &str = "dropped";
/// Chunk line fields.
const CHUNK_STREAM_FIELD: &str = "stream";
const CHUNK_SEQ_FIELD: &str = "seq";
const CHUNK_DATA_FIELD: &str = "data";
/// Result line field holding the canonical envelope.
const RESULT_FIELD: &str = "result";
/// `dropped` line field: how many bytes of tool output were lost.
const DROPPED_BYTES_FIELD: &str = "bytes";

/// How much not-yet-written NDJSON the host will hold for one streaming
/// call.
///
/// A byte budget, not a line count: one `cargo` progress redraw and one
/// base64 blob cost wildly different amounts of memory, and memory is
/// what is being protected. A client that opens `/stream` on a chatty
/// ten-minute build and then stops reading used to make the host buffer
/// the entire run; now it costs a bounded amount and the client is told,
/// in the body, exactly what it missed.
const STREAM_QUEUE_BUDGET_BYTES: usize = 1024 * 1024;

/// `POST /v1/tools/{id}/stream`
///
/// The surface is the request's own ([`origin_surface_from_headers`]) —
/// resolved once and used for all three answers this handler owes: the
/// visibility gate, the `_upeg.surface` stamp, and the principal. An
/// attached `upeg call … --stream` is answered as `cli` here exactly as
/// it is on the buffered route, so `_upeg.surface` and
/// `_upeg.principal.surface` can never name two different callers.
pub(super) async fn tools_call_stream(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let surface = origin_surface_from_headers(&state, &headers);
    if !is_callable_on_surface(&id, surface) {
        return unknown_tool_response(&id, surface);
    }
    let principal = principal_from_headers(&state, &headers);
    stream_dispatch(
        state.notifications_enabled,
        id,
        body,
        ExecutionContext::global(surface).with_principal(principal),
    )
}

/// `POST /v1/boards/{board}/tools/{id}/stream`
///
/// Same pin gate as `/v1/boards/{board}/tools/{id}`: the tool must be
/// pinned on that board *and* visible on this surface, and the pin's
/// saved args preset merges as defaults through the board execution
/// context.
pub(super) async fn board_tools_call_stream(
    State(state): State<HttpState>,
    Path((board, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let surface = origin_surface_from_headers(&state, &headers);
    let Some(context) = board_call_context(&board, &id, surface) else {
        return not_found_response("board tool", &format!("{board}/{id}")).into_response();
    };
    let context = context.with_principal(principal_from_headers(&state, &headers));
    stream_dispatch(state.notifications_enabled, id, body, context)
}

/// Board-scoped execution context, or `None` when the pin gate refuses.
fn board_call_context(board: &str, id: &str, surface: Surface) -> Option<ExecutionContext> {
    let state = pegboard::load_state();
    let board_key = BoardKey::parse(board).ok()?;
    let placement =
        pegboard::board_placement_on_surface_in(&state, board_key.as_str(), id, surface)?;
    Some(ExecutionContext::board(
        surface,
        board_key,
        placement
            .args_preset
            .clone()
            .unwrap_or_else(upeg_runtime::empty_args_preset),
    ))
}

/// Whether this id is a tool the calling surface may call at all.
///
/// Checked before the stream opens so an unknown id still gets an
/// ordinary `404` with a JSON body instead of a `200` NDJSON stream
/// whose only line is a failure. Registered-but-invisible collapses into
/// the same answer as unknown, exactly as `dispatch_tool_on_surface`
/// does — the surface gate must not be an enumeration oracle.
fn is_callable_on_surface(id: &str, surface: Surface) -> bool {
    toolbox_tool(id).is_some_and(|meta| meta.is_on_surface(surface))
}

fn unknown_tool_response(id: &str, surface: Surface) -> Response {
    let hint = crate::unknown_tool_hint(id, Some(surface));
    (
        StatusCode::NOT_FOUND,
        Json(
            crate::domain::execution::dispatch::dispatch_failure(
                "unknown_tool",
                format!("unknown tool `{}`{hint}", crate::display_id(id)),
            )
            .to_canonical_json(),
        ),
    )
        .into_response()
}

/// Open the NDJSON stream and run the dispatch behind it.
///
/// The dispatch is synchronous and can block for minutes, so it runs on
/// a blocking worker; the progress sink is installed *on that worker*
/// because the ambient sink is per-thread. Every line — chunks, drop
/// markers and the final result — travels through one queue, which is
/// what guarantees the result line lands last.
fn stream_dispatch(
    notifications_enabled: bool,
    id: String,
    body: Bytes,
    context: ExecutionContext,
) -> Response {
    let args = match parse_tool_call_body(&body) {
        Ok(args) => args,
        Err(response) => return response.into_response(),
    };

    let (lines, receiver, budget) = line_queue();
    let forwarder = Arc::new(ChunkForwarder::new(lines.clone()));
    let cancellation = CancellationToken::new();
    let dispatch_cancellation = cancellation.clone();
    tokio::task::spawn_blocking(move || {
        let sink: SharedProgressSink = forwarder;
        let outcome = with_cancellation(dispatch_cancellation, || {
            with_progress_sink(sink, || app::dispatch_tool_call(&id, args, &context, None))
        });
        lines.finish(result_line(&id, notifications_enabled, outcome));
    });

    let mut response = Response::new(Body::from_stream(NdjsonLines {
        receiver,
        budget,
        cancellation: CancelOnDrop::new(cancellation),
    }));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(NDJSON_CONTENT_TYPE),
    );
    response
}

/// Whether a line may be turned away when the queue is full.
///
/// Progress is best-effort by construction — the final envelope is the
/// contract — so a chunk that does not fit is dropped and accounted
/// for. The terminal `result` line and the last `dropped` marker in
/// front of it *are* the contract, so they are queued whatever the
/// budget says: both are bounded in size and arrive exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    WithinBudget,
    Guaranteed,
}

/// What happened to a line offered to the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Forwarding {
    /// Queued, or honestly accounted for as dropped.
    Continuing,
    /// The consumer is gone. Nothing more will be forwarded.
    Stopped,
}

/// Byte accounting shared by the producing side and the body.
///
/// `queued` rises when a line is handed to the channel and falls when
/// the body yields it, so it measures exactly what the host is holding
/// on behalf of a consumer that has not read it yet. `dropped`
/// accumulates the tool output turned away while `queued` was at the
/// budget, and is emptied into one `dropped` marker as soon as a line
/// fits again — coalescing, so a consumer stalled for a minute gets one
/// honest number rather than a marker per lost chunk.
#[derive(Default)]
struct QueueBudget {
    queued: AtomicUsize,
    dropped: AtomicUsize,
}

impl QueueBudget {
    /// Take `bytes` of the budget, or refuse when that would exceed it.
    fn reserve(&self, bytes: usize, admission: Admission) -> bool {
        if admission == Admission::Guaranteed {
            self.queued.fetch_add(bytes, Ordering::Relaxed);
            return true;
        }
        self.queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |queued| {
                let next = queued.saturating_add(bytes);
                (next <= STREAM_QUEUE_BUDGET_BYTES).then_some(next)
            })
            .is_ok()
    }

    /// How much the host is currently holding for a consumer that has
    /// not read it yet. A test observer: production code only ever
    /// reserves and releases, never reads the total.
    #[cfg(test)]
    fn queued_bytes(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }

    /// Give `bytes` back — the body has written that line out.
    fn release(&self, bytes: usize) {
        let _ = self
            .queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |queued| {
                Some(queued.saturating_sub(bytes))
            });
    }
}

/// Sender half of the body's line queue, with the budget in front of it.
#[derive(Clone)]
struct LineSender {
    sender: UnboundedSender<String>,
    budget: Arc<QueueBudget>,
}

/// The queue one streaming call's body is fed through.
fn line_queue() -> (LineSender, UnboundedReceiver<String>, Arc<QueueBudget>) {
    let (sender, receiver) = unbounded_channel::<String>();
    let budget = Arc::new(QueueBudget::default());
    (
        LineSender {
            sender,
            budget: Arc::clone(&budget),
        },
        receiver,
        budget,
    )
}

impl LineSender {
    /// Offer one `chunk` line carrying `payload_bytes` of tool output.
    ///
    /// `payload_bytes` — not the serialized line's length — is what a
    /// `dropped` marker reports, because the consumer's loss is measured
    /// in the tool's own output, not in JSON escaping.
    fn offer_chunk(&self, line: String, payload_bytes: usize) -> Forwarding {
        if self.sender.is_closed() {
            return Forwarding::Stopped;
        }
        let bytes = line.len();
        if !self.budget.reserve(bytes, Admission::WithinBudget) {
            self.budget
                .dropped
                .fetch_add(payload_bytes, Ordering::Relaxed);
            return Forwarding::Continuing;
        }
        // Deliberately *after* the reservation succeeded: a chunk
        // fitting again is the signal that the consumer caught up, and
        // it is the only moment a marker is owed. Flushing before the
        // reservation would emit one marker per refused chunk — the
        // budget always has room for a 30-byte marker even when it has
        // none for a 64 KiB line — which is the opposite of coalescing.
        self.flush_dropped();
        self.send(line, bytes)
    }

    /// Queue the terminal line, preceded by whatever drop marker is
    /// still owed. Both bypass the budget: a consumer that reads to the
    /// end is entitled to the envelope and to the full drop count.
    fn finish(&self, line: String) {
        self.flush_dropped();
        let bytes = line.len();
        if self.budget.reserve(bytes, Admission::Guaranteed) {
            let _ = self.send(line, bytes);
        }
    }

    /// Emit the accumulated drop count as one marker line, if any is
    /// owed.
    ///
    /// [`Admission::Guaranteed`]: a marker is tens of bytes, it is only
    /// ever written at a moment the caller established there is room,
    /// and a loss the consumer is never told about is worse than a
    /// budget overshot by one short line.
    fn flush_dropped(&self) {
        // Claimed with one `swap`, not read-then-subtract: two threads
        // that both read the same total would each report it, so a
        // consumer would be told it lost twice what it lost. The claim is
        // handed back on the one path that fails to deliver it, so the
        // count is never silently dropped either.
        let dropped = self.budget.dropped.swap(0, Ordering::Relaxed);
        if dropped == 0 {
            return;
        }
        let marker = dropped_line(dropped);
        let bytes = marker.len();
        if !self.budget.reserve(bytes, Admission::Guaranteed) {
            self.budget.dropped.fetch_add(dropped, Ordering::Relaxed);
            return;
        }
        if self.send(marker, bytes) == Forwarding::Stopped {
            self.budget.dropped.fetch_add(dropped, Ordering::Relaxed);
        }
    }

    /// Hand a line the caller has already reserved `reserved_bytes` for
    /// to the channel, giving the reservation back if the consumer turns
    /// out to be gone (by then the `String` is no longer ours to
    /// measure).
    fn send(&self, line: String, reserved_bytes: usize) -> Forwarding {
        if self.sender.send(line).is_err() {
            self.budget.release(reserved_bytes);
            return Forwarding::Stopped;
        }
        Forwarding::Continuing
    }
}

/// The progress sink behind a streaming response.
///
/// Holds a stop latch rather than re-checking the channel every time:
/// once the consumer is gone there is nothing to serialize, and the
/// remaining minutes of a build should not pay for JSON encoding whose
/// output has no reader.
struct ChunkForwarder {
    lines: LineSender,
    stopped: AtomicBool,
}

impl ChunkForwarder {
    const fn new(lines: LineSender) -> Self {
        Self {
            lines,
            stopped: AtomicBool::new(false),
        }
    }

    /// Whether forwarding has been abandoned for this call.
    fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }
}

impl ProgressSink for ChunkForwarder {
    fn emit(&self, event: ProgressEvent) {
        if self.is_stopped() {
            return;
        }
        let payload_bytes = event.chunk.len();
        let line = line(json!({
            EVENT_FIELD: EVENT_CHUNK,
            CHUNK_STREAM_FIELD: event.stream.wire_name(),
            CHUNK_SEQ_FIELD: event.seq,
            CHUNK_DATA_FIELD: event.chunk,
        }));
        if self.lines.offer_chunk(line, payload_bytes) == Forwarding::Stopped {
            self.stopped.store(true, Ordering::Relaxed);
        }
    }
}

/// The honest marker for output the host could not hold.
fn dropped_line(bytes: usize) -> String {
    line(json!({ EVENT_FIELD: EVENT_DROPPED, DROPPED_BYTES_FIELD: bytes }))
}

/// The terminal line, carrying whichever canonical envelope the dispatch
/// produced. Also the point where desktop notifications fire, so a
/// streamed call is as visible to the host's owner as a plain one.
fn result_line(id: &str, notifications_enabled: bool, outcome: Outcome) -> String {
    let envelope = match outcome {
        Outcome::Success(success) => {
            let text = crate::domain::execution::dispatch::success_primary_text(&success);
            notify::fire_if(notifications_enabled, id, true, &text);
            success.to_canonical_json()
        }
        Outcome::Failure(failure) => {
            notify::fire_if(notifications_enabled, id, false, &failure.error.message);
            failure.to_canonical_json()
        }
        // Reachable only if the tool was unregistered between the
        // pre-flight check and the dispatch — the stream is already
        // open, so it reports as a result line rather than a 404.
        Outcome::NotFound => crate::domain::execution::dispatch::dispatch_failure(
            "unknown_tool",
            format!("unknown tool `{}`", crate::display_id(id)),
        )
        .to_canonical_json(),
    };
    line(json!({ EVENT_FIELD: EVENT_RESULT, RESULT_FIELD: envelope }))
}

/// One NDJSON line: compact JSON plus its terminator.
fn line(value: Value) -> String {
    let mut text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

/// The response body: the queue's lines, in order, until the worker
/// drops its sender. Yielding a line is also what gives its bytes back
/// to [`QueueBudget`], so the budget tracks what the host still holds
/// rather than what it has ever produced.
struct NdjsonLines {
    receiver: UnboundedReceiver<String>,
    budget: Arc<QueueBudget>,
    /// Cancels the dispatch when this body goes away — the only signal
    /// a hung-up client actually produces.
    #[expect(
        dead_code,
        reason = "held for its Drop: dropping the body is what cancels the dispatch"
    )]
    cancellation: CancelOnDrop,
}

impl futures_core::Stream for NdjsonLines {
    type Item = Result<String, std::convert::Infallible>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.receiver.poll_recv(context).map(|line| {
            line.map(|line| {
                this.budget.release(line.len());
                Ok(line)
            })
        })
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
