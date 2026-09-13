//! The TUI's one in-flight dispatch.
//!
//! A dispatch can block for minutes (an `External` invoker waiting on a
//! child process), so it cannot run on the thread that also drives the
//! terminal — the UI would freeze and `Esc` would never be read. The
//! run therefore moves to a worker thread and talks back over two
//! channels: incremental output, and the one final [`Outcome`].
//!
//! Both optional halves of a live call are installed *on that worker*,
//! because both are thread-local scopes (see `upeg_runtime::progress`
//! and `upeg_runtime::cancel`) — the same shape the HTTP surface's
//! NDJSON route uses in `surfaces::http::stream`.
//!
//! Three things this type owes the loop, each of which used to be
//! missing:
//!
//!   * every message it forwards is stamped with the run's
//!     [`RunToken`], so a straggler cannot land in a later run's pane;
//!   * a worker that died without answering ends the run
//!     ([`Outcome::Failure`] with [`WORKER_PANICKED_ERROR_CODE`])
//!     instead of leaving `View::Running` on screen forever;
//!   * quitting waits, briefly, for the cancelled worker to acknowledge
//!     ([`RunningDispatch::cancel_and_wait`]) rather than trusting a
//!     flag nobody has read yet.

use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use upeg_runtime::{
    CancellationToken, ProgressEvent, SharedProgressSink, progress_channel, with_cancellation,
    with_progress_sink,
};

use crate::domain::execution::dispatch::{Outcome, dispatch_failure};

use super::super::model::{RunToken, State};
use super::super::msg::Msg;
use super::super::update::update;

/// How long the event loop blocks waiting for a terminal event when
/// nothing is running. Long enough that an idle session costs nothing.
pub(super) const IDLE_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How long it blocks while a dispatch is in flight. The live tail is
/// only as fresh as this cadence, so it is short enough to read as
/// "live" without turning an idle terminal into a spin loop.
///
/// Tradeoff, deliberately taken: the loop redraws at this rate, and each
/// frame re-derives `State::visible_placements` (one pegboard-snapshot
/// clone). At 20 fps over a few dozen pins that is negligible, and it
/// buys a tail that reads as live; revisit only if the pin count or the
/// snapshot cost grows.
pub(super) const RUNNING_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How long quitting waits for a cancelled run to acknowledge before
/// the session ends anyway.
///
/// Long enough for the `External` invoker to notice the token on its
/// next wait-loop tick, terminate the child's process group, and hand
/// back an envelope; short enough that a tool which never polls the
/// token cannot hold the terminal hostage. Cancellation is a request,
/// so the timeout is the honest upper bound on how long we honour our
/// side of it.
pub(super) const QUIT_CANCEL_GRACE: Duration = Duration::from_millis(1500);

/// `error.code` of the envelope synthesized when the worker thread died
/// without sending one — i.e. it panicked. A distinct code (not
/// `tool_error`) because nothing about the *tool* failed: the surface
/// lost its worker.
const WORKER_PANICKED_ERROR_CODE: &str = "worker_panicked";

/// What the terminal frames the loop shows: the worker died before
/// producing the one envelope every surface renders.
const WORKER_PANICKED_MESSAGE: &str = "dispatch worker ended without a result";

/// Whether a cancelled run answered before [`QUIT_CANCEL_GRACE`] ran
/// out. Named rather than a `bool` so a caller cannot read the polarity
/// backwards, and so the "we gave up" case has somewhere to be handled
/// if the surface ever grows a message for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunShutdown {
    /// The worker finished — either with its envelope or by ending the
    /// channel — inside the grace period.
    Acknowledged,
    /// The grace period expired with the worker still running. The
    /// thread is detached; the token stays tripped, so an invoker that
    /// polls it late still stops.
    Abandoned,
}

/// A dispatch running on its own thread, plus the two channels the UI
/// thread reads it through.
pub(super) struct RunningDispatch {
    /// Set by `Effect::CancelRun` (Esc) and by quitting mid-run, so the
    /// invoker is asked to stop instead of being orphaned.
    cancel: CancellationToken,
    progress: mpsc::Receiver<ProgressEvent>,
    outcome: mpsc::Receiver<Outcome>,
    /// Kept so quitting can actually *wait* for the worker rather than
    /// setting a flag and walking away.
    worker: JoinHandle<()>,
    run: RunToken,
    tool_id: &'static str,
}

impl RunningDispatch {
    /// Start `tool_id` on a worker thread. Every moved value is `Send`
    /// by construction: a `&'static str`, an owned `Value`, and the two
    /// owned board-scope values the UI thread resolved first.
    pub(super) fn spawn(
        run: RunToken,
        tool_id: &'static str,
        args: Value,
        board: Option<upeg_core::BoardKey>,
        preset: Option<upeg_core::ArgsPreset>,
    ) -> Self {
        Self::spawn_with(run, tool_id, move || {
            super::dispatch_through_host_or_local(tool_id, args, board, preset)
        })
    }

    /// [`Self::spawn`] with the dispatch body supplied by the caller.
    ///
    /// The two ambient scopes and the two channels are the part worth
    /// testing (cancellation acknowledgement, a dead worker); the body
    /// is what a test wants to replace.
    fn spawn_with(
        run: RunToken,
        tool_id: &'static str,
        body: impl FnOnce() -> Outcome + Send + 'static,
    ) -> Self {
        let (sink, progress) = progress_channel();
        let (outcome_sender, outcome) = mpsc::channel();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let worker = std::thread::spawn(move || {
            let sink: SharedProgressSink = sink;
            let result = with_cancellation(worker_cancel, || with_progress_sink(sink, body));
            // A gone receiver means the session already ended; the run's
            // result has nowhere to be rendered, which is not an error.
            let _ = outcome_sender.send(result);
        });
        Self {
            cancel,
            progress,
            outcome,
            worker,
            run,
            tool_id,
        }
    }

    pub(super) const fn tool_id(&self) -> &'static str {
        self.tool_id
    }

    /// Ask the run to stop. Idempotent — cancellation is a request, and
    /// the tool still ends in exactly one final envelope.
    pub(super) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Cancel and then wait, at most `grace`, for the worker to finish.
    ///
    /// Consumes the dispatch: this is the end of the session's
    /// relationship with it either way. Waiting on the outcome channel
    /// rather than on [`JoinHandle::join`] is what makes the wait
    /// bounded — sending the envelope is the worker's last act, so a
    /// send (or a closed channel, which is a worker that unwound) means
    /// there is nothing left to orphan.
    pub(super) fn cancel_and_wait(self, grace: Duration) -> RunShutdown {
        self.cancel.cancel();
        match self.outcome.recv_timeout(grace) {
            // Sent its envelope, or died: either way it is done running.
            Ok(_) | Err(RecvTimeoutError::Disconnected) => {
                // Bounded by construction — the worker is past its last
                // statement, so this only waits for the thread to unwind.
                let _ = self.worker.join();
                RunShutdown::Acknowledged
            }
            Err(RecvTimeoutError::Timeout) => RunShutdown::Abandoned,
        }
    }

    /// Fold everything the worker produced since the last frame into
    /// `state`, and answer with the final [`Outcome`] on the frame the
    /// run ends.
    ///
    /// A disconnected outcome channel is an *answer*, not "still
    /// running": the sender lives on the worker and is dropped when the
    /// worker unwinds, so a panicking dispatch reaches here as
    /// [`TryRecvError::Disconnected`] with nothing buffered. Reading
    /// that as "not finished" left `View::Running` on screen for the
    /// rest of the session with no key that could leave it.
    pub(super) fn pump(&self, state: &mut State) -> Option<Outcome> {
        self.drain_progress(state);
        let outcome = match self.outcome.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => worker_panicked_outcome(),
        };
        // Anything emitted between the drain above and the outcome
        // landing still belongs to this run's tail.
        self.drain_progress(state);
        Some(outcome)
    }

    fn drain_progress(&self, state: &mut State) {
        for event in self.progress.try_iter() {
            let _ = update(
                state,
                Msg::ToolProgress {
                    run: self.run,
                    event,
                },
            );
        }
    }
}

/// The one envelope a run whose worker vanished still owes the surface.
fn worker_panicked_outcome() -> Outcome {
    Outcome::Failure(dispatch_failure(
        WORKER_PANICKED_ERROR_CODE,
        WORKER_PANICKED_MESSAGE,
    ))
}

#[cfg(test)]
impl RunningDispatch {
    /// 테스트가 워커의 종료를 결정적으로 기다리기 위한 창구. 프로덕션
    /// 경로는 [`RunningDispatch::cancel_and_wait`]만 쓴다.
    fn wait_for_worker_exit(&self) {
        while !self.worker.is_finished() {
            std::thread::sleep(WORKER_EXIT_POLL_INTERVAL);
        }
    }
}

#[cfg(test)]
const WORKER_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(1);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces::tui::model::{LiveTail, View};

    /// 취소 토큰 폴링 주기. `External` invoker의 wait loop을 흉내 낸다.
    const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(5);
    /// 취소가 끝내 오지 않아도 테스트가 영원히 매달리지 않도록 하는 상한.
    const CANCEL_OBSERVE_LIMIT: Duration = Duration::from_secs(5);
    /// 취소를 무시하는 워커를 기다리는 시간. 실패 경로에서도 테스트가
    /// 이만큼만 지연되도록 짧게 잡는다.
    const IGNORED_CANCEL_GRACE: Duration = Duration::from_millis(80);

    const TEST_TOOL_ID: &str = "tui.run.test";
    const CANCELLED_CODE: &str = "cancelled";

    /// `start_run`이 만드는 것과 같은 모습: 모델이 이 run을 살아 있다고
    /// 여기고, 오른쪽 pane이 그 run의 실행 화면이다.
    fn running_session(tool_id: &'static str) -> (State, RunToken) {
        let mut state = State::default();
        let run = state.start_active_run(tool_id);
        state.view = View::Running {
            tool_id,
            tail: LiveTail::default(),
            cancelling: false,
        };
        (state, run)
    }

    /// 취소 토큰을 관측할 때까지 도는 tool 본문.
    fn cancellable_body() -> Outcome {
        let token = upeg_runtime::active_cancellation().expect("취소 범위가 설치되어야 한다");
        let deadline = std::time::Instant::now() + CANCEL_OBSERVE_LIMIT;
        while !token.is_cancelled() && std::time::Instant::now() < deadline {
            std::thread::sleep(CANCEL_POLL_INTERVAL);
        }
        Outcome::Failure(dispatch_failure(CANCELLED_CODE, "cancelled by the surface"))
    }

    #[test]
    fn 종료_시_취소는_워커의_응답을_기다린다() {
        let (_, run) = running_session(TEST_TOOL_ID);
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, cancellable_body);

        assert_eq!(
            dispatch.cancel_and_wait(CANCEL_OBSERVE_LIMIT),
            RunShutdown::Acknowledged,
            "취소를 관측하는 워커는 유예 안에 응답한다"
        );
    }

    #[test]
    fn 취소를_무시하는_워커는_유예가_끝나면_포기한다() {
        let (_, run) = running_session(TEST_TOOL_ID);
        let (release_sender, release) = mpsc::channel::<()>();
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, move || {
            // 취소를 절대 보지 않는 tool. 테스트가 끝날 때 풀어준다.
            let _ = release.recv();
            Outcome::NotFound
        });

        let shutdown = dispatch.cancel_and_wait(IGNORED_CANCEL_GRACE);

        assert_eq!(shutdown, RunShutdown::Abandoned);
        let _ = release_sender.send(());
    }

    #[test]
    fn 죽은_워커는_실행을_영원히_매달아두지_않는다() {
        let (mut state, run) = running_session(TEST_TOOL_ID);
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, || {
            std::panic::panic_any("워커가 죽는 상황을 흉내 낸다")
        });
        dispatch.wait_for_worker_exit();

        let outcome = dispatch.pump(&mut state).expect("죽은 워커도 결과를 낸다");

        match outcome {
            Outcome::Failure(failure) => {
                assert_eq!(failure.error.code, WORKER_PANICKED_ERROR_CODE);
            }
            other => panic!("실패 봉투를 기대했지만 {other:?}를 받았다"),
        }
    }

    #[test]
    fn 아직_끝나지_않은_실행은_결과를_내지_않는다() {
        let (mut state, run) = running_session(TEST_TOOL_ID);
        let (release_sender, release) = mpsc::channel::<()>();
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, move || {
            let _ = release.recv();
            Outcome::NotFound
        });

        assert!(dispatch.pump(&mut state).is_none());

        let _ = release_sender.send(());
        dispatch.wait_for_worker_exit();
    }

    #[test]
    fn 워커가_보낸_출력은_그_run의_tail에_쌓인다() {
        let (mut state, run) = running_session(TEST_TOOL_ID);
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, || {
            let reporter =
                upeg_runtime::ProgressReporter::capture().expect("진행 범위가 설치되어야 한다");
            reporter.report(
                upeg_runtime::ProgressStream::Stdout,
                "작업 중\n".to_string(),
            );
            Outcome::NotFound
        });
        dispatch.wait_for_worker_exit();

        assert!(dispatch.pump(&mut state).is_some());

        // pump 이후 view는 Result로 넘어가지 않는다(그건 update의 몫)이므로
        // tail이 그대로 남아 있어야 한다.
        match &state.view {
            View::Running { tail, .. } => {
                assert_eq!(tail.lines().collect::<Vec<_>>(), ["작업 중"]);
            }
            other => panic!("Running 보기를 기대했지만 {other:?}를 받았다"),
        }
    }

    #[test]
    fn 모델이_더_이상_살아_있다고_보지_않는_run의_출력은_버려진다() {
        let (mut state, run) = running_session(TEST_TOOL_ID);
        // 최종 봉투가 이미 도착해 모델이 run을 놓아준 상황.
        state.active_run = None;
        let dispatch = RunningDispatch::spawn_with(run, TEST_TOOL_ID, || {
            let reporter =
                upeg_runtime::ProgressReporter::capture().expect("진행 범위가 설치되어야 한다");
            reporter.report(
                upeg_runtime::ProgressStream::Stdout,
                "늦게 온 출력\n".to_string(),
            );
            Outcome::NotFound
        });
        dispatch.wait_for_worker_exit();

        let _ = dispatch.pump(&mut state);

        match &state.view {
            View::Running { tail, .. } => assert!(tail.is_empty(), "지난 run의 출력은 버린다"),
            other => panic!("Running 보기를 기대했지만 {other:?}를 받았다"),
        }
    }
}
