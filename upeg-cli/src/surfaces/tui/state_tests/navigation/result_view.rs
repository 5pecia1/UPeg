use super::*;
use crate::surfaces::tui::model::{PresentationFrame, PresentationOrigin};
use std::collections::BTreeMap;
use upeg_core::{ActionBinding, ActionScope, PresentationAction, ToolEffect, ToolPresentation};
use upeg_core::{InputSpec, OutputEntry, OutputKind, OutputValue};

fn follow_up_state(
    target_id: &'static str,
    target_local_id: &'static str,
    effect: ToolEffect,
    input: serde_json::Value,
) -> State {
    follow_up_state_with_availability(target_id, target_local_id, effect, input, None)
}

fn follow_up_state_with_availability(
    target_id: &'static str,
    target_local_id: &'static str,
    effect: ToolEffect,
    input: serde_json::Value,
    enabled: Option<bool>,
) -> State {
    let source_id: &'static str =
        Box::leak(format!("test.source_{target_local_id}").into_boxed_str());
    let source_local_id: &'static str =
        Box::leak(format!("source_{target_local_id}").into_boxed_str());
    let mut source = fixture_tools()[0].clone();
    source.id = source_id;
    source.local_id = source_local_id;
    source.presentation = Some(ToolPresentation {
        version: upeg_core::PRESENTATION_VERSION_V1,
        output: enabled.map(|_| "result".into()),
        rows: None,
        row_key: None,
        columns: Vec::new(),
        actions: vec![PresentationAction {
            id: "follow".into(),
            scope: ActionScope::Result,
            label: "Follow".into(),
            target_tool: target_id.into(),
            on_success: None,
            enabled_pointer: enabled.map(|_| "/view/actions/follow/enabled".into()),
            disabled_reason_pointer: None,
            bindings: BTreeMap::from([(
                "project".into(),
                ActionBinding::Input {
                    pointer: "/project".into(),
                },
            )]),
        }],
        title_pointer: None,
        subtitle_pointer: None,
        status: None,
        summary: Vec::new(),
        notices: None,
        detail: None,
        row_detail: None,
        empty_message_pointer: None,
    });
    upeg_runtime::toolbox_add_tool(source);
    let mut target = fixture_tools()[0].clone();
    target.id = target_id;
    target.local_id = target_local_id;
    target.effect = effect;
    target.input_spec =
        InputSpec::new(vec![string_field("project", true)]).expect("required project input");
    upeg_runtime::toolbox_add_tool(target);
    State {
        view: View::Result {
            tool_id: source_id,
            outputs: enabled.map_or_else(Vec::new, |enabled| {
                vec![OutputEntry {
                    id: "result".into(),
                    label: None,
                    kind: OutputKind::Json,
                    value: OutputValue::Json(
                        json!({"view":{"actions":{"follow":{"enabled":enabled}}}}),
                    ),
                }]
            }),
            text: "result".into(),
            is_error: false,
        },
        result_inputs: input,
        ..fresh()
    }
}

#[test]
fn disabled_result_follow_up_key_stays_on_result() {
    let mut state = follow_up_state_with_availability(
        "test.follow_disabled",
        "follow_disabled",
        ToolEffect::Write,
        json!({"project":"alpha"}),
        Some(false),
    );
    assert_eq!(
        handle_key(&mut state, Key::Char('a'), &fixture_tools()),
        Action::None
    );
    assert!(matches!(state.view, View::Result { .. }));
    assert_eq!(
        state.status_message.as_deref(),
        Some("현재 실행할 수 없습니다")
    );
}

#[test]
fn complete_read_follow_up_dispatches_without_opening_form() {
    let mut state = follow_up_state(
        "test.follow_read_complete",
        "follow_read_complete",
        ToolEffect::Read,
        json!({"project":"alpha"}),
    );
    match handle_key(&mut state, Key::Char('a'), &fixture_tools()) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.follow_read_complete");
            assert_eq!(args, json!({"project":"alpha"}));
        }
        other => panic!("complete read must dispatch, got {other:?}"),
    }
    assert!(matches!(state.view, View::Running { .. }));
}

#[test]
fn incomplete_read_follow_up_opens_prefilled_form() {
    let mut state = follow_up_state(
        "test.follow_read_incomplete",
        "follow_read_incomplete",
        ToolEffect::Read,
        json!({"project":null}),
    );
    assert_eq!(
        handle_key(&mut state, Key::Char('a'), &fixture_tools()),
        Action::None
    );
    assert!(matches!(
        state.view,
        View::Form {
            tool_id: "test.follow_read_incomplete",
            ..
        }
    ));
}

#[test]
fn write_and_unknown_follow_ups_keep_explicit_form() {
    for (id, local_id, effect) in [
        ("test.follow_write", "follow_write", ToolEffect::Write),
        ("test.follow_unknown", "follow_unknown", ToolEffect::Unknown),
    ] {
        let mut state = follow_up_state(id, local_id, effect, json!({"project":"alpha"}));
        assert_eq!(
            handle_key(&mut state, Key::Char('a'), &fixture_tools()),
            Action::None
        );
        assert!(matches!(state.view, View::Form { tool_id, .. } if tool_id == id));
    }
}

// ─── Result view semantics: rerun / close / keep, F2 copy ───

#[test]
fn enter_on_result_reruns_same_tool() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: "previous result".into(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    match handle_key(&mut s, Key::Enter, t) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, "test.simple");
            assert_eq!(args, json!({}));
        }
        other => panic!("Enter on Result must rerun the same tool, got {other:?}"),
    }
}

#[test]
fn f1_on_result_also_reruns() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    assert!(matches!(
        handle_key(&mut s, Key::F(1), t),
        Action::Dispatch {
            tool_id: "test.simple",
            ..
        }
    ));
}

#[test]
fn esc_and_q_on_result_close_to_list() {
    for key in [Key::Esc, Key::Char('q')] {
        let mut s = State {
            cursor: 0,
            view: View::Result {
                tool_id: "test.simple",
                outputs: Vec::new(),
                text: "result".into(),
                is_error: false,
            },
            ..fresh()
        };
        let t = fixture_tools();
        let t = t.as_slice();

        let effect = handle_key(&mut s, key, t);
        assert_eq!(effect, Action::None);
        assert_eq!(s.view, View::List, "{key:?} must close Result to List");
    }
}

#[test]
fn unconsumed_key_on_result_keeps_result() {
    // Old contract: any key closed back to the list. Under the new
    // contract all keys except Enter/F1 (rerun) and Esc/q (close) are
    // ignored and the result must stay.
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: "keep result".into(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::Char('x'), t);
    assert_eq!(effect, Action::None);
    assert!(
        matches!(&s.view, View::Result { text, .. } if text == "keep result"),
        "an unrelated key must keep the result view, got {:?}",
        s.view
    );
}

#[test]
fn action_navigation_is_bounded_by_actions_available_on_the_result() {
    let mut s = State {
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: "result".into(),
            is_error: false,
        },
        result_action: 1,
        ..fresh()
    };
    let tools = fixture_tools();

    assert_eq!(handle_key(&mut s, Key::Left, &tools), Action::None);
    assert_eq!(s.result_action, 0);
    assert_eq!(handle_key(&mut s, Key::Right, &tools), Action::None);
    assert_eq!(s.result_action, 0, "this fixture declares no actions");
    assert!(matches!(s.view, View::Result { .. }));
}

#[test]
fn failed_follow_up_consumes_refresh_intent() {
    let mut s = State {
        refresh_after_tool: Some("test.simple"),
        ..fresh()
    };
    let failure = upeg_core::ToolFailure {
        error: upeg_core::ToolError {
            code: "failed".into(),
            message: "failed".into(),
            details: None,
        },
    };

    let effect = apply_outcome(&mut s, "test.simple", Outcome::Failure(failure));

    assert_eq!(effect, Action::None);
    assert_eq!(s.refresh_after_tool, None);
}

fn state_with_refreshable_local_origin() -> (State, crate::surfaces::tui::model::RunToken) {
    let mut state = fresh();
    let origin_run = state.start_active_run("test.origin");
    state.active_run = None;
    state.presentation_origin = Some(PresentationOrigin {
        tool_id: "test.origin",
        inputs: json!({"project": "origin"}),
        selected_row_key: Some("row-1".into()),
        host: PresentationHost::LocalTui,
        result_run: origin_run,
    });
    state.presentation_frames.push(PresentationFrame {
        view: View::Result {
            tool_id: "test.origin",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        inputs: json!({"project": "origin"}),
        selected_row: 0,
        selected_action: 0,
        result_run: Some(origin_run),
    });
    state.refresh_after_tool = Some("test.follow_up");
    let follow_up_run = state.start_active_run("test.follow_up");
    (state, follow_up_run)
}

#[test]
fn local_follow_up_completion_refreshes_a_current_local_origin() {
    let (mut state, follow_up_run) = state_with_refreshable_local_origin();

    let effect = update(
        &mut state,
        Msg::ToolDone {
            run: follow_up_run,
            host: PresentationHost::LocalTui,
            tool_id: "test.follow_up",
            outcome: Outcome::Success(crate::domain::execution::dispatch::text_success("done")),
        },
    );

    assert!(matches!(
        effect,
        Action::Dispatch {
            tool_id: "test.origin",
            args,
            ..
        } if args == json!({"project": "origin"})
    ));
    assert!(matches!(
        state.view,
        View::Running {
            tool_id: "test.origin",
            ..
        }
    ));
    assert!(state.presentation_frames.is_empty());
}

#[test]
fn attached_follow_up_completion_does_not_refresh_a_local_origin() {
    let (mut state, follow_up_run) = state_with_refreshable_local_origin();

    let effect = update(
        &mut state,
        Msg::ToolDone {
            run: follow_up_run,
            host: PresentationHost::AttachedUnverified,
            tool_id: "test.follow_up",
            outcome: Outcome::Success(crate::domain::execution::dispatch::text_success("done")),
        },
    );

    assert_eq!(effect, Action::None);
    assert_eq!(state.refresh_after_tool, None);
    assert_eq!(
        state.result_host,
        Some(PresentationHost::AttachedUnverified)
    );
    assert!(matches!(
        state.view,
        View::Result {
            tool_id: "test.follow_up",
            is_error: false,
            ..
        }
    ));
    assert_eq!(state.presentation_frames.len(), 1);
}

#[test]
fn escape_from_follow_up_form_restores_the_exact_result_frame() {
    let previous = View::Result {
        tool_id: "test.simple",
        outputs: Vec::new(),
        text: "origin".into(),
        is_error: false,
    };
    let mut s = State {
        view: View::Form {
            tool_id: "test.with_input",
            form: TuiFormState::new(InputSpec::empty()),
        },
        result_inputs: json!({"project":"a"}),
        result_row: 0,
        result_action: 0,
        presentation_frames: vec![PresentationFrame {
            view: previous.clone(),
            inputs: json!({"project":"origin"}),
            selected_row: 3,
            selected_action: 1,
            result_run: None,
        }],
        ..fresh()
    };

    assert_eq!(handle_key(&mut s, Key::Esc, &fixture_tools()), Action::None);
    assert_eq!(s.view, previous);
    assert_eq!(s.result_inputs, json!({"project":"origin"}));
    assert_eq!(s.result_row, 3);
    assert_eq!(s.result_action, 1);
    assert!(s.presentation_frames.is_empty());
}

#[test]
fn escape_from_follow_up_result_pops_only_one_frame() {
    let previous = View::Result {
        tool_id: "test.simple",
        outputs: Vec::new(),
        text: "previous".into(),
        is_error: false,
    };
    let mut s = State {
        view: View::Result {
            tool_id: "test.with_input",
            outputs: Vec::new(),
            text: "follow-up".into(),
            is_error: false,
        },
        presentation_frames: vec![PresentationFrame {
            view: previous.clone(),
            inputs: json!({}),
            selected_row: 0,
            selected_action: 0,
            result_run: None,
        }],
        ..fresh()
    };

    assert_eq!(handle_key(&mut s, Key::Esc, &fixture_tools()), Action::None);
    assert_eq!(s.view, previous);
}

#[test]
fn f2_on_result_copies_error_message_first() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: vec![OutputEntry {
                id: "result".into(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String("should be ignored".into()),
            }],
            text: "boom".into(),
            is_error: true,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(effect, Action::CopyToClipboard("boom".to_string()));
}

#[test]
fn f2_on_result_copies_primary_output_text_when_not_error() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: vec![OutputEntry {
                id: "result".into(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String("42".into()),
            }],
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(effect, Action::CopyToClipboard("42".to_string()));
}

#[test]
fn f2_on_result_without_output_or_text_copies_nothing() {
    let mut s = State {
        cursor: 0,
        view: View::Result {
            tool_id: "test.simple",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    assert_eq!(handle_key(&mut s, Key::F(2), t), Action::None);
}

#[test]
fn f2_on_detail_copies_tool_id() {
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..fresh()
    };
    let t = fixture_tools();
    let t = t.as_slice();

    let effect = handle_key(&mut s, Key::F(2), t);
    assert_eq!(
        effect,
        Action::CopyToClipboard("test.with_input".to_string())
    );
}
