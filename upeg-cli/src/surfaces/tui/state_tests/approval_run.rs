//! 승인 게이트(Task 1)와 실행 중 라이브 출력 / 취소(Task 2)의 상태
//! 기계 테스트.
//!
//! 승인 정책 레지스트리(`upeg_runtime::set_tool_approval_policy`)는 테스트
//! 바이너리 전체가 공유하는 프로세스 전역 상태다. 그래서 모든 테스트는
//! 자기만의 tool id를 쓰고, 등록한 정책은 [`PolicyGuard`]가 Drop에서
//! `ToolApprovalPolicy::none()`으로 되돌린다.

use super::*;
use crate::surfaces::tui::model::{LiveTail, RunToken, TUI_LIVE_TAIL_MAX_LINES};
use crate::surfaces::tui::update::dispatch_or_confirm;
use upeg_runtime::{
    ProgressEvent, ProgressStream, ToolApprovalPolicy, set_tool_approval_policy,
    tool_approval_policy,
};

/// 등록한 승인 정책을 테스트가 끝날 때 반드시 지우는 가드. 패닉으로
/// 끝나도 다음 테스트에 정책이 새지 않는다.
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

/// 고유 id를 가진 무인자 도구. 정책 레지스트리가 전역이므로 테스트마다
/// 다른 id를 써야 서로를 오염시키지 않는다.
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

/// 한 chunk를 그 run의 것으로 각인한 진행 메시지. 이벤트 루프가
/// `RunningDispatch`에서 만드는 것과 같은 모습이다.
fn tool_progress(run: RunToken, chunk: &str) -> Msg<'static> {
    Msg::ToolProgress {
        run,
        event: progress(chunk),
    }
}

/// `start_run`이 실제로 만드는 모습 그대로: 모델이 이 run을 살아 있다고
/// 붙들고 있고, 오른쪽 pane에 포커스가 간 실행 중 상태. 키 라우팅
/// 테스트가 진짜 경로를 지나가도록 한다.
fn running_state(tool_id: &'static str) -> State {
    let (state, _) = running_state_with_run(tool_id);
    state
}

/// [`running_state`]와 같지만 이 run의 [`RunToken`]도 함께 돌려준다 —
/// 진행 이벤트는 그 token으로 각인되어야 한다.
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

/// 실행 중 pane을 이미 떠난 상태 — 취소 확인 overlay를 열었다가 물러난
/// 뒤의 모습이다. 실행 자체는 계속되므로 모델은 여전히 run을 붙들고 있다.
fn left_running_pane(tool_id: &'static str) -> (State, RunToken) {
    let (mut state, run) = running_state_with_run(tool_id);
    state.view = View::List;
    (state, run)
}

fn tail_lines(state: &State) -> Vec<String> {
    match &state.view {
        View::Running { tail, .. } => tail.lines().map(ToOwned::to_owned).collect(),
        other => panic!("Running 보기를 기대했지만 {other:?}를 받았다"),
    }
}

// ─── Task 1: 승인 게이트 ─────────────────────────────────────────

#[test]
fn 승인이_필요한_도구는_실행_대신_승인_화면을_연다() {
    let tool = no_input_tool("test.approval.tui.opens_prompt");
    let _guard = PolicyGuard::gated(tool.id, vec![Surface::Tui]);
    let tools = [tool];
    let mut s = State::default();

    let effect = handle_key(&mut s, Key::Enter, &tools);

    assert_eq!(
        effect,
        Action::None,
        "승인 전에는 dispatch가 나가면 안 된다"
    );
    match &s.view {
        View::ConfirmApproval { tool_id, args } => {
            assert_eq!(*tool_id, tool.id);
            assert_eq!(*args, json!({}));
        }
        other => panic!("ConfirmApproval을 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 승인_화면에서_확인하면_approve_true를_실은_dispatch가_나간다() {
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
                    "{key:?} 승인은 예약 키 approve=true를 실어야 한다"
                );
            }
            other => panic!("{key:?} 승인에서 Dispatch를 기대했지만 {other:?}를 받았다"),
        }
        assert!(
            matches!(s.view, View::Running { .. }),
            "{key:?} 승인은 실행 중 보기로 넘어가야 한다"
        );
    }
}

#[test]
fn 승인_화면에서_이스케이프는_목록으로_돌아가고_아무것도_실행하지_않는다() {
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

        assert_eq!(effect, Action::None, "{key:?}는 아무것도 실행하지 않는다");
        assert_eq!(s.view, View::List, "{key:?}는 목록으로 돌아간다");
    }
}

#[test]
fn 게이트가_없는_도구는_approve_키_없이_즉시_실행한다() {
    let tool = no_input_tool("test.approval.tui.ungated");
    // 정책을 등록하지 않는다: 장벽이 없는 도구의 기본 상태.
    assert!(!tool_approval_policy(tool.id).requires_approval());
    let tools = [tool];
    let mut s = State::default();

    match handle_key(&mut s, Key::Enter, &tools) {
        Action::Dispatch { tool_id, args, .. } => {
            assert_eq!(tool_id, tool.id);
            assert_eq!(args, json!({}), "장벽이 없으면 approve 키도 없어야 한다");
        }
        other => panic!("Dispatch를 기대했지만 {other:?}를 받았다"),
    }
    assert!(matches!(s.view, View::Running { .. }));
}

#[test]
fn tui를_인정하지_않는_승인_정책은_설명만_보여준다() {
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
            assert!(*is_error, "인정되지 않는 승인은 오류로 표시한다");
            assert!(
                text.contains("cli/http"),
                "승인을 인정하는 표면을 이름으로 밝혀야 하지만 `{text}`를 받았다"
            );
        }
        other => panic!("설명을 담은 Result를 기대했지만 {other:?}를 받았다"),
    }
    assert_eq!(s.focus, FocusArea::RightPane);
}

#[test]
fn 폼_경로도_승인_게이트를_지난다() {
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

    assert_eq!(effect, Action::None, "폼 실행도 승인 전에는 막혀야 한다");
    match &s.view {
        View::ConfirmApproval { tool_id: id, args } => {
            assert_eq!(*id, tool_id);
            assert_eq!(*args, json!({ "input": "0xff" }));
        }
        other => panic!("ConfirmApproval을 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 폼_경로의_승인은_폼이_만든_인자를_그대로_실어_보낸다() {
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
        other => panic!("승인 후 Dispatch를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 승인이_필요한_폼_도구는_상세에서도_폼부터_연다() {
    // 게이트는 dispatch 직전에만 선다: 입력이 필요한 도구는 여전히
    // 폼을 먼저 보여줘야 한다.
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

// ─── Task 2: 라이브 출력 + 취소 ──────────────────────────────────

#[test]
fn 진행_이벤트는_실행_중_tail에_쌓인다() {
    let (mut s, run) = running_state_with_run("test.live.append");

    update(&mut s, tool_progress(run, "first\n"));
    update(&mut s, tool_progress(run, "sec"));
    update(&mut s, tool_progress(run, "ond\n"));

    assert_eq!(tail_lines(&s), ["first", "second"]);
}

#[test]
fn tail은_마지막_n줄만_유지한다() {
    let (mut s, run) = running_state_with_run("test.live.cap");

    for index in 0..(TUI_LIVE_TAIL_MAX_LINES + 5) {
        update(&mut s, tool_progress(run, &format!("line {index}\n")));
    }

    let lines = tail_lines(&s);
    assert_eq!(lines.len(), TUI_LIVE_TAIL_MAX_LINES);
    assert_eq!(lines.first().map(String::as_str), Some("line 5"));
}

#[test]
fn 실행_중이_아닐_때의_진행_이벤트는_무시된다() {
    let (mut s, run) = running_state_with_run("test.live.late");
    // 최종 봉투가 이미 도착해 모델이 run을 놓아준 뒤.
    s.view = View::Result {
        tool_id: "test.live.late",
        outputs: Vec::new(),
        text: "끝난 결과".into(),
        is_error: false,
    };
    s.active_run = None;

    update(&mut s, tool_progress(run, "늦게_도착한_출력\n"));

    match &s.view {
        View::Result { text, .. } => assert_eq!(text, "끝난 결과"),
        other => panic!("늦은 진행 이벤트가 보기를 바꾸면 안 되지만 {other:?}를 받았다"),
    }
}

#[test]
fn 실행_중_이스케이프는_취소를_요청하고_취소중으로_표시한다() {
    let tools = fixture_tools();

    for key in [Key::Esc, Key::Char('q')] {
        let mut s = running_state("test.live.cancel");

        let effect = handle_key(&mut s, key, tools.as_slice());

        assert_eq!(effect, Action::CancelRun, "{key:?}는 실행 취소를 요청한다");
        match &s.view {
            View::Running { cancelling, .. } => {
                assert!(*cancelling, "{key:?} 이후에는 취소 중으로 표시되어야 한다");
            }
            other => panic!("{key:?} 이후에도 Running이어야 하지만 {other:?}를 받았다"),
        }
    }
}

#[test]
fn 취소가_이미_요청된_실행에서_다시_이스케이프하면_종료_확인을_연다() {
    // 취소는 요청일 뿐이라 token을 보지 않는 invoker에서는 영원히
    // 끝나지 않을 수 있다. 다른 키가 모두 삼켜지는 보기에서 탈출구가
    // 하나도 없으면 안 되므로 두 번째 Esc는 격상한다 — 다만 오종료
    // 방지 계약을 지나치지 않고 통과한다: 종료는 확인 대화상자를
    // 거쳐야 한다.
    let tools = fixture_tools();
    let mut s = running_state("test.live.escalate");

    assert_eq!(
        handle_key(&mut s, Key::Esc, tools.as_slice()),
        Action::CancelRun
    );
    assert_eq!(
        handle_key(&mut s, Key::Esc, tools.as_slice()),
        Action::CancelRun,
        "두 번째 Esc도 취소 요청을 다시 보내야 한다"
    );
    assert_eq!(
        s.view,
        View::ConfirmQuit,
        "두 번째 Esc는 세션을 바로 끝내지 않고 종료 확인을 연다"
    );
}

#[test]
fn 실행_중_종료_확인에서_예를_고르면_비로소_종료한다() {
    // 격상 경로의 끝: 확인은 여전히 사람의 의도적인 한 번의 y다.
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let mut s = running_state("test.live.escalate_confirm");

    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Esc, tools);

    assert_eq!(handle_key(&mut s, Key::Char('y'), tools), Action::Quit);
}

#[test]
fn 실행_중_종료_확인을_물리면_목록으로_돌아간다() {
    // 정직성 공백을 고정한다: 확인을 취소해도 실행 중 pane으로
    // 돌아가지는 않는다. 실행 자체는 계속되고 결과는 그대로 도착한다.
    let tools = fixture_tools();
    let tools = tools.as_slice();
    let mut s = running_state("test.live.escalate_back");

    handle_key(&mut s, Key::Esc, tools);
    handle_key(&mut s, Key::Esc, tools);

    assert_eq!(handle_key(&mut s, Key::Char('n'), tools), Action::None);
    assert_eq!(s.view, View::List);
}

#[test]
fn 실행_중_다른_키는_무시된다() {
    let tools = fixture_tools();

    for key in [Key::Char('o'), Key::Char('s'), Key::Char('n'), Key::Enter] {
        let mut s = running_state("test.live.swallow");

        let effect = handle_key(&mut s, key, tools.as_slice());

        assert_eq!(effect, Action::None, "{key:?}는 실행 중에 소비되지 않는다");
        assert!(
            matches!(
                s.view,
                View::Running {
                    cancelling: false,
                    ..
                }
            ),
            "{key:?}는 실행 중 보기를 바꾸면 안 된다"
        );
    }
}

#[test]
fn 실행_중_오른쪽_pane_스크롤은_계속_동작한다() {
    // 라이브 tail을 되짚어 보는 것은 실행을 방해하지 않는다 —
    // Result 보기와 같은 계약.
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
    assert!(s.right_scroll.get() > 0, "실행 중에도 pane은 스크롤된다");
}

#[test]
fn tool_done은_실행_중_보기를_결과로_교체한다() {
    let (mut s, run) = running_state_with_run("test.live.done");
    update(&mut s, tool_progress(run, "작업 중\n"));

    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
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
        other => panic!("Result를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 실행_중_보기는_오른쪽_pane을_지배한다() {
    // 실행 중에도 렌더/마우스/포커스가 같은 body 형태를 보도록,
    // Form/Result와 같은 presentation을 선언해야 한다.
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

// ─── 실행 상태는 View가 아니라 State가 쥔다 ──────────────────────

#[test]
fn 실행_중_pane을_떠나도_모델은_실행을_붙들고_있다() {
    // 취소 확인 overlay를 열었다가 물리면 목록으로 돌아오지만, 워커는
    // 계속 돌고 있다. 모델이 여기서 실행을 잊으면 이벤트 루프와 어긋난다.
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
        "pane을 떠나도 실행은 계속된다"
    );
}

#[test]
fn 실행이_남아_있으면_두_번째_실행은_거절된다() {
    let (mut s, run) = left_running_pane("test.live.first");

    let effect = dispatch_or_confirm(&mut s, "test.simple", json!({}));

    assert_eq!(
        effect,
        Action::None,
        "두 번째 dispatch는 아예 나가지 않는다"
    );
    assert_eq!(
        s.active_run.map(|active| active.run),
        Some(run),
        "먼저 시작한 실행이 그대로 유지된다"
    );
    assert!(
        s.status_message.is_some(),
        "무엇이 실행 중인지 사람에게 말해 준다"
    );
    match &s.view {
        View::Running { tool_id, .. } => assert_eq!(
            *tool_id, "test.live.first",
            "새 도구가 아니라 실행 중인 도구의 pane으로 돌아간다"
        ),
        other => panic!("Running 보기를 기대했지만 {other:?}를 받았다"),
    }
}

#[test]
fn 거절된_두_번째_실행의_pane에는_이전_run의_출력이_섞이지_않는다() {
    // 회귀: 예전에는 두 번째 Run이 새 View::Running을 열고, 이벤트 루프는
    // 그 dispatch를 조용히 버렸다. 그러면 새 pane이 *옛* run의 chunk로
    // 채워지고 옛 run의 결과가 새 도구의 결과인 양 렌더됐다.
    let (mut s, first_run) = left_running_pane("test.live.mixed_first");

    dispatch_or_confirm(&mut s, "test.simple", json!({}));
    update(&mut s, tool_progress(first_run, "먼저 시작한 출력\n"));

    match &s.view {
        View::Running { tool_id, .. } => assert_eq!(*tool_id, "test.live.mixed_first"),
        other => panic!("Running 보기를 기대했지만 {other:?}를 받았다"),
    }
    assert_eq!(tail_lines(&s), ["먼저 시작한 출력"]);
}

#[test]
fn 지난_run의_진행_이벤트는_다음_run의_tail에_들어가지_않는다() {
    let (mut s, first_run) = running_state_with_run("test.live.stale_first");
    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
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

    update(&mut s, tool_progress(first_run, "지난 run의 출력\n"));
    assert!(tail_lines(&s).is_empty(), "지난 run의 chunk는 버려진다");

    update(&mut s, tool_progress(second_run, "지금 run의 출력\n"));
    assert_eq!(tail_lines(&s), ["지금 run의 출력"]);
}

#[test]
fn 최종_봉투가_도착하면_모델은_실행을_놓아준다() {
    let (mut s, _) = running_state_with_run("test.live.release");

    let success = crate::domain::execution::dispatch::text_success("done");
    update(
        &mut s,
        Msg::ToolDone {
            tool_id: "test.live.release",
            outcome: Outcome::Success(success),
        },
    );

    assert!(s.active_run.is_none(), "끝난 실행은 붙들고 있지 않는다");
}

#[test]
fn 실행이_끝난_뒤에는_다시_실행할_수_있다() {
    let (mut s, _) = running_state_with_run("test.live.rerun");
    update(
        &mut s,
        Msg::ToolDone {
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
        "끝난 뒤의 Run은 정상적으로 나간다"
    );
}
