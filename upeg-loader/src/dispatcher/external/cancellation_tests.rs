//! Cancellation from the outside: an installed
//! [`upeg_runtime::CancellationToken`] stops a running child, and the
//! caller gets a typed `cancelled` envelope instead of a timeout or a
//! silent success.
//!
//! The assertions are on **timing plus disposition**. A run that
//! eventually returns the right envelope after the child finished on its
//! own would prove nothing, so every child here would outlive the test
//! by half a minute if nothing killed it.

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use upeg_core::ToolResult;
use upeg_runtime::{CancellationToken, DispatchArgs, with_cancellation};

use super::capture::{self, RunBudget, RunControls};
use super::error::{CaptureCompletion, CaptureLimit, CapturedOutput};
use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;

const GENEROUS_CAPTURE_LIMIT: u64 = 1024 * 1024;
/// How long the child is given to be visibly alive before the token
/// fires. Comfortably more than the capture layer's 10 ms poll tick.
const WAIT_BEFORE_CANCEL: Duration = Duration::from_millis(200);
/// Upper bound on how long a cancelled run may take to come back. A
/// hang detector: the child itself would run for 30 seconds.
const MAX_RETURN_TIME: Duration = Duration::from_secs(5);
const LONG_LIVED_CHILD_SECS: u64 = 30;

/// Canonical `error.code` and `error.details` key the cancelled envelope
/// carries. Spelled here so the test fails if either drifts.
const CANCEL_CODE: &str = "cancelled";
const CANCEL_DETAIL_KEY: &str = "cancelled";
const EXIT_CODE_DETAIL_KEY: &str = "exit_code";

fn long_lived_command() -> Command {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("/bin/echo alive; sleep {LONG_LIVED_CHILD_SECS}"),
    ]);
    command
}

fn run(command: &mut Command, token: CancellationToken) -> CapturedOutput {
    capture::run(
        command,
        CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
        RunBudget::unbounded(),
        RunControls::cancelled_by(token),
    )
    .expect("cancellation is not a capture error")
}

#[test]
fn cancelling_mid_run_stops_child_immediately() {
    // Given
    let token = CancellationToken::new();
    let mut command = long_lived_command();
    let start = Instant::now();

    // When: the capture runs on a worker so this thread can cancel while
    // the child is still alive.
    let runner = {
        let token = token.clone();
        thread::spawn(move || run(&mut command, token))
    };
    thread::sleep(WAIT_BEFORE_CANCEL);
    token.cancel();
    let captured = runner
        .join()
        .expect("a cancelled run still returns a result");
    let elapsed = start.elapsed();

    // Then
    assert!(
        matches!(captured.completion, CaptureCompletion::Cancelled),
        "a cancelled run ends as cancelled: {:?}",
        captured.completion
    );
    assert!(
        elapsed < MAX_RETURN_TIME,
        "held onto the child for {elapsed:?} after cancelling"
    );
    assert_eq!(
        String::from_utf8_lossy(&captured.stdout),
        "alive\n",
        "what it wrote before dying stays intact"
    );
}

#[test]
fn already_cancelled_token_stops_child_almost_immediately() {
    // Given
    let token = CancellationToken::new();
    token.cancel();
    let mut command = long_lived_command();

    // When
    let start = Instant::now();
    let captured = run(&mut command, token);
    let elapsed = start.elapsed();

    // Then
    assert!(matches!(captured.completion, CaptureCompletion::Cancelled));
    assert!(
        elapsed < MAX_RETURN_TIME,
        "waited {elapsed:?} even though the token was already cancelled"
    );
}

#[test]
fn without_cancellation_run_ends_normally() {
    // The control: installing a token that never fires must not change
    // a single thing about an ordinary run.
    let mut command = Command::new("printf");
    command.args(["%s", "ok"]);

    let captured = run(&mut command, CancellationToken::new());

    assert!(matches!(
        captured.completion,
        CaptureCompletion::Exited(status) if status.success()
    ));
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "ok");
}

fn long_lived_tool() -> ToolToml {
    ToolToml {
        id: "test.slow".to_string(),
        toolkit: "test".to_string(),
        invoker: Some("External".to_string()),
        command: Some("sh".to_string()),
        args_template: Some(vec![
            "-c".to_string(),
            format!("sleep {LONG_LIVED_CHILD_SECS}"),
        ]),
        ..ToolToml::default()
    }
}

#[test]
fn cancelling_via_ambient_token_returns_cancelled_failure_envelope() {
    // Given
    let tool = long_lived_tool();
    let dispatcher = external_dispatcher_for(&tool, None).expect("External dispatcher");
    let token = CancellationToken::new();
    let canceller = {
        let token = token.clone();
        thread::spawn(move || {
            thread::sleep(WAIT_BEFORE_CANCEL);
            token.cancel();
        })
    };

    // When: the dispatcher never sees the token as an argument — it
    // reads the ambient scope the caller installed around the call.
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("empty object args");
    let start = Instant::now();
    let result = with_cancellation(token, || dispatcher(parsed));
    let elapsed = start.elapsed();
    canceller.join().expect("cancel thread finishes");

    // Then
    assert!(
        elapsed < MAX_RETURN_TIME,
        "dispatch took {elapsed:?} despite cancellation"
    );
    let ToolResult::Failure(failure) = result else {
        panic!("a cancelled run ends in a failure envelope");
    };
    assert_eq!(failure.error.code, CANCEL_CODE);
    let details = failure
        .error
        .details
        .expect("the cancelled envelope carries details");
    assert_eq!(details[CANCEL_DETAIL_KEY], Value::Bool(true));
    assert_eq!(
        details[EXIT_CODE_DETAIL_KEY],
        Value::Null,
        "a cancelled child had no chance to report an exit code"
    );
}
