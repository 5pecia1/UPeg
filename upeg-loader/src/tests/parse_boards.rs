//! Top-level `[[boards]]` — the Project Manifest's board declarations.

use crate::LoadError;
use crate::parse::parse_manifest;

fn 매니페스트(boards: &str) -> String {
    format!(
        r#"id = "proj"
{boards}

[[tools]]
id = "echo"
pegboard_units = "U1"
invoker = "External"
command = "echo"
"#
    )
}

#[test]
fn 보드_설명과_마크다운_지침을_매니페스트에서_읽는다() {
    let parsed = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "upeg-dev"
description = "개발 작업 공간"
instructions = """
# 작업 순서
1. 변경을 확인한다.
2. 테스트를 실행한다.
""""#,
    ));

    let parsed = parsed.expect("보드 안내 파싱");
    assert_eq!(parsed.boards[0].guidance.description, "개발 작업 공간");
    assert_eq!(
        parsed.boards[0].guidance.instructions,
        "# 작업 순서\n1. 변경을 확인한다.\n2. 테스트를 실행한다.\n"
    );
}

#[test]
fn 보드_안내는_각각_생략할_수_있다() {
    let parsed = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "empty"
[[boards]]
id = "described"
description = "설명만"
[[boards]]
id = "instructed"
instructions = "지침만""#,
    ))
    .expect("파싱");

    assert_eq!(
        parsed.boards[0].guidance,
        upeg_core::BoardGuidance::default()
    );
    assert_eq!(parsed.boards[1].guidance.description, "설명만");
    assert!(parsed.boards[1].guidance.instructions.is_empty());
    assert!(parsed.boards[2].guidance.description.is_empty());
    assert_eq!(parsed.boards[2].guidance.instructions, "지침만");
}

#[test]
fn boards_선언은_id와_라벨을_내린다() {
    let parsed = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "upeg-dev"
label = "upeg dev""#,
    ))
    .expect("파싱");

    assert_eq!(parsed.boards.len(), 1);
    assert_eq!(parsed.boards[0].id.as_str(), "upeg-dev");
    assert_eq!(parsed.boards[0].label, "upeg dev");
}

#[test]
fn 라벨이_없으면_id를_그대로_쓴다() {
    let parsed = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "upeg-dev""#,
    ))
    .expect("파싱");

    assert_eq!(parsed.boards[0].label, "upeg-dev");
}

#[test]
fn boards_없는_매니페스트는_보드를_선언하지_않는다() {
    let parsed = parse_manifest(&매니페스트("")).expect("파싱");
    assert!(parsed.boards.is_empty());
}

#[test]
fn 내장_보드_id는_거부된다() {
    let error = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "dev""#,
    ))
    .expect_err("내장 보드 shadowing은 거부되어야 한다");

    assert!(
        matches!(
            &error,
            LoadError::BuiltinProjectBoardId { position: 0, board } if board == "dev"
        ),
        "예상 밖 오류: {error}"
    );
}

#[test]
fn 중복_보드_id는_거부된다() {
    let error = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "one"

[[boards]]
id = "one""#,
    ))
    .expect_err("중복 선언은 거부되어야 한다");

    assert!(
        matches!(
            error,
            LoadError::DuplicateProjectBoardId { position: 1, .. }
        ),
        "예상 밖 오류: {error}"
    );
}

#[test]
fn 예약된_구분자를_품은_보드_id는_거부된다() {
    let error = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "project:abc:dev""#,
    ))
    .expect_err("`:` 는 store key 예약 문자다");

    assert!(
        matches!(error, LoadError::InvalidProjectBoardId { position: 0, .. }),
        "예상 밖 오류: {error}"
    );
}

#[test]
fn 빈_보드_id는_거부된다() {
    let error = parse_manifest(&매니페스트(
        r#"[[boards]]
id = "  ""#,
    ))
    .expect_err("빈 id는 거부되어야 한다");

    assert!(
        matches!(error, LoadError::InvalidProjectBoardId { position: 0, .. }),
        "예상 밖 오류: {error}"
    );
}

#[test]
fn toolkit_디렉터리_매니페스트의_boards는_등록에서_거부된다() {
    let dir =
        std::env::temp_dir().join(format!("upeg-loader-boards-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("디렉터리 생성");
    std::fs::write(
        dir.join("kit.toml"),
        매니페스트(
            r#"[[boards]]
id = "kit-board""#,
        ),
    )
    .expect("매니페스트 쓰기");

    let outcome = crate::load_and_register_dir_verbose(&dir);

    assert!(
        outcome.loaded.is_empty(),
        "보드를 선언한 toolkit은 등록되면 안 된다"
    );
    assert!(
        outcome
            .failed
            .iter()
            .any(|(_, error)| matches!(error, LoadError::BoardsOutsideProjectManifest)),
        "예상 밖 실패 목록: {:?}",
        outcome.failed
    );

    let _ = std::fs::remove_dir_all(&dir);
}
