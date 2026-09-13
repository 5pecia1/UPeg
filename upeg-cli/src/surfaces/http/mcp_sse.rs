//! MCP over HTTP, the server→client direction: the `text/event-stream`
//! half of the Streamable HTTP transport (MCP 2025-03-26).
//!
//! `POST /mcp` used to be the whole lane — one request in, one JSON
//! response out — which made two things impossible. A `tools/call` on a
//! ten-minute build said nothing until it finished, while the stdio lane
//! streamed the same output as `notifications/message` frames. And the
//! host had no way to say `notifications/tools/list_changed` when its
//! deferred MCP-import load finished, so an attached client could only
//! poll `/healthz` and re-read `tools/list` on a hunch.
//!
//! Both are the same missing thing — a direction — and this module adds
//! it in the shape the transport already defines:
//!
//!   * **`POST /mcp` with `Accept: text/event-stream`** answers with an
//!     SSE body instead of a JSON one. The running tool's output arrives
//!     as `notifications/message` events (byte-for-byte the frames the
//!     stdio lane writes), and the JSON-RPC response is the **last**
//!     event on the stream, which then closes. A consumer reads until it
//!     sees a frame carrying its request `id`.
//!   * **`GET /mcp` with `Accept: text/event-stream`** is the long-lived
//!     stream for frames that belong to no request. Today exactly one:
//!     `notifications/tools/list_changed`, pushed the moment the import
//!     load finishes ([`crate::infrastructure::mcp_imports::subscribe_phase_changes`]).
//!     Keep-alive comments every [`SSE_KEEPALIVE_INTERVAL`] hold the
//!     connection open through idle proxies.
//!
//! Four deliberate limits:
//!
//!   * **Accept decides, and `*/*` does not count.** Only an explicit
//!     `text/event-stream` opts into a stream, so every client written
//!     against the JSON lane — `curl` sends `*/*` — keeps the response
//!     it has always had.
//!   * **A session id is issued, never demanded.** `initialize` answers
//!     with `Mcp-Session-Id` and later requests may carry it back, which
//!     is what gives `logging/setLevel` something to move
//!     ([`session_gate`]). A request without it is served exactly the
//!     same, at the default floor. The transport allows a server to
//!     *require* the header; this one does not, and nothing here is
//!     authorization — the bearer token is (`require_bearer`).
//!   * **A slow consumer loses notifications, not the host's memory.**
//!     Progress frames queue against a byte budget
//!     ([`SSE_QUEUE_BUDGET_BYTES`]); what does not fit is dropped and
//!     accounted for in one coalesced marker frame. The JSON-RPC
//!     response bypasses the budget — it is the contract, the
//!     notifications are not.
//!   * **A disconnected client stops the tool.** Hanging up drops the
//!     SSE body, and the body owns a [`CancelOnDrop`] holding the
//!     dispatch's [`upeg_runtime::CancellationToken`] — the same
//!     contract `POST …/stream` has (`surfaces/http/stream.rs`). Without
//!     it a client that opened a stream and left took a ten-minute
//!     `External` child down with nobody left to read its output, which
//!     is exactly the run worth stopping.
//!
//! The bounded-queue design is the NDJSON streaming route's
//! (`surfaces/http/stream.rs`), reimplemented rather than shared: that
//! module's `QueueBudget`/`LineSender` speak in NDJSON lines and build
//! `{"event":"dropped"}` markers, so reusing them would mean
//! parameterizing another surface's line vocabulary instead of writing
//! this one's forty lines of accounting.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{
    Json,
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::{Interval, MissedTickBehavior, interval};
use upeg_core::{BoardKey, Principal};
use upeg_runtime::{CancellationToken, with_cancellation};

use crate::infrastructure::mcp_imports::{self, McpImportPhase};
use crate::surfaces::http::cancel_on_drop::CancelOnDrop;
use crate::surfaces::mcp::{self, LogLevelGate, McpLogWriter, McpNotificationSink};

// === SSE framing ===

/// Media type of an event stream, in `Accept` and in `Content-Type`.
const SSE_CONTENT_TYPE: &str = "text/event-stream";
/// `Cache-Control` on a stream: an intermediary must never replay it.
const SSE_CACHE_CONTROL: &str = "no-store";
/// SSE event name every JSON-RPC frame travels under. The transport
/// fixes it — a client dispatches on the frame's own `method`/`id`, not
/// on this.
const SSE_EVENT_NAME: &str = "message";
/// Field names of one SSE event, and the blank line that ends it.
const SSE_EVENT_FIELD: &str = "event";
const SSE_DATA_FIELD: &str = "data";
const SSE_FIELD_SEPARATOR: &str = ": ";
const SSE_EVENT_TERMINATOR: &str = "\n\n";
/// An SSE comment carries no event — it exists to put bytes on an idle
/// connection so proxies and clients do not time it out.
const SSE_COMMENT_PREFIX: &str = ":";
const SSE_KEEPALIVE_TEXT: &str = " keepalive";
/// How often an idle `/mcp` stream writes a keep-alive.
///
/// Both bodies need it, not just the long-lived `GET` one: a `POST`
/// stream carrying a ten-minute silent build puts no bytes on the socket
/// either, and an idle-timeout proxy between the client and this host
/// cannot tell that from a dead connection.
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// A fresh keep-alive timer for one stream body.
///
/// [`MissedTickBehavior::Delay`] on purpose: a stream that was starved
/// for a minute owes its client one keep-alive, not a minute's worth in
/// a burst.
fn keepalive_timer() -> Interval {
    let mut keepalive = interval(SSE_KEEPALIVE_INTERVAL);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);
    keepalive
}
/// Serialization fallback. `serde_json` cannot fail on a `Value`, but an
/// empty object keeps a malformed line off the wire if it ever did.
const EMPTY_JSON_OBJECT: &str = "{}";

/// One SSE `message` event carrying a JSON-RPC frame.
///
/// Compact JSON on purpose: an SSE `data:` line may not contain a
/// newline, and `serde_json::to_string` never emits one.
fn sse_event(frame: &Value) -> String {
    let payload = serde_json::to_string(frame).unwrap_or_else(|_| EMPTY_JSON_OBJECT.to_string());
    format!(
        "{SSE_EVENT_FIELD}{SSE_FIELD_SEPARATOR}{SSE_EVENT_NAME}\n\
         {SSE_DATA_FIELD}{SSE_FIELD_SEPARATOR}{payload}{SSE_EVENT_TERMINATOR}"
    )
}

/// An SSE comment — bytes with no event, for keep-alive.
fn sse_comment(text: &str) -> String {
    format!("{SSE_COMMENT_PREFIX}{text}{SSE_EVENT_TERMINATOR}")
}

/// Which body a `/mcp` request asked for.
///
/// A type rather than a bool so the two call sites read as the contract
/// they are: `Json` is the lane as it always was, `EventStream` is the
/// opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ResponseShape {
    Json,
    EventStream,
}

/// Read the shape off `Accept`.
///
/// Exact media-type match only. A wildcard (`*/*`, which is what `curl`
/// and most HTTP clients send by default) means "anything", and turning
/// that into a stream would change the answer every existing client
/// gets. Parameters (`;q=…`) are ignored; the client either named the
/// type or it did not.
pub(super) fn response_shape(headers: &HeaderMap) -> ResponseShape {
    let accepts_stream = headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|entry| {
            entry
                .split(';')
                .next()
                .is_some_and(|media_type| media_type.trim() == SSE_CONTENT_TYPE)
        });
    if accepts_stream {
        ResponseShape::EventStream
    } else {
        ResponseShape::Json
    }
}

/// Build a `text/event-stream` response around `body`.
fn sse_response(body: Body) -> Response {
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(SSE_CONTENT_TYPE),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(SSE_CACHE_CONTROL),
    );
    response
}

// === Sessions ===

/// Header the session id travels on, both directions.
///
/// Visible to the whole HTTP surface because the CORS policy has to name
/// it too (`surfaces/http/cors.rs`): a browser client cannot send a
/// header the preflight did not allow, nor read one the response did not
/// expose.
pub(in crate::surfaces::http) const MCP_SESSION_HEADER: &str = "mcp-session-id";

/// How many sessions' severity floors the host remembers.
///
/// A cap, not a lifetime: HTTP gives this lane no "session ended" event,
/// so the only alternative to forgetting the oldest is growing forever
/// on a client that reconnects in a loop. A forgotten session falls back
/// to the default floor — the same thing a client that never called
/// `logging/setLevel` sees.
const MAX_TRACKED_SESSIONS: usize = 64;

/// The id a `/mcp` client carries between requests.
///
/// A newtype so it cannot be confused with the bearer token it sits
/// beside in the same request: this one names a session, the other one
/// authorizes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SessionId(String);

impl SessionId {
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Every session whose severity floor this host is still tracking,
/// oldest first.
static SESSIONS: Mutex<Vec<(SessionId, LogLevelGate)>> = Mutex::new(Vec::new());

/// The session named by this request, if it named one.
///
/// A header this host never issued is accepted as-is: it simply names a
/// session with nothing recorded against it, which is the same state a
/// fresh one is in. Rejecting it would be enforcement, and this lane
/// does not enforce.
pub(super) fn session_from_headers(headers: &HeaderMap) -> Option<SessionId> {
    let raw = headers.get(MCP_SESSION_HEADER)?;
    let value = raw.to_str().ok()?.trim();
    (!value.is_empty()).then(|| SessionId(value.to_string()))
}

/// Issue a new session id and start tracking its severity floor.
///
/// Cryptographically random via the same generator that mints bearer
/// tokens — the transport requires session ids to be unguessable, and a
/// second entropy path would be a second thing to get wrong. `None` when
/// the OS could not supply entropy: the id is a convenience, so a host
/// that cannot mint one still answers the handshake.
pub(super) fn open_session() -> Option<SessionId> {
    let id = SessionId(crate::infrastructure::auth::generate_token().ok()?);
    let mut sessions = SESSIONS.lock().unwrap_or_else(PoisonError::into_inner);
    if sessions.len() >= MAX_TRACKED_SESSIONS {
        sessions.remove(0);
    }
    sessions.push((id.clone(), LogLevelGate::default()));
    Some(id)
}

/// The severity floor `logging/setLevel` moves for this request.
///
/// The floor belongs to the session, not to the request, which is the
/// whole reason a session id exists on this lane: one request sets the
/// floor and a later one has to see it. A request naming no session — or
/// one the host has already forgotten — gets a fresh gate, so its
/// `logging/setLevel` is honored for that request and remembered by
/// nobody.
pub(super) fn session_gate(session: Option<&SessionId>) -> LogLevelGate {
    let Some(session) = session else {
        return LogLevelGate::default();
    };
    let sessions = SESSIONS.lock().unwrap_or_else(PoisonError::into_inner);
    sessions
        .iter()
        .find(|(id, _)| id == session)
        .map(|(_, gate)| gate.clone())
        .unwrap_or_default()
}

/// Name the session on the way out — only when this response is the one
/// that issued it, per the transport's `initialize` rule.
pub(super) fn with_session_header(mut response: Response, issued: Option<&SessionId>) -> Response {
    if let Some(id) = issued
        && let Ok(value) = HeaderValue::from_str(id.as_str())
    {
        response.headers_mut().insert(MCP_SESSION_HEADER, value);
    }
    response
}

// === POST /mcp — one call's stream ===

/// How much not-yet-written SSE body the host holds for one `/mcp` call.
///
/// A byte budget, not a frame count, for the reason the NDJSON route
/// gives: one shell prompt redraw and one base64 blob are both "a
/// frame", and memory is what is being protected.
const SSE_QUEUE_BUDGET_BYTES: usize = 1024 * 1024;

/// Whether a frame may be turned away when the queue is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    /// Progress notifications: best-effort by construction.
    WithinBudget,
    /// The JSON-RPC response, and the drop marker that accounts for what
    /// was lost. Both are bounded in size and arrive exactly once.
    Guaranteed,
}

/// What happened to a frame offered to the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Forwarding {
    /// Queued, or honestly accounted for as dropped.
    Continuing,
    /// The consumer is gone. Nothing more will be forwarded.
    Stopped,
}

/// Byte accounting shared by the producing side and the body.
///
/// `queued` rises when an event is handed to the channel and falls when
/// the body yields it, so it measures what the host is holding for a
/// consumer that has not read it yet. `dropped` counts the notification
/// frames turned away while `queued` sat at the budget, and empties into
/// one marker as soon as a frame fits again — coalescing, so a consumer
/// stalled for a minute is told one honest number.
#[derive(Default)]
struct FrameBudget {
    queued: AtomicUsize,
    dropped: AtomicUsize,
}

impl FrameBudget {
    /// Take `bytes` of the budget, or refuse when that would exceed it.
    fn reserve(&self, bytes: usize, admission: Admission) -> bool {
        if admission == Admission::Guaranteed {
            self.queued.fetch_add(bytes, Ordering::Relaxed);
            return true;
        }
        self.queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |queued| {
                let next = queued.saturating_add(bytes);
                (next <= SSE_QUEUE_BUDGET_BYTES).then_some(next)
            })
            .is_ok()
    }

    /// Give `bytes` back — the body has written that event out.
    fn release(&self, bytes: usize) {
        let _ = self
            .queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |queued| {
                Some(queued.saturating_sub(bytes))
            });
    }

    /// How much the host is holding for a consumer that has not read it
    /// yet. A test observer: production code only reserves and releases.
    #[cfg(test)]
    fn queued_bytes(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }
}

/// Sender half of one call's event queue, with the budget in front of it.
#[derive(Clone)]
struct FrameSender {
    sender: UnboundedSender<String>,
    budget: Arc<FrameBudget>,
}

/// The queue one streamed call's body is fed through.
fn frame_queue() -> (FrameSender, UnboundedReceiver<String>, Arc<FrameBudget>) {
    let (sender, receiver) = unbounded_channel::<String>();
    let budget = Arc::new(FrameBudget::default());
    (
        FrameSender {
            sender,
            budget: Arc::clone(&budget),
        },
        receiver,
        budget,
    )
}

impl FrameSender {
    /// Offer one droppable notification frame.
    fn offer(&self, frame: &Value) -> Forwarding {
        if self.sender.is_closed() {
            return Forwarding::Stopped;
        }
        let event = sse_event(frame);
        let bytes = event.len();
        if !self.budget.reserve(bytes, Admission::WithinBudget) {
            self.budget.dropped.fetch_add(1, Ordering::Relaxed);
            return Forwarding::Continuing;
        }
        // Deliberately *after* the reservation succeeded: a frame
        // fitting again is the signal that the consumer caught up, and
        // the only moment a marker is owed. Flushing first would emit one
        // marker per refused frame — the budget always has room for a
        // short marker even when it has none for a 64 KiB frame — which
        // is the opposite of coalescing.
        self.flush_dropped();
        self.send(event, bytes)
    }

    /// Queue the terminal JSON-RPC response, preceded by whatever drop
    /// marker is still owed. Both bypass the budget: a consumer that
    /// reads to the end is entitled to its response and to the full drop
    /// count.
    fn finish(&self, frame: &Value) {
        self.flush_dropped();
        let event = sse_event(frame);
        let bytes = event.len();
        if self.budget.reserve(bytes, Admission::Guaranteed) {
            let _ = self.send(event, bytes);
        }
    }

    /// Emit the accumulated drop count as one marker frame, if any is
    /// owed.
    fn flush_dropped(&self) {
        // Claimed with one `swap`, not read-then-subtract: two threads
        // that both read the same total would each report it, so a
        // consumer would be told it lost twice what it lost. The claim is
        // handed back on the paths that fail to deliver it, so the count
        // is never silently dropped either.
        let dropped = self.budget.dropped.swap(0, Ordering::Relaxed);
        if dropped == 0 {
            return;
        }
        let event = sse_event(&mcp::dropped_notifications_frame(dropped));
        let bytes = event.len();
        if !self.budget.reserve(bytes, Admission::Guaranteed) {
            self.budget.dropped.fetch_add(dropped, Ordering::Relaxed);
            return;
        }
        if self.send(event, bytes) == Forwarding::Stopped {
            self.budget.dropped.fetch_add(dropped, Ordering::Relaxed);
        }
    }

    /// Hand an event the caller has already reserved `reserved_bytes`
    /// for to the channel, giving the reservation back if the consumer
    /// turns out to be gone.
    fn send(&self, event: String, reserved_bytes: usize) -> Forwarding {
        if self.sender.send(event).is_err() {
            self.budget.release(reserved_bytes);
            return Forwarding::Stopped;
        }
        Forwarding::Continuing
    }
}

impl McpNotificationSink for FrameSender {
    /// Best-effort, as the trait promises: a consumer that hung up
    /// mid-run must not take the running tool down with it, and a frame
    /// that does not fit the budget is counted rather than kept.
    fn emit(&self, frame: &Value) {
        let _ = self.offer(frame);
    }
}

/// Answer a `/mcp` request with an SSE stream: the running tool's output
/// as `notifications/message` events, then the JSON-RPC response as the
/// last event before the stream closes.
///
/// The dispatch is synchronous and can block for minutes, so it runs on a
/// blocking worker — and the progress sink is installed *on that worker*,
/// because the ambient sink is per-thread. Every event travels through
/// one queue, which is what guarantees the response lands last.
///
/// `caller` rather than a bare surface: this lane authenticates, so the
/// role its bearer proved is a fact only the listener holds
/// (`surfaces/http/rpc.rs`), and the Chain approval gate decides against
/// it.
pub(super) fn streamed_response(
    request: Value,
    caller: Principal,
    board: Option<BoardKey>,
    session: Option<SessionId>,
) -> Response {
    let (frames, receiver, budget) = frame_queue();
    let log = McpLogWriter::with_level(Arc::new(frames.clone()), session_gate(session.as_ref()));
    let cancellation = CancellationToken::new();
    let dispatch_cancellation = cancellation.clone();
    tokio::task::spawn_blocking(move || {
        let response = with_cancellation(dispatch_cancellation, || {
            mcp::handle_with_log(request, caller, board.as_ref(), Some(&log))
        });
        if let Some(response) = response {
            frames.finish(&response);
        }
    });
    sse_response(Body::from_stream(SseEvents {
        receiver,
        budget,
        keepalive: keepalive_timer(),
        cancellation: CancelOnDrop::new(cancellation),
    }))
}

/// The response body: the queue's events, in order, until the worker
/// drops its sender. Yielding an event is also what gives its bytes back
/// to [`FrameBudget`], so the budget tracks what the host still holds
/// rather than what it has ever produced.
struct SseEvents {
    receiver: UnboundedReceiver<String>,
    budget: Arc<FrameBudget>,
    /// Bytes for an idle connection: a `tools/call` on a silent build
    /// produces no frame for minutes, and a proxy in between cannot tell
    /// that from a dead socket.
    keepalive: Interval,
    /// Cancels the dispatch when this body goes away — the only signal a
    /// hung-up client actually produces.
    #[expect(
        dead_code,
        reason = "held for its Drop: dropping the body is what cancels the dispatch"
    )]
    cancellation: CancelOnDrop,
}

impl futures_core::Stream for SseEvents {
    type Item = Result<String, Infallible>;

    /// Frames first, keep-alive only when there is nothing else to
    /// write — a stream that is producing output does not need a comment
    /// to prove it is alive.
    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.receiver.poll_recv(context) {
            Poll::Ready(Some(event)) => {
                this.budget.release(event.len());
                return Poll::Ready(Some(Ok(event)));
            }
            // The worker dropped its sender: the response frame is
            // already out and this stream is over. A keep-alive after it
            // would hold a connection open with nothing left to say.
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }
        match this.keepalive.poll_tick(context) {
            Poll::Ready(_) => Poll::Ready(Some(Ok(sse_comment(SSE_KEEPALIVE_TEXT)))),
            Poll::Pending => Poll::Pending,
        }
    }
}

// === GET /mcp — the server's own stream ===

/// `GET /mcp`: the long-lived stream for frames that belong to no
/// request.
///
/// `406` rather than a JSON body when the client did not ask for a
/// stream: there is no other representation of this resource to fall
/// back to, and answering something else would hide the mistake.
pub(super) async fn mcp_events(headers: HeaderMap) -> Response {
    if response_shape(&headers) != ResponseShape::EventStream {
        return (
            StatusCode::NOT_ACCEPTABLE,
            Json(json!({ "error": format!("GET /mcp requires `Accept: {SSE_CONTENT_TYPE}`") })),
        )
            .into_response();
    }
    sse_response(Body::from_stream(McpEventStream::new()))
}

/// Does this phase transition mean the tool list may have changed?
///
/// Only a finished load registers tools. `Loading` is the window
/// opening, and `Skipped`/`NotStarted` mean nothing was ever fetched —
/// telling a client to re-read on those would be noise. `Done` with zero
/// tools still counts: "the window closed and nothing arrived" is
/// exactly what a client waiting on imports needs to hear.
const fn changes_tool_list(phase: McpImportPhase) -> bool {
    matches!(phase, McpImportPhase::Done(_))
}

/// The long-lived stream's body: import-phase transitions turned into
/// notifications, with keep-alive comments in between.
struct McpEventStream {
    phases: mcp_imports::PhaseSubscription,
    keepalive: Interval,
}

impl McpEventStream {
    fn new() -> Self {
        Self {
            phases: mcp_imports::subscribe_phase_changes(),
            keepalive: keepalive_timer(),
        }
    }
}

impl futures_core::Stream for McpEventStream {
    type Item = Result<String, Infallible>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match this.phases.poll_recv(context) {
                // A transition nobody has to hear about — keep draining
                // rather than waking on the next one for nothing.
                Poll::Ready(Some(phase)) if !changes_tool_list(phase) => continue,
                Poll::Ready(Some(_)) => {
                    return Poll::Ready(Some(Ok(sse_event(&mcp::tools_list_changed_frame()))));
                }
                // Unreachable while this stream holds the receiver, and
                // the honest answer if it ever happened: no further
                // server-initiated frame can arrive.
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => {}
            }
            return match this.keepalive.poll_tick(context) {
                Poll::Ready(_) => Poll::Ready(Some(Ok(sse_comment(SSE_KEEPALIVE_TEXT)))),
                Poll::Pending => Poll::Pending,
            };
        }
    }
}

#[cfg(test)]
#[path = "mcp_sse_tests.rs"]
mod tests;
