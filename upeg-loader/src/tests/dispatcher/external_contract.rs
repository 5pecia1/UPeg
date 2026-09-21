//! External invoker execution contract: structured failures, stdin
//! isolation, timeouts, working directory, environment, and input
//! defaults.
//!
//! These are the behaviors a real developer toolkit depends on — a
//! failing `cargo clippy` has to hand back the stdout it wrote, a `cat`
//! must not hang forever on an inherited stdin, and a project tool must
//! run at the project root no matter which subdirectory invoked it.

use serde_json::json;
use upeg_core::{EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_CWD, ToolFailure, ToolResult};

use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;
use crate::manifest_origin::ManifestOrigin;
use crate::tests::{call_dispatcher_result, single_tool_toml_str};

const TOOL_ERROR_CODE: &str = "tool_error";
const INVALID_ARGS_CODE: &str = "invalid_args";
const EXIT_CODE_KEY: &str = "exit_code";
const STDOUT_KEY: &str = "stdout";
const STDERR_KEY: &str = "stderr";
const TIMED_OUT_KEY: &str = "timed_out";
const TIMEOUT_MS: u64 = 200;
const WAIT_COMMAND_SECS: u64 = 5;
// CI containers have no init reaper, so a SIGKILLed descendant lingers
// briefly as a zombie (state `Z`) reparented to PID 1 — `/proc/<pid>`
// still exists for an already-dead process. A single fixed sleep races
// under CI load, so poll the state up to a cap.
#[cfg(target_os = "linux")]
const DESCENDANT_REAP_WAIT_CAP_MS: u64 = 5_000;
#[cfg(target_os = "linux")]
const DESCENDANT_REAP_POLL_INTERVAL_MS: u64 = 50;
#[cfg(target_os = "linux")]
const ZOMBIE_STATE: char = 'Z';
#[cfg(target_os = "linux")]
const DEAD_STATE: char = 'X';

/// Read the state character from `/proc/<pid>/stat`. The comm field
/// (`(...)`) may contain spaces or parentheses, so the state is the
/// token right after the last `)`.
#[cfg(target_os = "linux")]
fn parse_process_state(stat: &str) -> Option<char> {
    let (_, after_comm) = stat.rsplit_once(')')?;
    after_comm.split_whitespace().next()?.chars().next()
}

#[cfg(target_os = "linux")]
fn process_state(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_process_state(&stat)
}

/// Decide whether a descendant is actually running: no `/proc/<pid>`,
/// or zombie (`Z`) / dead (`X`) state, counts as already dead.
#[cfg(target_os = "linux")]
fn process_is_alive(pid: u32) -> bool {
    !matches!(process_state(pid), None | Some(ZOMBIE_STATE | DEAD_STATE))
}

fn tool(flat_tool: &str) -> ToolToml {
    let manifest = single_tool_toml_str(flat_tool);
    let (_, tools) = crate::parse_toolkit_full(&manifest).expect("the test manifest parses");
    tools.into_iter().next().expect("one tool").1
}

fn run(flat_tool: &str, args: serde_json::Value) -> ToolResult {
    run_with_origin(flat_tool, args, None)
}

fn run_with_origin(
    flat_tool: &str,
    args: serde_json::Value,
    origin: Option<&ManifestOrigin>,
) -> ToolResult {
    let parsed = tool(flat_tool);
    let f = external_dispatcher_for(&parsed, origin).expect("the External dispatcher is built");
    call_dispatcher_result(&f, args)
}

fn failure(result: ToolResult) -> ToolFailure {
    match result {
        ToolResult::Failure(failure) => failure,
        ToolResult::Success(success) => panic!("expected failure but succeeded: {success:?}"),
    }
}

fn primary_output(result: &ToolResult) -> String {
    match result {
        ToolResult::Success(success) => upeg_runtime::tool_success_primary_text(success),
        ToolResult::Failure(failure) => panic!("expected success but failed: {failure:?}"),
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "upeg_external_contract_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    dir
}

#[test]
fn failing_command_carries_stdout_and_exit_code_in_details() {
    // The whole point of the structured envelope: `cargo fmt --check`,
    // clippy, and `flutter analyze` write diagnostics to stdout, which
    // the old `Err(String)` path threw away entirely.
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out; echo err >&2; exit 3"]"#,
        json!({}),
    );

    let failure = failure(result);
    assert_eq!(failure.error.code, TOOL_ERROR_CODE);
    assert_eq!(failure.error.message, "`sh` exited with code 3: err");
    let details = failure.error.details.expect("details exist");
    assert_eq!(details[EXIT_CODE_KEY], json!(3));
    assert_eq!(details[STDOUT_KEY], json!("out\n"));
    assert_eq!(details[STDERR_KEY], json!("err\n"));
    assert_eq!(details.get(TIMED_OUT_KEY), None);
}

#[test]
fn failure_message_without_stderr_does_not_end_in_colon() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "false""#,
        json!({}),
    );

    let failure = failure(result);
    assert_eq!(failure.error.message, "`false` exited with code 1");
}

#[test]
fn null_stdin_lets_stdin_reading_command_exit_immediately() {
    // With an inherited stdin this blocks forever and takes the calling
    // surface down with it.
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "cat""#,
        json!({}),
    );

    assert_eq!(primary_output(&result), "");
}

#[test]
fn command_past_timeout_becomes_timed_out_failure() {
    let result = run(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sleep"
args_template = ["{WAIT_COMMAND_SECS}"]
timeout_ms = {TIMEOUT_MS}"#
        ),
        json!({}),
    );

    let failure = failure(result);
    assert_eq!(
        failure.error.message,
        format!("`sleep` timed out after {TIMEOUT_MS} ms")
    );
    let details = failure.error.details.expect("details exist");
    assert_eq!(details[TIMED_OUT_KEY], json!(true));
    assert_eq!(details[EXIT_CODE_KEY], serde_json::Value::Null);
}

#[cfg(target_os = "linux")]
#[test]
fn timeout_kills_process_group_including_descendants() {
    // Killing only the direct child would leave a detached `sleep`
    // (or a spawned build server) running after the tool "failed".
    let dir = temp_dir("timeout_group");
    let pid_path = dir.join("descendant.pid");
    let script = format!(
        "sleep {WAIT_COMMAND_SECS} & echo $! > '{}'; sleep {WAIT_COMMAND_SECS}",
        pid_path.display()
    );

    let result = run(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "{script}"]
timeout_ms = {TIMEOUT_MS}"#
        ),
        json!({}),
    );
    failure(result);

    let pid = std::fs::read_to_string(&pid_path)
        .expect("the descendant writes the pid file")
        .trim()
        .parse::<u32>()
        .expect("parse the pid");

    let wait_start = std::time::Instant::now();
    while process_is_alive(pid)
        && wait_start.elapsed() < std::time::Duration::from_millis(DESCENDANT_REAP_WAIT_CAP_MS)
    {
        std::thread::sleep(std::time::Duration::from_millis(
            DESCENDANT_REAP_POLL_INTERVAL_MS,
        ));
    }
    assert!(
        !process_is_alive(pid),
        "on timeout the descendant process must be reaped too: pid={pid}, observed state={:?}",
        process_state(pid)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn zombie_state_parses_from_stat_sample() {
    let sample = "123 (sleep) Z 1 1 1 0 -1 4194560 122 0 0 0 0 0 0 0 20 0 1 0 12345 0 0";
    assert_eq!(parse_process_state(sample), Some(ZOMBIE_STATE));
}

#[cfg(target_os = "linux")]
#[test]
fn state_is_read_after_last_paren_despite_spaces_and_parens_in_comm() {
    // comm is wrapped in `(...)` and may itself contain spaces/parens,
    // so the state field sits after the *last* `)`, not the first.
    let sample = "456 (weird (proc) name) S 1 1 1 0 -1 4194304 10 0 0 0 0 0 0 0 20 0 1 0 9 0 0";
    assert_eq!(parse_process_state(sample), Some('S'));
}

#[cfg(target_os = "linux")]
#[test]
fn nonexistent_pid_has_no_state() {
    assert!(process_state(0).is_none());
    assert!(!process_is_alive(0));
}

#[test]
fn without_timeout_slow_command_runs_to_completion() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "sleep 0.3; printf done"]"#,
        json!({}),
    );

    assert_eq!(primary_output(&result), "done");
}

#[test]
fn command_runs_in_declared_cwd() {
    let dir = temp_dir("declared_cwd");
    std::fs::write(dir.join("marker.txt"), "x").expect("write the marker file");

    let result = run(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls"
cwd = "{}""#,
            dir.display()
        ),
        json!({}),
    );

    assert!(primary_output(&result).contains("marker.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn relative_cwd_resolves_against_manifest_dir() {
    let root = temp_dir("relative_cwd");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).expect("create the subdirectory");
    std::fs::write(nested.join("nested-marker.txt"), "x").expect("write the marker file");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("the origin is built");

    let result = run_with_origin(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls"
cwd = "nested""#,
        json!({}),
        Some(&origin),
    );

    assert!(primary_output(&result).contains("nested-marker.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_manifest_tool_runs_in_manifest_dir() {
    let root = temp_dir("project_default_cwd");
    std::fs::write(root.join("project-marker.txt"), "x").expect("write the marker file");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("the origin is built");

    let result = run_with_origin(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({}),
        Some(&origin),
    );

    assert!(primary_output(&result).contains("project-marker.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn caller_supplied_cwd_is_respected() {
    let dir = temp_dir("caller_cwd");
    std::fs::write(dir.join("caller-marker.txt"), "x").expect("write the marker file");

    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: dir.to_string_lossy() } }),
    );

    assert!(primary_output(&result).contains("caller-marker.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn declared_cwd_wins_over_caller_cwd() {
    let declared = temp_dir("precedence_declared");
    let caller = temp_dir("precedence_caller");
    std::fs::write(declared.join("declared-marker.txt"), "x").expect("write the marker file");
    std::fs::write(caller.join("caller-marker.txt"), "x").expect("write the marker file");

    let result = run(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls"
cwd = "{}""#,
            declared.display()
        ),
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: caller.to_string_lossy() } }),
    );

    let out = primary_output(&result);
    assert!(out.contains("declared-marker.txt"), "{out}");
    assert!(!out.contains("caller-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&declared);
    let _ = std::fs::remove_dir_all(&caller);
}

#[test]
fn relative_caller_cwd_is_rejected_as_invalid_args() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: "relative/dir" } }),
    );

    let failure = failure(result);
    assert_eq!(failure.error.code, INVALID_ARGS_CODE);
    assert!(failure.error.message.contains("absolute"), "{failure:?}");
}

#[test]
fn missing_caller_cwd_is_rejected_as_invalid_args() {
    let missing = std::env::temp_dir().join("upeg_external_contract_missing_dir_zzz");
    let _ = std::fs::remove_dir_all(&missing);

    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: missing.to_string_lossy() } }),
    );

    let failure = failure(result);
    assert_eq!(failure.error.code, INVALID_ARGS_CODE);
    assert!(
        failure.error.message.contains("existing directory"),
        "{failure:?}"
    );
}

#[test]
fn declared_env_is_visible_to_child_process() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf '%s' \"$UPEG_TEST_ENV\""]
env = [{ name = "UPEG_TEST_ENV", value = "visible" }]"#,
        json!({}),
    );

    assert_eq!(primary_output(&result), "visible");
}

#[test]
fn missing_input_substitutes_declared_default() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "echo"
args_template = ["-n", "{count}"]
inputs = [{ name = "count", type = "integer", default = 10 }]"#,
        json!({}),
    );

    assert_eq!(primary_output(&result), "10");
}

#[test]
fn caller_value_beats_default() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "echo"
args_template = ["-n", "{count}"]
inputs = [{ name = "count", type = "integer", default = 10 }]"#,
        json!({ "count": 3 }),
    );

    assert_eq!(primary_output(&result), "3");
}

#[test]
fn token_for_optional_input_without_default_vanishes() {
    // `printf '[%s]'` with no argument prints `[]`; with a dropped
    // token it still prints `[]`, but with an empty-string argument it
    // would also print `[]` — so assert on argument *count* instead by
    // echoing `$#`.
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf '%s' \"$#\"", "upeg-test", "{maybe}"]
inputs = [{ name = "maybe", type = "string" }]"#,
        json!({}),
    );

    assert_eq!(
        primary_output(&result),
        "0",
        "an empty token drops out of the argument list"
    );
}

#[test]
fn stderr_of_successful_command_remains_as_auxiliary_output() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf out; printf warn >&2"]"#,
        json!({}),
    );

    let ToolResult::Success(success) = result else {
        panic!("expected success");
    };
    assert_eq!(success.primary_output_id.as_deref(), Some("result"));
    let stderr = success
        .outputs
        .iter()
        .find(|entry| entry.id == "stderr")
        .expect("a stderr auxiliary output exists");
    assert_eq!(
        stderr.value,
        upeg_core::OutputValue::String("warn".to_string())
    );
}

#[test]
fn empty_stderr_creates_no_auxiliary_output() {
    let result = run(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "printf"
args_template = ["out"]"#,
        json!({}),
    );

    let ToolResult::Success(success) = result else {
        panic!("expected success");
    };
    assert_eq!(success.outputs.len(), 1);
}

// ─── Working directory boundary ──────────────────────────────────
//
// A Project Manifest tool belongs to its project. The caller's
// `_upeg.cwd` can only move the tool within that project.

const LIST_COMMAND: &str = r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#;

#[test]
fn caller_cwd_inside_project_is_respected() {
    let root = temp_dir("inside_caller_cwd");
    let nested = root.join("member");
    std::fs::create_dir_all(&nested).expect("create the subdirectory");
    std::fs::write(root.join("root-marker.txt"), "x").expect("write the marker file");
    std::fs::write(nested.join("member-marker.txt"), "x").expect("write the marker file");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("the origin is built");

    let result = run_with_origin(
        LIST_COMMAND,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: nested.to_string_lossy() } }),
        Some(&origin),
    );

    let out = primary_output(&result);
    assert!(out.contains("member-marker.txt"), "{out}");
    assert!(!out.contains("root-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn caller_cwd_outside_project_is_ignored_for_manifest_dir() {
    let root = temp_dir("outside_project_root");
    let outside = temp_dir("outside_caller_cwd");
    std::fs::write(root.join("root-marker.txt"), "x").expect("write the marker file");
    std::fs::write(outside.join("outside-marker.txt"), "x").expect("write the marker file");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("the origin is built");

    let result = run_with_origin(
        LIST_COMMAND,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: outside.to_string_lossy() } }),
        Some(&origin),
    );

    let out = primary_output(&result);
    assert!(out.contains("root-marker.txt"), "{out}");
    assert!(!out.contains("outside-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn toolkit_dir_tool_respects_any_caller_cwd() {
    let toolkits = temp_dir("toolkit_origin_dir");
    let elsewhere = temp_dir("toolkit_caller_cwd");
    std::fs::write(elsewhere.join("elsewhere-marker.txt"), "x").expect("write the marker file");
    let origin =
        ManifestOrigin::toolkit_file(&toolkits.join("dev.toml")).expect("the origin is built");

    let result = run_with_origin(
        LIST_COMMAND,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: elsewhere.to_string_lossy() } }),
        Some(&origin),
    );

    assert!(primary_output(&result).contains("elsewhere-marker.txt"));
    let _ = std::fs::remove_dir_all(&toolkits);
    let _ = std::fs::remove_dir_all(&elsewhere);
}

// ─── Argument slot stability ──────────────────────────────────────
//
// `printf '<%s>'` repeats the format per remaining argument, so the
// output alone distinguishes a dropped token from an empty argument.

fn slot_check(input_decl: &str, token: &str) -> String {
    primary_output(&run(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "printf"
args_template = ["<%s>", "before", "{token}", "after"]
{input_decl}"#
        ),
        json!({}),
    ))
}

const OPTIONAL_INPUT: &str = r#"
[[inputs]]
name = "dir"
type = "string""#;

const REQUIRED_INPUT: &str = r#"
[[inputs]]
name = "dir"
type = "string"
required = true"#;

#[test]
fn missing_required_input_token_holds_slot_as_empty_arg() {
    assert_eq!(slot_check(REQUIRED_INPUT, "{dir}"), "<before><><after>");
}

#[test]
fn only_lone_token_of_missing_optional_input_vanishes() {
    assert_eq!(slot_check(OPTIONAL_INPUT, "{dir}"), "<before><after>");
}

#[test]
fn token_with_literal_suffix_holds_slot_despite_missing_value() {
    assert_eq!(
        slot_check(OPTIONAL_INPUT, "{dir}/build"),
        "<before></build><after>",
        "`rm -rf {{dir}}/build` must not collapse to `rm -rf`"
    );
}
