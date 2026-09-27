use serde_json::Value;
use upeg_core::OutputEntry;

use crate::domain::execution::dispatch::Outcome;

use super::super::effects::Effect;
use super::super::model::{
    FocusArea, PresentationHost, PresentationOrigin, State, TuiFormState, View,
};
use super::super::scroll::ScrollOffset;
use super::{dispatch_or_confirm, start_run};

fn presentation_outputs(outputs: &[OutputEntry]) -> Value {
    Value::Object(
        outputs
            .iter()
            .map(|entry| (entry.id.clone(), entry.value.to_json_value()))
            .collect(),
    )
}

pub(super) fn result_navigation_bounds(state: &State) -> (usize, usize) {
    let View::Result {
        tool_id,
        outputs,
        is_error: false,
        ..
    } = &state.view
    else {
        return (0, 0);
    };
    let Some(presentation) =
        upeg_runtime::toolbox_tool(tool_id).and_then(|tool| tool.presentation.as_ref())
    else {
        return (0, 0);
    };
    let rows = upeg_core::resolve_rows(presentation, &presentation_outputs(outputs));
    let has_row = rows.row_actions_enabled && !rows.rows.is_empty();
    let action_count = presentation
        .actions
        .iter()
        .filter(|action| matches!(action.scope, upeg_core::ActionScope::Result) || has_row)
        .count();
    (rows.rows.len(), action_count)
}

pub(super) fn open_result_action(state: &mut State) -> Effect {
    let View::Result {
        tool_id,
        outputs,
        is_error,
        ..
    } = &state.view
    else {
        return Effect::None;
    };
    if *is_error {
        return Effect::None;
    }
    let Some(source) = upeg_runtime::toolbox_tool(tool_id) else {
        return Effect::None;
    };
    let Some(presentation) = source.presentation.as_ref() else {
        return Effect::None;
    };
    let output_values = presentation_outputs(outputs);
    let rows = upeg_core::resolve_rows(presentation, &output_values);
    let row = rows
        .rows
        .get(state.result_row.min(rows.rows.len().saturating_sub(1)))
        .map(|row| &row.value);
    let actions = presentation
        .actions
        .iter()
        .filter(|action| match action.scope {
            upeg_core::ActionScope::Row => rows.row_actions_enabled && row.is_some(),
            upeg_core::ActionScope::Result => true,
        })
        .collect::<Vec<_>>();
    let Some(action) = actions.get(state.result_action.min(actions.len().saturating_sub(1))) else {
        state.status_message = Some(
            rows.diagnostics
                .first()
                .cloned()
                .unwrap_or_else(|| "no follow-up action is available".to_string()),
        );
        return Effect::None;
    };
    let availability = upeg_core::resolve_action_availability(presentation, action, &output_values);
    if !availability.enabled {
        state.status_message = availability.reason;
        return Effect::None;
    }
    if matches!(action.scope, upeg_core::ActionScope::Row)
        && source.effect == upeg_core::ToolEffect::Read
        && let (Some(result_run), Some(PresentationHost::LocalTui)) =
            (state.result_run, state.result_host)
    {
        state.presentation_origin = Some(PresentationOrigin {
            tool_id: source.id,
            inputs: state.result_inputs.clone(),
            selected_row_key: rows
                .rows
                .get(state.result_row.min(rows.rows.len().saturating_sub(1)))
                .map(|row| row.key.clone()),
            host: PresentationHost::LocalTui,
            result_run,
        });
    }
    let Some(target) = upeg_runtime::toolbox_tool(&action.target_tool) else {
        state.status_message = Some(format!(
            "target tool `{}` is not installed",
            action.target_tool
        ));
        return Effect::None;
    };
    let resolved = upeg_core::resolve_bindings(
        action,
        &state.result_inputs,
        row,
        &output_values,
        &target.input_spec,
    );
    if let Some(diagnostic) = resolved.diagnostics.first() {
        state.status_message = Some(diagnostic.clone());
        return Effect::None;
    }
    let values = Value::Object(resolved.values.into_iter().collect());
    state.refresh_after_tool = (action.on_success == Some(upeg_core::ActionSuccess::RefreshOrigin)
        && state.presentation_origin.is_some())
    .then_some(target.id);
    state.right_scroll = ScrollOffset::ZERO;
    state
        .presentation_frames
        .push(super::super::model::PresentationFrame {
            view: state.view.clone(),
            inputs: state.result_inputs.clone(),
            selected_row: state.result_row,
            selected_action: state.result_action,
            result_run: state.result_run,
        });
    if target.effect == upeg_core::ToolEffect::Read && resolved.unbound_required_inputs.is_empty() {
        return dispatch_or_confirm(state, target.id, values);
    }
    state.view = View::Form {
        tool_id: target.id,
        form: TuiFormState::with_initial_values(target.input_spec.clone(), &values),
    };
    Effect::None
}

pub(super) fn apply_outcome_with_host(
    state: &mut State,
    tool_id: &'static str,
    outcome: Outcome,
    host: PresentationHost,
) -> Effect {
    let completed_run = state.active_run.map(|active| active.run);
    state.active_run = None;
    let refresh_origin = matches!(outcome, Outcome::Success(_))
        .then(|| state.refresh_after_tool.filter(|target| *target == tool_id))
        .flatten()
        .and_then(|_| state.presentation_origin.clone())
        .filter(|origin| host == origin.host && origin_is_current(state, origin));
    if state.refresh_after_tool == Some(tool_id) {
        state.refresh_after_tool = None;
    }
    let (text, is_error) = match outcome {
        Outcome::Success(success) => {
            state.right_scroll = ScrollOffset::ZERO;
            state.focus = FocusArea::RightPane;
            state.view = View::Result {
                tool_id,
                outputs: success.outputs,
                text: String::new(),
                is_error: false,
            };
            state.result_run = completed_run;
            state.result_host = Some(host);
            if let Some(origin) = refresh_origin {
                let selected_row_key = origin.selected_row_key.clone();
                let effect = start_run(state, origin.tool_id, origin.inputs);
                state.restore_result_row_key = selected_row_key;
                state.presentation_frames.clear();
                return effect;
            }
            restore_result_row_selection(state);
            return Effect::None;
        }
        Outcome::Failure(failure) => (
            crate::domain::execution::dispatch::failure_text(&failure),
            true,
        ),
        Outcome::NotFound => (
            crate::i18n::t(state.tweaks.locale, "tui.error.tool_not_found").to_string(),
            true,
        ),
    };
    state.right_scroll = ScrollOffset::ZERO;
    state.focus = FocusArea::RightPane;
    state.view = View::Result {
        tool_id,
        outputs: Vec::new(),
        text,
        is_error,
    };
    Effect::None
}

fn origin_is_current(state: &State, origin: &PresentationOrigin) -> bool {
    origin.host == PresentationHost::LocalTui
        && state.presentation_frames.iter().any(|frame| {
            frame.result_run == Some(origin.result_run)
                && frame.inputs == origin.inputs
                && matches!(&frame.view, View::Result { tool_id, .. } if *tool_id == origin.tool_id)
        })
}

fn restore_result_row_selection(state: &mut State) {
    let Some(key) = state.restore_result_row_key.take() else {
        return;
    };
    let View::Result {
        tool_id,
        outputs,
        is_error: false,
        ..
    } = &state.view
    else {
        return;
    };
    let Some(presentation) =
        upeg_runtime::toolbox_tool(tool_id).and_then(|tool| tool.presentation.as_ref())
    else {
        return;
    };
    let rows = upeg_core::resolve_rows(presentation, &presentation_outputs(outputs));
    if let Some(index) = rows.rows.iter().position(|row| row.key == key) {
        state.result_row = index;
    }
}
