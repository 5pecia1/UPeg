//! State-machine tests for the approval gate (Task 1) and running live
//! output / cancellation (Task 2).
//!
//! The approval-policy registry
//! (`upeg_runtime::set_tool_approval_policy`) is process-global state
//! shared by the whole test binary. So every test uses its own tool
//! id, and registered policies are restored to
//! `ToolApprovalPolicy::none()` by [`PolicyGuard`] on Drop.

use super::*;
use crate::surfaces::tui::model::{LiveTail, PresentationHost, RunToken, TUI_LIVE_TAIL_MAX_LINES};
use crate::surfaces::tui::update::dispatch_or_confirm;
use upeg_runtime::{
    ProgressEvent, ProgressStream, ToolApprovalPolicy, set_tool_approval_policy,
    tool_approval_policy,
};

/// Guard that always clears the registered approval policy when the
/// test ends. Even on panic the policy does not leak into the next
/// test.
struct PolicyGuard(&'static str);

impl PolicyGuard {
    fn gated(tool_id: &'static str, surfaces: Vec<Surface>) -> Self {
        set_tool_approval_policy(tool_id, ToolApprovalPolicy::gated(surfaces));
        Self(tool_id)
    }
}

impl Drop for PolicyGuard {
    fn drop(&mut self) {
        set_tool_approval_policy(self.0, ToolApprovalPolicy::none());
    }
}

/// A no-arg tool with a unique id. Since the policy registry is
/// global, each test must use a different id to avoid
/// cross-contamination.
fn no_input_tool(id: &'static str) -> &'static ToolMeta {
    Box::leak(Box::new(ToolMeta {
        id,
        toolkit: "test",
        local_id: "approval",
        tags: &[],
        display_label: "Approval tool",
        description: "chain with an approval step",
        input_spec: InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: Invoker::Chain,
        surfaces: &[Surface::Cli, Surface::Tui],
        boards: &[],
    }))
}

fn progress(chunk: &str) -> ProgressEvent {
    ProgressEvent {
        stream: ProgressStream::Stdout,
        seq: 0,
        chunk: chunk.to_string(),
    }
}

/// A progress message stamped as belonging to that run for one chunk.
/// Same shape as what the event loop builds in `RunningDispatch`.
fn tool_progress(run: RunToken, chunk: &str) -> Msg<'static> {
    Msg::ToolProgress {
        run,
        event: progress(chunk),
    }
}

/// Exactly what `start_run` really produces: the model is holding this
/// run as alive with focus on the right pane, in the running state.
/// Lets key-routing tests travel the real path.
fn running_state(tool_id: &'static str) -> State {
    let (state, _) = running_state_with_run(tool_id);
    state
}

/// Like [`running_state`] but also returns this run's [`RunToken`] —
/// progress events must be stamped with that token.
fn running_state_with_run(tool_id: &'static str) -> (State, RunToken) {
    let mut state = State {
        focus: FocusArea::RightPane,
        ..State::default()
    };
    let run = state.start_active_run(tool_id);
    state.view = View::Running {
        tool_id,
        tail: LiveTail::default(),
        cancelling: false,
    };
    (state, run)
}

/// A state that has already left the running pane — as after opening
/// the cancel-confirm overlay and backing out. The run itself
/// continues, so the model is still holding the run.
fn left_running_pane(tool_id: &'static str) -> (State, RunToken) {
    let (mut state, run) = running_state_with_run(tool_id);
    state.view = View::List;
    (state, run)
}

fn tail_lines(state: &State) -> Vec<String> {
    match &state.view {
        View::Running { tail, .. } => tail.lines().map(ToOwned::to_owned).collect(),
        other => panic!("expected the Running view but got {other:?}"),
    }
}

// ─── Task 1: approval gate ─────────────────────────────────────────

#[test]
fn tool_requiring_approval_opens_approval_screen_instead_of_running() {
    let tool = no_input_tool("test.approval.tui.opens_prompt");
    let _guard = PolicyGuard::gated(tool.id, vec![Surface::Tui]);
    let tools = [tool];
    let mut s = State::default();

    let effect = handle_key(&mut s, Key::Enter, &tools);

    assert_eq!(
        effect,
        Action::None,
        "dispatch must not go out before approval"
    );
    match &s.view {
        View::ConfirmApproval { tool_id, args } => {
            assert_eq!(*tool_id, tool.id);
            assert_eq!(*args, json!({}));
        }
        other => panic!("expected ConfirmApproval but got {other:?}"),
    }
}

#[test]
fn confirm_on_approval_screen_sends_dispatch_carrying_approve_true() {
    let tool = no_input_tool("test.approval.tui.confirm_keys");
    let _guard = PolicyGuard::gated(tool.id, vec![Surface::Tui]);
    let tools = [tool];

    for key in [Key::Enter, Key::Char('y'), Key::F(1)] {
        let mut s = State {
            view: View::ConfirmApproval {
                tool_id: tool.id,
                args: json!({ "target": "prod" }),
            },
            ..State::default()
        };

        match handle_key(&mut s, key, &tools) {
            Action::Dispatch { tool_id, args, .. } => {
                assert_eq!(tool_id, tool.id);
                assert_eq!(
                    args,
                    json!({ "target": "prod", "approve": true }),
                    "{key:?} approval must carry the reserved key approve=true"
                );
            }
            other => panic!("expected Dispatch on {key:?} approval but got {other:?}"),
        }
        assert!(
            matches!(s.view, View::Running { .. }),
            "{key:?} approval must move to the running view"
        );
    }
}

#[test]
fn esc_on_approval_screen_returns_to_list_without_running_anything() {
    let tool = no_input_tool("test.approval.tui.cancel");
    let _guard = PolicyGuard::gated(tool.id, vec![Surface::Tui]);
    let tools = [tool];

    for key in [Key::Esc, Key::Char('n')] {
        let mut s = State {
            view: View::ConfirmApproval {
                tool_id: tool.id,
                args: json!({}),
            },
            ..State::default()
        };

        let effect = handle_key(&mut s, key, &tools);

        assert_eq!(effect, Action::None, "{key:?} runs nothing");
        assert_eq!(s.view, View::List, "{key:?} returns to the list");
    }
}

#[test]
fn ungated_tool_runs_immediately_without_approve_key() {
    let tool = no_input_tool("test.approval.tui.ungated");
    // No policy is registered: the default state of an ungated tool.
    assert!(!tool_approval_policy(tool.id).requires_approval());
    let tools = [tool];
    let mut s = State::default();

    match handle_key(&mut s, Key::Enter, &tools) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, tool.id);
            assert_eq!(
                args,
                json!({}),
                "without a gate there must be no approve key either"
            );
        }
        other => panic!("expected Dispatch but got {other:?}"),
    }
    assert!(matches!(s.view, View::Running { .. }));
}

#[test]
fn approval_policy_not_honoring_tui_only_shows_explanation() {
    let tool = no_input_tool("test.approval.tui.not_honored");
    let _guard = PolicyGuard::gated(tool.id, vec![Surface::Cli, Surface::Http]);
    let tools = [tool];
    let mut s = State::default();

    let effect = handle_key(&mut s, Key::Enter, &tools);

    assert_eq!(effect, Action::None);
    match &s.view {
        View::Result {
            tool_id,
            text,
            is_error,
            ..
        } => {
            assert_eq!(*tool_id, tool.id);
            assert!(*is_error, "an approval not honored is shown as an error");
            assert!(
                text.contains("cli/http"),
                "must name the surfaces that honor approval but got `{text}`"
            );
        }
        other => panic!("expected a Result carrying the explanation but got {other:?}"),
    }
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn form_path_also_passes_through_approval_gate() {
    let tool_id = "test.approval.tui.form_path";
    let _guard = PolicyGuard::gated(tool_id, vec![Surface::Tui]);
    let tools = fixture_tools();
    let mut s = State {
        view: View::Form {
            tool_id,
            form: string_form("input", "0xff", true),
        },
        ..State::default()
    };

    let effect = handle_key(&mut s, Key::Enter, tools.as_slice());

    assert_eq!(
        effect,
        Action::None,
        "form runs must also be blocked before approval"
    );
    match &s.view {
        View::ConfirmApproval { tool_id: id, args } => {
            assert_eq!(*id, tool_id);
            assert_eq!(*args, json!({ "input": "0xff" }));
        }
        other => panic!("expected ConfirmApproval but got {other:?}"),
    }
}

#[test]
fn approval_on_form_path_forwards_form_built_args_verbatim() {
    let tool_id = "test.approval.tui.form_args";
    let _guard = PolicyGuard::gated(tool_id, vec![Surface::Tui]);
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let mut s = State {
        view: View::Form {
            tool_id,
            form: string_form("input", "0xff", true),
        },
        ..State::default()
    };

    handle_key(&mut s, Key::Enter, tools);
    match handle_key(&mut s, Key::Enter, tools) {
        Action::Dispatch {
            tool_id: id, args, ..
        } => {
            assert_eq!(id, tool_id);
            assert_eq!(args, json!({ "input": "0xff", "approve": true }));
        }
        other => panic!("expected Dispatch after approval but got {other:?}"),
    }
}

#[test]
fn approval_required_form_tool_still_opens_form_first_from_detail() {
    // The gate stands only just before dispatch: a tool that needs
    // input must still show the form first.
    let _guard = PolicyGuard::gated("test.with_input", vec![Surface::Tui]);
    let tools = fixture_tools();
    let mut s = State {
        cursor: 1,
        view: View::Detail,
        ..State::default()
    };

    let effect = handle_key(&mut s, Key::Enter, tools.as_slice());

    assert_eq!(effect, Action::None);
    assert!(matches!(s.view, View::Form { .. }));
}

// ─── Task 2: live output + cancel ──────────────────────────────────

#[test]
fn progress_events_accumulate_in_running_tail() {
    let (mut s, run) = running_state_with_run("test.live.append");

    update(&mut s, tool_progress(run, "first\n"));
    update(&mut s, tool_progress(run, "sec"));
    update(&mut s, tool_progress(run, "ond\n"));

    assert_eq!(tail_lines(&s), ["first", "second"]);
}

#[test]
fn tail_keeps_only_last_n_lines() {
    let (mut s, run) = running_state_with_run("test.live.cap");

    for index in 0..(TUI_LIVE_TAIL_MAX_LINES + 5) {
        update(&mut s, tool_progress(run, &format!("line {index}\n")));
    }

    let lines = tail_lines(&s);
    assert_eq!(lines.len(), TUI_LIVE_TAIL_MAX_LINES);
    assert_eq!(lines.first().map(String::as_str), Some("line 5"));
}

#[test]
fn progress_events_while_not_running_are_ignored() {
    let (mut s, run) = running_state_with_run("test.live.late");
    // After the final envelope already arrived and the model released
    // the run.
    s.view = View::Result {
        tool_id: "test.live.late",
        outputs: Vec::new(),
        text: "finished result".into(),
        is_error: false,
    };
    s.active_run = None;

    update(&mut s, tool_progress(run, "late_arriving_output\n"));

    match &s.view {
        View::Result { text, .. } => assert_eq!(text, "finished result"),
        other => panic!("a late progress event must not change the view but got {other:?}"),
    }
}

#[test]
fn esc_while_running_requests_cancel_and_marks_cancelling() {
    let tools = fixture_tools();

    for key in [Key::Esc, Key::Char('q')] {
        let mut s = running_state("test.live.cancel");

        let effect = handle_key(&mut s, key, tools.as_slice());

        assert_eq!(
            effect,
            Action::CancelRun,
            "{key:?} requests run cancellation"
        );
        match &s.view {
            View::Running { cancelling, .. } => {
                assert!(*cancelling, "after {key:?} it must be marked cancelling");
            }
            other => panic!("must still be Running after {key:?} but got {other:?}"),
        }
    }
}

#[test]
fn esc_again_on_already_cancelled_run_opens_quit_confirm() {
    // Cancel is only a request, so an invoker that ignores the token
    // may never finish. A view that swallows every other key must not
    // lack an escape hatch, so a second Esc escalates — but without
    // bypassing the quit-guard contract: quitting must go through the
    // confirm dialog.
    let tools = fixture_tools();
    let mut s = running_state("test.live.escalate");

    assert_eq!(
        handle_key(&mut s, Key::Esc, tools.as_slice()),
        Action::CancelRun
    );
    assert_eq!(
        handle_key(&mut s, Key::Esc, tools.as_slice()),
        Action::CancelRun,
        "the second Esc must also re-send the cancel request"
    );
    assert_eq!(
        s.view,
        View::ConfirmQuit,
        "the second Esc opens the quit confirm instead of ending the session outright"
    );
}

#[test]
fn yes_on_quit_confirm_while_running_finally_quits() {
    // The end of the escalation path: confirmation is still one
    // deliberate human y.
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let mut s = running_state("test.live.escalate_confirm");

    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Esc, tools);

    assert_eq!(handle_key(&mut s, Key::Char('y'), tools), Action::Quit);
}

#[test]
fn dismissing_quit_confirm_while_running_returns_to_list() {
    // Pins an honesty gap: dismissing the confirm does not return to
    // the running pane. The run itself continues and the result still
    // arrives.
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let mut s = running_state("test.live.escalate_back");

    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Esc, tools);

    assert_eq!(handle_key(&mut s, Key::Char('n'), tools), Action::None);
    assert_eq!(s.view, View::List);
}

#[test]
fn other_keys_are_ignored_while_running() {
    let tools = fixture_tools();

    for key in [Key::Char('o'), Key::Char('s'), Key::Char('n'), Key::Enter] {
        let mut s = running_state("test.live.swallow");

        let effect = handle_key(&mut s, key, tools.as_slice());

        assert_eq!(
            effect,
            Action::None,
            "{key:?} is not consumed while running"
        );
        assert!(
            matches!(
                s.view,
                View::Running {
                    cancelling: false,
                    ..
                }
            ),
            "{key:?} must not change the running view"
        );
    }
}

#[test]
fn right_pane_scroll_keeps_working_while_running() {
    // Scrolling back through the live tail does not disturb the run —
    // same contract as the Result view.
    let tools = fixture_tools();
    let mut s = running_state("test.live.scroll");

    let effect = handle_key(&mut s, Key::Down, tools.as_slice());

    assert_eq!(effect, Action::None);
    assert!(matches!(
        s.view,
        View::Running {
            cancelling: false,
            ..
        }
    ));
    assert!(
        s.right_scroll.get() > 0,
        "the pane scrolls even while running"
    );
}

#[test]
fn tool_done_replaces_running_view_with_result() {
    let (mut s, run) = running_state_with_run("test.live.done");
    update(&mut s, tool_progress(run, "working\n"));

    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
            run,
            host: PresentationHost::LocalTui,
            tool_id: "test.live.done",
            outcome: Outcome::Success(success),
        },
    );

    match &s.view {
        View::Result {
            tool_id, is_error, ..
        } => {
            assert_eq!(*tool_id, "test.live.done");
            assert!(!*is_error);
        }
        other => panic!("expected Result but got {other:?}"),
    }
}

#[test]
fn running_view_dominates_right_pane() {
    // Running must declare the same presentation as Form/Result so
    // render/mouse/focus see the same body shape while running.
    let running = running_state("test.live.presentation");
    let result = State {
        view: View::Result {
            tool_id: "test.live.presentation",
            outputs: Vec::new(),
            text: String::new(),
            is_error: false,
        },
        ..State::default()
    };

    assert_eq!(
        running.view.body_presentation(),
        result.view.body_presentation()
    );
}

// ─── Run state is held by State, not View ──────────────────────

#[test]
fn model_keeps_run_alive_after_leaving_running_pane() {
    // Opening then dismissing the cancel-confirm overlay returns to the
    // list, but the worker keeps running. If the model forgot the run
    // here it would diverge from the event loop.
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let (mut s, run) = running_state_with_run("test.live.keeps_run");

    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Char('n'), tools);

    assert_eq!(s.view, View::List);
    assert_eq!(
        s.active_run.map(|active| active.run),
        Some(run),
        "the run continues even after leaving the pane"
    );
}

#[test]
fn second_run_is_rejected_while_run_remains() {
    let (mut s, run) = left_running_pane("test.live.first");

    let effect = dispatch_or_confirm(&mut s, "test.simple", json!({}));

    assert_eq!(effect, Action::None, "the second dispatch never goes out");
    assert_eq!(
        s.active_run.map(|active| active.run),
        Some(run),
        "the earlier run stays intact"
    );
    assert!(s.status_message.is_some(), "tells the user what is running");
    match &s.view {
        View::Running { tool_id, .. } => assert_eq!(
            *tool_id, "test.live.first",
            "returns to the running tool's pane, not the new tool"
        ),
        other => panic!("expected the Running view but got {other:?}"),
    }
}

#[test]
fn rejected_second_run_pane_is_not_mixed_with_previous_run_output() {
    // Regression: previously a second Run opened a new View::Running
    // and the event loop silently dropped that dispatch. The new pane
    // then filled with the *old* run's chunks and the old run's result
    // rendered as if it were the new tool's.
    let (mut s, first_run) = left_running_pane("test.live.mixed_first");

    dispatch_or_confirm(&mut s, "test.simple", json!({}));
    update(&mut s, tool_progress(first_run, "earlier run output\n"));

    match &s.view {
        View::Running { tool_id, .. } => assert_eq!(*tool_id, "test.live.mixed_first"),
        other => panic!("expected the Running view but got {other:?}"),
    }
    assert_eq!(tail_lines(&s), ["earlier run output"]);
}

#[test]
fn stale_run_progress_events_do_not_enter_next_run_tail() {
    let (mut s, first_run) = running_state_with_run("test.live.stale_first");
    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
            run: first_run,
            host: PresentationHost::LocalTui,
            tool_id: "test.live.stale_first",
            outcome: Outcome::Success(success),
        },
    );

    let second_run = s.start_active_run("test.live.stale_second");
    s.view = View::Running {
        tool_id: "test.live.stale_second",
        tail: LiveTail::default(),
        cancelling: false,
    };

    update(&mut s, tool_progress(first_run, "stale run output\n"));
    assert!(tail_lines(&s).is_empty(), "stale run chunks are dropped");

    update(&mut s, tool_progress(second_run, "current run output\n"));
    assert_eq!(tail_lines(&s), ["current run output"]);
}

#[test]
fn stale_final_envelope_cannot_replace_a_newer_run() {
    let (mut s, stale_run) = running_state_with_run("test.live.same_tool");
    let current_run = s.start_active_run("test.live.same_tool");
    s.view = View::Running {
        tool_id: "test.live.same_tool",
        tail: LiveTail::default(),
        cancelling: false,
    };

    let effect = update(
        &mut s,
        Msg::ToolDone {
            run: stale_run,
            host: PresentationHost::LocalTui,
            tool_id: "test.live.same_tool",
            outcome: Outcome::Success(crate::domain::execution::dispatch::text_success("stale")),
        },
    );

    assert_eq!(effect, Action::None);
    assert_eq!(
        s.active_run.expect("new run remains active").run,
        current_run
    );
    assert!(matches!(s.view, View::Running { .. }));
}

#[test]
fn completed_result_records_an_unverified_attached_host() {
    let (mut s, run) = running_state_with_run("test.live.attached");
    update(
        &mut s,
        Msg::ToolDone {
            run,
            host: PresentationHost::AttachedUnverified,
            tool_id: "test.live.attached",
            outcome: Outcome::Success(crate::domain::execution::dispatch::text_success("done")),
        },
    );

    assert_eq!(s.result_host, Some(PresentationHost::AttachedUnverified));
}

#[test]
fn model_releases_run_when_final_envelope_arrives() {
    let (mut s, run) = running_state_with_run("test.live.release");

    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
            run,
            host: PresentationHost::LocalTui,
            tool_id: "test.live.release",
            outcome: Outcome::Success(success),
        },
    );

    assert!(s.active_run.is_none(), "a finished run is not held");
}

#[test]
fn can_run_again_after_run_finishes() {
    let (mut s, run) = running_state_with_run("test.live.rerun");
    update(
        &mut s,
        Msg::ToolDone {
            run,
            host: PresentationHost::LocalTui,
            tool_id: "test.live.rerun",
            outcome: Outcome::NotFound,
        },
    );

    let effect = dispatch_or_confirm(&mut s, "test.simple", json!({}));

    assert!(
        matches!(
            effect,
            Action::Dispatch {
                tool_id: "test.simple",
                ..
            }
        ),
        "a Run after completion goes out normally"
    );
}
