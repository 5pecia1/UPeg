//! Ambient, scoped cancellation for one dispatch.
//!
//! `dispatch` answers with one final [`upeg_core::ToolResult`] envelope,
//! and its dispatcher type is the registry's shared
//! `Fn(DispatchArgs) -> ToolResult` — a signature with nowhere to put a
//! cancellation argument. That signature is not going to change: every
//! built-in, WASM plugin, and TOML invoker implements it, and almost
//! none of them can be cancelled at all.
//!
//! So cancellation arrives the same way progress does (see
//! [`super::progress`]): the caller *installs* it around the call.
//! [`with_cancellation`] stores a [`CancellationToken`] in a
//! thread-local for the calling thread, which means a nested dispatch —
//! a `Chain` runs its steps on the dispatching thread — inherits it for
//! free. An invoker that can honour it captures the token with
//! [`active_cancellation`] *before* spawning its own threads and polls
//! [`CancellationToken::is_cancelled`] wherever it already ticks; every
//! other invoker never learns the capability exists and keeps behaving
//! exactly as it does today.
//!
//! The token itself is deliberately the smallest thing that works: an
//! `Arc<AtomicBool>` that only ever goes false → true. Cancellation is a
//! request, not a guarantee — what an invoker does with it (the
//! `External` invoker terminates the child's process group and fails
//! with a `cancelled` envelope) is the invoker's contract, and a tool
//! that ignores it still ends in the one final envelope every surface
//! renders.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A one-way "stop what you are doing" flag shared by a caller and
/// whatever it installed itself around.
///
/// Cloning shares the flag, so the consumer side (an HTTP response body
/// that is about to be dropped) and the producer side (the invoker's
/// wait loop) hold the same latch. It never goes back to false: a
/// cancelled call stays cancelled, which is what lets a poller read it
/// with a single relaxed-ordering load.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// A fresh, not-yet-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask whatever this token was installed around to stop.
    ///
    /// Idempotent, and callable from any thread — including from a
    /// `Drop` running on the async runtime while the dispatch itself
    /// blocks a worker thread.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Whether [`Self::cancel`] has been called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

thread_local! {
    /// The token installed for the dispatch currently running on this
    /// thread, if any.
    static ACTIVE_TOKEN: RefCell<Option<CancellationToken>> = const { RefCell::new(None) };
}

/// Restores the previously installed token on scope exit — including on
/// unwind, so a panicking tool cannot leak its caller's token into the
/// next dispatch on the same thread.
struct ActiveTokenGuard {
    previous: Option<CancellationToken>,
}

impl Drop for ActiveTokenGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_TOKEN.with(|active| {
            *active.borrow_mut() = previous;
        });
    }
}

/// Run `f` with `token` installed as this thread's ambient cancellation
/// token.
///
/// Nesting is well defined: the inner token wins for its duration and
/// the outer one is restored afterwards. An outer token is *not*
/// forwarded into the inner scope — a caller that wants one token for a
/// whole tree of calls installs that one token, rather than relying on
/// two scopes agreeing.
pub fn with_cancellation<T>(token: CancellationToken, f: impl FnOnce() -> T) -> T {
    let previous = ACTIVE_TOKEN.with(|active| active.borrow_mut().replace(token));
    let _guard = ActiveTokenGuard { previous };
    f()
}

/// The token installed for the dispatch running on this thread, or
/// `None` when nobody asked for cancellability — the signal for an
/// invoker to skip its polling work entirely.
///
/// Captured on the dispatching thread and then moved by clone into
/// whatever loop actually polls it: the thread-local is not visible from
/// an invoker's own reader or wait threads.
#[must_use]
pub fn active_cancellation() -> Option<CancellationToken> {
    ACTIVE_TOKEN.with(|active| active.borrow().clone())
}

#[cfg(test)]
#[path = "cancel_tests.rs"]
mod tests;
