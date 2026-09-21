//! Streaming dispatch for the Flutter desktop surface: live output while
//! a Tool runs, and a way to stop it.
//!
//! `api::tools::dispatch_tool` / `dispatch_tool_async` answer once, at
//! the end. A Tool that shells out can run for minutes, and the person
//! watching the modal has neither a sign of life nor a way out. This
//! module runs the *same* dispatch body with the two ambient scopes the
//! runtime already defines around it:
//!
//! - [`with_progress_sink`] — the invoker's reader threads call
//!   [`ProgressSink::emit`] with each slice of output while the
//!   dispatching thread is still blocked inside the Tool. Every event is
//!   forwarded straight into the Dart [`StreamSink`] as a
//!   [`DispatchStreamEventDto::Chunk`].
//! - [`with_cancellation`] — a token the Dart side can trip through
//!   [`cancel_dispatch`], which the invoker's wait loop polls.
//!
//! Both scopes are thread-local and cover exactly one dispatch call
//! (nested Chain steps inherit them), which is why the entry point is a
//! plain blocking function: flutter_rust_bridge runs it to completion on
//! one worker thread, the same shape the HTTP surface gets from
//! `tokio::task::spawn_blocking` (`upeg-cli/src/surfaces/http/stream.rs`).
//!
//! One stream carries both progress and the final result, so Dart holds
//! a single subscription whose last event is always the answer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use flutter_rust_bridge::frb;

use upeg_runtime::{
    CancellationToken, ProgressEvent, ProgressSink, SharedProgressSink, with_cancellation,
    with_progress_sink,
};

use crate::frb_generated::StreamSink;

use super::boot::FrbError;
use super::tools::{CanonicalToolResult, dispatch_tool_impl};

/// Validation field name reported when a `run_id` is already in flight.
const RUN_ID_FIELD: &str = "run_id";

/// One event on a streamed dispatch.
///
/// A single sealed enum rather than two streams: the ordering guarantee
/// the UI actually needs is "every chunk I will ever see arrives before
/// the result", and one channel gives that for free. Two streams would
/// make the tail and the outcome race each other on the Dart side for no
/// gain.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum DispatchStreamEventDto {
    /// One incremental slice of the running Tool's output. `stream` is
    /// [`upeg_runtime::ProgressStream::wire_name`] (`stdout` / `stderr`)
    /// — the same spelling the HTTP NDJSON and MCP log wires use, so the
    /// desktop surface introduces no second vocabulary.
    Chunk {
        stream: String,
        seq: u64,
        chunk: String,
    },
    /// The dispatch settled. Always the last event on the stream.
    Done { result: CanonicalToolResult },
}

/// Forwards runtime progress into the Dart stream.
///
/// [`ProgressSink::emit`] is called from the invoker's reader threads,
/// so this must be `Send + Sync` — [`StreamSink`] is both, and its `add`
/// is the "subscriber went away" signal, which is best-effort by
/// contract: a dropped chunk is never an error, the `Done` event is.
struct SinkProgressForwarder {
    sink: StreamSink<DispatchStreamEventDto>,
}

impl ProgressSink for SinkProgressForwarder {
    fn emit(&self, event: ProgressEvent) {
        let _ = self.sink.add(DispatchStreamEventDto::Chunk {
            stream: event.stream.wire_name().to_string(),
            seq: event.seq,
            chunk: event.chunk,
        });
    }
}

// ─── Cancellation registry ─────────────────────────────────────
//
// A streamed dispatch and the cancel request for it arrive as two
// independent FFI calls, so the token has to live somewhere both can
// reach. Keyed by `run_id`, which the *caller* mints: the dispatch entry
// point returns a stream rather than a value, so it has no return slot
// to hand an id back through, and an id invented in Rust would only
// reach Dart as a stream event — i.e. after the point where a user might
// already want to cancel.

/// Distinguishes two runs that reused the same `run_id`.
///
/// The id is caller-minted, so nothing stops Dart from reusing one. The
/// occupancy check below refuses a *concurrent* reuse, but a
/// sequentially reused id is legitimate and the registry must be able to
/// tell the second registration from the first — otherwise the first
/// one's `Drop` would evict the second one's entry and leave a live run
/// uncancellable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RunSeq(u64);

impl RunSeq {
    /// The next unused sequence. Process-wide because the registry is;
    /// relaxed ordering is enough because the only property required is
    /// uniqueness, which `fetch_add` gives on its own.
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// What one live run is, as the registry sees it.
struct RegisteredRun {
    seq: RunSeq,
    token: CancellationToken,
}

type RunRegistry = HashMap<String, RegisteredRun>;

fn run_registry() -> &'static Mutex<RunRegistry> {
    static RUNS: OnceLock<Mutex<RunRegistry>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(RunRegistry::new()))
}

/// A run's entry in the registry, removed on every exit path of the
/// dispatch — normal return, early return, and unwind — because the
/// removal is a `Drop`. A leaked entry would keep a dead run
/// cancellable, which is worse than not being able to cancel: the id
/// would answer `true` forever.
///
/// `Drop` is the *only* remover, and it removes only its own
/// [`RunSeq`]. Cancelling used to remove the entry, which freed the id
/// while the run it named was still going: a re-registration under that
/// id then owned an entry the still-running first dispatch would evict
/// on its way out.
struct RunRegistration {
    run_id: String,
    seq: RunSeq,
}

impl RunRegistration {
    /// Register `token` under `run_id`, or refuse when that id is
    /// already in flight — two runs sharing an id would leave one of
    /// them uncancellable the moment the other finishes.
    fn open(run_id: String, token: CancellationToken) -> Result<Self, FrbError> {
        let mut guard = run_registry().lock().map_err(|_| FrbError::Internal {
            message: "dispatch run registry poisoned".to_string(),
        })?;
        if guard.contains_key(&run_id) {
            return Err(FrbError::Validation {
                field: RUN_ID_FIELD.to_string(),
                reason: format!("run id `{run_id}` is already dispatching"),
            });
        }
        let seq = RunSeq::next();
        guard.insert(run_id.clone(), RegisteredRun { seq, token });
        Ok(Self { run_id, seq })
    }
}

impl Drop for RunRegistration {
    fn drop(&mut self) {
        let Ok(mut guard) = run_registry().lock() else {
            return;
        };
        // Only this registration's own entry. Anything else under the id
        // belongs to a later run that is still alive.
        if guard
            .get(&self.run_id)
            .is_some_and(|registered| registered.seq == self.seq)
        {
            guard.remove(&self.run_id);
        }
    }
}

/// Run the named tool and stream its output, then its result.
///
/// Same arguments as [`super::tools::dispatch_tool`] — including the
/// typed `approve` flag, which is the only way a desktop dispatch lifts
/// a Chain's approval barrier — plus a caller-minted `run_id` that
/// [`cancel_dispatch`] uses to reach this run's cancellation token.
///
/// Chunks only ever arrive from an invoker that reports progress (the
/// `External` invoker does; in-process function tools have nothing to
/// report), so a stream that carries a lone `Done` is a normal outcome,
/// not a failure. That is also the whole story on wasm32, which has no
/// External invoker at all.
#[frb]
pub fn dispatch_tool_streamed(
    tool_id: String,
    args_json: String,
    board_key: Option<String>,
    approve: bool,
    run_id: String,
    sink: StreamSink<DispatchStreamEventDto>,
) -> Result<(), FrbError> {
    let token = CancellationToken::new();
    let _registration = RunRegistration::open(run_id, token.clone())?;

    let forwarder: SharedProgressSink = Arc::new(SinkProgressForwarder { sink: sink.clone() });
    let result = with_cancellation(token, || {
        with_progress_sink(forwarder, || {
            dispatch_tool_impl(&tool_id, &args_json, board_key.as_deref(), approve)
        })
    });

    let _ = sink.add(DispatchStreamEventDto::Done { result });
    Ok(())
}

/// Ask the run registered under `run_id` to stop.
///
/// Returns whether a live run was found. `false` means the run already
/// settled (or never started) — the honest answer for a Cancel button
/// the user pressed a moment too late, and never an error.
///
/// Cancellation is cooperative: the token is tripped here and the
/// invoker's wait loop notices it. The run still ends by sending its
/// `Done` event, so the Dart side has exactly one completion path.
///
/// The entry stays in the registry. Removing it here would free the id
/// while the run it names is still running — a request to stop is not
/// the moment the run ended, and [`RunRegistration::drop`] is the one
/// place that knows which moment that is.
#[frb(sync)]
pub fn cancel_dispatch(run_id: String) -> bool {
    let Ok(guard) = run_registry().lock() else {
        return false;
    };
    let Some(registered) = guard.get(&run_id) else {
        return false;
    };
    registered.token.cancel();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_unknown_run_id_returns_false() {
        assert!(!cancel_dispatch("dispatch_stream.unknown.run".to_string()));
    }

    #[test]
    fn registered_run_is_cancelled() {
        let run_id = "dispatch_stream.registered.run".to_string();
        let token = CancellationToken::new();
        let registration =
            RunRegistration::open(run_id.clone(), token.clone()).expect("first registration");

        assert!(cancel_dispatch(run_id.clone()));
        assert!(token.is_cancelled());

        drop(registration);
        assert!(
            !cancel_dispatch(run_id),
            "a finished run can no longer be cancelled"
        );
    }

    #[test]
    fn cancel_does_not_erase_the_entry() {
        // Cancel is a request, not the end of the run. Erasing the entry
        // here would make a still-running run's id immediately
        // re-registrable.
        let run_id = "dispatch_stream.cancel_keeps.run".to_string();
        let _registration =
            RunRegistration::open(run_id.clone(), CancellationToken::new()).expect("registration");

        assert!(cancel_dispatch(run_id.clone()));

        assert!(
            matches!(
                RunRegistration::open(run_id, CancellationToken::new()),
                Err(FrbError::Validation { .. })
            ),
            "a still-live run's id cannot be re-registered"
        );
    }

    #[test]
    fn drop_of_past_registration_does_not_erase_new_run_with_same_id() {
        // Regression: if Drop did not check ownership, a run started
        // earlier would push the live run re-registered under the same
        // id out of the registry when it ended.
        let run_id = "dispatch_stream.reused.run".to_string();
        let stale = RunRegistration::open(run_id.clone(), CancellationToken::new())
            .expect("first registration");
        // After the earlier run ends and vacates the slot, a new run
        // starts under the same id.
        drop(stale);
        let fresh_token = CancellationToken::new();
        let _fresh = RunRegistration::open(run_id.clone(), fresh_token.clone())
            .expect("second registration");

        // Simulates the late-arriving Drop of the old registration.
        drop(RunRegistrationCorpse {
            run_id: run_id.clone(),
        });

        assert!(cancel_dispatch(run_id), "the new run is still cancellable");
        assert!(fresh_token.is_cancelled());
    }

    /// A value mimicking the Drop of an already-consumed old
    /// `RunRegistration` — points at the same id but its [`RunSeq`]
    /// never overlaps.
    struct RunRegistrationCorpse {
        run_id: String,
    }

    impl Drop for RunRegistrationCorpse {
        fn drop(&mut self) {
            let corpse = RunRegistration {
                run_id: std::mem::take(&mut self.run_id),
                seq: RunSeq::next(),
            };
            drop(corpse);
        }
    }

    #[test]
    fn run_id_is_no_longer_cancellable_after_registration_ends() {
        let run_id = "dispatch_stream.dropped.run".to_string();
        {
            let _registration = RunRegistration::open(run_id.clone(), CancellationToken::new())
                .expect("registration");
        }

        assert!(!cancel_dispatch(run_id));
    }

    #[test]
    fn registering_same_run_id_twice_is_rejected() {
        let run_id = "dispatch_stream.duplicate.run".to_string();
        let _first =
            RunRegistration::open(run_id.clone(), CancellationToken::new()).expect("first");

        let second = RunRegistration::open(run_id, CancellationToken::new());

        assert!(matches!(second, Err(FrbError::Validation { .. })));
    }
}
