//! Render regression tests for the approval dialog and the running
//! pane.
//!
//! The state-machine tests pin "what happens"; here we pin "what the
//! user reads". Regressions where the approval wording disappears or
//! the live tail never reaches the screen must not pass CI.

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
fn approval_dialog_shows_title_tool_id_and_choices() {
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
        "the approval dialog title must be visible. got: {buf}"
    );
    assert!(
        buf.contains("requires approval"),
        "the reason approval is needed must be visible. got: {buf}"
    );
    assert!(
        buf.contains("deploy.prod"),
        "must say which tool is being approved by id. got: {buf}"
    );
    assert!(
        buf.contains("approve"),
        "the approve key hint must be visible. got: {buf}"
    );
    assert!(
        buf.contains("cancel"),
        "the cancel key hint must be visible. got: {buf}"
    );
    assert!(
        !buf.contains("Pegboard grid"),
        "the approval dialog must occupy the whole body; the grid title must not leak"
    );
}

#[test]
fn running_pane_shows_tail_lines() {
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
        "the pane title must say it is running. got: {buf}"
    );
    assert!(
        buf.contains("esc cancel"),
        "the pane title must advertise the cancel key. got: {buf}"
    );
    assert!(
        buf.contains("esc esc quit"),
        "the pane title must also say what a second esc does. got: {buf}"
    );
    assert!(
        buf.contains("slow.tool"),
        "must say which tool is running by id. got: {buf}"
    );
    for line in ["step one", "step two", "step th"] {
        assert!(
            buf.contains(line),
            "the live tail line {line:?} must be visible. got: {buf}"
        );
    }
}

#[test]
fn running_pane_without_output_shows_placeholder() {
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
        "the pre-output placeholder must be visible. got: {buf}"
    );
}

#[test]
fn running_pane_with_cancel_requested_says_cancelling() {
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
        "must say cancelling after a cancel request. got: {buf}"
    );
}
