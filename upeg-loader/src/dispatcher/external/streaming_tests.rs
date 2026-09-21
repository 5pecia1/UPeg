//! Proof that External output reaches a consumer *while the child is
//! still running*, not only when the final envelope is built.
//!
//! The whole point of the progress contract is timing, so these tests
//! assert on timing: a command that writes, sleeps, then writes again
//! must deliver its first chunk before it delivers its last, and both
//! before `run` returns.

use std::process::Command;
use std::time::{Duration, Instant};

use upeg_runtime::{ProgressReporter, ProgressStream, progress_channel};

use super::capture::{self, RunBudget, RunControls};
use super::error::{CaptureCompletion, CaptureLimit};

/// The child's pause between its two writes. Long enough that a
/// buffered-until-exit implementation cannot accidentally pass.
const MID_PAUSE: Duration = Duration::from_millis(300);
/// Upper bound on how late the first chunk may arrive and still count as
/// "live". Half the pause, so the assertion cannot flake into being
/// satisfied by the second write.
const LIVE_SLACK: Duration = Duration::from_millis(150);
const GENEROUS_CAPTURE_LIMIT: u64 = 1024 * 1024;

fn write_pause_write_command() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", "echo first; sleep 0.3; echo second 1>&2"]);
    command
}

#[test]
fn first_chunk_arrives_before_child_exits() {
    // Given
    let (sink, receiver) = progress_channel();
    let mut command = write_pause_write_command();
    let start = Instant::now();

    // When: the capture runs on a worker thread so this thread can watch
    // the channel while the child is still alive.
    let run_handle = std::thread::spawn(move || {
        capture::run(
            &mut command,
            CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
            RunBudget::unbounded(),
            RunControls::with_reporter(ProgressReporter::new(sink)),
        )
    });

    let first_chunk = receiver
        .recv_timeout(MID_PAUSE)
        .expect("the first line arrives while the child sleeps");
    let first_chunk_delay = start.elapsed();

    // Then
    assert_eq!(first_chunk.stream, ProgressStream::Stdout);
    assert_eq!(first_chunk.chunk, "first\n");
    assert_eq!(first_chunk.seq, 0);
    assert!(
        first_chunk_delay < LIVE_SLACK,
        "the first line took {first_chunk_delay:?} — it looks buffered until exit"
    );

    let second_chunk = receiver
        .recv_timeout(MID_PAUSE * 2)
        .expect("the second line arrives too");
    assert_eq!(second_chunk.stream, ProgressStream::Stderr);
    assert_eq!(second_chunk.chunk, "second\n");
    assert_eq!(
        second_chunk.seq, 1,
        "the sequence number continues across both streams"
    );

    let captured = run_handle
        .join()
        .expect("capture thread")
        .expect("capture succeeds");

    // And: streaming did not consume the capture — the final envelope
    // still carries both streams in full.
    assert!(matches!(
        captured.completion,
        CaptureCompletion::Exited(status) if status.success()
    ));
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "first\n");
    assert_eq!(String::from_utf8_lossy(&captured.stderr), "second\n");
}

#[test]
fn without_sink_capture_result_is_unchanged() {
    // Given
    let mut command = write_pause_write_command();

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
        RunBudget::unbounded(),
        RunControls::default(),
    )
    .expect("a run without a sink also succeeds");

    // Then
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "first\n");
    assert_eq!(String::from_utf8_lossy(&captured.stderr), "second\n");
}

#[test]
fn output_ending_without_newline_still_streams_at_the_end() {
    // Given
    let (sink, receiver) = progress_channel();
    let mut command = Command::new("printf");
    command.args(["%s", "no-newline-tail"]);

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
        RunBudget::unbounded(),
        RunControls::with_reporter(ProgressReporter::new(sink)),
    )
    .expect("capture succeeds");

    // Then
    let chunk = receiver.recv().expect("the tail is delivered too");
    assert_eq!(chunk.chunk, "no-newline-tail");
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "no-newline-tail");
}

#[test]
fn timed_out_child_still_delivers_what_it_wrote() {
    // Given: a child that prints, then hangs past its budget.
    let (sink, receiver) = progress_channel();
    let mut command = Command::new("sh");
    command.args(["-c", "echo alive; sleep 30"]);

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
        RunBudget::from_timeout(Some(MID_PAUSE)),
        RunControls::with_reporter(ProgressReporter::new(sink)),
    )
    .expect("a timeout is not a capture error");

    // Then
    assert!(matches!(
        captured.completion,
        CaptureCompletion::TimedOut { .. }
    ));
    let chunk = receiver
        .recv()
        .expect("the line written before dying already went out");
    assert_eq!(chunk.chunk, "alive\n");
}
