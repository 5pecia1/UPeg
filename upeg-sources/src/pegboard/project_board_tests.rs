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

fn 임시_스토어(label: &str) -> (PathBuf, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("upeg-project-board-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = state_path_from_root(&root);
    (root, path)
}

fn 스코프(manifest: &str, ids: &[&str]) -> ProjectBoardScope {
    ProjectBoardScope::for_manifest(
        Path::new(manifest),
        ids.iter()
            .map(|id| {
                ProjectBoardDecl::new(BoardKey::parse(id).expect("보드 id"), format!("{id} board"))
            })
            .collect(),
    )
}

fn 보드_키(state: &PegboardState) -> Vec<String> {
    state.boards.iter().map(|b| b.key.clone()).collect()
}

fn 안내_가시성(description: &str, instructions: &str) -> BoardVisibility {
    BoardVisibility::for_project(ProjectBoardScope::for_manifest(
        Path::new("/guidance/upeg.toml"),
        vec![
            ProjectBoardDecl::new(
                BoardKey::parse("project-work").expect("보드 id"),
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
fn 저장한_레이아웃은_현재_매니페스트_안내를_가리지_않는다() {
    let (root, path) = 임시_스토어("guidance-refresh");
    let before = 안내_가시성("이전 설명", "# 이전 지침");
    let after = 안내_가시성("새 설명", "# 새 지침\n\n    cargo test\n");
    let mut state = load_state_from_path_in(&path, &before).expect("로드");
    pin_tool(&mut state, "project-work", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &before).expect("저장");

    let loaded = load_state_from_path_in(&path, &after).expect("재로드");
    let board = loaded
        .boards
        .iter()
        .find(|board| board.key == "project-work")
        .expect("프로젝트 보드");
    assert_eq!(board.guidance.description, "새 설명");
    assert_eq!(board.guidance.instructions, "# 새 지침\n\n    cargo test\n");
    assert!(placement_in(&loaded, "project-work", PINNED_TOOL).is_some());

    let removed = load_state_from_path_in(&path, &안내_가시성("", "")).expect("안내 제거 후 로드");
    assert_eq!(
        removed
            .boards
            .iter()
            .find(|board| board.key == "project-work")
            .expect("프로젝트 보드")
            .guidance,
        BoardGuidance::default()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_안내는_개인_스토어에_복사되지_않는다() {
    let (root, path) = 임시_스토어("guidance-source");
    let visibility = 안내_가시성("프로젝트 설명", "# 프로젝트 지침");
    let state = load_state_from_path_in(&path, &visibility).expect("로드");
    save_state_to_path_in(&path, &state, &visibility).expect("저장");

    let stored = Store::open_at(&path)
        .expect("스토어")
        .load_state(&visibility)
        .expect("행 읽기");
    assert_eq!(
        stored
            .boards
            .iter()
            .find(|board| board.key == "project-work")
            .expect("저장한 행")
            .guidance,
        BoardGuidance::default()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_안내_수정은_원본_경로와_함께_거부된다() {
    let visibility = 안내_가시성("프로젝트 설명", "# 프로젝트 지침");
    let mut state = default_state_in(&visibility);
    let before = state.clone();
    let error = set_board_guidance_in(
        &mut state,
        "project-work",
        BoardGuidance::default(),
        &visibility,
    )
    .expect_err("프로젝트 안내 수정 금지");

    assert!(
        matches!(error, BoardGuidanceEditError::ProjectManifest { board, path }
        if board == "project-work" && path == Path::new("/guidance/upeg.toml"))
    );
    assert_eq!(state, before);
}

#[test]
fn 프로젝트_보드는_매니페스트가_탐지된_동안에만_보인다() {
    let (root, path) = 임시_스토어("visibility");
    let inside = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["proj-a"]));
    let outside = BoardVisibility::global_only();

    let state = load_state_from_path_in(&path, &inside).expect("로드");
    assert!(
        보드_키(&state).contains(&"proj-a".to_string()),
        "프로젝트 안에서는 선언한 보드가 보여야 한다: {:?}",
        보드_키(&state)
    );

    let state = load_state_from_path_in(&path, &outside).expect("로드");
    assert!(
        !보드_키(&state).contains(&"proj-a".to_string()),
        "프로젝트 밖에서는 보이면 안 된다: {:?}",
        보드_키(&state)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_보드_핀은_같은_프로젝트로_돌아오면_남아있다() {
    let (root, path) = 임시_스토어("roundtrip");
    let inside = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["proj-a"]));

    let mut state = load_state_from_path_in(&path, &inside).expect("로드");
    assert_eq!(
        pin_tool(&mut state, "proj-a", PINNED_TOOL),
        PinAction::Pinned
    );
    save_state_to_path_in(&path, &state, &inside).expect("저장");

    let reloaded = load_state_from_path_in(&path, &inside).expect("재로드");
    assert!(
        placement_in(&reloaded, "proj-a", PINNED_TOOL).is_some(),
        "같은 프로젝트로 돌아오면 핀이 남아 있어야 한다"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 다른_프로젝트는_같은_id의_보드를_공유하지_않는다() {
    let (root, path) = 임시_스토어("isolation");
    let a = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["shared"]));
    let b = BoardVisibility::for_project(스코프("/proj-b/upeg.toml", &["shared"]));

    let mut state = load_state_from_path_in(&path, &a).expect("A 로드");
    pin_tool(&mut state, "shared", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &a).expect("A 저장");

    let in_b = load_state_from_path_in(&path, &b).expect("B 로드");
    assert!(
        보드_키(&in_b).contains(&"shared".to_string()),
        "B도 같은 id의 보드를 선언했으므로 보드 자체는 보인다"
    );
    assert!(
        placement_in(&in_b, "shared", PINNED_TOOL).is_none(),
        "다른 프로젝트의 핀이 새어 들어오면 안 된다"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_밖에서_저장해도_프로젝트_보드_핀은_지워지지_않는다() {
    let (root, path) = 임시_스토어("no-sweep");
    let inside = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["proj-a"]));
    let outside = BoardVisibility::global_only();

    let mut state = load_state_from_path_in(&path, &inside).expect("로드");
    pin_tool(&mut state, "proj-a", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &inside).expect("저장");

    // 프로젝트 밖에서 평범한 편집을 하고 전체 상태를 저장한다.
    let mut global = load_state_from_path_in(&path, &outside).expect("밖에서 로드");
    pin_tool(&mut global, "dev", PINNED_TOOL);
    save_state_to_path_in(&path, &global, &outside).expect("밖에서 저장");

    let back = load_state_from_path_in(&path, &inside).expect("다시 로드");
    assert!(
        placement_in(&back, "proj-a", PINNED_TOOL).is_some(),
        "밖에서의 save_state가 프로젝트 보드 행을 tombstone 하면 안 된다"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 선언에서_빠진_프로젝트_보드는_다음_로드에서_사라진다() {
    let (root, path) = 임시_스토어("undeclared");
    let before = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["retired"]));
    let after = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["current"]));

    let mut state = load_state_from_path_in(&path, &before).expect("로드");
    pin_tool(&mut state, "retired", PINNED_TOOL);
    save_state_to_path_in(&path, &state, &before).expect("저장");

    let state = load_state_from_path_in(&path, &after).expect("재로드");
    assert!(!보드_키(&state).contains(&"retired".to_string()));
    assert!(보드_키(&state).contains(&"current".to_string()));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_보드는_매니페스트가_선언한_라벨을_제목으로_쓴다() {
    let (root, path) = 임시_스토어("label");
    let inside = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["proj-a"]));

    let state = load_state_from_path_in(&path, &inside).expect("로드");
    let board = state
        .boards
        .iter()
        .find(|board| board.key == "proj-a")
        .expect("프로젝트 보드");
    assert_eq!(board.title, "proj-a board");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn 프로젝트_보드는_선언한_전역_보드_키를_덮어쓰지_않는다() {
    // `is_builtin_board` 가 로더에서 이미 내장 키를 막지만, 사용자가
    // 만든 전역 보드와의 충돌은 런타임에만 알 수 있다. 이때는 전역
    // 보드의 행이 살아남아야 한다 (tombstone 판정은 가시 키 공간에서
    // 이뤄지므로).
    let (root, path) = 임시_스토어("shadow");
    let outside = BoardVisibility::global_only();
    let inside = BoardVisibility::for_project(스코프("/proj-a/upeg.toml", &["ops"]));

    let mut global = load_state_from_path_in(&path, &outside).expect("로드");
    add_board(&mut global, "Ops").expect("전역 보드 추가");
    pin_tool(&mut global, "ops", PINNED_TOOL);
    save_state_to_path_in(&path, &global, &outside).expect("저장");

    // 프로젝트 안에서는 프로젝트 보드가 이긴다 (비어 있다).
    let mut shadowed = load_state_from_path_in(&path, &inside).expect("프로젝트 로드");
    assert!(placement_in(&shadowed, "ops", PINNED_TOOL).is_none());
    pin_tool(&mut shadowed, "ops", "num.decimal_to_hex");
    save_state_to_path_in(&path, &shadowed, &inside).expect("프로젝트 저장");

    // 밖으로 나오면 전역 보드의 핀이 그대로 돌아온다.
    let back = load_state_from_path_in(&path, &outside).expect("다시 밖에서 로드");
    assert!(
        placement_in(&back, "ops", PINNED_TOOL).is_some(),
        "가려졌던 전역 보드의 핀이 사라지면 안 된다"
    );
    assert!(
        placement_in(&back, "ops", "num.decimal_to_hex").is_none(),
        "프로젝트 보드의 핀이 전역 보드로 새면 안 된다"
    );

    let _ = std::fs::remove_dir_all(&root);
}
