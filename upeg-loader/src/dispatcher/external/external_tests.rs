use std::process::Command;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use super::process_test_support::종료되지_않은_프로세스_상태;
use super::{CaptureLimit, ExternalProcessError, run_command_with_limit};

const 작은_캡처_한도: u64 = 4 * 1024;
const 큰_표준오류_바이트: usize = 96 * 1024;
#[cfg(target_os = "linux")]
const 프로세스_종료_대기: Duration = Duration::from_millis(250);

fn printf_명령(payload: &str) -> Command {
    let mut command = Command::new("printf");
    command.args(["%s", payload]);
    command
}

#[test]
fn 외부_출력이_정확히_한도이면_허용한다() {
    // Given
    let payload =
        "x".repeat(usize::try_from(작은_캡처_한도).expect("테스트 한도는 usize에 들어간다"));
    let mut command = printf_명령(&payload);

    // When
    let output = run_command_with_limit(&mut command, CaptureLimit::new(작은_캡처_한도))
        .expect("정확한 경계는 허용한다");

    // Then
    assert_eq!(
        u64::try_from(output.stdout.len()).expect("출력 길이는 u64에 들어간다"),
        작은_캡처_한도
    );
}

#[test]
fn 외부_표준출력이_한도를_한_바이트_넘으면_누적하지_않고_거부한다() {
    // Given
    let payload =
        "x".repeat(usize::try_from(작은_캡처_한도 + 1).expect("테스트 한도는 usize에 들어간다"));
    let mut command = printf_명령(&payload);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(작은_캡처_한도))
        .expect_err("cap + 1 출력은 거부한다");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::StreamLimitExceeded {
            stream: super::OutputStream::Stdout,
            max: 작은_캡처_한도
        }
    ));
}

#[test]
fn 외부_표준출력과_표준오류의_합계가_한도를_넘으면_거부한다() {
    // Given
    let 한도 = usize::try_from(작은_캡처_한도).expect("테스트 한도는 usize에 들어간다");
    let stdout = "o".repeat(한도 / 2 + 1);
    let stderr = "e".repeat(한도 / 2);
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "printf '%s' \"$1\"; printf '%s' \"$2\" >&2",
        "upeg-test",
        &stdout,
        &stderr,
    ]);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(작은_캡처_한도))
        .expect_err("두 스트림의 합계도 한도를 지켜야 한다");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::AggregateLimitExceeded {
            max: 작은_캡처_한도
        }
    ));
}

#[test]
fn 큰_표준오류를_동시에_비워_교착하지_않는다() {
    // Given
    let stderr = "e".repeat(큰_표준오류_바이트);
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "printf '%s' \"$1\" >&2; printf ok",
        "upeg-test",
        &stderr,
    ]);
    let limit = CaptureLimit::new(u64::try_from(큰_표준오류_바이트 + 2).expect("테스트 한도 변환"));

    // When
    let output =
        run_command_with_limit(&mut command, limit).expect("stderr를 함께 비우면 완료한다");

    // Then
    assert_eq!(output.stdout, b"ok");
    assert_eq!(output.stderr.len(), 큰_표준오류_바이트);
}

#[cfg(target_os = "linux")]
#[test]
fn 출력_한도_초과_프로세스를_종료하고_회수한다() {
    // Given
    let pid_path =
        std::env::temp_dir().join(format!("upeg_external_cap_reap_{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&pid_path);
    let payload =
        "x".repeat(usize::try_from(작은_캡처_한도 + 1).expect("테스트 한도는 usize에 들어간다"));
    let script = format!(
        "echo $$ > '{}'; printf '%s' \"$1\"; while :; do :; done",
        pid_path.display()
    );
    let mut command = Command::new("sh");
    command.args(["-c", &script, "upeg-test", &payload]);

    // When
    let error = run_command_with_limit(&mut command, CaptureLimit::new(작은_캡처_한도))
        .expect_err("한도 초과 프로세스는 실패한다");

    // Then
    assert!(matches!(
        error,
        ExternalProcessError::StreamLimitExceeded {
            stream: super::OutputStream::Stdout,
            ..
        }
    ));
    let pid = std::fs::read_to_string(&pid_path)
        .expect("프로세스가 pid 파일을 쓴다")
        .trim()
        .parse::<u32>()
        .expect("pid를 파싱한다");
    assert!(
        !std::path::Path::new("/proc").join(pid.to_string()).exists(),
        "한도 초과 프로세스가 회수되어야 한다"
    );
    std::fs::remove_file(pid_path).expect("테스트 pid 파일을 정리한다");
}

#[cfg(target_os = "linux")]
#[test]
fn 반대_pipe를_상속한_background_프로세스도_한도_초과시_제한시간_안에_종료한다() {
    // Given
    const 최대_반환_시간: Duration = Duration::from_secs(1);
    const BACKGROUND_수명_초: u64 = 2;
    let pid_path = std::env::temp_dir().join(format!(
        "upeg_external_descendant_reap_{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let payload =
        "x".repeat(usize::try_from(작은_캡처_한도 + 1).expect("테스트 한도는 usize에 들어간다"));
    let script = format!(
        "sh -c 'trap \"\" HUP TERM; exec sleep {BACKGROUND_수명_초}' upeg-background >&2 & \
         echo $! > '{}'; printf '%s' \"$1\"",
        pid_path.display()
    );
    let mut command = Command::new("sh");
    command.args(["-c", &script, "upeg-test", &payload]);

    // When
    let started = Instant::now();
    let error = run_command_with_limit(&mut command, CaptureLimit::new(작은_캡처_한도))
        .expect_err("한도 초과 출력은 실패한다");
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
        elapsed < 최대_반환_시간,
        "background descendant의 반대 pipe 때문에 {elapsed:?} 동안 반환하지 못했다"
    );
    let descendant = std::fs::read_to_string(&pid_path)
        .expect("background descendant가 pid 파일을 쓴다")
        .trim()
        .parse::<u32>()
        .expect("descendant pid를 파싱한다");
    let 잔존_상태 = 종료되지_않은_프로세스_상태(descendant, 프로세스_종료_대기);
    assert!(
        잔존_상태.is_none(),
        "background descendant도 종료되어야 한다: pid={descendant}, status={잔존_상태:?}"
    );
    std::fs::remove_file(pid_path).expect("테스트 pid 파일을 정리한다");
}
