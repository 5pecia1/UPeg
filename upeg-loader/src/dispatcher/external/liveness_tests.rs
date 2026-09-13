#![cfg(unix)]

use std::process::Command;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use super::process_test_support::종료되지_않은_프로세스_상태;
#[cfg(target_os = "linux")]
use super::scope::{reset_scope_scan_count, scope_scan_count};
use super::{CaptureLimit, run_command_with_limit};

const 테스트_캡처_한도: u64 = 4 * 1024;
const 최대_반환_시간: Duration = Duration::from_secs(1);
const BACKGROUND_수명_초: u64 = 2;
#[cfg(target_os = "linux")]
const ESCAPED_프로세스_수명_초: u64 = 30;
#[cfg(target_os = "linux")]
const ESCAPED_프로세스_종료_대기: Duration = Duration::from_secs(1);
#[cfg(target_os = "linux")]
const 동시_실행_수: usize = 16;

#[test]
fn 정상_종료한_parent의_pipe를_background가_상속해도_제한시간_안에_반환한다() {
    // Given
    let mut command = Command::new("sh");
    command.args(["-c", &format!("sleep {BACKGROUND_수명_초} >&2 & printf ok")]);

    // When
    let started = Instant::now();
    let output = run_command_with_limit(&mut command, CaptureLimit::new(테스트_캡처_한도))
        .expect("정상 종료한 parent의 출력을 반환한다");
    let elapsed = started.elapsed();

    // Then
    assert_eq!(output.stdout, b"ok");
    assert!(
        elapsed < 최대_반환_시간,
        "background descendant의 pipe 때문에 {elapsed:?} 동안 반환하지 못했다"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn 정상_종료한_parent를_동시에_실행해도_scope_scan을_시작하지_않는다() {
    // Given
    let barrier = Arc::new(Barrier::new(동시_실행_수));
    let handles = (0..동시_실행_수)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                reset_scope_scan_count();
                let mut command = Command::new("sh");
                command.args(["-c", &format!("sleep {BACKGROUND_수명_초} >&2 & printf ok")]);
                barrier.wait();

                // When
                let output =
                    run_command_with_limit(&mut command, CaptureLimit::new(테스트_캡처_한도))
                        .expect("동시에 실행한 정상 parent의 출력을 반환한다");

                (output.stdout, scope_scan_count())
            })
        })
        .collect::<Vec<_>>();

    // Then
    for handle in handles {
        let (stdout, scans) = handle.join().expect("동시 실행 thread가 완료된다");
        assert_eq!(stdout, b"ok");
        assert_eq!(
            scans, 0,
            "정상 종료 경로는 /proc scope scan을 생략해야 한다"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn process_group을_벗어난_background가_pipe를_유지해도_제한시간_안에_반환한다() {
    // Given
    let pid_path = std::env::temp_dir().join(format!(
        "upeg_external_escaped_descendant_{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let mut command = Command::new("sh");
    let script = format!(
        "setsid sh -c 'exec sleep {ESCAPED_프로세스_수명_초}' >&2 & \
         echo $! > '{}'; printf ok",
        pid_path.display()
    );
    command.args(["-c", &script]);

    // When
    let started = Instant::now();
    let output = run_command_with_limit(&mut command, CaptureLimit::new(테스트_캡처_한도))
        .expect("process group을 벗어난 후손과 무관하게 parent 출력을 반환한다");
    let elapsed = started.elapsed();

    // Then
    assert_eq!(output.stdout, b"ok");
    assert!(
        elapsed < 최대_반환_시간,
        "process group을 벗어난 pipe writer 때문에 {elapsed:?} 동안 반환하지 못했다"
    );
    let escaped_pid = std::fs::read_to_string(&pid_path)
        .expect("escaped descendant가 pid 파일을 쓴다")
        .trim()
        .parse::<u32>()
        .expect("escaped descendant pid를 파싱한다");
    let 잔존_상태 =
        종료되지_않은_프로세스_상태(escaped_pid, ESCAPED_프로세스_종료_대기);
    if 잔존_상태.is_some() {
        let _ = Command::new("kill")
            .args(["-KILL", &escaped_pid.to_string()])
            .status();
    }
    std::fs::remove_file(pid_path).expect("escaped descendant pid 파일을 정리한다");
    assert!(
        잔존_상태.is_none(),
        "process group을 벗어난 descendant도 종료되어야 한다: pid={escaped_pid}, status={잔존_상태:?}"
    );
}
