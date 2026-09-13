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

const 넉넉한_캡처_한도: u64 = 1024 * 1024;
/// How long the child is given to be visibly alive before the token
/// fires. Comfortably more than the capture layer's 10 ms poll tick.
const 취소까지_대기: Duration = Duration::from_millis(200);
/// Upper bound on how long a cancelled run may take to come back. A
/// hang detector: the child itself would run for 30 seconds.
const 최대_반환_시간: Duration = Duration::from_secs(5);
const 오래_사는_자식_초: u64 = 30;

/// Canonical `error.code` and `error.details` key the cancelled envelope
/// carries. Spelled here so the test fails if either drifts.
const 취소_코드: &str = "cancelled";
const 취소_상세_키: &str = "cancelled";
const 종료코드_상세_키: &str = "exit_code";

fn 오래_사는_명령() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", &format!("/bin/echo alive; sleep {오래_사는_자식_초}")]);
    command
}

fn 실행(command: &mut Command, token: CancellationToken) -> CapturedOutput {
    capture::run(
        command,
        CaptureLimit::new(넉넉한_캡처_한도),
        RunBudget::unbounded(),
        RunControls::cancelled_by(token),
    )
    .expect("취소는 캡처 오류가 아니다")
}

#[test]
fn 실행_중_취소하면_자식이_곧바로_종료된다() {
    // Given
    let token = CancellationToken::new();
    let mut command = 오래_사는_명령();
    let 시작 = Instant::now();

    // When: the capture runs on a worker so this thread can cancel while
    // the child is still alive.
    let 실행자 = {
        let token = token.clone();
        thread::spawn(move || 실행(&mut command, token))
    };
    thread::sleep(취소까지_대기);
    token.cancel();
    let captured = 실행자.join().expect("취소된 실행도 결과를 돌려준다");
    let 걸린_시간 = 시작.elapsed();

    // Then
    assert!(
        matches!(captured.completion, CaptureCompletion::Cancelled),
        "취소된 실행은 취소로 끝난다: {:?}",
        captured.completion
    );
    assert!(
        걸린_시간 < 최대_반환_시간,
        "취소했는데 {걸린_시간:?} 동안 자식을 붙들고 있었다"
    );
    assert_eq!(
        String::from_utf8_lossy(&captured.stdout),
        "alive\n",
        "죽기 전에 쓴 것은 그대로 남는다"
    );
}

#[test]
fn 이미_취소된_토큰이면_자식은_거의_바로_멈춘다() {
    // Given
    let token = CancellationToken::new();
    token.cancel();
    let mut command = 오래_사는_명령();

    // When
    let 시작 = Instant::now();
    let captured = 실행(&mut command, token);
    let 걸린_시간 = 시작.elapsed();

    // Then
    assert!(matches!(captured.completion, CaptureCompletion::Cancelled));
    assert!(
        걸린_시간 < 최대_반환_시간,
        "이미 취소된 토큰인데 {걸린_시간:?} 동안 기다렸다"
    );
}

#[test]
fn 취소하지_않으면_평소처럼_끝난다() {
    // The control: installing a token that never fires must not change
    // a single thing about an ordinary run.
    let mut command = Command::new("printf");
    command.args(["%s", "ok"]);

    let captured = 실행(&mut command, CancellationToken::new());

    assert!(matches!(
        captured.completion,
        CaptureCompletion::Exited(status) if status.success()
    ));
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "ok");
}

fn 오래_사는_도구() -> ToolToml {
    ToolToml {
        id: "test.slow".to_string(),
        toolkit: "test".to_string(),
        invoker: Some("External".to_string()),
        command: Some("sh".to_string()),
        args_template: Some(vec!["-c".to_string(), format!("sleep {오래_사는_자식_초}")]),
        ..ToolToml::default()
    }
}

#[test]
fn 주변_토큰으로_취소하면_봉투가_취소_실패다() {
    // Given
    let tool = 오래_사는_도구();
    let dispatcher = external_dispatcher_for(&tool, None).expect("External dispatcher");
    let token = CancellationToken::new();
    let 취소자 = {
        let token = token.clone();
        thread::spawn(move || {
            thread::sleep(취소까지_대기);
            token.cancel();
        })
    };

    // When: the dispatcher never sees the token as an argument — it
    // reads the ambient scope the caller installed around the call.
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("빈 객체 인자");
    let 시작 = Instant::now();
    let result = with_cancellation(token, || dispatcher(parsed));
    let 걸린_시간 = 시작.elapsed();
    취소자.join().expect("취소 스레드가 끝난다");

    // Then
    assert!(
        걸린_시간 < 최대_반환_시간,
        "취소했는데 dispatch가 {걸린_시간:?} 걸렸다"
    );
    let ToolResult::Failure(failure) = result else {
        panic!("취소된 실행은 실패 봉투로 끝난다");
    };
    assert_eq!(failure.error.code, 취소_코드);
    let details = failure.error.details.expect("취소 봉투는 상세를 싣는다");
    assert_eq!(details[취소_상세_키], Value::Bool(true));
    assert_eq!(
        details[종료코드_상세_키],
        Value::Null,
        "취소된 자식은 종료 코드를 보고할 기회가 없었다"
    );
}
