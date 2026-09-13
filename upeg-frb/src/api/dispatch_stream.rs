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
    fn 모르는_run_id_취소는_거짓을_돌려준다() {
        assert!(!cancel_dispatch("dispatch_stream.unknown.run".to_string()));
    }

    #[test]
    fn 등록된_run은_취소된다() {
        let run_id = "dispatch_stream.registered.run".to_string();
        let token = CancellationToken::new();
        let registration =
            RunRegistration::open(run_id.clone(), token.clone()).expect("first registration");

        assert!(cancel_dispatch(run_id.clone()));
        assert!(token.is_cancelled());

        drop(registration);
        assert!(
            !cancel_dispatch(run_id),
            "끝난 run은 더 이상 취소되지 않는다"
        );
    }

    #[test]
    fn 취소는_항목을_지우지_않는다() {
        // 취소는 요청이지 실행의 끝이 아니다. 여기서 항목을 지우면 아직
        // 돌고 있는 run의 id가 곧바로 재등록 가능해진다.
        let run_id = "dispatch_stream.cancel_keeps.run".to_string();
        let _registration =
            RunRegistration::open(run_id.clone(), CancellationToken::new()).expect("registration");

        assert!(cancel_dispatch(run_id.clone()));

        assert!(
            matches!(
                RunRegistration::open(run_id, CancellationToken::new()),
                Err(FrbError::Validation { .. })
            ),
            "아직 살아 있는 run의 id는 다시 등록되지 않는다"
        );
    }

    #[test]
    fn 지난_등록의_drop은_같은_id의_새_run을_지우지_않는다() {
        // 회귀: Drop이 소유권을 확인하지 않으면, 먼저 시작한 run이 끝날 때
        // 같은 id로 새로 등록된 살아 있는 run을 레지스트리에서 밀어냈다.
        let run_id = "dispatch_stream.reused.run".to_string();
        let stale = RunRegistration::open(run_id.clone(), CancellationToken::new())
            .expect("first registration");
        // 먼저 시작한 run이 끝나 자리를 비운 뒤, 같은 id로 새 run이 시작된다.
        drop(stale);
        let fresh_token = CancellationToken::new();
        let _fresh = RunRegistration::open(run_id.clone(), fresh_token.clone())
            .expect("second registration");

        // 뒤늦게 도착한 옛 등록의 Drop을 흉내 낸다.
        drop(RunRegistrationCorpse {
            run_id: run_id.clone(),
        });

        assert!(cancel_dispatch(run_id), "새 run은 여전히 취소할 수 있다");
        assert!(fresh_token.is_cancelled());
    }

    /// 이미 소비된 옛 `RunRegistration`의 Drop을 흉내 내는 값 — 같은 id를
    /// 가리키지만 [`RunSeq`]는 절대 겹치지 않는다.
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
    fn 등록이_끝나면_run_id는_더_이상_취소되지_않는다() {
        let run_id = "dispatch_stream.dropped.run".to_string();
        {
            let _registration = RunRegistration::open(run_id.clone(), CancellationToken::new())
                .expect("registration");
        }

        assert!(!cancel_dispatch(run_id));
    }

    #[test]
    fn 같은_run_id를_두_번_등록하면_거절한다() {
        let run_id = "dispatch_stream.duplicate.run".to_string();
        let _first =
            RunRegistration::open(run_id.clone(), CancellationToken::new()).expect("first");

        let second = RunRegistration::open(run_id, CancellationToken::new());

        assert!(matches!(second, Err(FrbError::Validation { .. })));
    }
}
