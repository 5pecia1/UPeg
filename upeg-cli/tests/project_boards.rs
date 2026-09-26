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
//!     refuses (`upeg_sources::project` module docs); and
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
fn the_project_manifest_example_declares_boards() {
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
        "the example's point is the `[[boards]]` declaration"
    );
    assert!(
        tools
            .iter()
            .all(|(meta, _)| meta.boards.contains(&"demo-app")),
        "every Tool in the example must point at the declared project board"
    );
}

/// The repo dogfoods its own project board: `/<repo>/upeg.toml` declares
/// `upeg-dev` and every maintenance tool pins there instead of onto the
/// built-in `dev` board it used to share with the shipped tools.
#[test]
fn the_repo_dogfood_manifest_gathers_tools_on_a_dedicated_board() {
    const PROJECT_BOARD: &str = "upeg-dev";

    let path = workspace_root().join("upeg.toml");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let (_toolkit, tools) =
        upeg_loader::parse_toolkit_full(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert!(
        raw.contains(&format!("id = \"{PROJECT_BOARD}\"")),
        "the dogfood manifest declares the `{PROJECT_BOARD}` board"
    );
    let off_board: Vec<&str> = tools
        .iter()
        .filter(|(meta, _)| !meta.boards.contains(&PROJECT_BOARD))
        .map(|(meta, _)| meta.id)
        .collect();
    assert!(
        off_board.is_empty(),
        "every dev tool must be on `{PROJECT_BOARD}`: {off_board:?}"
    );
    assert!(
        tools.iter().all(|(meta, _)| !meta.boards.contains(&"dev")),
        "no tool may be left on the built-in `dev` board"
    );
}
