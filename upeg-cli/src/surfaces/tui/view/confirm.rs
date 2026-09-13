//! Full-body yes/no dialogs.
//!
//! Every confirmation the TUI shows — delete a board, quit the session,
//! approve a gated run — is the same three lines (prompt, blank,
//! options) inside the same accent-bordered block, driven by the one
//! shared `KeyboardScope::ConfirmDelete` resolver. Rendering them
//! through one [`ConfirmDialog`] keeps that sameness a fact instead of
//! a coincidence, so a fourth dialog cannot quietly look different.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::i18n::{t, t_args};

use super::super::model::State;

/// One rendered confirmation: what it is about, what it asks, and which
/// keys answer it.
struct ConfirmDialog<'a> {
    header: &'a str,
    prompt: String,
    options: &'a str,
    accent: Color,
}

fn render_confirm_dialog(frame: &mut Frame, area: Rect, dialog: ConfirmDialog<'_>) {
    let ConfirmDialog {
        header,
        prompt,
        options,
        accent,
    } = dialog;
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(prompt, Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled(
            options.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
    ];
    let panel = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(Style::default().fg(accent)),
    );
    frame.render_widget(panel, area);
}

/// Full-body confirmation prompt for board delete.
pub(super) fn render_confirm_delete_board(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    title: &str,
    accent: Color,
) {
    let locale = state.tweaks.locale;
    render_confirm_dialog(
        frame,
        area,
        ConfirmDialog {
            header: t(locale, "tui.board.delete.confirm.title"),
            prompt: t_args(
                locale,
                "tui.board.delete.confirm.prompt",
                &[("title", title)],
            ),
            options: t(locale, "tui.board.delete.confirm.options"),
            accent,
        },
    );
}

/// Full-body confirmation prompt for the modeless quit guard.
pub(super) fn render_confirm_quit(frame: &mut Frame, area: Rect, state: &State, accent: Color) {
    let locale = state.tweaks.locale;
    render_confirm_dialog(
        frame,
        area,
        ConfirmDialog {
            header: t(locale, "tui.quit.confirm.title"),
            prompt: t(locale, "tui.quit.confirm.prompt").to_string(),
            options: t(locale, "tui.quit.confirm.options"),
            accent,
        },
    );
}

/// Full-body approval barrier for a gated Tool. The tool id is part of
/// the prompt: an approval the user cannot attribute to a specific run
/// is not informed consent.
pub(super) fn render_confirm_approval(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    tool_id: &str,
    accent: Color,
) {
    let locale = state.tweaks.locale;
    render_confirm_dialog(
        frame,
        area,
        ConfirmDialog {
            header: t(locale, "tui.approval.confirm.title"),
            prompt: t_args(
                locale,
                "tui.approval.confirm.prompt",
                &[("tool_id", tool_id)],
            ),
            options: t(locale, "tui.approval.confirm.options"),
            accent,
        },
    );
}
