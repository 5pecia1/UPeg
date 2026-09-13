//! Agent-facing Board discovery and effective argument contracts.

use super::*;

#[test]
fn 보드_연결은_지침_조회_방법을_안내한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-initialize",
        |_| {},
        || {
            let board = BoardKey::parse("dev").unwrap();
            let response = handle_with_board(
                json!({"jsonrpc":"2.0", "id":1, "method":"initialize"}),
                Surface::Mcp,
                Some(&board),
            )
            .unwrap();
            assert!(
                response["result"]["instructions"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("upeg.board_context")
            );
        },
    );
}

#[test]
fn 보드_목록은_프리셋으로_충족한_입력을_선택으로_보여준다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-schema",
        |state| {
            state.layouts.insert(
                "dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
                        upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).unwrap(),
                    )),
                ],
            );
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let response = handle_with_board(
                json!({"jsonrpc":"2.0", "id":1, "method":"tools/list"}),
                Surface::Mcp,
                Some(&board),
            )
            .unwrap();
            let tools = response["result"]["tools"].as_array().unwrap();
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == "num.hex_to_decimal")
                .unwrap();
            assert_eq!(
                tool["inputSchema"]["properties"]["input"]["default"],
                "0xff"
            );
            assert!(
                !tool["inputSchema"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("input"))
            );
            assert!(
                tools
                    .iter()
                    .any(|tool| tool["name"] == "upeg.board_context")
            );
        },
    );
}

#[test]
fn 보드_지침_조회는_선택한_보드와_실제_기본값만_반환한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-context",
        |state| {
            let board = state
                .boards
                .iter_mut()
                .find(|board| board.key == "dev")
                .unwrap();
            board.guidance.description = "숫자 변환을 검증할 때 사용".into();
            board.guidance.instructions = "변환 결과를 십진수로 설명한다.".into();
            state.layouts.insert(
                "dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
                        upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).unwrap(),
                    )),
                ],
            );
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let response = handle_with_board(
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":"upeg.board_context"}}),
                Surface::Mcp,
                Some(&board),
            )
            .unwrap();
            let context = &response["result"]["structuredContent"];
            assert_eq!(context["instructions"], "변환 결과를 십진수로 설명한다.");
            assert_eq!(context["tools"].as_array().unwrap().len(), 1);
            assert_eq!(context["tools"][0]["defaults"]["input"], "0xff");
            assert!(!context["revision"].as_str().unwrap().is_empty());
            assert!(
                std::path::Path::new(context["working_directory"].as_str().unwrap()).is_absolute()
            );
        },
    );
}

#[test]
fn 없는_보드는_초기화와_목록에서_명확히_거부한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-missing",
        |_| {},
        || {
            let board = BoardKey::parse("missing-guide-board").unwrap();
            for method in ["initialize", "tools/list"] {
                let response = handle_with_board(
                    json!({"jsonrpc":"2.0","id":1,"method":method}),
                    Surface::Mcp,
                    Some(&board),
                )
                .unwrap();
                assert_eq!(response["error"]["code"], -32001);
                assert!(
                    response["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("unknown board")
                );
            }
        },
    );
}

#[test]
fn 프리셋이_바뀐_연결은_실행을_멈추고_재연결을_요구한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-reconnect",
        |state| {
            state.layouts.insert(
                "dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
                        upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).unwrap(),
                    )),
                ],
            );
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let session = board_guidance::BoardSession::new(Some(&board));
            let mut state = upeg_sources::pegboard::load_state();
            state.layouts.get_mut("dev").unwrap()[0].args_preset =
                Some(upeg_core::ArgsPreset::parse(r#"{"input":"0x10"}"#).unwrap());
            upeg_sources::pegboard::save_state(&state).unwrap();
            let request = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"num.hex_to_decimal"}});
            let response = session.handle(request.clone(), Some(&board), None).unwrap();
            assert_eq!(response["error"]["data"]["reconnectRequired"], true);
            let reconnected = board_guidance::BoardSession::new(Some(&board));
            let response = reconnected.handle(request, Some(&board), None).unwrap();
            assert_eq!(response["result"]["content"][0]["text"], "16");
        },
    );
}

#[test]
fn 검증한_snapshot으로_호출하면_이후_store의_프리셋을_다시_읽지_않는다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-snapshot-dispatch",
        |state| {
            state.layouts.insert(
                "dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
                        upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).unwrap(),
                    )),
                ],
            );
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let snapshot = crate::board_agent::board_snapshot(&board).unwrap();
            let mut changed = upeg_sources::pegboard::load_state();
            changed.layouts.get_mut("dev").unwrap()[0].args_preset =
                Some(upeg_core::ArgsPreset::parse(r#"{"input":"0x10"}"#).unwrap());
            upeg_sources::pegboard::save_state(&changed).unwrap();

            let response = handle_in_lane(
                json!({
                    "jsonrpc":"2.0",
                    "id":7,
                    "method":"tools/call",
                    "params":{"name":"num.hex_to_decimal"}
                }),
                Principal::for_surface(Surface::Mcp),
                Some(&board),
                None,
                Some(&snapshot.state),
            )
            .unwrap();

            assert_eq!(response["result"]["content"][0]["text"], "255");
        },
    );
}

#[test]
fn 핀의_배치만_바꾸면_기존_연결을_계속_사용한다() {
    const TOOL: &str = "num.hex_to_decimal";
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-layout-change",
        |state| {
            state
                .layouts
                .insert("dev".into(), vec![upeg_core::Placement::new(TOOL, 0, 0)]);
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let session = board_guidance::BoardSession::new(Some(&board));
            let mut state = upeg_sources::pegboard::load_state();
            state
                .layouts
                .insert("dev".into(), vec![upeg_core::Placement::new(TOOL, 2, 3)]);
            upeg_sources::pegboard::save_state(&state).unwrap();

            let response = session
                .handle(
                    json!({"jsonrpc":"2.0", "id":1, "method":"tools/list"}),
                    Some(&board),
                    None,
                )
                .unwrap();

            assert!(response.get("error").is_none(), "{response}");
            assert!(
                response["result"]["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|tool| tool["name"] == TOOL)
            );
        },
    );
}

#[test]
fn 보드의_알_수_없는_프리셋_입력은_실행_전에_거부한다() {
    crate::test_support::with_seeded_pegboard_home(
        "board-guide-invalid-preset",
        |state| {
            state.layouts.insert(
                "dev".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0).with_args_preset(Some(
                        upeg_core::ArgsPreset::parse(r#"{"input":"ff","unknown":true}"#).unwrap(),
                    )),
                ],
            );
        },
        || {
            let board = BoardKey::parse("dev").unwrap();
            let response = handle_with_board(
                json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
                    "params":{"name":"num.hex_to_decimal"}}),
                Surface::Mcp,
                Some(&board),
            )
            .unwrap();

            assert_eq!(response["error"]["code"], -32602, "{response}");
            assert!(
                response["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("unknown")
            );
        },
    );
}
