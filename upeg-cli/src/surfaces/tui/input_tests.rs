//! Input-routing and pure-helper tests for the TUI surface.
//!
//! Covers key handling (q-back, movement), mouse routing, filter-bar
//! scrolling, and small layout helpers. Companion to
//! [`super::state_tests`], which owns the fixture-based State/Action
//! coverage — this file imports that fixture and adds only `#[test]`
//! items. Split out for the workspace 1000-LoC file-size budget.

use super::state_tests::{filter_option_column, fixture_tools, fresh};
use crate::domain::execution::dispatch::Outcome;
use crate::surfaces::tui::*;
use ratatui::{Terminal, layout::Rect};
use upeg_core::{
    DraftInputValue, InputFieldSpec, InputKind, InputName, InputSpec, Invoker, PinKind, Surface,
    ToolMeta,
};

fn input_field(name: &str, kind: InputKind, required: bool) -> InputFieldSpec {
    InputFieldSpec::new(
        InputName::new(name).expect("test input name"),
        None,
        None,
        required,
        kind,
    )
    .expect("test input field")
}

fn form_from_fields(fields: Vec<InputFieldSpec>) -> TuiFormState {
    TuiFormState::new(InputSpec::new(fields).expect("test input spec"))
}

fn set_form_text(form: &mut TuiFormState, index: usize, value: &str) {
    form.fields[index].draft = DraftInputValue::Text(value.to_string());
}

fn string_form(name: &str, value: &str, required: bool) -> TuiFormState {
    let mut form = form_from_fields(vec![input_field(name, InputKind::String, required)]);
    set_form_text(&mut form, 0, value);
    form
}

fn string_form_with_names(names: &[&str]) -> TuiFormState {
    form_from_fields(
        names
            .iter()
            .map(|name| input_field(name, InputKind::String, false))
            .collect(),
    )
}

fn detail_input_tool() -> &'static ToolMeta {
    Box::leak(Box::new(ToolMeta {
        id: "num.hex_to_decimal",
        toolkit: "convert",
        local_id: "hex_to_decimal",
        tags: &[],
        display_label: "Hex to Dec",
        description: "Parse a hex string into decimal.",
        input_spec: InputSpec::new(vec![input_field("input", InputKind::String, true)])
            .expect("test input spec"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &["dev"],
    }))
}

fn detail_no_input_tool() -> &'static ToolMeta {
    static TOOL: ToolMeta = ToolMeta {
        id: "id.uuid_v7",
        toolkit: "id",
        local_id: "uuid_v7",
        tags: &[],
        display_label: "UUID v7",
        description: "Generate a UUID v7.",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Function,
        surfaces: &[Surface::Tui],
        boards: &["dev"],
    };
    &TOOL
}
use upeg_runtime::ToolMetaRuntimeExt;

mod detail;
mod filter_bar;
mod form_args;
mod keyboard_filters;
mod routing;
