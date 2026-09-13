#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    clippy::print_stdout,
    clippy::string_add,
    clippy::manual_let_else,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Integration test for the inventory-backed Toolbox.
//!
//! Runs as a separate test binary so we can `inventory::submit!` and assert
//! the entries flow through `toolbox_tools()`. PRD v2.1 §5.1 — single
//! manifest, every surface.

use upeg_core::{
    ALL_SURFACES, Invoker, PegboardUnits, PinKind, StaticInputSpec, StaticToolMeta, Surface,
    ToolMeta, inventory, tool,
};

fn toolbox_tools() -> impl Iterator<Item = ToolMeta> {
    inventory::iter::<StaticToolMeta>().map(|meta| {
        meta.assert_valid();
        ToolMeta::from_static(meta).expect("test StaticToolMeta materializes")
    })
}

fn toolbox_tool(id: &str) -> Option<ToolMeta> {
    toolbox_tools().find(|tool| tool.id == id)
}

// (1) Manual submission — what the proc-macro emission expands to.
inventory::submit! {
    StaticToolMeta {
        id: "test.manual_submit",
        toolkit: "test",
        local_id: "manual_submit",
        tags: &[],
        display_label: "Manual submit",
        description: "Manual inventory submission for tests",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::StaticSource::UserInput,
        pin: PinKind::Inline,
        pegboard_units: PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: ALL_SURFACES,
        boards: &[],
    }
}

// (2) Required-minimum path — proc-macro fills in pin=Inline,
//     invoker=Function, surfaces=ALL_SURFACES, boards=[].
#[tool(id = "test.macro_defaults", toolkit = "test", pegboard_units = U1)]
const fn _macro_defaults_fn() -> &'static str {
    "ok"
}

// (3) Fully-specified path — every optional argument given. Pin to two
//     boards, restrict surfaces to GUI (mimicking a Passive Embed Tool).
#[tool(
    id = "test.macro_full",
    toolkit = "test",
    display_label = "Macro full",
    description = "Fully-specified macro test tool.",
    inputs = [
        required input: String = "Text to process",
        optional n: Integer = "Repeat count",
    ],
    pin = Embed,
    pegboard_units = U2,
    invoker = Static,
    surfaces = [Desktop, Pwa, Ext],
    boards = ["dev", "trading"],
)]
const fn _macro_full_fn() {}

#[test]
fn 수동_제출은_반복_가능하다() {
    let ids: Vec<_> = toolbox_tools().map(|t| t.id).collect();
    assert!(
        ids.contains(&"test.manual_submit"),
        "manual submit missing; saw {ids:?}"
    );
}

#[test]
fn 매크로는_기본값이_있는_도구를_툴박스에_등록한다() {
    let t = toolbox_tool("test.macro_defaults").expect("default-args macro tool not registered");
    assert_eq!(t.toolkit, "test");
    assert_eq!(t.pin, PinKind::Inline);
    assert_eq!(t.pegboard_units, PegboardUnits::U1);
    assert_eq!(t.invoker, Invoker::Function);
    assert_eq!(t.surfaces, ALL_SURFACES);
    assert!(t.boards.is_empty());
}

#[test]
fn 매크로는_모든_인자가_있는_도구를_툴박스에_등록한다() {
    let t = toolbox_tool("test.macro_full").expect("full-args macro tool not registered");
    assert_eq!(t.pin, PinKind::Embed);
    assert_eq!(t.pegboard_units, PegboardUnits::U2);
    assert_eq!(t.invoker, Invoker::Static);
    assert_eq!(t.surfaces, &[Surface::Desktop, Surface::Pwa, Surface::Ext]);
    assert_eq!(t.boards, &["dev", "trading"]);
    assert_eq!(t.description, "Fully-specified macro test tool.");
    let schema = t.input_schema_value();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["input"]["type"], "string");
    assert_eq!(
        schema["properties"]["input"]["description"],
        "Text to process"
    );
    assert_eq!(schema["properties"]["n"]["type"], "integer");
    assert_eq!(schema["required"], serde_json::json!(["input"]));
}

#[test]
fn 매크로는_설명이_없으면_빈_문자열을_기본값으로_사용한다() {
    let t = toolbox_tool("test.macro_defaults").expect("default-args tool");
    assert_eq!(t.description, "");
}

#[test]
fn 등록된_도구는_알수없는_id에_없음을_반환한다() {
    assert!(toolbox_tool("test.does_not_exist").is_none());
}

#[test]
fn 툴박스는_세_테스트_도구를_모두_포함한다() {
    let ids: std::collections::HashSet<_> = toolbox_tools().map(|t| t.id).collect();
    for expected in [
        "test.manual_submit",
        "test.macro_defaults",
        "test.macro_full",
    ] {
        assert!(ids.contains(expected), "{expected} missing; saw {ids:?}");
    }
}

#[test]
fn 매크로는_원래_함수_본문을_보존한다() {
    // Confirms the attribute macro doesn't eat the function it annotates.
    assert_eq!(_macro_defaults_fn(), "ok");
}
