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

const GENEROUS_CAPTURE_LIMIT: u64 = 1024 * 1024;
/// Budget for the children that are supposed to be killed rather than
/// to finish. Long enough that a slow machine cannot make a passing run
/// look like a timeout, short enough that the suite stays quick.
const SHORT_BUDGET: Duration = Duration::from_millis(300);
/// How long a terminated run may take to come back. An order of
/// magnitude over the budget, so this is a hang detector, not a
/// stopwatch.
const MAX_RETURN_TIME: Duration = Duration::from_secs(5);
const LONG_LIVED_CHILD_SECS: u64 = 30;
#[cfg(target_os = "linux")]
const ESCAPED_PROCESS_EXIT_WAIT: Duration = Duration::from_secs(1);

/// `test -t <fd>` reported as one word, so the assertion reads the way
/// the question does.
const TTY_TRUE: &str = "TTY\n";
const TTY_FALSE: &str = "NOTTY\n";

/// Where a pseudoterminal's child side lives, so "is this descriptor a
/// terminal" is asked once and spelled once.
#[cfg(target_os = "linux")]
const TERMINAL_DIR: &str = "/dev/pts";
/// What `/proc/<pid>/fd/N` appends when its target has been unlinked.
#[cfg(target_os = "linux")]
const DELETED_NODE_MARKER: &str = " (deleted)";

fn tty_probe_command(fd: u8) -> Command {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("test -t {fd} && echo {} || echo {}", "TTY", "NOTTY"),
    ]);
    command
}

fn run_on_pty(command: &mut Command) -> CapturedOutput {
    run(command, RunControls::on_pty(), RunBudget::unbounded())
}

fn run_on_pipe(command: &mut Command) -> CapturedOutput {
    run(command, RunControls::default(), RunBudget::unbounded())
}

fn run(command: &mut Command, controls: RunControls, budget: RunBudget) -> CapturedOutput {
    capture::run(
        command,
        CaptureLimit::new(GENEROUS_CAPTURE_LIMIT),
        budget,
        controls,
    )
    .expect("capture succeeds")
}

fn stdout_text(captured: &CapturedOutput) -> String {
    String::from_utf8_lossy(&captured.stdout).into_owned()
}

#[test]
fn pty_mode_child_sees_stdout_as_tty() {
    // Given
    let mut command = tty_probe_command(1);

    // When
    let captured = run_on_pty(&mut command);

    // Then
    assert_eq!(stdout_text(&captured), TTY_TRUE);
}

#[test]
fn pty_mode_child_sees_stderr_as_tty() {
    // Given
    let mut command = tty_probe_command(2);

    // When
    let captured = run_on_pty(&mut command);

    // Then: fd 2 is the same terminal, so its answer comes back on the
    // one merged stream.
    assert_eq!(stdout_text(&captured), TTY_TRUE);
}

#[test]
fn pipe_mode_is_still_not_a_tty() {
    // The control for the two above: without the declaration nothing
    // changed, which is the whole promise of `pty` being opt-in.
    let mut command = tty_probe_command(1);

    let captured = run_on_pipe(&mut command);

    assert_eq!(stdout_text(&captured), TTY_FALSE);
}

#[test]
fn pty_mode_merges_both_streams_in_write_order() {
    // Given: three separate processes, so each one's output is flushed
    // by its own exit and the order on the terminal is the order they
    // ran in — not an artifact of one shell's buffering.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "/bin/echo out-one; /bin/echo err-one 1>&2; /bin/echo out-two",
    ]);

    // When
    let captured = run_on_pty(&mut command);

    // Then
    assert_eq!(stdout_text(&captured), "out-one\nerr-one\nout-two\n");
    assert!(
        captured.stderr.is_empty(),
        "a terminal has a single buffer, so stderr is empty"
    );
}

#[test]
fn pipe_mode_keeps_both_streams_separate() {
    // The same command through the default path, so the difference the
    // merge makes is visible rather than asserted in prose.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "/bin/echo out-one; /bin/echo err-one 1>&2; /bin/echo out-two",
    ]);

    let captured = run_on_pipe(&mut command);

    assert_eq!(stdout_text(&captured), "out-one\nout-two\n");
    assert_eq!(String::from_utf8_lossy(&captured.stderr), "err-one\n");
}

#[test]
fn pty_output_is_not_polluted_by_carriage_returns() {
    // A terminal's default output post-processing rewrites every `\n`
    // into `\r\n`. upeg turns it off, so a captured line is the bytes
    // the program wrote — the same promise the pipe path makes.
    let mut command = Command::new("sh");
    command.args(["-c", "/bin/echo one; /bin/echo two"]);

    let captured = run_on_pty(&mut command);

    assert_eq!(stdout_text(&captured), "one\ntwo\n");
}

#[test]
fn pty_mode_timeout_still_kills_child() {
    // Given: writes, then hangs well past its budget.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        &format!("/bin/echo alive; sleep {LONG_LIVED_CHILD_SECS}"),
    ]);

    // When
    let start = Instant::now();
    let captured = run(
        &mut command,
        RunControls::on_pty(),
        RunBudget::from_timeout(Some(SHORT_BUDGET)),
    );
    let elapsed = start.elapsed();

    // Then
    assert!(
        matches!(captured.completion, CaptureCompletion::TimedOut { .. }),
        "on the pty path an over-budget run still ends in a timeout: {:?}",
        captured.completion
    );
    assert!(
        elapsed < MAX_RETURN_TIME,
        "a timed-out pty child blocked return for {elapsed:?}"
    );
    assert_eq!(
        stdout_text(&captured),
        "alive\n",
        "what it wrote before dying stays intact"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn pty_mode_reaps_background_that_left_process_group() {
    use super::process_test_support::lingering_process_state;

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
        "setsid sh -c 'exec sleep {LONG_LIVED_CHILD_SECS}' >&2 & \
         /bin/echo $! > '{}'; /bin/echo ok",
        pid_path.display()
    );
    command.args(["-c", &script]);

    // When
    let start = Instant::now();
    let captured = run_on_pty(&mut command);
    let elapsed = start.elapsed();

    // Then
    assert_eq!(stdout_text(&captured), "ok\n");
    assert!(
        elapsed < MAX_RETURN_TIME,
        "an escaped process holding the pts blocked return for {elapsed:?}"
    );
    let escaped_pid = std::fs::read_to_string(&pid_path)
        .expect("the escaped descendant writes its pid file")
        .trim()
        .parse::<u32>()
        .expect("parse the escaped descendant's pid");
    let lingering_state = lingering_process_state(escaped_pid, ESCAPED_PROCESS_EXIT_WAIT);
    if lingering_state.is_some() {
        let _ = Command::new("kill")
            .args(["-KILL", &escaped_pid.to_string()])
            .status();
    }
    std::fs::remove_file(pid_path).expect("clean up the escaped descendant's pid file");
    assert!(
        lingering_state.is_none(),
        "on the pty path a descendant outside the process group must be killed too: pid={escaped_pid}, status={lingering_state:?}"
    );
}

/// A tool whose command reports whether it is talking to a terminal and
/// what the color convention told it.
fn tool(pty: Option<bool>) -> ToolToml {
    color_declared_tool(pty, None)
}

fn color_declared_tool(pty: Option<bool>, color: Option<&str>) -> ToolToml {
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

fn run_tool(tool: &ToolToml) -> String {
    let dispatcher = external_dispatcher_for(tool, None).expect("External dispatcher");
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("empty object args");
    match dispatcher(parsed) {
        ToolResult::Success(success) => tool_success_primary_text(&success),
        ToolResult::Failure(failure) => panic!("child run failed: {}", failure.error.message),
    }
}

#[test]
fn pty_tool_gets_terminal_and_color_convention() {
    // `pty = true` implies `color = "force"`: a manifest that asked for
    // a real terminal wants color out of the programs that read the
    // environment as well as out of the ones that call `isatty`.
    assert_eq!(run_tool(&tool(Some(true))), "TTY|1");
}

#[test]
fn undeclared_pty_has_no_terminal_or_forced_color() {
    let output = run_tool(&tool(None));

    assert!(
        output.starts_with("NOTTY"),
        "an undeclared tool sees pipes: {output}"
    );
    assert!(
        !output.ends_with("|1"),
        "color forcing must not leak into an undeclared tool: {output}"
    );
}

#[test]
fn pty_tool_declared_color_wins() {
    // The implication is a default, not an override: a tool that spells
    // `color` out means it, exactly as a declared `env` entry beats the
    // policy that would otherwise have set the same variable.
    let inherited_force_color = std::env::var("FORCE_COLOR").unwrap_or_default();

    assert_eq!(
        run_tool(&color_declared_tool(Some(true), Some("inherit"))),
        format!("TTY|{inherited_force_color}")
    );
}

/// The terminals this process holds open right now, by device path.
///
/// `/proc/self/fd` is what the escaped-descendant sweep reads about
/// *other* processes, so a descriptor upeg itself forgot shows up here
/// exactly the way containment sees its own targets.
#[cfg(target_os = "linux")]
fn parent_open_terminals() -> std::collections::BTreeSet<std::path::PathBuf> {
    const PARENT_FD_DIR: &str = "/proc/self/fd";

    std::fs::read_dir(PARENT_FD_DIR)
        .expect("read own open descriptor list")
        .flatten()
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .filter(|target| target.starts_with(TERMINAL_DIR))
        .map(|target| live_path(&target))
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
fn live_path(target: &std::path::Path) -> std::path::PathBuf {
    target
        .to_str()
        .and_then(|rendered| rendered.strip_suffix(DELETED_NODE_MARKER))
        .map_or_else(|| target.to_path_buf(), std::path::PathBuf::from)
}

#[cfg(target_os = "linux")]
#[test]
fn pty_run_ends_when_child_exits_and_leaves_no_terminal() {
    // Given: a child that names its own terminal and exits immediately.
    // The name matters because other tests in this binary open
    // pseudoterminals at the same time — only the device this child
    // actually wrote to may be looked for afterwards.
    let terminals_before = parent_open_terminals();
    let mut command = Command::new("sh");
    command.args(["-c", "readlink /proc/self/fd/1"]);

    // When
    let start = Instant::now();
    let captured = run_on_pty(&mut command);
    let elapsed = start.elapsed();
    let terminals_after = parent_open_terminals();

    // Then: end-of-file came from the child exiting. A pty drain has
    // exactly one other way to finish — the escaped-descendant grace
    // period — and the two are a quarter of a second apart, so the clock
    // is what tells them apart.
    assert!(
        elapsed < capture::DRAIN_COMPLETION_GRACE,
        "a child that exits immediately took {elapsed:?} — the parent is holding the child-side descriptor so EOF only arrives after the grace period"
    );

    // And: the `Command` the caller still owns carries none of this
    // run's terminal away. A spawn duplicates the `Stdio` values it was
    // handed rather than consuming them, so a missing release leaves
    // exactly this trace.
    let child_terminal = std::path::PathBuf::from(stdout_text(&captured).trim());
    assert!(
        child_terminal.starts_with(TERMINAL_DIR),
        "the child must report writing to a terminal: {child_terminal:?}"
    );
    let leftover_terminals: std::collections::BTreeSet<_> =
        terminals_after.difference(&terminals_before).collect();
    assert!(
        !leftover_terminals.contains(&child_terminal),
        "after the run the parent still holds {child_terminal:?} open: {leftover_terminals:?}"
    );
}
