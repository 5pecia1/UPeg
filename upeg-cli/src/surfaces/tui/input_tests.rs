//! TUI 표면의 입력 라우팅과 순수 헬퍼 테스트.
//!
//! 키 처리(q-back, 이동), 마우스 라우팅, 필터 막대 스크롤, 작은 레이아웃
//! 헬퍼를 다룬다. fixture 기반 State/Action 커버리지를 가진
//! [`super::state_tests`]의 짝이며, 이 파일은 해당 fixture를 가져와
//! `#[test]` 항목만 추가한다. 워크스페이스 1000-LoC 파일 크기 예산 때문에
//! 분리했다.

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
        InputName::new(name).expect("테스트 입력 이름"),
        None,
        None,
        required,
        kind,
    )
    .expect("테스트 입력 필드")
}

fn form_from_fields(fields: Vec<InputFieldSpec>) -> TuiFormState {
    TuiFormState::new(InputSpec::new(fields).expect("테스트 입력 명세"))
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
            .expect("테스트 입력 명세"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
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
