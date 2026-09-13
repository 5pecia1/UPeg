use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use upeg_runtime::{CancellationToken, ProgressReporter};

use super::cancel::DrainControl;
use super::child::{ChildGuard, ChildOutput};
use super::drain::{self, CaptureBudget, DrainEvent};
use super::error::{
    CaptureCompletion, CaptureLimit, CapturedOutput, ExternalProcessError, OutputStream,
};
use super::progress::ProgressForwarder;
use super::pty::TerminalMode;
use super::scope::InvocationScope;

const DRAIN_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// How long a finished child's streams may stay open before upeg
/// concludes something that escaped the process group is holding them,
/// and sweeps.
///
/// `pub(super)` so the pty tests can assert a run ends *because the
/// child exited*, not because this grace elapsed — the two are
/// indistinguishable from the outside except by the clock.
pub(super) const DRAIN_COMPLETION_GRACE: Duration = Duration::from_millis(250);
const DRAIN_STOP_CONFIRMATION_POLLS: u8 = 2;

/// Wall-clock budget for one External invocation.
///
/// There is deliberately no default: a manifest that declares no
/// `timeout_ms` must be able to run `just verify` for twenty minutes.
/// Only an explicit declaration arms the deadline.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RunBudget {
    timeout: Option<Duration>,
}

impl RunBudget {
    #[cfg(test)]
    pub(super) const fn unbounded() -> Self {
        Self { timeout: None }
    }

    pub(super) const fn from_timeout(timeout: Option<Duration>) -> Self {
        Self { timeout }
    }

    fn deadline(self) -> Option<Instant> {
        self.timeout.map(|timeout| Instant::now() + timeout)
    }

    fn timeout_ms(self) -> u64 {
        self.timeout.map_or(0, |timeout| {
            u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX)
        })
    }
}

/// The optional capabilities one invocation runs under, and the shape
/// of its output.
///
/// All three are "nobody asked" by default, and all three cost the child
/// nothing then: no progress forwarding, no cancellation polling, two
/// ordinary pipes. Grouped into one value so the capture entry point
/// keeps a readable signature as capabilities accumulate.
#[derive(Default)]
pub(super) struct RunControls {
    /// Live-output tap — see [`super::progress`].
    reporter: Option<ProgressReporter>,
    /// The caller's ambient cancellation token — see
    /// [`upeg_runtime::with_cancellation`].
    cancellation: Option<CancellationToken>,
    /// Pipes or pseudoterminal — see [`super::pty`].
    terminal: TerminalMode,
}

impl RunControls {
    pub(super) const fn new(
        reporter: Option<ProgressReporter>,
        cancellation: Option<CancellationToken>,
        terminal: TerminalMode,
    ) -> Self {
        Self {
            reporter,
            cancellation,
            terminal,
        }
    }

    #[cfg(test)]
    pub(super) const fn with_reporter(reporter: ProgressReporter) -> Self {
        Self {
            reporter: Some(reporter),
            cancellation: None,
            terminal: TerminalMode::Pipes,
        }
    }

    #[cfg(all(test, unix))]
    pub(super) const fn on_pty() -> Self {
        Self {
            reporter: None,
            cancellation: None,
            terminal: TerminalMode::Pty,
        }
    }

    #[cfg(test)]
    pub(super) const fn cancelled_by(token: CancellationToken) -> Self {
        Self {
            reporter: None,
            cancellation: Some(token),
            terminal: TerminalMode::Pipes,
        }
    }
}

/// Why upeg stopped a child that had not finished on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopCause {
    /// The declared `timeout_ms` elapsed.
    Deadline,
    /// The ambient cancellation token fired.
    Cancellation,
}

/// What `await_events` observed while the child ran.
struct DrainOutcome {
    error: Option<ExternalProcessError>,
    stopped: Option<StopCause>,
}

/// One stream being drained, and the thread doing it.
///
/// A run has two of these for the pipe path and exactly one for the pty
/// path, which is why the wait loop is written over a slice instead of
/// over a pair of named locals.
struct Drain {
    stream: OutputStream,
    handle: JoinHandle<Option<Vec<u8>>>,
    /// Whether this stream has already reported its terminal event.
    reported: bool,
    /// Consecutive polls that found the thread finished without an
    /// event. Confirmed over several polls so a thread that has sent its
    /// event but not yet been observed is not mistaken for a dead one.
    stopped_polls: u8,
}

/// Both captured streams, however many drains produced them.
struct CapturedStreams {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// What the drain threads handed back, before the "a stream nobody
/// finished is a broken run" rule is applied.
///
/// A stream starts out as an empty success rather than as a missing one:
/// the pty path has no `stderr` drain at all, and an empty stderr is the
/// truthful answer for it — not a stopped thread.
struct JoinedStreams {
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
}

impl JoinedStreams {
    fn empty() -> Self {
        Self {
            stdout: Some(Vec::new()),
            stderr: Some(Vec::new()),
        }
    }

    fn set(&mut self, stream: OutputStream, captured: Option<Vec<u8>>) {
        match stream {
            OutputStream::Stdout => self.stdout = captured,
            OutputStream::Stderr => self.stderr = captured,
        }
    }

    /// Both streams, or the failure for whichever drain ended without
    /// producing one.
    fn require(self) -> Result<CapturedStreams, ExternalProcessError> {
        Ok(CapturedStreams {
            stdout: self
                .stdout
                .ok_or(ExternalProcessError::DrainThreadStopped {
                    stream: OutputStream::Stdout,
                })?,
            stderr: self
                .stderr
                .ok_or(ExternalProcessError::DrainThreadStopped {
                    stream: OutputStream::Stderr,
                })?,
        })
    }
}

/// Run one child under bounded, deadlock-free, group-terminating
/// capture.
///
/// `controls` carries the optional capabilities (see [`RunControls`]);
/// the all-default case — no progress consumer, no cancellation, two
/// pipes — is the common one and behaves exactly as it always has.
pub(super) fn run(
    command: &mut Command,
    limit: CaptureLimit,
    run_budget: RunBudget,
    controls: RunControls,
) -> Result<CapturedOutput, ExternalProcessError> {
    let (mut child, output) = ChildGuard::spawn(command, controls.terminal)?;
    let scope = InvocationScope::new(&output)?;

    let budget = Arc::new(CaptureBudget::new(limit));
    let control = Arc::new(DrainControl::new());
    let (events, receiver) = mpsc::channel();
    let mut drains = spawn_drains(output, limit, &budget, &control, &events, controls.reporter);
    // The parent must not keep a sender: a disconnected channel is how
    // the wait loop learns that every drain thread is gone.
    drop(events);

    let capture_result = await_events(
        &mut child,
        &scope,
        &control,
        &receiver,
        &mut drains,
        run_budget.deadline(),
        controls.cancellation.as_ref(),
    );
    let joined = join_drains(drains);
    let outcome = capture_result?;
    let joined = joined?;
    // The drain's own reported failure — an exceeded output cap, say —
    // outranks the fact that the stream it was reading therefore has no
    // captured bytes: the first is the diagnosis, the second is its
    // consequence.
    if let Some(error) = outcome.error {
        return Err(error);
    }
    let streams = joined.require()?;
    // A child stopped by upeg was already terminated and reaped while
    // the deadline (or the cancellation) fired, so `wait` must not run
    // again for it.
    let completion = match outcome.stopped {
        Some(StopCause::Deadline) => CaptureCompletion::TimedOut {
            timeout_ms: run_budget.timeout_ms(),
        },
        Some(StopCause::Cancellation) => CaptureCompletion::Cancelled,
        None => CaptureCompletion::Exited(child.wait()?),
    };
    Ok(CapturedOutput {
        completion,
        stdout: streams.stdout,
        stderr: streams.stderr,
    })
}

/// Start one drain thread per stream the child actually has.
///
/// The pty path is a single drain deliberately tagged
/// [`OutputStream::Stdout`]: a terminal has one buffer, so what comes
/// back is both of the child's streams already interleaved, and every
/// consumer downstream — capture caps, progress forwarding, the failure
/// envelope — reads it as stdout.
fn spawn_drains(
    output: ChildOutput,
    limit: CaptureLimit,
    budget: &Arc<CaptureBudget>,
    control: &Arc<DrainControl>,
    events: &mpsc::Sender<DrainEvent>,
    reporter: Option<ProgressReporter>,
) -> Vec<Drain> {
    let shared = SharedDrainState {
        limit,
        budget,
        control,
        events,
        reporter,
    };
    match output {
        ChildOutput::Pipes { stdout, stderr } => vec![
            shared.start(stdout, OutputStream::Stdout),
            shared.start(stderr, OutputStream::Stderr),
        ],
        #[cfg(unix)]
        ChildOutput::Merged { terminal } => vec![shared.start(terminal, OutputStream::Stdout)],
    }
}

/// Everything every drain thread of one invocation shares. Exists so
/// [`SharedDrainState::start`] can be generic over the reader type,
/// which a closure cannot be.
struct SharedDrainState<'a> {
    limit: CaptureLimit,
    budget: &'a Arc<CaptureBudget>,
    control: &'a Arc<DrainControl>,
    events: &'a mpsc::Sender<DrainEvent>,
    reporter: Option<ProgressReporter>,
}

impl SharedDrainState<'_> {
    fn start<R: drain::DrainReader + 'static>(&self, reader: R, stream: OutputStream) -> Drain {
        Drain {
            stream,
            handle: drain::spawn(
                reader,
                stream,
                self.limit,
                Arc::clone(self.budget),
                Arc::clone(self.control),
                self.events.clone(),
                ProgressForwarder::new(self.reporter.clone(), stream),
            ),
            reported: false,
            stopped_polls: 0,
        }
    }
}

/// Join every drain thread and sort what they read into the two
/// canonical streams. A stream nobody drained stays empty — that is the
/// pty path's `stderr`.
///
/// Every thread is joined even after one of them has failed: leaving a
/// reader attached to a dead child's stream is how a "returns quickly"
/// guarantee turns into a leak.
fn join_drains(drains: Vec<Drain>) -> Result<JoinedStreams, ExternalProcessError> {
    let mut joined = JoinedStreams::empty();
    let mut first_error = None;
    for drain in drains {
        let stream = drain.stream;
        match drain::join(drain.handle, stream) {
            Ok(captured) => joined.set(stream, captured),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    first_error.map_or(Ok(joined), Err)
}

fn await_events(
    child: &mut ChildGuard,
    scope: &InvocationScope,
    control: &DrainControl,
    receiver: &mpsc::Receiver<DrainEvent>,
    drains: &mut [Drain],
    deadline: Option<Instant>,
    cancellation: Option<&CancellationToken>,
) -> Result<DrainOutcome, ExternalProcessError> {
    let mut first_error = None;
    let mut stopped = None;
    let mut parent_reaped = false;
    let mut escaped_sweep_deadline = None;
    while drains.iter().any(|drain| !drain.reported) {
        match receiver.recv_timeout(DRAIN_EVENT_POLL_INTERVAL) {
            Ok(DrainEvent::Complete(stream)) => mark_reported(drains, stream),
            Ok(DrainEvent::Failed(stream, error)) => {
                mark_reported(drains, stream);
                if first_error.is_none() {
                    stop_and_cancel(child, scope, control)?;
                    parent_reaped = true;
                    escaped_sweep_deadline = None;
                    first_error = Some(error);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if !parent_reaped
                    && stopped.is_none()
                    && let Some(cause) = stop_cause(deadline, cancellation)
                {
                    stop_and_cancel(child, scope, control)?;
                    parent_reaped = true;
                    escaped_sweep_deadline = None;
                    stopped = Some(cause);
                }
                if escaped_sweep_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    terminate_escaped_and_cancel(scope, control)?;
                    escaped_sweep_deadline = None;
                }
                if !parent_reaped && child.poll_direct_exit()? {
                    terminate_contained_processes(child, control)?;
                    parent_reaped = true;
                    escaped_sweep_deadline = Some(Instant::now() + DRAIN_COMPLETION_GRACE);
                }
                if let Some(stream) = confirmed_stopped_drain(drains) {
                    stop_and_cancel(child, scope, control)?;
                    return Err(ExternalProcessError::DrainThreadStopped { stream });
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                stop_and_cancel(child, scope, control)?;
                let stream = drains
                    .iter()
                    .find(|drain| !drain.reported)
                    .map_or(OutputStream::Stdout, |drain| drain.stream);
                return Err(ExternalProcessError::DrainThreadStopped { stream });
            }
        }
    }
    Ok(DrainOutcome {
        error: first_error,
        stopped,
    })
}

/// Why the child should be stopped right now, if it should be at all.
///
/// Cancellation is checked first: when a caller has hung up *and* the
/// deadline happens to elapse in the same 10 ms tick, the cause the
/// operator asked for is the honest one to report.
fn stop_cause(
    deadline: Option<Instant>,
    cancellation: Option<&CancellationToken>,
) -> Option<StopCause> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Some(StopCause::Cancellation);
    }
    deadline
        .is_some_and(|deadline| Instant::now() >= deadline)
        .then_some(StopCause::Deadline)
}

/// The stream whose drain thread has been finished-without-reporting for
/// [`DRAIN_STOP_CONFIRMATION_POLLS`] consecutive polls.
fn confirmed_stopped_drain(drains: &mut [Drain]) -> Option<OutputStream> {
    for drain in drains.iter_mut() {
        if drain.handle.is_finished() && !drain.reported {
            drain.stopped_polls += 1;
            if drain.stopped_polls >= DRAIN_STOP_CONFIRMATION_POLLS {
                return Some(drain.stream);
            }
        } else {
            drain.stopped_polls = 0;
        }
    }
    None
}

fn stop_and_cancel(
    child: &mut ChildGuard,
    scope: &InvocationScope,
    control: &DrainControl,
) -> Result<(), ExternalProcessError> {
    let termination = child.terminate_and_reap();
    let scope_termination = scope.terminate_stream_holders();
    let cancellation = cancel_drains(control);
    termination?;
    scope_termination?;
    cancellation
}

fn terminate_contained_processes(
    child: &mut ChildGuard,
    control: &DrainControl,
) -> Result<(), ExternalProcessError> {
    if let Err(error) = child.terminate_and_reap() {
        let _ = control.cancel();
        return Err(error);
    }
    Ok(())
}

fn terminate_escaped_and_cancel(
    scope: &InvocationScope,
    control: &DrainControl,
) -> Result<(), ExternalProcessError> {
    let scope_termination = scope.terminate_stream_holders();
    let cancellation = cancel_drains(control);
    scope_termination?;
    cancellation
}

fn cancel_drains(control: &DrainControl) -> Result<(), ExternalProcessError> {
    control
        .cancel()
        .map_err(|source| ExternalProcessError::DrainCancel { source })
}

fn mark_reported(drains: &mut [Drain], stream: OutputStream) {
    if let Some(drain) = drains.iter_mut().find(|drain| drain.stream == stream) {
        drain.reported = true;
    }
}
