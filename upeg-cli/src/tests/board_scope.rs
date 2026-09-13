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
    let preset = upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("유효 preset");
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
fn 보드_스코프_call은_핀_preset을_기본값으로_병합한다() {
    crate::test_support::with_seeded_pegboard_home("cli-board-call", seed_preset_board, || {
        // 인자를 주지 않아도 핀 preset의 input=0xff가 병합된다.
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

        // 명시 인자는 preset을 덮어쓴다.
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

/// 전역 `--board` 플래그로 부르는 `upeg call`도 `board <b> call`과
/// 같은 순서(파싱 → preset 병합 → 검증)를 따라야 한다. 검증이 먼저
/// 돌면 preset이 채워 줄 필수 입력을 "missing"으로 거절한다.
#[test]
fn 전역_board_플래그_call은_preset이_채우는_필수_입력을_거절하지_않는다() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-flag-preset",
        |state| {
            let preset =
                upeg_core::ArgsPreset::parse(r#"{"from":"a","to":"b"}"#).expect("유효 preset");
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
            // from/to는 필수지만 핀 preset이 공급한다 — 호출자는 input만 준다.
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
            .expect("preset이 채운 필수 입력은 거절되면 안 된다");
            assert_eq!(out, "bbc\n");

            // 원시 JSON 경로도 같은 순서를 따른다.
            let out = run(parse(&[
                "upeg",
                "--board",
                "flag-preset",
                "call",
                "text.replace",
                r#"{"input":"aaa"}"#,
                "--local",
            ]))
            .expect("원시 JSON 경로도 preset 병합 뒤에 검증해야 한다");
            assert_eq!(out, "bbb\n");

            // preset이 있어도 알 수 없는 입력은 여전히 거절된다.
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
            .expect_err("알 수 없는 입력은 preset과 무관하게 거절되어야 한다");
            assert!(
                err.message().contains("unknown input `nope`"),
                "got: {}",
                err.message()
            );
        },
    );
}

/// 보드 목록(표면 필터링)과 호출 게이트가 같은 판단을 써야 한다.
/// `memo.scratch`는 GUI 전용이라 `board <b> list`에 나오지 않으므로,
/// `board <b> call`도 "핀되지 않음"으로 거절해야 한다.
#[test]
fn 보드_스코프_call_게이트는_이_표면에_없는_핀을_핀되지_않은_것으로_본다() {
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
                    // Desktop/Pwa/Ext 전용 — cli 표면에는 없다.
                    upeg_core::Placement::new("memo.scratch", 1, 0),
                ],
            );
        },
        || {
            let listed = run(parse(&["upeg", "board", "gui-pin", "list"])).expect("board list");
            assert!(
                !listed.contains("memo.scratch"),
                "cli 목록에는 GUI 전용 핀이 없어야 한다: {listed}"
            );

            let err = run(parse(&[
                "upeg",
                "board",
                "gui-pin",
                "call",
                "memo.scratch",
                "--local",
            ]))
            .expect_err("목록에 없는 핀은 호출도 안 되어야 한다");
            let message = err.message();
            assert!(
                message.contains("not pinned on board `gui-pin`"),
                "목록과 같은 판단이어야 한다: {message}"
            );
            assert!(
                !message.contains("memo.scratch,"),
                "제안 목록도 같은 표면으로 걸러야 한다: {message}"
            );
            assert!(
                message.contains("num.hex_to_decimal"),
                "이 표면에서 부를 수 있는 핀을 제안해야 한다: {message}"
            );
        },
    );
}

#[test]
fn 보드_스코프_call은_핀되지_않은_도구에_핀_목록을_제안한다() {
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
            panic!("ToolFailed 오류여야 한다: {err:?}");
        };
        assert!(
            message.contains("not pinned on board `cli-preset`"),
            "명확한 미핀 오류여야 한다: {message}"
        );
        assert!(
            message.contains("num.hex_to_decimal"),
            "그 보드의 핀 목록을 제안해야 한다: {message}"
        );
    });
}

#[test]
fn 보드_스코프_list는_핀_목록과_preset을_보여준다() {
    crate::test_support::with_seeded_pegboard_home("cli-board-list", seed_preset_board, || {
        let out = run(parse(&["upeg", "board", "cli-preset", "list"])).expect("board list");
        assert!(
            out.lines()
                .any(|line| line.starts_with("num.hex_to_decimal\t")),
            "핀 행이 나와야 한다: {out}"
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
fn 알수없는_보드는_보드_키_목록을_제안한다() {
    crate::test_support::with_seeded_pegboard_home("cli-board-unknown", seed_preset_board, || {
        let err = run(parse(&["upeg", "board", "no-such-board", "list"]))
            .expect_err("unknown board must fail");
        let crate::error::CliError::ToolFailed(message) = err else {
            panic!("ToolFailed 오류여야 한다: {err:?}");
        };
        assert!(
            message.contains("unknown board `no-such-board`"),
            "{message}"
        );
        assert!(message.contains("cli-preset"), "보드 목록 제안: {message}");
    });
}

#[test]
fn mcp_보드_옵션은_알수없는_보드에_즉시_실패한다() {
    crate::test_support::with_seeded_pegboard_home("cli-mcp-board", seed_preset_board, || {
        let err = run(parse(&["upeg", "mcp", "--board", "no-such-board"]))
            .expect_err("unknown board must fail before serving");
        let crate::error::CliError::ToolFailed(message) = err else {
            panic!("ToolFailed 오류여야 한다: {err:?}");
        };
        assert!(
            message.contains("unknown board `no-such-board`"),
            "{message}"
        );
    });
}

#[test]
fn 보드_context는_json_출력을_선택할_수_있다() {
    let cli = parse(&["upeg", "board", "dev", "context", "--json"]);

    let Some(Command::Board {
        action: BoardAction::Scoped(tokens),
    }) = cli.command
    else {
        panic!("board scoped command여야 한다");
    };
    let scoped = BoardScopedCli::try_parse_from(tokens.into_iter().skip(1))
        .expect("context 문법을 파싱해야 한다");

    assert!(matches!(
        scoped.action,
        BoardScopedAction::Context { json: true }
    ));
}

#[test]
fn 보드_connect는_추가_옵션_없이_파싱된다() {
    let scoped = BoardScopedCli::try_parse_from(["connect"]).expect("connect 문법을 파싱해야 한다");

    assert!(matches!(scoped.action, BoardScopedAction::Connect));
}

#[test]
fn 보드_describe는_설명과_지침을_독립적으로_수정한다() {
    let scoped = BoardScopedCli::try_parse_from([
        "describe",
        "--description",
        "Release work",
        "--instructions",
        "Run checks first",
        "--json",
    ])
    .expect("describe patch 문법을 파싱해야 한다");

    let BoardScopedAction::Describe {
        description,
        clear_description,
        instructions,
        clear_instructions,
        json,
    } = scoped.action
    else {
        panic!("describe action이어야 한다");
    };
    assert_eq!(description.as_deref(), Some("Release work"));
    assert!(!clear_description);
    assert_eq!(instructions.as_deref(), Some("Run checks first"));
    assert!(!clear_instructions);
    assert!(json);
}

#[test]
fn 보드_describe는_설명_입력과_삭제를_동시에_받지_않는다() {
    let error = BoardScopedCli::try_parse_from([
        "describe",
        "--description",
        "Release work",
        "--clear-description",
    ])
    .expect_err("같은 필드의 수정과 삭제는 충돌해야 한다");

    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn working_directory는_모든_명령에_앞서_파싱된다() {
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
fn 보드_context_json은_안내와_핀된_도구를_함께_보여준다() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-context",
        |state| {
            seed_preset_board(state);
            let board = state
                .boards
                .iter_mut()
                .find(|board| board.key == "cli-preset")
                .expect("보드");
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
fn 보드_connect는_고정된_작업_경로와_board를_구조화된_인자로_출력한다() {
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
                std::env::current_dir().expect("현재 작업 경로"),
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
fn 보드_describe는_생략한_필드를_보존하고_저장한다() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-describe",
        |state| {
            seed_preset_board(state);
            let board = state
                .boards
                .iter_mut()
                .find(|board| board.key == "cli-preset")
                .expect("보드");
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
                .expect("저장된 보드");
            assert_eq!(board.guidance.description, "After");
            assert_eq!(board.guidance.instructions, "Keep this");
        },
    );
}

#[test]
fn 보드_describe는_수정할_필드가_없으면_거절한다() {
    crate::test_support::with_seeded_pegboard_home(
        "cli-board-describe-empty",
        seed_preset_board,
        || {
            let error = run(parse(&["upeg", "board", "cli-preset", "describe"]))
                .expect_err("빈 patch는 거절해야 한다");

            assert!(
                error.message().contains("nothing to update"),
                "{}",
                error.message()
            );
        },
    );
}
