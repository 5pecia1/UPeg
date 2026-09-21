#![cfg(unix)]

use std::process::Command;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use super::process_test_support::lingering_process_state;
#[cfg(target_os = "linux")]
use super::scope::{reset_scope_scan_count, scope_scan_count};
use super::{CaptureLimit, run_command_with_limit};

const TEST_CAPTURE_LIMIT: u64 = 4 * 1024;
const MAX_RETURN_TIME: Duration = Duration::from_secs(1);
const BACKGROUND_LIFETIME_SECS: u64 = 2;
#[cfg(target_os = "linux")]
const ESCAPED_PROCESS_LIFETIME_SECS: u64 = 30;
#[cfg(target_os = "linux")]
const ESCAPED_PROCESS_EXIT_WAIT: Duration = Duration::from_secs(1);
#[cfg(target_os = "linux")]
const CONCURRENT_RUNS: usize = 16;

#[test]
fn returns_within_limit_when_background_inherits_exited_parents_pipe() {
    // Given
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("sleep {BACKGROUND_LIFETIME_SECS} >&2 & printf ok"),
    ]);

    // When
    let started = Instant::now();
    let output = run_command_with_limit(&mut command, CaptureLimit::new(TEST_CAPTURE_LIMIT))
        .expect("return output of a normally exited parent");
    let elapsed = started.elapsed();

    // Then
    assert_eq!(output.stdout, b"ok");
    assert!(
        elapsed < MAX_RETURN_TIME,
        "a background descendant's pipe blocked return for {elapsed:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn concurrent_exited_parents_do_not_trigger_scope_scan() {
    // Given
    let barrier = Arc::new(Barrier::new(CONCURRENT_RUNS));
    let handles = (0..CONCURRENT_RUNS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                reset_scope_scan_count();
                let mut command = Command::new("sh");
                command.args([
                    "-c",
                    &format!("sleep {BACKGROUND_LIFETIME_SECS} >&2 & printf ok"),
                ]);
                barrier.wait();

                // When
                let output =
                    run_command_with_limit(&mut command, CaptureLimit::new(TEST_CAPTURE_LIMIT))
                        .expect("return output of concurrent normally exited parents");

                (output.stdout, scope_scan_count())
            })
        })
        .collect::<Vec<_>>();

    // Then
    for handle in handles {
        let (stdout, scans) = handle.join().expect("concurrent run thread completes");
        assert_eq!(stdout, b"ok");
        assert_eq!(
            scans, 0,
            "the normal-exit path must skip the /proc scope scan"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn returns_within_limit_when_escaped_background_holds_pipe() {
    // Given
    let pid_path = std::env::temp_dir().join(format!(
        "upeg_external_escaped_descendant_{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let mut command = Command::new("sh");
    let script = format!(
        "setsid sh -c 'exec sleep {ESCAPED_PROCESS_LIFETIME_SECS}' >&2 & \
         echo $! > '{}'; printf ok",
        pid_path.display()
    );
    command.args(["-c", &script]);

    // When
    let started = Instant::now();
    let output = run_command_with_limit(&mut command, CaptureLimit::new(TEST_CAPTURE_LIMIT))
        .expect("return parent output regardless of a descendant outside the process group");
    let elapsed = started.elapsed();

    // Then
    assert_eq!(output.stdout, b"ok");
    assert!(
        elapsed < MAX_RETURN_TIME,
        "a pipe writer outside the process group blocked return for {elapsed:?}"
    );
    let escaped_pid = std::fs::read_to_string(&pid_path)
        .expect("the escaped descendant writes its pid file")
        .trim()
        .parse::<u32>()
        .expect("parse the escaped descendant pid");
    let lingering_state = lingering_process_state(escaped_pid, ESCAPED_PROCESS_EXIT_WAIT);
    if lingering_state.is_some() {
        let _ = Command::new("kill")
            .args(["-KILL", &escaped_pid.to_string()])
            .status();
    }
    std::fs::remove_file(pid_path).expect("clean up the escaped descendant pid file");
    assert!(
        lingering_state.is_none(),
        "a descendant outside the process group must be killed too: pid={escaped_pid}, status={lingering_state:?}"
    );
}
