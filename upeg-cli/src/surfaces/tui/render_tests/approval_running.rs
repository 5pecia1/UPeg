//! 승인 대화상자와 실행 중 pane의 렌더 회귀 테스트.
//!
//! 상태 기계 테스트는 "무엇이 일어나는가"를, 여기서는 "사용자가 무엇을
//! 읽는가"를 고정한다. 승인 문구가 사라지거나 라이브 tail이 화면에
//! 닿지 않는 회귀는 CI를 지나가면 안 된다.

use super::*;
use crate::surfaces::tui::model::LiveTail;
use ratatui::backend::TestBackend;

const RENDER_WIDTH: u16 = 140;
const RENDER_HEIGHT: u16 = 24;

fn rendered(state: &State) -> String {
    let backend = TestBackend::new(RENDER_WIDTH, RENDER_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    let tools = list_tools();
    terminal.draw(|f| render(f, state, &tools)).unwrap();
    format!("{:?}", terminal.backend().buffer())
}

fn tail_of(chunks: &[&str]) -> LiveTail {
    let mut tail = LiveTail::default();
    for chunk in chunks {
        tail.push_chunk(chunk);
    }
    tail
}

#[test]
fn 승인_대화상자는_제목과_도구_id와_선택지를_보여준다() {
    let state = State {
        view: View::ConfirmApproval {
            tool_id: "deploy.prod",
            args: serde_json::json!({}),
        },
        ..fresh()
    };

    let buf = rendered(&state);

    assert!(
        buf.contains("Approval required"),
        "승인 대화상자 제목이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("requires approval"),
        "승인이 필요하다는 이유가 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("deploy.prod"),
        "무엇을 승인하는지 도구 id로 밝혀야 한다. got: {buf}"
    );
    assert!(
        buf.contains("approve"),
        "승인 키 안내가 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("cancel"),
        "취소 키 안내가 보여야 한다. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "승인 대화상자는 전체 본문을 차지해야 하며 그리드 제목이 새면 안 된다"
    );
}

#[test]
fn 실행_중_pane은_tail의_line들을_보여준다() {
    let state = State {
        view: View::Running {
            tool_id: "slow.tool",
            tail: tail_of(&["step one\n", "step two\n", "step th"]),
            cancelling: false,
        },
        focus: FocusArea::RightPane,
        ..fresh()
    };

    let buf = rendered(&state);

    assert!(
        buf.contains("Running"),
        "pane 제목이 실행 중임을 밝혀야 한다. got: {buf}"
    );
    assert!(
        buf.contains("esc cancel"),
        "pane 제목은 취소 키를 안내해야 한다. got: {buf}"
    );
    assert!(
        buf.contains("esc esc quit"),
        "pane 제목은 두 번째 esc가 무엇을 하는지도 알려야 한다. got: {buf}"
    );
    assert!(
        buf.contains("slow.tool"),
        "무엇이 실행 중인지 도구 id로 밝혀야 한다. got: {buf}"
    );
    for line in ["step one", "step two", "step th"] {
        assert!(
            buf.contains(line),
            "라이브 tail의 {line:?} 줄이 보여야 한다. got: {buf}"
        );
    }
}

#[test]
fn 출력이_아직_없는_실행_중_pane은_자리표시자를_보여준다() {
    let state = State {
        view: View::Running {
            tool_id: "slow.tool",
            tail: LiveTail::default(),
            cancelling: false,
        },
        focus: FocusArea::RightPane,
        ..fresh()
    };

    let buf = rendered(&state);

    assert!(
        buf.contains("no output yet"),
        "출력 전 자리표시자가 보여야 한다. got: {buf}"
    );
}

#[test]
fn 취소를_요청한_실행_중_pane은_취소_중이라고_말한다() {
    let running = State {
        view: View::Running {
            tool_id: "slow.tool",
            tail: tail_of(&["working\n"]),
            cancelling: false,
        },
        focus: FocusArea::RightPane,
        ..fresh()
    };
    let cancelling = State {
        view: View::Running {
            tool_id: "slow.tool",
            tail: tail_of(&["working\n"]),
            cancelling: true,
        },
        focus: FocusArea::RightPane,
        ..fresh()
    };

    assert!(rendered(&running).contains("running"));
    let buf = rendered(&cancelling);
    assert!(
        buf.contains("cancelling"),
        "취소 요청 뒤에는 취소 중이라고 말해야 한다. got: {buf}"
    );
}
