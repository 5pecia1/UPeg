#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Project-declared boards (`[[boards]]` in a Project Manifest).
//!
//! Two contracts meet here and are checked together, because a change to
//! either one silently breaks the other:
//!
//!   * the *grammar* — what a manifest may declare and what the loader
//!     refuses (`docs/architecture/project-manifest.md`); and
//!   * the *repo's own dogfood manifest*, which declares `upeg-dev` and
//!     pins all 15 maintenance tools there. If the grammar drifts, the
//!     manifest the maintainers actually run every day is the first
//!     thing that should go red.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// The `[[boards]]` example the docs point at.
#[test]
fn 프로젝트_매니페스트_예제는_boards를_선언한다() {
    let path = workspace_root().join("examples/project-manifest/upeg.toml");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    // `parse_toolkit_full` runs the same `[[boards]]` validation the
    // Project Manifest loader does (canonical id, no `:`, no built-in
    // shadowing, no duplicates), so a bad declaration fails here.
    let (_toolkit, tools) =
        upeg_loader::parse_toolkit_full(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert!(
        raw.contains("[[boards]]"),
        "예제의 핵심은 `[[boards]]` 선언이다"
    );
    assert!(
        tools
            .iter()
            .all(|(meta, _)| meta.boards.contains(&"demo-app")),
        "예제의 모든 Tool 은 선언한 프로젝트 board 를 가리켜야 한다"
    );
}

/// The repo dogfoods its own project board: `/<repo>/upeg.toml` declares
/// `upeg-dev` and every maintenance tool pins there instead of onto the
/// built-in `dev` board it used to share with the shipped tools.
#[test]
fn 저장소_dogfood_매니페스트는_전용_board에_도구를_모은다() {
    const PROJECT_BOARD: &str = "upeg-dev";

    let path = workspace_root().join("upeg.toml");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let (_toolkit, tools) =
        upeg_loader::parse_toolkit_full(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert!(
        raw.contains(&format!("id = \"{PROJECT_BOARD}\"")),
        "dogfood 매니페스트는 `{PROJECT_BOARD}` board 를 선언한다"
    );
    let off_board: Vec<&str> = tools
        .iter()
        .filter(|(meta, _)| !meta.boards.contains(&PROJECT_BOARD))
        .map(|(meta, _)| meta.id)
        .collect();
    assert!(
        off_board.is_empty(),
        "dev 도구는 모두 `{PROJECT_BOARD}` 에 있어야 한다: {off_board:?}"
    );
    assert!(
        tools.iter().all(|(meta, _)| !meta.boards.contains(&"dev")),
        "내장 `dev` board 에 남아 있는 도구가 있으면 안 된다"
    );
}
