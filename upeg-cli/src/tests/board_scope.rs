//! `upeg board <board> list|call` — board-scoped CLI route tests.
//!
//! The gate and the enumeration read the user's PegboardState (seeded
//! into a scratch `UPEG_HOME` per test), never the manifest's static
//! `boards` arrays.

use super::common::parse;
use crate::surfaces::cli::{BoardScopedAction, BoardScopedCli};
use crate::*;
use clap::Parser as _;

fn seed_preset_board(state: &mut upeg_sources::pegboard::PegboardState) {
    let preset = upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("valid preset");
    state.boards.push(upeg_sources::pegboard::BoardData {
        key: "cli-preset".into(),
        title: "CLI Preset".into(),
        guidance: upeg_core::BoardGuidance::default(),
    });
    state.layouts.insert(
        "cli-preset".into(),
        vec![upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(preset))],
    );
}

#[test]
fn board_scoped_call_merges_the_pin_preset_as_defaults() {
    crate::test_support::with_seeded_pegboard_home("cli-board-call", seed_preset_board, || {
        // The pin preset supplies input=0xff even when no args are provided.
        let out = run(parse(&[
            "upeg",
            "board",
            "cli-preset",
            "call",
            "num.hex_to_decimal",
            "--local",
        ]))
        .expect("board call");
        assert_eq!(out, "255\n");

        // Explicit args override the preset.
        let out = run(parse(&[
            "upeg",
            "board",
            "cli-preset",
            "call",
            "num.hex_to_decimal",
            "-a",
            "input=0x10",
            "--local",
        ]))
        .expect("board call with override");
        assert_eq!(out, "16\n");
    });
}

/// `upeg call` with the global `--board` flag must follow the same order as
/// `board <b> call`: parse → merge preset → validate. Validating first would
/// reject required inputs that the preset supplies as "missing".
#[test]
fn global_board_flag_call_accepts_required_inputs_supplied_by_the_preset() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-flag-preset",
        |state| {
            let preset =
                upeg_core::ArgsPreset::parse(r#"{"from":"a","to":"b"}"#).expect("valid preset");
            state.boards.push(upeg_sources::pegboard::BoardData {
                key: "flag-preset".into(),
                title: "Flag Preset".into(),
                guidance: upeg_core::BoardGuidance::default(),
            });
            state.layouts.insert(
                "flag-preset".into(),
                vec![
                    upeg_core::Placement::new("text.replace", 0, 0).with_args_preset(Some(preset)),
                ],
            );
        },
        || {
            // The pin preset supplies required from/to fields; the caller only provides input.
            let out = run(parse(&[
                "upeg",
                "--board",
                "flag-preset",
                "call",
                "text.replace",
                "-a",
                "input=abc",
                "--local",
            ]))
            .expect("required inputs supplied by the preset must not be rejected");
            assert_eq!(out, "bbc\n");

            // The raw JSON path follows the same order.
            let out = run(parse(&[
                "upeg",
                "--board",
                "flag-preset",
                "call",
                "text.replace",
                r#"{"input":"aaa"}"#,
                "--local",
            ]))
            .expect("raw JSON args must also be validated after the preset merge");
            assert_eq!(out, "bbb\n");

            // Unknown inputs are still rejected even when a preset exists.
            let err = run(parse(&[
                "upeg",
                "--board",
                "flag-preset",
                "call",
                "text.replace",
                "-a",
                "input=abc",
                "-a",
                "nope=1",
                "--local",
            ]))
            .expect_err("unknown inputs must be rejected regardless of the preset");
            assert!(
                err.message().contains("unknown input `nope`"),
                "got: {}",
                err.message()
            );
        },
    );
}

/// The surface-filtered board list and call gate must agree.
/// GUI-only `memo.scratch` is absent from `board <b> list`, so
/// `board <b> call` must reject it as "not pinned" too.
#[test]
fn board_scoped_call_treats_off_surface_pins_as_unpinned() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-surface-gate",
        |state| {
            state.boards.push(upeg_sources::pegboard::BoardData {
                key: "gui-pin".into(),
                title: "GUI Pin".into(),
                guidance: upeg_core::BoardGuidance::default(),
            });
            state.layouts.insert(
                "gui-pin".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0),
                    // Desktop/Pwa/Ext only; unavailable on the CLI surface.
                    upeg_core::Placement::new("memo.scratch", 1, 0),
                ],
            );
        },
        || {
            let listed = run(parse(&["upeg", "board", "gui-pin", "list"])).expect("board list");
            assert!(
                !listed.contains("memo.scratch"),
                "the CLI list must exclude GUI-only pins: {listed}"
            );

            let err = run(parse(&[
                "upeg",
                "board",
                "gui-pin",
                "call",
                "memo.scratch",
                "--local",
            ]))
            .expect_err("pins absent from the list must not be callable either");
            let message = err.message();
            assert!(
                message.contains("not pinned on board `gui-pin`"),
                "the call gate must agree with the list: {message}"
            );
            assert!(
                !message.contains("memo.scratch,"),
                "suggestions must use the same surface filter: {message}"
            );
            assert!(
                message.contains("num.hex_to_decimal"),
                "suggestions must include pins callable on this surface: {message}"
            );
        },
    );
}

#[test]
fn board_scoped_call_suggests_pinned_tools_for_an_unpinned_tool() {
    crate::test_support::with_seeded_pegboard_home("cli-board-unpinned", seed_preset_board, || {
        let err = run(parse(&[
            "upeg",
            "board",
            "cli-preset",
            "call",
            "convert.base64_encode",
            "--local",
        ]))
        .expect_err("unpinned tool must fail");
        let crate::error::CliError::ToolFailed(message) = err else {
            panic!("expected a ToolFailed error: {err:?}");
        };
        assert!(
            message.contains("not pinned on board `cli-preset`"),
            "the error must clearly identify the unpinned tool: {message}"
        );
        assert!(
            message.contains("num.hex_to_decimal"),
            "the error must suggest tools pinned on that board: {message}"
        );
    });
}

#[test]
fn board_scoped_list_shows_pinned_tools_and_presets() {
    crate::test_support::with_seeded_pegboard_home("cli-board-list", seed_preset_board, || {
        let out = run(parse(&["upeg", "board", "cli-preset", "list"])).expect("board list");
        assert!(
            out.lines()
                .any(|line| line.starts_with("num.hex_to_decimal\t")),
            "the pinned tool row must appear: {out}"
        );

        let json_out = run(parse(&["upeg", "board", "cli-preset", "list", "--json"]))
            .expect("board list --json");
        let entries: serde_json::Value = serde_json::from_str(&json_out).expect("json array");
        let entry = entries
            .as_array()
            .and_then(|list| list.first())
            .expect("one pinned entry");
        assert_eq!(entry["name"], "num.hex_to_decimal");
        assert_eq!(entry["argsPreset"]["input"], "0xff");
    });
}

#[test]
fn unknown_board_errors_suggest_available_board_keys() {
    crate::test_support::with_seeded_pegboard_home("cli-board-unknown", seed_preset_board, || {
        let err = run(parse(&["upeg", "board", "no-such-board", "list"]))
            .expect_err("unknown board must fail");
        let crate::error::CliError::ToolFailed(message) = err else {
            panic!("expected a ToolFailed error: {err:?}");
        };
        assert!(
            message.contains("unknown board `no-such-board`"),
            "{message}"
        );
        assert!(
            message.contains("cli-preset"),
            "the error must suggest available boards: {message}"
        );
    });
}

#[test]
fn mcp_board_option_fails_immediately_for_an_unknown_board() {
    crate::test_support::with_seeded_pegboard_home("cli-mcp-board", seed_preset_board, || {
        let err = run(parse(&["upeg", "mcp", "--board", "no-such-board"]))
            .expect_err("unknown board must fail before serving");
        let crate::error::CliError::ToolFailed(message) = err else {
            panic!("expected a ToolFailed error: {err:?}");
        };
        assert!(
            message.contains("unknown board `no-such-board`"),
            "{message}"
        );
    });
}

#[test]
fn board_context_accepts_json_output() {
    let cli = parse(&["upeg", "board", "dev", "context", "--json"]);

    let Some(Command::Board {
        action: BoardAction::Scoped(tokens),
    }) = cli.command
    else {
        panic!("expected a board-scoped command");
    };
    let scoped = BoardScopedCli::try_parse_from(tokens.into_iter().skip(1))
        .expect("context syntax must parse");

    assert!(matches!(
        scoped.action,
        BoardScopedAction::Context { json: true }
    ));
}

#[test]
fn board_connect_parses_without_extra_options() {
    let scoped = BoardScopedCli::try_parse_from(["connect"]).expect("connect syntax must parse");

    assert!(matches!(scoped.action, BoardScopedAction::Connect));
}

#[test]
fn board_describe_updates_description_and_instructions_independently() {
    let scoped = BoardScopedCli::try_parse_from([
        "describe",
        "--description",
        "Release work",
        "--instructions",
        "Run checks first",
        "--json",
    ])
    .expect("describe patch syntax must parse");

    let BoardScopedAction::Describe {
        description,
        clear_description,
        instructions,
        clear_instructions,
        json,
    } = scoped.action
    else {
        panic!("expected a describe action");
    };
    assert_eq!(description.as_deref(), Some("Release work"));
    assert!(!clear_description);
    assert_eq!(instructions.as_deref(), Some("Run checks first"));
    assert!(!clear_instructions);
    assert!(json);
}

#[test]
fn board_describe_rejects_setting_and_clearing_description_together() {
    let error = BoardScopedCli::try_parse_from([
        "describe",
        "--description",
        "Release work",
        "--clear-description",
    ])
    .expect_err("setting and clearing the same field must conflict");

    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn working_directory_parses_before_subcommands() {
    let cli = parse(&[
        "upeg",
        "--working-directory",
        "/tmp/upeg-project",
        "board",
        "dev",
        "context",
    ]);

    assert_eq!(
        cli.working_directory,
        Some(std::path::PathBuf::from("/tmp/upeg-project"))
    );
}

#[test]
fn board_context_json_shows_guidance_and_pinned_tools() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-context",
        |state| {
            seed_preset_board(state);
            let board = state
                .boards
                .iter_mut()
                .find(|board| board.key == "cli-preset")
                .expect("seeded board");
            board.guidance.description = "Release work".into();
            board.guidance.instructions = "Run checks first".into();
        },
        || {
            let output = run(parse(&["upeg", "board", "cli-preset", "context", "--json"]))
                .expect("board context");
            let context: serde_json::Value = serde_json::from_str(&output).expect("context json");

            assert_eq!(context["board"], "cli-preset");
            assert_eq!(context["description"], "Release work");
            assert_eq!(context["instructions"], "Run checks first");
            assert_eq!(context["tools"][0]["id"], "num.hex_to_decimal");
            assert_eq!(context["tools"][0]["defaults"]["input"], "0xff");
            assert!(context["working_directory"].is_string());
        },
    );
}

#[test]
fn board_connect_emits_the_working_directory_and_board_as_structured_args() {
    crate::test_support::with_seeded_pegboard_home("cli-board-connect", seed_preset_board, || {
        let output =
            run(parse(&["upeg", "board", "cli-preset", "connect"])).expect("board connect");
        let config: serde_json::Value = serde_json::from_str(&output).expect("connection json");
        let server = &config["mcpServers"]["upeg-cli-preset"];

        assert!(server["command"].is_string());
        assert_eq!(
            server["args"],
            serde_json::json!([
                "--working-directory",
                std::env::current_dir().expect("current working directory"),
                "mcp",
                "--board",
                "cli-preset"
            ])
        );
        assert!(server["env"]["UPEG_PROJECT_MANIFEST_PATH"].is_string());
        assert!(server["env"]["UPEG_HOME"].is_string());
    });
}

#[test]
fn board_describe_preserves_omitted_fields_and_persists() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-describe",
        |state| {
            seed_preset_board(state);
            let board = state
                .boards
                .iter_mut()
                .find(|board| board.key == "cli-preset")
                .expect("seeded board");
            board.guidance.description = "Before".into();
            board.guidance.instructions = "Keep this".into();
        },
        || {
            let output = run(parse(&[
                "upeg",
                "board",
                "cli-preset",
                "describe",
                "--description",
                "After",
                "--json",
            ]))
            .expect("board describe");
            let guidance: serde_json::Value = serde_json::from_str(&output).expect("guidance json");
            assert_eq!(guidance["description"], "After");
            assert_eq!(guidance["instructions"], "Keep this");

            let persisted = upeg_sources::pegboard::load_state();
            let board = persisted
                .boards
                .iter()
                .find(|board| board.key == "cli-preset")
                .expect("persisted board");
            assert_eq!(board.guidance.description, "After");
            assert_eq!(board.guidance.instructions, "Keep this");
        },
    );
}

#[test]
fn board_describe_rejects_an_empty_patch() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-describe-empty",
        seed_preset_board,
        || {
            let error = run(parse(&["upeg", "board", "cli-preset", "describe"]))
                .expect_err("an empty patch must be rejected");

            assert!(
                error.message().contains("nothing to update"),
                "{}",
                error.message()
            );
        },
    );
}
