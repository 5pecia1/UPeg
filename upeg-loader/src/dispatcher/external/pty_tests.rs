//! `pty = true` from the outside: what the child sees, what upeg
//! captures, and that none of the containment guarantees weakened on the
//! way.
//!
//! Every assertion here is one the pipe path cannot satisfy — a real
//! `isatty(1)`, one merged stream instead of two — or one it already
//! satisfied and the pty path must not lose: a timed-out child is still
//! reaped with its group, and a descendant that escaped that group is
//! still found and killed.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::json;
use upeg_core::ToolResult;
use upeg_runtime::{DispatchArgs, tool_success_primary_text};

use super::capture::{self, RunBudget, RunControls};
use super::error::{CaptureCompletion, CaptureLimit, CapturedOutput};
use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;

const 넉넉한_캡처_한도: u64 = 1024 * 1024;
/// Budget for the children that are supposed to be killed rather than
/// to finish. Long enough that a slow machine cannot make a passing run
/// look like a timeout, short enough that the suite stays quick.
const 짧은_예산: Duration = Duration::from_millis(300);
/// How long a terminated run may take to come back. An order of
/// magnitude over the budget, so this is a hang detector, not a
/// stopwatch.
const 최대_반환_시간: Duration = Duration::from_secs(5);
const 오래_사는_자식_초: u64 = 30;
#[cfg(target_os = "linux")]
const 탈출_프로세스_종료_대기: Duration = Duration::from_secs(1);

/// `test -t <fd>` reported as one word, so the assertion reads the way
/// the question does.
const TTY_참: &str = "TTY\n";
const TTY_거짓: &str = "NOTTY\n";

/// Where a pseudoterminal's child side lives, so "is this descriptor a
/// terminal" is asked once and spelled once.
#[cfg(target_os = "linux")]
const 터미널_디렉터리: &str = "/dev/pts";
/// What `/proc/<pid>/fd/N` appends when its target has been unlinked.
#[cfg(target_os = "linux")]
const 삭제된_노드_표시: &str = " (deleted)";

fn tty_확인_명령(fd: u8) -> Command {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("test -t {fd} && echo {} || echo {}", "TTY", "NOTTY"),
    ]);
    command
}

fn pty로_실행(command: &mut Command) -> CapturedOutput {
    실행(command, RunControls::on_pty(), RunBudget::unbounded())
}

fn 파이프로_실행(command: &mut Command) -> CapturedOutput {
    실행(command, RunControls::default(), RunBudget::unbounded())
}

fn 실행(command: &mut Command, controls: RunControls, budget: RunBudget) -> CapturedOutput {
    capture::run(
        command,
        CaptureLimit::new(넉넉한_캡처_한도),
        budget,
        controls,
    )
    .expect("캡처는 성공한다")
}

fn 표준출력(captured: &CapturedOutput) -> String {
    String::from_utf8_lossy(&captured.stdout).into_owned()
}

#[test]
fn pty_모드에서_자식은_표준출력이_tty라고_본다() {
    // Given
    let mut command = tty_확인_명령(1);

    // When
    let captured = pty로_실행(&mut command);

    // Then
    assert_eq!(표준출력(&captured), TTY_참);
}

#[test]
fn pty_모드에서_자식은_표준오류도_tty라고_본다() {
    // Given
    let mut command = tty_확인_명령(2);

    // When
    let captured = pty로_실행(&mut command);

    // Then: fd 2 is the same terminal, so its answer comes back on the
    // one merged stream.
    assert_eq!(표준출력(&captured), TTY_참);
}

#[test]
fn 파이프_모드에서는_여전히_tty가_아니다() {
    // The control for the two above: without the declaration nothing
    // changed, which is the whole promise of `pty` being opt-in.
    let mut command = tty_확인_명령(1);

    let captured = 파이프로_실행(&mut command);

    assert_eq!(표준출력(&captured), TTY_거짓);
}

#[test]
fn pty_모드는_두_스트림을_쓴_순서대로_합친다() {
    // Given: three separate processes, so each one's output is flushed
    // by its own exit and the order on the terminal is the order they
    // ran in — not an artifact of one shell's buffering.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "/bin/echo out-one; /bin/echo err-one 1>&2; /bin/echo out-two",
    ]);

    // When
    let captured = pty로_실행(&mut command);

    // Then
    assert_eq!(표준출력(&captured), "out-one\nerr-one\nout-two\n");
    assert!(
        captured.stderr.is_empty(),
        "터미널은 버퍼가 하나뿐이므로 stderr는 비어 있다"
    );
}

#[test]
fn 파이프_모드는_두_스트림을_따로_담는다() {
    // The same command through the default path, so the difference the
    // merge makes is visible rather than asserted in prose.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "/bin/echo out-one; /bin/echo err-one 1>&2; /bin/echo out-two",
    ]);

    let captured = 파이프로_실행(&mut command);

    assert_eq!(표준출력(&captured), "out-one\nout-two\n");
    assert_eq!(String::from_utf8_lossy(&captured.stderr), "err-one\n");
}

#[test]
fn pty_출력은_캐리지리턴으로_오염되지_않는다() {
    // A terminal's default output post-processing rewrites every `\n`
    // into `\r\n`. upeg turns it off, so a captured line is the bytes
    // the program wrote — the same promise the pipe path makes.
    let mut command = Command::new("sh");
    command.args(["-c", "/bin/echo one; /bin/echo two"]);

    let captured = pty로_실행(&mut command);

    assert_eq!(표준출력(&captured), "one\ntwo\n");
}

#[test]
fn pty_모드에서도_제한시간이_자식을_종료한다() {
    // Given: writes, then hangs well past its budget.
    let mut command = Command::new("sh");
    command.args(["-c", &format!("/bin/echo alive; sleep {오래_사는_자식_초}")]);

    // When
    let 시작 = Instant::now();
    let captured = 실행(
        &mut command,
        RunControls::on_pty(),
        RunBudget::from_timeout(Some(짧은_예산)),
    );
    let 걸린_시간 = 시작.elapsed();

    // Then
    assert!(
        matches!(captured.completion, CaptureCompletion::TimedOut { .. }),
        "pty 경로에서도 예산 초과는 시간초과로 끝난다: {:?}",
        captured.completion
    );
    assert!(
        걸린_시간 < 최대_반환_시간,
        "시간초과된 pty 자식 때문에 {걸린_시간:?} 동안 반환하지 못했다"
    );
    assert_eq!(
        표준출력(&captured),
        "alive\n",
        "죽기 전에 쓴 것은 그대로 남는다"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn pty_모드에서_process_group을_벗어난_background도_회수된다() {
    use super::process_test_support::종료되지_않은_프로세스_상태;

    // Given: a descendant that leaves the process group with `setsid`
    // and keeps the terminal open. Under pipes it is found by pipe
    // inode; under a pty it has to be found by the `/dev/pts/N` device
    // it holds instead.
    let pid_path = std::env::temp_dir().join(format!(
        "upeg_external_pty_escaped_{}.pid",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pid_path);
    let mut command = Command::new("sh");
    let script = format!(
        "setsid sh -c 'exec sleep {오래_사는_자식_초}' >&2 & \
         /bin/echo $! > '{}'; /bin/echo ok",
        pid_path.display()
    );
    command.args(["-c", &script]);

    // When
    let 시작 = Instant::now();
    let captured = pty로_실행(&mut command);
    let 걸린_시간 = 시작.elapsed();

    // Then
    assert_eq!(표준출력(&captured), "ok\n");
    assert!(
        걸린_시간 < 최대_반환_시간,
        "pts를 붙들고 있는 탈출 프로세스 때문에 {걸린_시간:?} 동안 반환하지 못했다"
    );
    let 탈출_pid = std::fs::read_to_string(&pid_path)
        .expect("탈출한 후손이 pid 파일을 쓴다")
        .trim()
        .parse::<u32>()
        .expect("탈출한 후손의 pid를 파싱한다");
    let 잔존_상태 = 종료되지_않은_프로세스_상태(탈출_pid, 탈출_프로세스_종료_대기);
    if 잔존_상태.is_some() {
        let _ = Command::new("kill")
            .args(["-KILL", &탈출_pid.to_string()])
            .status();
    }
    std::fs::remove_file(pid_path).expect("탈출한 후손의 pid 파일을 정리한다");
    assert!(
        잔존_상태.is_none(),
        "pty 경로에서도 process group을 벗어난 후손은 종료되어야 한다: pid={탈출_pid}, status={잔존_상태:?}"
    );
}

/// A tool whose command reports whether it is talking to a terminal and
/// what the color convention told it.
fn 도구(pty: Option<bool>) -> ToolToml {
    색상_선언_도구(pty, None)
}

fn 색상_선언_도구(pty: Option<bool>, color: Option<&str>) -> ToolToml {
    ToolToml {
        id: "test.pty_probe".to_string(),
        toolkit: "test".to_string(),
        invoker: Some("External".to_string()),
        command: Some("sh".to_string()),
        args_template: Some(vec![
            "-c".to_string(),
            r#"test -t 1 && printf 'TTY' || printf 'NOTTY'; printf '|%s' "$FORCE_COLOR""#
                .to_string(),
        ]),
        pty,
        color: color.map(str::to_string),
        ..ToolToml::default()
    }
}

fn 도구_실행(tool: &ToolToml) -> String {
    let dispatcher = external_dispatcher_for(tool, None).expect("External dispatcher");
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("빈 객체 인자");
    match dispatcher(parsed) {
        ToolResult::Success(success) => tool_success_primary_text(&success),
        ToolResult::Failure(failure) => panic!("자식 실행 실패: {}", failure.error.message),
    }
}

#[test]
fn pty_도구는_터미널과_색상_규약을_함께_받는다() {
    // `pty = true` implies `color = "force"`: a manifest that asked for
    // a real terminal wants color out of the programs that read the
    // environment as well as out of the ones that call `isatty`.
    assert_eq!(도구_실행(&도구(Some(true))), "TTY|1");
}

#[test]
fn pty를_선언하지_않으면_터미널도_강제_색상도_없다() {
    let 출력 = 도구_실행(&도구(None));

    assert!(
        출력.starts_with("NOTTY"),
        "선언하지 않은 도구는 파이프를 본다: {출력}"
    );
    assert!(
        !출력.ends_with("|1"),
        "선언하지 않은 도구에 색상 강제가 새면 안 된다: {출력}"
    );
}

#[test]
fn pty_도구도_선언된_color가_이긴다() {
    // The implication is a default, not an override: a tool that spells
    // `color` out means it, exactly as a declared `env` entry beats the
    // policy that would otherwise have set the same variable.
    let 상속된_force_color = std::env::var("FORCE_COLOR").unwrap_or_default();

    assert_eq!(
        도구_실행(&색상_선언_도구(Some(true), Some("inherit"))),
        format!("TTY|{상속된_force_color}")
    );
}

/// The terminals this process holds open right now, by device path.
///
/// `/proc/self/fd` is what the escaped-descendant sweep reads about
/// *other* processes, so a descriptor upeg itself forgot shows up here
/// exactly the way containment sees its own targets.
#[cfg(target_os = "linux")]
fn 부모가_연_터미널들() -> std::collections::BTreeSet<std::path::PathBuf> {
    const 부모_기술자_디렉터리: &str = "/proc/self/fd";

    std::fs::read_dir(부모_기술자_디렉터리)
        .expect("자기 자신의 열린 기술자 목록을 읽는다")
        .flatten()
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .filter(|target| target.starts_with(터미널_디렉터리))
        .map(|target| 살아_있던_경로(&target))
        .collect()
}

/// The device path a `/proc` link target names, with the kernel's
/// unlinked marker removed.
///
/// A pts node disappears from `/dev/pts` the moment the terminal side
/// closes, and from then on the link renders as `/dev/pts/0 (deleted)`.
/// That says nothing about who still holds the descriptor — which is the
/// only question here — so the two spellings have to name one terminal.
#[cfg(target_os = "linux")]
fn 살아_있던_경로(target: &std::path::Path) -> std::path::PathBuf {
    target
        .to_str()
        .and_then(|rendered| rendered.strip_suffix(삭제된_노드_표시))
        .map_or_else(|| target.to_path_buf(), std::path::PathBuf::from)
}

#[cfg(target_os = "linux")]
#[test]
fn pty_실행은_자식이_끝나는_순간_끝나고_터미널을_남기지_않는다() {
    // Given: a child that names its own terminal and exits immediately.
    // The name matters because other tests in this binary open
    // pseudoterminals at the same time — only the device this child
    // actually wrote to may be looked for afterwards.
    let 실행_전_터미널 = 부모가_연_터미널들();
    let mut command = Command::new("sh");
    command.args(["-c", "readlink /proc/self/fd/1"]);

    // When
    let 시작 = Instant::now();
    let captured = pty로_실행(&mut command);
    let 걸린_시간 = 시작.elapsed();
    let 실행_후_터미널 = 부모가_연_터미널들();

    // Then: end-of-file came from the child exiting. A pty drain has
    // exactly one other way to finish — the escaped-descendant grace
    // period — and the two are a quarter of a second apart, so the clock
    // is what tells them apart.
    assert!(
        걸린_시간 < capture::DRAIN_COMPLETION_GRACE,
        "즉시 끝나는 자식인데 {걸린_시간:?} 걸렸다 — 부모가 자식 쪽 기술자를 붙들고 있어 EOF가 유예 시간에만 온다"
    );

    // And: the `Command` the caller still owns carries none of this
    // run's terminal away. A spawn duplicates the `Stdio` values it was
    // handed rather than consuming them, so a missing release leaves
    // exactly this trace.
    let 자식이_쓴_터미널 = std::path::PathBuf::from(표준출력(&captured).trim());
    assert!(
        자식이_쓴_터미널.starts_with(터미널_디렉터리),
        "자식은 터미널에 썼다고 답해야 한다: {자식이_쓴_터미널:?}"
    );
    let 실행이_남긴_터미널: std::collections::BTreeSet<_> =
        실행_후_터미널.difference(&실행_전_터미널).collect();
    assert!(
        !실행이_남긴_터미널.contains(&자식이_쓴_터미널),
        "실행이 끝난 뒤에도 부모가 {자식이_쓴_터미널:?}를 열고 있다: {실행이_남긴_터미널:?}"
    );
}
