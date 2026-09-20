//! Top-level `[[boards]]` — the Project Manifest's board declarations.

use crate::LoadError;
use crate::parse::parse_manifest;

fn manifest(boards: &str) -> String {
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
fn board_description_and_markdown_instructions_are_read_from_manifest() {
    let parsed = parse_manifest(&manifest(
        r#"[[boards]]
id = "upeg-dev"
description = "development workspace"
instructions = """
# Work order
1. Review the change.
2. Run the tests.
""""#,
    ));

    let parsed = parsed.expect("parse board guidance");
    assert_eq!(
        parsed.boards[0].guidance.description,
        "development workspace"
    );
    assert_eq!(
        parsed.boards[0].guidance.instructions,
        "# Work order\n1. Review the change.\n2. Run the tests.\n"
    );
}

#[test]
fn board_guidance_fields_can_each_be_omitted() {
    let parsed = parse_manifest(&manifest(
        r#"[[boards]]
id = "empty"
[[boards]]
id = "described"
description = "description only"
[[boards]]
id = "instructed"
instructions = "instructions only""#,
    ))
    .expect("parse");

    assert_eq!(
        parsed.boards[0].guidance,
        upeg_core::BoardGuidance::default()
    );
    assert_eq!(parsed.boards[1].guidance.description, "description only");
    assert!(parsed.boards[1].guidance.instructions.is_empty());
    assert!(parsed.boards[2].guidance.description.is_empty());
    assert_eq!(parsed.boards[2].guidance.instructions, "instructions only");
}

#[test]
fn boards_declaration_lowers_id_and_label() {
    let parsed = parse_manifest(&manifest(
        r#"[[boards]]
id = "upeg-dev"
label = "upeg dev""#,
    ))
    .expect("parse");

    assert_eq!(parsed.boards.len(), 1);
    assert_eq!(parsed.boards[0].id.as_str(), "upeg-dev");
    assert_eq!(parsed.boards[0].label, "upeg dev");
}

#[test]
fn missing_label_falls_back_to_id() {
    let parsed = parse_manifest(&manifest(
        r#"[[boards]]
id = "upeg-dev""#,
    ))
    .expect("parse");

    assert_eq!(parsed.boards[0].label, "upeg-dev");
}

#[test]
fn manifest_without_boards_declares_none() {
    let parsed = parse_manifest(&manifest("")).expect("parse");
    assert!(parsed.boards.is_empty());
}

#[test]
fn builtin_board_id_is_rejected() {
    let error = parse_manifest(&manifest(
        r#"[[boards]]
id = "dev""#,
    ))
    .expect_err("built-in board shadowing must be rejected");

    assert!(
        matches!(
            &error,
            LoadError::BuiltinProjectBoardId { position: 0, board } if board == "dev"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn duplicate_board_id_is_rejected() {
    let error = parse_manifest(&manifest(
        r#"[[boards]]
id = "one"

[[boards]]
id = "one""#,
    ))
    .expect_err("duplicate declarations must be rejected");

    assert!(
        matches!(
            error,
            LoadError::DuplicateProjectBoardId { position: 1, .. }
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn board_id_with_reserved_separator_is_rejected() {
    let error = parse_manifest(&manifest(
        r#"[[boards]]
id = "project:abc:dev""#,
    ))
    .expect_err("`:` is a reserved store key character");

    assert!(
        matches!(error, LoadError::InvalidProjectBoardId { position: 0, .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn empty_board_id_is_rejected() {
    let error = parse_manifest(&manifest(
        r#"[[boards]]
id = "  ""#,
    ))
    .expect_err("empty ids must be rejected");

    assert!(
        matches!(error, LoadError::InvalidProjectBoardId { position: 0, .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn boards_in_toolkit_directory_manifest_fail_registration() {
    let dir =
        std::env::temp_dir().join(format!("upeg-loader-boards-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create directory");
    std::fs::write(
        dir.join("kit.toml"),
        manifest(
            r#"[[boards]]
id = "kit-board""#,
        ),
    )
    .expect("write manifest");

    let outcome = crate::load_and_register_dir_verbose(&dir);

    assert!(
        outcome.loaded.is_empty(),
        "a toolkit declaring boards must not register"
    );
    assert!(
        outcome
            .failed
            .iter()
            .any(|(_, error)| matches!(error, LoadError::BoardsOutsideProjectManifest)),
        "unexpected failure list: {:?}",
        outcome.failed
    );

    let _ = std::fs::remove_dir_all(&dir);
}
