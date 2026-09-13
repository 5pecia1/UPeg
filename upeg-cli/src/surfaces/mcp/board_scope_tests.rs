//! Board-scoped MCP surface tests ("board = server", `upeg mcp
//! --board <b>`): pinned-only tools/list, pin-gated tools/call with
//! args-preset merge. Sibling file so `tests.rs` stays inside the
//! ~1000 LoC budget.

use crate::surfaces::mcp::*;
use serde_json::json;

// ─── 보드 스코프 ("board = server") ─────────────────

#[test]
fn 보드_스코프_tools_list는_핀된_도구와_안내_조회를_노출한다() {
    crate::test_support::with_seeded_pegboard_home(
        "mcp-board-list",
        |state| {
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: "mcp-dev".into(),
                title: "MCP Dev".into(),
            });
            state.layouts.insert(
                "mcp-dev".into(),
                vec![upeg_core::Placement::new("num.hex_to_decimal", 0, 0)],
            );
        },
        || {
            let board = upeg_core::BoardKey::parse("mcp-dev").expect("보드 키");
            let resp = handle_with_board(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
                upeg_core::Surface::Mcp,
                Some(&board),
            )
            .expect("response owed");
            let tools = resp["result"]["tools"].as_array().expect("tools array");
            let names: Vec<&str> = tools
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .collect();
            assert_eq!(
                names,
                vec!["num.hex_to_decimal", "upeg.board_context"],
                "핀된 실행 도구와 안내 조회만 노출되어야 한다"
            );
        },
    );
}

#[test]
fn 보드_스코프_tools_call은_핀_preset을_병합하고_핀되지_않은_도구를_거부한다() {
    crate::test_support::with_seeded_pegboard_home(
        "mcp-board-call",
        |state| {
            let preset = upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("유효 preset");
            state.boards.push(upeg_sources::pegboard::BoardData {
                guidance: upeg_core::BoardGuidance::default(),
                key: "mcp-preset".into(),
                title: "MCP Preset".into(),
            });
            state.layouts.insert(
                "mcp-preset".into(),
                vec![
                    upeg_core::Placement::new("num.hex_to_decimal", 0, 0)
                        .with_args_preset(Some(preset)),
                ],
            );
        },
        || {
            let board = upeg_core::BoardKey::parse("mcp-preset").expect("보드 키");

            // 인자 없는 호출 — 핀 preset이 기본값으로 병합된다.
            let resp = handle_with_board(
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": { "name": "num.hex_to_decimal" },
                }),
                upeg_core::Surface::Mcp,
                Some(&board),
            )
            .expect("response owed");
            assert_eq!(
                resp["result"]["content"][0]["text"], "255",
                "preset input=0xff가 기본값이 되어야 한다: {resp}"
            );

            // 명시 인자는 preset을 덮어쓴다.
            let resp = handle_with_board(
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": { "name": "num.hex_to_decimal", "arguments": { "input": "0x10" } },
                }),
                upeg_core::Surface::Mcp,
                Some(&board),
            )
            .expect("response owed");
            assert_eq!(resp["result"]["content"][0]["text"], "16");

            // 핀되지 않은 도구는 알 수 없는 메서드와 같은 -32601이며,
            // 힌트는 그 보드의 핀 목록으로 한정된다.
            let resp = handle_with_board(
                json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": { "name": "convert.base64_encode", "arguments": { "input": "x" } },
                }),
                upeg_core::Surface::Mcp,
                Some(&board),
            )
            .expect("response owed");
            assert_eq!(resp["error"]["code"], -32601);
            let message = resp["error"]["message"].as_str().expect("message");
            assert!(
                message.contains("num.hex_to_decimal"),
                "보드 핀 목록이 힌트로 나와야 한다: {message}"
            );
        },
    );
}
