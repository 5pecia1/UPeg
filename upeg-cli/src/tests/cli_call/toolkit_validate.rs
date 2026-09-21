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
fn toolkit_validate_reports_all_files_ok_in_a_valid_directory() {
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
fn toolkit_validate_returns_the_full_report_when_one_file_fails() {
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
fn toolkit_validate_skips_non_toolkit_toml_and_validates_the_rest() {
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
fn toolkit_validate_reports_no_files_for_an_empty_directory() {
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
fn toolkit_validate_returns_a_clear_error_for_a_missing_directory() {
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
fn host_skipped_tool(id: &str) -> upeg_loader::SkippedTool {
    upeg_loader::SkippedTool {
        id: id.to_string(),
        reason: upeg_loader::LoadError::PtyUnsupportedOnHost,
    }
}

#[test]
fn host_skipped_tools_are_indented_below_the_file_row_with_reasons() {
    let rendered = crate::surfaces::cli::formatters::render_tool_skip_lines(&[
        host_skipped_tool("tkv.terminal"),
        host_skipped_tool("tkv.shell"),
    ]);

    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 2, "got:\n{rendered}");
    for (line, id) in lines.iter().zip(["tkv.terminal", "tkv.shell"]) {
        assert!(
            line.starts_with(&format!("    ~ {id}: skipped (")),
            "tool rows must be indented one level below the file row (`  ok:`): {line}"
        );
        assert!(
            line.contains(&upeg_loader::LoadError::PtyUnsupportedOnHost.to_string()),
            "skipped tools must include a reason rather than leave an unexplained gap: {line}"
        );
    }
    assert!(
        !rendered.contains("error:"),
        "a skipped tool is not a validation failure: {rendered}"
    );
}

#[test]
fn summary_distinguishes_skipped_tool_counts_from_skipped_file_counts() {
    use crate::surfaces::cli::formatters::render_toolkit_validate_summary;

    // Omit the tool-skip clause when no tools were skipped; repeating
    // "skipped" in every run would make the two counts ambiguous.
    assert_eq!(
        render_toolkit_validate_summary(1, 0, 1, 0),
        "1 ok, 0 failed, 1 skipped\n"
    );

    let summary = render_toolkit_validate_summary(1, 0, 1, 2);
    assert!(
        summary.starts_with("1 ok, 0 failed, 1 skipped, 2 "),
        "the skipped-file count must remain unchanged: {summary}"
    );
    assert!(
        summary.contains("tool(s)"),
        "the count must identify what it measures: {summary}"
    );
    assert_ne!(
        summary.matches("skipped").count(),
        summary.matches("skipped (not a toolkit manifest)").count(),
        "reusing the file-skip wording would make the two counts appear identical"
    );
}

#[test]
fn toolkit_validate_omits_tilde_rows_when_no_tools_are_skipped() {
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
        "no skip rows should appear when nothing was skipped:\n{out}"
    );
    assert!(out.contains("1 ok, 0 failed, 0 skipped\n"), "got:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
