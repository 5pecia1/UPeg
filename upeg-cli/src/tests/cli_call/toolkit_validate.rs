//! `upeg toolkit validate [dir]` — batched sibling of `upeg tool validate`.
//! Extracted alongside `validate.rs` for the same file-size-budget reason.

use super::*;

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn 유효한_디렉터리는_모든_파일을_ok로_보고한다() {
    let dir = scratch_dir("upeg_cli_toolkit_validate_valid");
    std::fs::write(
        dir.join("a.toml"),
        v21_single_tool_toml(
            r#"id = "tkv_a.one"
toolkit = "tkv_a"
invoker = "External"
command = "echo""#,
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("b.toml"),
        v21_single_tool_toml(
            r#"id = "tkv_b.two"
toolkit = "tkv_b"
invoker = "External"
command = "echo""#,
        ),
    )
    .unwrap();

    let out = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        dir.to_str().unwrap(),
    ]))
    .expect("all-valid directory must succeed");
    assert!(out.contains("ok: a.toml"), "got:\n{out}");
    assert!(out.contains("ok: b.toml"), "got:\n{out}");
    assert!(out.contains("2 ok, 0 failed"), "got:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 깨진_파일이_하나면_전체_보고서를_출력하고_실패한다() {
    let dir = scratch_dir("upeg_cli_toolkit_validate_broken");
    std::fs::write(
        dir.join("good.toml"),
        v21_single_tool_toml(
            r#"id = "tkv_good.one"
toolkit = "tkv_good"
invoker = "External"
command = "echo""#,
        ),
    )
    .unwrap();
    // Has the toolkit-manifest shape (top-level `id` + `tools`) so it is a
    // real error, not a skip — but `not_a_real_field` trips
    // `ToolkitToml`'s `deny_unknown_fields`, so parse_toolkit_full must
    // still reject it.
    std::fs::write(
        dir.join("broken.toml"),
        "id = \"tkv_broken\"\ntools = [{ id = \"tkv_broken.one\" }]\nnot_a_real_field = true\n",
    )
    .unwrap();

    let r = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        dir.to_str().unwrap(),
    ]));
    match r {
        Err(CliError::StdoutFailure { stdout }) => {
            assert!(stdout.contains("ok: good.toml"), "got:\n{stdout}");
            assert!(stdout.contains("error: broken.toml"), "got:\n{stdout}");
            assert!(stdout.contains("1 ok, 1 failed"), "got:\n{stdout}");
        }
        other => panic!("expected StdoutFailure carrying the full report, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 툴킷_형태가_아닌_toml은_건너뛰고_나머지는_정상_검증한다() {
    let dir = scratch_dir("upeg_cli_toolkit_validate_non_toolkit_shape");
    std::fs::write(
        dir.join("a_toolkit.toml"),
        v21_single_tool_toml(
            r#"id = "tkv_shape.one"
toolkit = "tkv_shape"
invoker = "External"
command = "echo""#,
        ),
    )
    .unwrap();
    // Shaped like a Cargo.toml, not a toolkit manifest: no top-level `id`
    // string + `tools` array, so it must be skipped rather than failed.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    )
    .unwrap();

    let out = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        dir.to_str().unwrap(),
    ]))
    .expect("a non-toolkit-shaped TOML file must not fail the whole run");
    assert!(out.contains("ok: a_toolkit.toml"), "got:\n{out}");
    assert!(
        out.contains("Cargo.toml: skipped (not a toolkit manifest)"),
        "got:\n{out}"
    );
    assert!(out.contains("1 ok, 0 failed, 1 skipped"), "got:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 비어있는_디렉터리는_파일이_없다는_메시지를_보고한다() {
    let dir = scratch_dir("upeg_cli_toolkit_validate_empty");
    let out = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        dir.to_str().unwrap(),
    ]))
    .expect("an existing-but-empty directory is not an error");
    assert!(
        out.contains("no *.toml files found"),
        "empty dir must say so; got:\n{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 존재하지_않는_디렉터리는_깨끗한_오류를_반환한다() {
    let r = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        "/no/such/upeg/toolkit/validate/dir",
    ]));
    match r {
        Err(CliError::ToolFailed(msg)) => assert!(
            msg.contains("not found"),
            "missing dir must say `not found`; got: {msg}"
        ),
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

/// A skip only happens on a host that cannot honour the declaration —
/// every unix opens a pseudoterminal, so no fixture manifest can produce
/// one here. These two exercise the rendering directly instead, which is
/// the half that was missing: `LoadOutcome::skipped` was recorded and
/// then never printed anywhere.
fn 건너뛴_도구(id: &str) -> upeg_loader::SkippedTool {
    upeg_loader::SkippedTool {
        id: id.to_string(),
        reason: upeg_loader::LoadError::PtyUnsupportedOnHost,
    }
}

#[test]
fn 호스트가_건너뛴_도구는_파일_줄_아래에_사유와_함께_들여쓰기된다() {
    let rendered = crate::surfaces::cli::formatters::render_tool_skip_lines(&[
        건너뛴_도구("tkv.terminal"),
        건너뛴_도구("tkv.shell"),
    ]);

    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 2, "got:\n{rendered}");
    for (line, id) in lines.iter().zip(["tkv.terminal", "tkv.shell"]) {
        assert!(
            line.starts_with(&format!("    ~ {id}: skipped (")),
            "파일 줄(`  ok:`)보다 한 단계 더 들여써야 소속이 분명하다: {line}"
        );
        assert!(
            line.contains(&upeg_loader::LoadError::PtyUnsupportedOnHost.to_string()),
            "사유 없는 건너뜀은 구멍만 남긴다: {line}"
        );
    }
    assert!(
        !rendered.contains("error:"),
        "건너뜀은 검증 실패가 아니다: {rendered}"
    );
}

#[test]
fn 요약의_도구_건너뜀_집계는_파일_건너뜀_집계와_구별된다() {
    use crate::surfaces::cli::formatters::render_toolkit_validate_summary;

    // 도구를 건너뛴 게 없으면 집계 절도 붙지 않는다 — 매 실행마다
    // "skipped"가 두 번 나오면 서로 헷갈린다.
    assert_eq!(
        render_toolkit_validate_summary(1, 0, 1, 0),
        "1 ok, 0 failed, 1 skipped\n"
    );

    let 요약 = render_toolkit_validate_summary(1, 0, 1, 2);
    assert!(
        요약.starts_with("1 ok, 0 failed, 1 skipped, 2 "),
        "파일 건너뜀 집계는 그대로 남아야 한다: {요약}"
    );
    assert!(
        요약.contains("tool(s)"),
        "무엇을 센 숫자인지 말해야 한다: {요약}"
    );
    assert_ne!(
        요약.matches("skipped").count(),
        요약.matches("skipped (not a toolkit manifest)").count(),
        "파일 단위 건너뜀 문구를 재사용하면 두 집계가 같은 것으로 읽힌다"
    );
}

#[test]
fn 도구를_건너뛰지_않은_디렉터리_검증은_물결표_줄을_남기지_않는다() {
    let dir = scratch_dir("upeg_cli_toolkit_validate_no_tool_skip");
    std::fs::write(
        dir.join("a.toml"),
        v21_single_tool_toml(
            r#"id = "tkv_noskip.one"
toolkit = "tkv_noskip"
invoker = "External"
command = "echo""#,
        ),
    )
    .unwrap();

    let out = run(parse(&[
        "upeg",
        "toolkit",
        "validate",
        dir.to_str().unwrap(),
    ]))
    .expect("valid dir");
    assert!(out.contains("ok: a.toml"), "got:\n{out}");
    assert!(
        !out.contains(" ~ "),
        "건너뛴 게 없으면 조용해야 한다:\n{out}"
    );
    assert!(out.contains("1 ok, 0 failed, 0 skipped\n"), "got:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
