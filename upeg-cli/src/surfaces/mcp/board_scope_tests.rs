//! Board-scoped MCP surface tests ("board = server", `upeg mcp
//! --board <b>`): pinned-only tools/list, pin-gated tools/call with
//! args-preset merge. Sibling file so `tests.rs` stays inside the
//! ~1000 LoC budget.

use crate::surfaces::mcp::*;
use serde_json::json;

// ─── Board scope ("board = server") ─────────────────

#[test]
fn board_scoped_tools_list_exposes_pinned_tools_and_guidance_query() {
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
            let board = upeg_core::BoardKey::parse("mcp-dev").expect("board key");
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
                "only the pinned tool and the guidance query should be exposed"
            );
        },
    );
}

#[test]
fn board_scoped_tools_call_merges_pin_preset_and_rejects_unpinned_tool() {
    crate::test_support::with_seeded_pegboard_home(
        "mcp-board-call",
        |state| {
            let preset = upeg_core::ArgsPreset::parse(r#"{"input":"0xff"}"#).expect("valid preset");
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
            let board = upeg_core::BoardKey::parse("mcp-preset").expect("board key");

            // Argument-free call — the pin preset merges as defaults.
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
                "preset input=0xff should become the default: {resp}"
            );

            // Explicit args override the preset.
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

            // An unpinned tool gets the same -32601 as an unknown method,
            // and the hint is scoped to that board's pin list.
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
                "the board pin list should appear in the hint: {message}"
            );
        },
    );
}
