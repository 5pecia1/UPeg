use std::ffi::OsString;
use std::path::PathBuf;

use upeg_runtime::execution_requirements::{CommandSearchPath, tool_execution_requirements};

use crate::load_and_register_dir_verbose;

fn 도구_디렉터리(label: &str, command: &str, extra: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("upeg-requirements-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("디렉터리 생성");
    std::fs::write(
        root.join("kit.toml"),
        format!(
            r#"
id = "requirements-{label}"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "External"
command = "{command}"
{extra}
"#
        ),
    )
    .expect("매니페스트 쓰기");
    root
}

#[test]
fn 등록된_실행_요건은_명령과_매니페스트_기준_작업경로를_보존한다() {
    let root = 도구_디렉터리("cwd", " git ", r#"cwd = " workspace ""#);
    let outcome = load_and_register_dir_verbose(&root);
    assert!(outcome.failed.is_empty(), "등록 실패: {:?}", outcome.failed);
    let requirements = tool_execution_requirements("requirements-cwd.run").expect("실행 요건");

    assert_eq!(requirements.command.as_deref(), Some("git"));
    assert_eq!(
        requirements.declared_working_directory,
        Some(root.join("workspace"))
    );
    assert_eq!(requirements.project_root, None);
    assert_eq!(requirements.search_path, CommandSearchPath::Inherit);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 프로젝트_실행_요건은_호출자가_벗어날_수_없는_루트를_보존한다() {
    let root = 도구_디렉터리("project", "git", "");
    let outcome = crate::load_and_register_file_verbose(&root.join("kit.toml"));
    assert!(outcome.failed.is_empty(), "등록 실패: {:?}", outcome.failed);
    let requirements = tool_execution_requirements("requirements-project.run").expect("실행 요건");

    assert_eq!(requirements.project_root, Some(root.clone()));
    assert_eq!(requirements.declared_working_directory, None);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn 명령_탐색_경로는_마지막_선언과_크리덴셜_우선순위를_보존한다() {
    let root = 도구_디렉터리(
        "path",
        "git",
        r#"env = [{ name = " PATH ", value = "/old" }, { name = "PATH", value = "/new" }]"#,
    );
    let outcome = load_and_register_dir_verbose(&root);
    assert!(outcome.failed.is_empty(), "등록 실패: {:?}", outcome.failed);
    assert_eq!(
        tool_execution_requirements("requirements-path.run")
            .expect("실행 요건")
            .search_path,
        CommandSearchPath::Declared(OsString::from("/new"))
    );

    let secret_root = 도구_디렉터리(
        "secret",
        "git",
        r#"
env = [{ name = "PATH", value = "/plain" }]
credentials = [{ name = "command-path", target = " PATH " }]
"#,
    );
    let outcome = load_and_register_dir_verbose(&secret_root);
    assert!(
        outcome.failed.is_empty(),
        "크리덴셜을 읽지 않고 등록: {:?}",
        outcome.failed
    );
    assert_eq!(
        tool_execution_requirements("requirements-secret.run")
            .expect("실행 요건")
            .search_path,
        CommandSearchPath::Credential
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(secret_root);
}

#[test]
fn 다른_인보커로_재등록하면_이전_실행_요건은_사라진다() {
    let root = 도구_디렉터리("replace", "git", "");
    assert!(load_and_register_dir_verbose(&root).failed.is_empty());
    assert!(tool_execution_requirements("requirements-replace.run").is_some());
    std::fs::write(
        root.join("kit.toml"),
        r#"
id = "requirements-replace"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "Http"
url = "https://example.org"
"#,
    )
    .expect("매니페스트 교체");
    let outcome = load_and_register_dir_verbose(&root);
    assert!(outcome.failed.is_empty(), "등록 실패: {:?}", outcome.failed);

    assert!(tool_execution_requirements("requirements-replace.run").is_none());
    let _ = std::fs::remove_dir_all(root);
}
