use std::process::Command;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use super::process_test_support::lingering_process_state;
use super::{CaptureLimit, ExternalProcessError, run_command_with_limit};

const SMALL_CAPTURE_LIMIT: u64 = 4 * 1024;
const LARGE_STDERR_BYTES: usize = 96 * 1024;
#[cfg(target_os = "linux")]
const PROCESS_EXIT_WAIT: Duration = Duration::from_millis(250);

fn printf_command(payload: &str) -> Command {
    let mut command = Command::new("printf");
    command.args(["%s", payload]);
    command
}

#[test]
fn external_output_exactly_at_limit_is_allowed() {
    // Given
    let payload =
        "x".repeat(usize::try_from(SMALL_CAPTURE_LIMIT).expect("the test limit fits in usize"));
    let mut command = printf_command(&payload);

    // When
    let output = run_command_with_limit(&mut command, CaptureLimit::new(SMALL_CAPTURE_LIMIT))
        .expect("an exact boundary is allowed");

    // Then
    assert_eq!(
        u64::try_from(output.stdout.len()).expect("the output length fits in u64"),
        SMALL_CAPTURE_LIMIT
    );
}

#[test]
fn external_stdout_one_byte_over_limit_is_rejected() {
    // Given
    let payload =
        "x".repeat(usize::try_from(SMALL_CAPTURE_LIMIT + 1).expect("the test limit fits in usize"));
    let mut command = printf_command(&payload);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(SMALL_CAPTURE_LIMIT))
        .expect_err("cap + 1 output is rejected");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::StreamLimitExceeded {
            stream: super::OutputStream::Stdout,
            max: SMALL_CAPTURE_LIMIT
        }
    ));
}

#[test]
fn external_stdout_plus_stderr_over_limit_is_rejected() {
    // Given
    let cap = usize::try_from(SMALL_CAPTURE_LIMIT).expect("the test limit fits in usize");
    let stdout = "o".repeat(cap / 2 + 1);
    let stderr = "e".repeat(cap / 2);
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "printf '%s' \"$1\"; printf '%s' \"$2\" >&2",
        "upeg-test",
        &stdout,
        &stderr,
    ]);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(SMALL_CAPTURE_LIMIT))
        .expect_err("the sum of both streams must respect the limit");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::AggregateLimitExceeded {
            max: SMALL_CAPTURE_LIMIT
        }
    ));
}

#[test]
fn drains_large_stderr_concurrently_without_deadlock() {
    // Given
    let stderr = "e".repeat(LARGE_STDERR_BYTES);
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "printf '%s' \"$1\" >&2; printf ok",
        "upeg-test",
        &stderr,
    ]);
    let limit =
        CaptureLimit::new(u64::try_from(LARGE_STDERR_BYTES + 2).expect("test limit conversion"));

    // When
    let output =
        run_command_with_limit(&mut command, limit).expect("draining stderr alongside completes");

    // Then
    assert_eq!(output.stdout, b"ok");
    assert_eq!(output.stderr.len(), LARGE_STDERR_BYTES);
}

#[cfg(target_os = "linux")]
#[test]
fn kills_and_reaps_process_that_exceeded_output_limit() {
    // Given
    let pid_path =
        std::env::temp_dir().join(format!("upeg_external_cap_reap_{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&pid_path);
    let payload =
        "x".repeat(usize::try_from(SMALL_CAPTURE_LIMIT + 1).expect("the test limit fits in usize"));
    let script = format!(
        "echo $$ > '{}'; printf '%s' \"$1\"; while :; do :; done",
        pid_path.display()
    );
    let mut command = Command::new("sh");
    command.args(["-c", &script, "upeg-test", &payload]);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(SMALL_CAPTURE_LIMIT))
        .expect_err("an over-limit process fails");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::StreamLimitExceeded {
            stream: super::OutputStream::Stdout,
            ..
        }
    ));
    let pid = std::fs::read_to_string(&pid_path)
        .expect("the process writes its pid file")
        .trim()
        .parse::<u32>()
        .expect("parse the pid");
    assert!(
        !std::path::Path::new("/proc").join(pid.to_string()).exists(),
        "the over-limit process must be reaped"
    );
    std::fs::remove_file(pid_path).expect("clean up the test pid file");
}

#[cfg(target_os = "linux")]
#[test]
fn background_inheriting_opposite_pipe_dies_within_limit_on_cap_exceeded() {
    // Given
    const MAX_RETURN_TIME: Duration = Duration::from_secs(1);
    const BACKGROUND_LIFETIME_SECS: u64 = 2;
    let pid_path = std::env::temp_dir().join(format!(
        "upeg_external_descendant_reap_{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let payload =
        "x".repeat(usize::try_from(SMALL_CAPTURE_LIMIT + 1).expect("the test limit fits in usize"));
    let script = format!(
        "sh -c 'trap \"\" HUP TERM; exec sleep {BACKGROUND_LIFETIME_SECS}' upeg-background >&2 & \
         echo $! > '{}'; printf '%s' \"$1\"",
        pid_path.display()
    );
    let mut command = Command::new("sh");
    command.args(["-c", &script, "upeg-test", &payload]);

    // When
    let started = Instant::now();
    let error = run_command_with_limit(&mut command, CaptureLimit::new(SMALL_CAPTURE_LIMIT))
        .expect_err("over-limit output fails");
    let elapsed = started.elapsed();

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::StreamLimitExceeded {
            stream: super::OutputStream::Stdout,
            ..
        }
    ));
    assert!(
        elapsed < MAX_RETURN_TIME,
        "a background descendant's opposite pipe blocked return for {elapsed:?}"
    );
    let descendant = std::fs::read_to_string(&pid_path)
        .expect("the background descendant writes its pid file")
        .trim()
        .parse::<u32>()
        .expect("parse the descendant pid");
    let lingering_state = lingering_process_state(descendant, PROCESS_EXIT_WAIT);
    assert!(
        lingering_state.is_none(),
        "the background descendant must be killed too: pid={descendant}, status={lingering_state:?}"
    );
    std::fs::remove_file(pid_path).expect("clean up the test pid file");
}
