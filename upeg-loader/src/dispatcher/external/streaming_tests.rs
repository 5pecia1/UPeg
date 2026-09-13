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
const 중간_휴지: Duration = Duration::from_millis(300);
/// Upper bound on how late the first chunk may arrive and still count as
/// "live". Half the pause, so the assertion cannot flake into being
/// satisfied by the second write.
const 생중계_여유: Duration = Duration::from_millis(150);
const 넉넉한_캡처_한도: u64 = 1024 * 1024;

fn 두_번_쓰고_사이에_쉬는_명령() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", "echo first; sleep 0.3; echo second 1>&2"]);
    command
}

#[test]
fn 첫_청크는_자식이_끝나기_전에_도착한다() {
    // Given
    let (sink, receiver) = progress_channel();
    let mut command = 두_번_쓰고_사이에_쉬는_명령();
    let 시작 = Instant::now();

    // When: the capture runs on a worker thread so this thread can watch
    // the channel while the child is still alive.
    let 실행 = std::thread::spawn(move || {
        capture::run(
            &mut command,
            CaptureLimit::new(넉넉한_캡처_한도),
            RunBudget::unbounded(),
            RunControls::with_reporter(ProgressReporter::new(sink)),
        )
    });

    let 첫_청크 = receiver
        .recv_timeout(중간_휴지)
        .expect("첫 줄은 자식이 자는 동안 도착한다");
    let 첫_청크_지연 = 시작.elapsed();

    // Then
    assert_eq!(첫_청크.stream, ProgressStream::Stdout);
    assert_eq!(첫_청크.chunk, "first\n");
    assert_eq!(첫_청크.seq, 0);
    assert!(
        첫_청크_지연 < 생중계_여유,
        "첫 줄이 {첫_청크_지연:?}만에 왔다 — 종료까지 버퍼링된 것으로 보인다"
    );

    let 둘째_청크 = receiver
        .recv_timeout(중간_휴지 * 2)
        .expect("둘째 줄도 도착한다");
    assert_eq!(둘째_청크.stream, ProgressStream::Stderr);
    assert_eq!(둘째_청크.chunk, "second\n");
    assert_eq!(둘째_청크.seq, 1, "순번은 두 스트림에 걸쳐 이어진다");

    let captured = 실행.join().expect("캡처 스레드").expect("캡처 성공");

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
fn sink이_없으면_캡처_결과는_예전과_같다() {
    // Given
    let mut command = 두_번_쓰고_사이에_쉬는_명령();

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(넉넉한_캡처_한도),
        RunBudget::unbounded(),
        RunControls::default(),
    )
    .expect("sink 없는 실행도 성공한다");

    // Then
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "first\n");
    assert_eq!(String::from_utf8_lossy(&captured.stderr), "second\n");
}

#[test]
fn 줄바꿈_없이_끝난_출력도_마지막에_흘러나온다() {
    // Given
    let (sink, receiver) = progress_channel();
    let mut command = Command::new("printf");
    command.args(["%s", "no-newline-tail"]);

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(넉넉한_캡처_한도),
        RunBudget::unbounded(),
        RunControls::with_reporter(ProgressReporter::new(sink)),
    )
    .expect("캡처 성공");

    // Then
    let 청크 = receiver.recv().expect("꼬리도 전달된다");
    assert_eq!(청크.chunk, "no-newline-tail");
    assert_eq!(String::from_utf8_lossy(&captured.stdout), "no-newline-tail");
}

#[test]
fn 시간초과로_죽은_자식도_그때까지_쓴_것은_전달한다() {
    // Given: a child that prints, then hangs past its budget.
    let (sink, receiver) = progress_channel();
    let mut command = Command::new("sh");
    command.args(["-c", "echo alive; sleep 30"]);

    // When
    let captured = capture::run(
        &mut command,
        CaptureLimit::new(넉넉한_캡처_한도),
        RunBudget::from_timeout(Some(중간_휴지)),
        RunControls::with_reporter(ProgressReporter::new(sink)),
    )
    .expect("시간초과는 캡처 오류가 아니다");

    // Then
    assert!(matches!(
        captured.completion,
        CaptureCompletion::TimedOut { .. }
    ));
    let 청크 = receiver.recv().expect("죽기 전에 쓴 줄은 이미 나갔다");
    assert_eq!(청크.chunk, "alive\n");
}
