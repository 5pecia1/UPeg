//! Incremental progress for a still-running Tool.
//!
//! `dispatch` answers with one final [`upeg_core::ToolResult`] envelope.
//! That is the whole contract for every surface that only knows how to
//! render a result — and it stays that way. This module adds the second,
//! *optional* half: a surface that can show output while the tool is
//! still working installs a [`ProgressSink`] for the duration of one
//! dispatch, and invokers that produce output incrementally (today the
//! `External` invoker's child process) push chunks into it as they
//! arrive.
//!
//! Why an ambient, scoped sink instead of a dispatcher argument: the
//! dispatcher signature (`Fn(DispatchArgs) -> ToolResult`) is the
//! registry's shared type, implemented by built-ins, WASM plugins, chain
//! steps, and every TOML invoker. Threading an `Option<&dyn
//! ProgressSink>` through it would make every dispatcher — including the
//! ones that can never report progress — pay for the capability in its
//! signature. Installing it around the call instead keeps the capability
//! genuinely optional: nothing installed means the invoker skips the
//! forwarding work entirely and the caller sees exactly today's
//! behaviour.
//!
//! Threading model: [`with_progress_sink`] stores the sink in a
//! thread-local for the calling thread, so a nested dispatch (a `Chain`
//! step runs on the dispatching thread) inherits it for free. Invokers
//! that read output on their own threads capture a [`ProgressReporter`]
//! *before* spawning and move a clone into each reader thread — hence
//! `Send + Sync + 'static` on the sink.
//!
//! The same shape carries the other optional half of a live call:
//! [`super::cancel`] installs a [`super::CancellationToken`] the same
//! way, for the same reason, and an invoker captures it on the same
//! thread at the same moment. Progress flows out of a running tool;
//! cancellation flows into one.
//!
//! Sequence numbers belong to the *installed scope*, not to the
//! reporter: one `with_progress_sink` call is one consumer's stream, and
//! that consumer was promised `seq` counts 0, 1, 2 … without repeats. A
//! `Chain` of three `External` steps captures three reporters inside the
//! same scope, so all three draw from the scope's one counter
//! ([`ProgressScope::next_seq`]) instead of restarting at 0 per step.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

/// Which of a running Tool's two output streams a chunk came from.
///
/// Deliberately its own type rather than a bool or a `&str`: it is
/// serialized onto three wires (HTTP NDJSON, MCP log notifications, the
/// CLI's live mirror) and every one of them must spell the same two
/// names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgressStream {
    Stdout,
    Stderr,
}

/// Wire spelling of [`ProgressStream::Stdout`].
pub const PROGRESS_STREAM_STDOUT: &str = "stdout";
/// Wire spelling of [`ProgressStream::Stderr`].
pub const PROGRESS_STREAM_STDERR: &str = "stderr";

impl ProgressStream {
    /// The name this stream carries on every wire.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Stdout => PROGRESS_STREAM_STDOUT,
            Self::Stderr => PROGRESS_STREAM_STDERR,
        }
    }
}

impl std::fmt::Display for ProgressStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.wire_name())
    }
}

/// One incremental slice of a still-running Tool's output.
///
/// `chunk` is whatever the invoker had ready — the `External` invoker
/// emits whole lines (including their trailing newline) so a consumer
/// can forward the text verbatim, and flushes any unterminated tail when
/// the stream ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressEvent {
    pub stream: ProgressStream,
    /// 0-based and contiguous across *both* streams and *every* step of
    /// the call the consumer installed its sink around, so a consumer
    /// that interleaves them can still report a total order.
    pub seq: u64,
    pub chunk: String,
}

/// Consumer of incremental progress.
///
/// [`Self::emit`] is called from the invoker's reader threads while the
/// dispatching thread is still blocked inside the tool, so an
/// implementation must be `Send + Sync` and must not call back into
/// dispatch. It must also never panic on a consumer that has gone away —
/// progress is best-effort by construction; the final envelope is the
/// contract.
pub trait ProgressSink: Send + Sync {
    fn emit(&self, event: ProgressEvent);
}

/// Shared, cloneable handle to a [`ProgressSink`].
pub type SharedProgressSink = Arc<dyn ProgressSink>;

impl<F> ProgressSink for F
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    fn emit(&self, event: ProgressEvent) {
        self(event);
    }
}

/// A sink that forwards events to an [`mpsc::Receiver`].
///
/// The natural shape for a consumer that lives on another thread (the
/// HTTP surface streams the receiver into an NDJSON body while the
/// dispatch runs on a blocking worker). A disconnected receiver is not
/// an error: the send result is dropped.
struct ChannelProgressSink {
    sender: mpsc::Sender<ProgressEvent>,
}

impl ProgressSink for ChannelProgressSink {
    fn emit(&self, event: ProgressEvent) {
        let _ = self.sender.send(event);
    }
}

/// A sink plus the receiver its events arrive on.
#[must_use]
pub fn progress_channel() -> (SharedProgressSink, mpsc::Receiver<ProgressEvent>) {
    let (sender, receiver) = mpsc::channel();
    (Arc::new(ChannelProgressSink { sender }), receiver)
}

/// One consumer's installed progress stream: where events go, and the
/// sequence they are numbered in.
///
/// The counter lives here rather than on [`ProgressReporter`] because
/// the *consumer* is what a `seq` is meaningful to. Every reporter
/// captured while this scope is installed — one per `External` step of
/// a `Chain`, one per nested dispatch — shares it, so the consumer sees
/// a single contiguous run of sequence numbers for the whole call.
#[derive(Clone)]
struct ProgressScope {
    sink: SharedProgressSink,
    next_seq: Arc<AtomicU64>,
}

impl ProgressScope {
    /// A freshly installed scope always starts its consumer at 0.
    fn new(sink: SharedProgressSink) -> Self {
        Self {
            sink,
            next_seq: Arc::new(AtomicU64::new(FIRST_SEQ)),
        }
    }
}

/// The sequence number the first reported chunk of a scope carries.
const FIRST_SEQ: u64 = 0;

thread_local! {
    /// The scope installed for the dispatch currently running on this
    /// thread, if any.
    static ACTIVE_SCOPE: RefCell<Option<ProgressScope>> = const { RefCell::new(None) };
}

/// Restores the previously installed scope on scope exit — including on
/// unwind, so a panicking tool cannot leak its caller's sink into the
/// next dispatch on the same thread.
struct ActiveScopeGuard {
    previous: Option<ProgressScope>,
}

impl Drop for ActiveScopeGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_SCOPE.with(|active| {
            *active.borrow_mut() = previous;
        });
    }
}

/// Run `f` with `sink` installed as this thread's ambient progress sink.
///
/// Nesting is well defined: the inner scope wins for its duration and
/// the outer sink is restored afterwards. Each installation opens a
/// fresh sequence — a different consumer is a different stream.
pub fn with_progress_sink<T>(sink: SharedProgressSink, f: impl FnOnce() -> T) -> T {
    let previous =
        ACTIVE_SCOPE.with(|active| active.borrow_mut().replace(ProgressScope::new(sink)));
    let _guard = ActiveScopeGuard { previous };
    f()
}

/// The scope installed for the dispatch running on this thread, if any.
fn active_progress_scope() -> Option<ProgressScope> {
    ACTIVE_SCOPE.with(|active| active.borrow().clone())
}

/// The sink installed for the dispatch running on this thread, if any.
#[must_use]
pub fn active_progress_sink() -> Option<SharedProgressSink> {
    active_progress_scope().map(|scope| scope.sink)
}

/// An invoker's handle for reporting progress during one dispatch.
///
/// Shares the installed scope's sequence counter, so every clone handed
/// to a reader thread — and every *other* reporter captured inside the
/// same [`with_progress_sink`] scope — draws from one monotonic
/// sequence and the consumer can order stdout against stderr, and step
/// against step. Constructed with [`Self::capture`] on the dispatching
/// thread — after that it travels by clone, because the thread-local
/// scope is not visible from the reader threads.
#[derive(Clone)]
pub struct ProgressReporter {
    sink: SharedProgressSink,
    next_seq: Arc<AtomicU64>,
}

impl ProgressReporter {
    /// Capture this thread's ambient scope, or `None` when no consumer
    /// installed one — the signal for an invoker to skip its forwarding
    /// work entirely.
    ///
    /// The captured reporter continues the scope's sequence rather than
    /// starting a new one: a `Chain` dispatches its `External` steps one
    /// after another on this thread, and its consumer is owed one
    /// stream, not one per step.
    #[must_use]
    pub fn capture() -> Option<Self> {
        active_progress_scope().map(|scope| Self {
            sink: scope.sink,
            next_seq: scope.next_seq,
        })
    }

    /// Build a reporter over an explicit sink, with a sequence of its
    /// own (tests, and consumers that hold a sink without installing
    /// it).
    #[must_use]
    pub fn new(sink: SharedProgressSink) -> Self {
        Self {
            sink,
            next_seq: Arc::new(AtomicU64::new(FIRST_SEQ)),
        }
    }

    /// Report one chunk. Empty chunks are dropped rather than burning a
    /// sequence number on a no-op.
    pub fn report(&self, stream: ProgressStream, chunk: String) {
        if chunk.is_empty() {
            return;
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        self.sink.emit(ProgressEvent { stream, seq, chunk });
    }
}

impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgressReporter")
            .field("next_seq", &self.next_seq.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
