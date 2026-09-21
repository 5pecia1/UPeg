//! Project-declared boards: visibility inside/outside the project, and
//! the namespaced persistence that keeps two projects apart.
//!
//! Every test drives an explicit [`BoardVisibility`] instead of the
//! process-global scope, so they are order-independent and can put "in
//! project A", "in project B", and "outside any project" in one run.

use std::path::Path;

use upeg_core::BoardKey;
use upeg_runtime::pegboard_project::{ProjectBoardDecl, ProjectBoardScope};

use super::*;

const PINNED_TOOL: &str = "num.hex_to_decimal";

fn temp_store(label: &str) -> (PathBuf, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("upeg-project-board-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    (root, path)
}

fn scope(manifest: &str, ids: &[&str]) -> ProjectBoardScope {
    ProjectBoardScope::for_manifest(
        Path::new(manifest),
        ids.iter()
            .map(|id| {
                ProjectBoardDecl::new(
                    BoardKey::parse(id).expect("board id"),
                    format!("{id} board"),
                )
            })
            .collect(),
    )
}

fn board_keys(state: &PegboardState) -> Vec<String> {
    state.boards.iter().map(|b| b.key.clone()).collect()
}

fn guidance_visibility(description: &str, instructions: &str) -> BoardVisibility {
    BoardVisibility::for_project(ProjectBoardScope::for_manifest(
        Path::new("/guidance/upeg.toml"),
        vec![
            ProjectBoardDecl::new(
                BoardKey::parse("project-work").expect("board id"),
                "작업".into(),
            )
            .with_guidance(BoardGuidance {
                description: description.into(),
                instructions: instructions.into(),
            }),
        ],
    ))
}

#[test]
fn saved_layout_does_not_shadow_current_manifest_guidance() {
    let (root, path) = temp_store("guidance-refresh");
    let before = guidance_visibility("이전 설명", "# 이전 지침");
    let after = guidance_visibility("새 설명", "# 새 지침\n\n    cargo test\n");
    let mut state = load_state_from_path_in(&path, &before).expect("load");
    pin_tool(&mut state, "project-work", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &before).expect("save");

    let loaded = load_state_from_path_in(&path, &after).expect("reload");
    let board = loaded
        .boards
        .iter()
        .find(|board| board.key == "project-work")
        .expect("project board");
    assert_eq!(board.guidance.description, "새 설명");
    assert_eq!(board.guidance.instructions, "# 새 지침\n\n    cargo test\n");
    assert!(placement_in(&loaded, "project-work", PINNED_TOOL).is_some());

    let removed = load_state_from_path_in(&path, &guidance_visibility("", ""))
        .expect("load after guidance removal");
    assert_eq!(
        removed
            .boards
            .iter()
            .find(|board| board.key == "project-work")
            .expect("project board")
            .guidance,
        BoardGuidance::default()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_guidance_is_not_copied_into_personal_store() {
    let (root, path) = temp_store("guidance-source");
    let visibility = guidance_visibility("프로젝트 설명", "# 프로젝트 지침");
    let state = load_state_from_path_in(&path, &visibility).expect("load");
    save_state_to_path_in(&path, &state, &visibility).expect("save");

    let stored = Store::open_at(&path)
        .expect("store")
        .load_state(&visibility)
        .expect("read rows");
    assert_eq!(
        stored
            .boards
            .iter()
            .find(|board| board.key == "project-work")
            .expect("stored row")
            .guidance,
        BoardGuidance::default()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_guidance_edit_is_rejected_with_source_path() {
    let visibility = guidance_visibility("프로젝트 설명", "# 프로젝트 지침");
    let mut state = default_state_in(&visibility);
    let before = state.clone();
    let error = set_board_guidance_in(
        &mut state,
        "project-work",
        BoardGuidance::default(),
        &visibility,
    )
    .expect_err("project guidance edit forbidden");

    assert!(
        matches!(error, BoardGuidanceEditError::ProjectManifest { board, path }
        if board == "project-work" && path == Path::new("/guidance/upeg.toml"))
    );
    assert_eq!(state, before);
}

#[test]
fn project_board_visible_only_while_manifest_detected() {
    let (root, path) = temp_store("visibility");
    let inside = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["proj-a"]));
    let outside = BoardVisibility::global_only();

    let state = load_state_from_path_in(&path, &inside).expect("load");
    assert!(
        board_keys(&state).contains(&"proj-a".to_string()),
        "inside the project the declared board must be visible: {:?}",
        board_keys(&state)
    );

    let state = load_state_from_path_in(&path, &outside).expect("load");
    assert!(
        !board_keys(&state).contains(&"proj-a".to_string()),
        "outside the project it must not be visible: {:?}",
        board_keys(&state)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_board_pin_survives_return_to_same_project() {
    let (root, path) = temp_store("roundtrip");
    let inside = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["proj-a"]));

    let mut state = load_state_from_path_in(&path, &inside).expect("load");
    assert_eq!(
        pin_tool(&mut state, "proj-a", PINNED_TOOL),
        PinAction::Pinned
    );
    save_state_to_path_in(&path, &state, &inside).expect("save");

    let reloaded = load_state_from_path_in(&path, &inside).expect("reload");
    assert!(
        placement_in(&reloaded, "proj-a", PINNED_TOOL).is_some(),
        "the pin must survive returning to the same project"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn different_projects_do_not_share_same_id_board() {
    let (root, path) = temp_store("isolation");
    let a = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["shared"]));
    let b = BoardVisibility::for_project(scope("/proj-b/upeg.toml", &["shared"]));

    let mut state = load_state_from_path_in(&path, &a).expect("load A");
    pin_tool(&mut state, "shared", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &a).expect("save A");

    let in_b = load_state_from_path_in(&path, &b).expect("load B");
    assert!(
        board_keys(&in_b).contains(&"shared".to_string()),
        "B declared a board with the same id, so the board itself is visible"
    );
    assert!(
        placement_in(&in_b, "shared", PINNED_TOOL).is_none(),
        "another project's pin must not leak in"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saving_outside_project_does_not_erase_project_board_pins() {
    let (root, path) = temp_store("no-sweep");
    let inside = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["proj-a"]));
    let outside = BoardVisibility::global_only();

    let mut state = load_state_from_path_in(&path, &inside).expect("load");
    pin_tool(&mut state, "proj-a", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &inside).expect("save");

    // Do an ordinary edit outside the project and save the whole state.
    let mut global = load_state_from_path_in(&path, &outside).expect("load outside");
    pin_tool(&mut global, "dev", PINNED_TOOL);
    save_state_to_path_in(&path, &global, &outside).expect("save outside");

    let back = load_state_from_path_in(&path, &inside).expect("load again");
    assert!(
        placement_in(&back, "proj-a", PINNED_TOOL).is_some(),
        "an outside save_state must not tombstone project board rows"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_board_removed_from_declaration_disappears_on_next_load() {
    let (root, path) = temp_store("undeclared");
    let before = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["retired"]));
    let after = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["current"]));

    let mut state = load_state_from_path_in(&path, &before).expect("load");
    pin_tool(&mut state, "retired", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &before).expect("save");

    let state = load_state_from_path_in(&path, &after).expect("reload");
    assert!(!board_keys(&state).contains(&"retired".to_string()));
    assert!(board_keys(&state).contains(&"current".to_string()));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_board_uses_manifest_declared_label_as_title() {
    let (root, path) = temp_store("label");
    let inside = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["proj-a"]));

    let state = load_state_from_path_in(&path, &inside).expect("load");
    let board = state
        .boards
        .iter()
        .find(|board| board.key == "proj-a")
        .expect("project board");
    assert_eq!(board.title, "proj-a board");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_board_does_not_overwrite_same_keyed_global_board() {
    // `is_builtin_board` already blocks built-in keys in the loader,
    // but a collision with a user-made global board can only be known
    // at runtime. In that case the global board's rows must survive
    // (tombstone decisions happen in the visible key space).
    let (root, path) = temp_store("shadow");
    let outside = BoardVisibility::global_only();
    let inside = BoardVisibility::for_project(scope("/proj-a/upeg.toml", &["ops"]));

    let mut global = load_state_from_path_in(&path, &outside).expect("load");
    add_board(&mut global, "Ops").expect("add global board");
    pin_tool(&mut global, "ops", PINNED_TOOL);
    save_state_to_path_in(&path, &global, &outside).expect("save");

    // Inside the project the project board wins (it is empty).
    let mut shadowed = load_state_from_path_in(&path, &inside).expect("project load");
    assert!(placement_in(&shadowed, "ops", PINNED_TOOL).is_none());
    pin_tool(&mut shadowed, "ops", "num.decimal_to_hex");
    save_state_to_path_in(&path, &shadowed, &inside).expect("project save");

    // Back outside, the global board's pins return unchanged.
    let back = load_state_from_path_in(&path, &outside).expect("load outside again");
    assert!(
        placement_in(&back, "ops", PINNED_TOOL).is_some(),
        "the shadowed global board's pin must not disappear"
    );
    assert!(
        placement_in(&back, "ops", "num.decimal_to_hex").is_none(),
        "a project board pin must not leak into the global board"
    );

    let _ = std::fs::remove_dir_all(&root);
}
