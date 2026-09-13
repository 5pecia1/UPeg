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
const 제한시간_밀리초: u64 = 200;
const 대기_명령_초: u64 = 5;
// CI 컨테이너에는 init reaper가 없어 SIGKILL된 자손이 PID 1에
// 재부모화된 좀비(state `Z`)로 잠시 남는다 — `/proc/<pid>`가 여전히
// 존재해도 이미 죽은 프로세스다. 고정 sleep 한 번으로 확인하면
// CI 부하 아래서 레이스가 나므로, 상한까지 상태를 폴링한다.
#[cfg(target_os = "linux")]
const 자손_회수_대기_상한_밀리초: u64 = 5_000;
#[cfg(target_os = "linux")]
const 자손_회수_폴링_간격_밀리초: u64 = 50;
#[cfg(target_os = "linux")]
const 좀비_상태: char = 'Z';
#[cfg(target_os = "linux")]
const 소멸_상태: char = 'X';

/// `/proc/<pid>/stat`에서 상태 문자를 읽는다. comm 필드(`(...)`)가
/// 공백이나 괄호를 담을 수 있으므로, 상태는 마지막 `)` 바로 뒤 토큰이다.
#[cfg(target_os = "linux")]
fn 프로세스_상태_파싱(stat: &str) -> Option<char> {
    let (_, comm_뒤) = stat.rsplit_once(')')?;
    comm_뒤.split_whitespace().next()?.chars().next()
}

#[cfg(target_os = "linux")]
fn 프로세스_상태(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    프로세스_상태_파싱(&stat)
}

/// 자손이 실제로 실행 중인지 판단한다: `/proc/<pid>`가 없거나,
/// 좀비(`Z`)나 소멸(`X`) 상태면 이미 죽은 것으로 취급한다.
#[cfg(target_os = "linux")]
fn 프로세스가_살아있다(pid: u32) -> bool {
    !matches!(프로세스_상태(pid), None | Some(좀비_상태 | 소멸_상태))
}

fn 도구(flat_tool: &str) -> ToolToml {
    let 매니페스트 = single_tool_toml_str(flat_tool);
    let (_, tools) = crate::parse_toolkit_full(&매니페스트).expect("테스트 매니페스트가 파싱된다");
    tools.into_iter().next().expect("도구 하나").1
}

fn 실행(flat_tool: &str, args: serde_json::Value) -> ToolResult {
    실행_원본(flat_tool, args, None)
}

fn 실행_원본(
    flat_tool: &str,
    args: serde_json::Value,
    origin: Option<&ManifestOrigin>,
) -> ToolResult {
    let parsed = 도구(flat_tool);
    let f = external_dispatcher_for(&parsed, origin).expect("External dispatcher가 만들어진다");
    call_dispatcher_result(&f, args)
}

fn 실패(result: ToolResult) -> ToolFailure {
    match result {
        ToolResult::Failure(failure) => failure,
        ToolResult::Success(success) => panic!("실패를 기대했지만 성공: {success:?}"),
    }
}

fn 기본_출력(result: &ToolResult) -> String {
    match result {
        ToolResult::Success(success) => upeg_runtime::tool_success_primary_text(success),
        ToolResult::Failure(failure) => panic!("성공을 기대했지만 실패: {failure:?}"),
    }
}

fn 임시_디렉터리(이름: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "upeg_external_contract_{이름}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("테스트 디렉터리를 만든다");
    dir
}

#[test]
fn 실패한_명령은_표준출력과_종료코드를_details에_담는다() {
    // The whole point of the structured envelope: `cargo fmt --check`,
    // clippy, and `flutter analyze` write diagnostics to stdout, which
    // the old `Err(String)` path threw away entirely.
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "echo out; echo err >&2; exit 3"]"#,
        json!({}),
    );

    let failure = 실패(result);
    assert_eq!(failure.error.code, TOOL_ERROR_CODE);
    assert_eq!(failure.error.message, "`sh` exited with code 3: err");
    let details = failure.error.details.expect("details가 있다");
    assert_eq!(details[EXIT_CODE_KEY], json!(3));
    assert_eq!(details[STDOUT_KEY], json!("out\n"));
    assert_eq!(details[STDERR_KEY], json!("err\n"));
    assert_eq!(details.get(TIMED_OUT_KEY), None);
}

#[test]
fn 표준오류가_없는_실패_메시지는_콜론으로_끝나지_않는다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "false""#,
        json!({}),
    );

    let failure = 실패(result);
    assert_eq!(failure.error.message, "`false` exited with code 1");
}

#[test]
fn 표준입력은_널이라_stdin을_읽는_명령이_즉시_끝난다() {
    // With an inherited stdin this blocks forever and takes the calling
    // surface down with it.
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "cat""#,
        json!({}),
    );

    assert_eq!(기본_출력(&result), "");
}

#[test]
fn 제한시간을_넘긴_명령은_timed_out_실패가_된다() {
    let result = 실행(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sleep"
args_template = ["{대기_명령_초}"]
timeout_ms = {제한시간_밀리초}"#
        ),
        json!({}),
    );

    let failure = 실패(result);
    assert_eq!(
        failure.error.message,
        format!("`sleep` timed out after {제한시간_밀리초} ms")
    );
    let details = failure.error.details.expect("details가 있다");
    assert_eq!(details[TIMED_OUT_KEY], json!(true));
    assert_eq!(details[EXIT_CODE_KEY], serde_json::Value::Null);
}

#[cfg(target_os = "linux")]
#[test]
fn 제한시간_초과는_자손까지_포함한_process_group을_종료한다() {
    // Killing only the direct child would leave a detached `sleep`
    // (or a spawned build server) running after the tool "failed".
    let dir = 임시_디렉터리("timeout_group");
    let pid_path = dir.join("descendant.pid");
    let script = format!(
        "sleep {대기_명령_초} & echo $! > '{}'; sleep {대기_명령_초}",
        pid_path.display()
    );

    let result = 실행(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "{script}"]
timeout_ms = {제한시간_밀리초}"#
        ),
        json!({}),
    );
    실패(result);

    let pid = std::fs::read_to_string(&pid_path)
        .expect("자손이 pid 파일을 쓴다")
        .trim()
        .parse::<u32>()
        .expect("pid를 파싱한다");

    let 대기_시작 = std::time::Instant::now();
    while 프로세스가_살아있다(pid)
        && 대기_시작.elapsed() < std::time::Duration::from_millis(자손_회수_대기_상한_밀리초)
    {
        std::thread::sleep(std::time::Duration::from_millis(자손_회수_폴링_간격_밀리초));
    }
    assert!(
        !프로세스가_살아있다(pid),
        "제한시간 초과 시 자손 프로세스도 회수되어야 한다: pid={pid}, 관측된 상태={:?}",
        프로세스_상태(pid)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn stat_샘플에서_좀비_상태를_파싱한다() {
    let 샘플 = "123 (sleep) Z 1 1 1 0 -1 4194560 122 0 0 0 0 0 0 0 20 0 1 0 12345 0 0";
    assert_eq!(프로세스_상태_파싱(샘플), Some(좀비_상태));
}

#[cfg(target_os = "linux")]
#[test]
fn comm에_공백과_괄호가_있어도_마지막_괄호_뒤_상태를_읽는다() {
    // comm은 `(...)`로 감싸이고 그 안에 공백/괄호를 그대로 담을 수
    // 있으므로, 첫 `)`가 아니라 마지막 `)` 뒤가 상태 필드다.
    let 샘플 = "456 (weird (proc) name) S 1 1 1 0 -1 4194304 10 0 0 0 0 0 0 0 20 0 1 0 9 0 0";
    assert_eq!(프로세스_상태_파싱(샘플), Some('S'));
}

#[cfg(target_os = "linux")]
#[test]
fn 존재하지_않는_pid의_상태는_없다() {
    assert!(프로세스_상태(0).is_none());
    assert!(!프로세스가_살아있다(0));
}

#[test]
fn 제한시간이_없으면_오래_걸려도_완주한다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "sleep 0.3; printf done"]"#,
        json!({}),
    );

    assert_eq!(기본_출력(&result), "done");
}

#[test]
fn 선언된_cwd에서_명령이_실행된다() {
    let dir = 임시_디렉터리("declared_cwd");
    std::fs::write(dir.join("marker.txt"), "x").expect("표식 파일을 쓴다");

    let result = 실행(
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

    assert!(기본_출력(&result).contains("marker.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 상대_cwd는_매니페스트_디렉터리를_기준으로_해석된다() {
    let root = 임시_디렉터리("relative_cwd");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).expect("하위 디렉터리를 만든다");
    std::fs::write(nested.join("nested-marker.txt"), "x").expect("표식 파일을 쓴다");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("origin이 만들어진다");

    let result = 실행_원본(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls"
cwd = "nested""#,
        json!({}),
        Some(&origin),
    );

    assert!(기본_출력(&result).contains("nested-marker.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_매니페스트_도구는_매니페스트_디렉터리에서_실행된다() {
    let root = 임시_디렉터리("project_default_cwd");
    std::fs::write(root.join("project-marker.txt"), "x").expect("표식 파일을 쓴다");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("origin이 만들어진다");

    let result = 실행_원본(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({}),
        Some(&origin),
    );

    assert!(기본_출력(&result).contains("project-marker.txt"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 호출자가_보낸_cwd를_존중한다() {
    let dir = 임시_디렉터리("caller_cwd");
    std::fs::write(dir.join("caller-marker.txt"), "x").expect("표식 파일을 쓴다");

    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: dir.to_string_lossy() } }),
    );

    assert!(기본_출력(&result).contains("caller-marker.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 선언된_cwd가_호출자_cwd보다_우선한다() {
    let declared = 임시_디렉터리("precedence_declared");
    let caller = 임시_디렉터리("precedence_caller");
    std::fs::write(declared.join("declared-marker.txt"), "x").expect("표식 파일을 쓴다");
    std::fs::write(caller.join("caller-marker.txt"), "x").expect("표식 파일을 쓴다");

    let result = 실행(
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

    let out = 기본_출력(&result);
    assert!(out.contains("declared-marker.txt"), "{out}");
    assert!(!out.contains("caller-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&declared);
    let _ = std::fs::remove_dir_all(&caller);
}

#[test]
fn 상대_경로인_호출자_cwd는_invalid_args로_거부된다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: "relative/dir" } }),
    );

    let failure = 실패(result);
    assert_eq!(failure.error.code, INVALID_ARGS_CODE);
    assert!(failure.error.message.contains("absolute"), "{failure:?}");
}

#[test]
fn 존재하지_않는_호출자_cwd는_invalid_args로_거부된다() {
    let missing = std::env::temp_dir().join("upeg_external_contract_missing_dir_zzz");
    let _ = std::fs::remove_dir_all(&missing);

    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: missing.to_string_lossy() } }),
    );

    let failure = 실패(result);
    assert_eq!(failure.error.code, INVALID_ARGS_CODE);
    assert!(
        failure.error.message.contains("existing directory"),
        "{failure:?}"
    );
}

#[test]
fn 선언된_env가_자식_프로세스에_보인다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf '%s' \"$UPEG_TEST_ENV\""]
env = [{ name = "UPEG_TEST_ENV", value = "visible" }]"#,
        json!({}),
    );

    assert_eq!(기본_출력(&result), "visible");
}

#[test]
fn 값이_없는_입력은_선언된_default로_치환된다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "echo"
args_template = ["-n", "{count}"]
inputs = [{ name = "count", type = "integer", default = 10 }]"#,
        json!({}),
    );

    assert_eq!(기본_출력(&result), "10");
}

#[test]
fn 호출자_값이_default를_이긴다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "echo"
args_template = ["-n", "{count}"]
inputs = [{ name = "count", type = "integer", default = 10 }]"#,
        json!({ "count": 3 }),
    );

    assert_eq!(기본_출력(&result), "3");
}

#[test]
fn default가_없는_선택_입력의_토큰은_통째로_사라진다() {
    // `printf '[%s]'` with no argument prints `[]`; with a dropped
    // token it still prints `[]`, but with an empty-string argument it
    // would also print `[]` — so assert on argument *count* instead by
    // echoing `$#`.
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf '%s' \"$#\"", "upeg-test", "{maybe}"]
inputs = [{ name = "maybe", type = "string" }]"#,
        json!({}),
    );

    assert_eq!(기본_출력(&result), "0", "빈 토큰은 인자 목록에서 빠진다");
}

#[test]
fn 성공한_명령의_표준오류는_보조_출력으로_남는다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "sh"
args_template = ["-c", "printf out; printf warn >&2"]"#,
        json!({}),
    );

    let ToolResult::Success(success) = result else {
        panic!("성공을 기대한다");
    };
    assert_eq!(success.primary_output_id.as_deref(), Some("result"));
    let stderr = success
        .outputs
        .iter()
        .find(|entry| entry.id == "stderr")
        .expect("stderr 보조 출력이 있다");
    assert_eq!(
        stderr.value,
        upeg_core::OutputValue::String("warn".to_string())
    );
}

#[test]
fn 표준오류가_비면_보조_출력을_만들지_않는다() {
    let result = 실행(
        r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "printf"
args_template = ["out"]"#,
        json!({}),
    );

    let ToolResult::Success(success) = result else {
        panic!("성공을 기대한다");
    };
    assert_eq!(success.outputs.len(), 1);
}

// ─── 작업 디렉터리 경계 ─────────────────────────────────────────
//
// Project Manifest 도구는 자기 프로젝트에 속한다. 호출자의
// `_upeg.cwd`는 그 프로젝트 안으로만 도구를 옮길 수 있다.

const 목록_명령: &str = r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "ls""#;

#[test]
fn 프로젝트_안을_가리키는_호출자_cwd는_존중된다() {
    let root = 임시_디렉터리("inside_caller_cwd");
    let nested = root.join("member");
    std::fs::create_dir_all(&nested).expect("하위 디렉터리를 만든다");
    std::fs::write(root.join("root-marker.txt"), "x").expect("표식 파일을 쓴다");
    std::fs::write(nested.join("member-marker.txt"), "x").expect("표식 파일을 쓴다");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("origin이 만들어진다");

    let result = 실행_원본(
        목록_명령,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: nested.to_string_lossy() } }),
        Some(&origin),
    );

    let out = 기본_출력(&result);
    assert!(out.contains("member-marker.txt"), "{out}");
    assert!(!out.contains("root-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_밖을_가리키는_호출자_cwd는_무시되고_매니페스트_디렉터리를_쓴다() {
    let root = 임시_디렉터리("outside_project_root");
    let outside = 임시_디렉터리("outside_caller_cwd");
    std::fs::write(root.join("root-marker.txt"), "x").expect("표식 파일을 쓴다");
    std::fs::write(outside.join("outside-marker.txt"), "x").expect("표식 파일을 쓴다");
    let origin =
        ManifestOrigin::project_manifest(&root.join("upeg.toml")).expect("origin이 만들어진다");

    let result = 실행_원본(
        목록_명령,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: outside.to_string_lossy() } }),
        Some(&origin),
    );

    let out = 기본_출력(&result);
    assert!(out.contains("root-marker.txt"), "{out}");
    assert!(!out.contains("outside-marker.txt"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn 툴킷_디렉터리_도구는_어떤_호출자_cwd든_존중한다() {
    let toolkits = 임시_디렉터리("toolkit_origin_dir");
    let elsewhere = 임시_디렉터리("toolkit_caller_cwd");
    std::fs::write(elsewhere.join("elsewhere-marker.txt"), "x").expect("표식 파일을 쓴다");
    let origin =
        ManifestOrigin::toolkit_file(&toolkits.join("dev.toml")).expect("origin이 만들어진다");

    let result = 실행_원본(
        목록_명령,
        json!({ EXECUTION_CONTEXT_ARG: { EXECUTION_CONTEXT_CWD: elsewhere.to_string_lossy() } }),
        Some(&origin),
    );

    assert!(기본_출력(&result).contains("elsewhere-marker.txt"));
    let _ = std::fs::remove_dir_all(&toolkits);
    let _ = std::fs::remove_dir_all(&elsewhere);
}

// ─── 인자 자리 안정성 ───────────────────────────────────────────
//
// `printf '<%s>'`는 남은 인자마다 형식을 다시 쓰므로, 토큰이
// 사라졌는지 빈 인자로 남았는지를 출력만 보고 구분할 수 있다.

fn 자리_확인(입력_선언: &str, 토큰: &str) -> String {
    기본_출력(&실행(
        &format!(
            r#"id = "y.x"
toolkit = "y"
invoker = "External"
command = "printf"
args_template = ["<%s>", "before", "{토큰}", "after"]
{입력_선언}"#
        ),
        json!({}),
    ))
}

const 선택_입력: &str = r#"
[[inputs]]
name = "dir"
type = "string""#;

const 필수_입력: &str = r#"
[[inputs]]
name = "dir"
type = "string"
required = true"#;

#[test]
fn 값이_없는_필수_입력_토큰은_빈_인자로_자리를_지킨다() {
    assert_eq!(자리_확인(필수_입력, "{dir}"), "<before><><after>");
}

#[test]
fn 값이_없는_선택_입력의_단독_토큰만_사라진다() {
    assert_eq!(자리_확인(선택_입력, "{dir}"), "<before><after>");
}

#[test]
fn 리터럴이_붙은_토큰은_값이_없어도_자리를_지킨다() {
    assert_eq!(
        자리_확인(선택_입력, "{dir}/build"),
        "<before></build><after>",
        "`rm -rf {{dir}}/build`가 `rm -rf`로 줄어들면 안 된다"
    );
}
