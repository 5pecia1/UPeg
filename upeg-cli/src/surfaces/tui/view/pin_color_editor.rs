use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::i18n::t;

use super::super::model::{PIN_COLOR_PALETTE, PinColorEditor, State, pin_color_palette_for_digit};

pub(super) fn render_pin_color_editor(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    editor: &PinColorEditor,
    accent: Color,
) {
    let locale = state.tweaks.locale;
    let mut body = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                t(locale, "tui.pin_color.editor.tool"),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(editor.tool_id.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(
                t(locale, "tui.pin_color.editor.current"),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                editor.draft_label(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "█",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            t(locale, "tui.pin_color.editor.palette"),
            Style::default().fg(Color::Gray),
        )),
    ];
    for (index, color) in PIN_COLOR_PALETTE.iter().enumerate() {
        let digit = (index + 1).to_string();
        let digit_char = digit.chars().next().unwrap_or_default();
        let is_selected = pin_color_palette_for_digit(digit_char).is_some_and(|palette| {
            editor.color_to_apply().ok().flatten().as_ref() == Some(&palette)
        });
        let style = if is_selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        body.push(Line::from(vec![
            Span::styled(format!("  {digit}. "), style),
            Span::styled((*color).to_string(), style),
        ]));
    }
    body.push(Line::from(""));
    body.push(Line::from(Span::styled(
        t(locale, "tui.pin_color.editor.hint"),
        Style::default().fg(Color::DarkGray),
    )));
    if let Some(error) = &editor.error {
        body.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }
    let panel = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(t(locale, "tui.pin_color.editor.title"))
            .border_style(Style::default().fg(accent)),
    );
    frame.render_widget(panel, area);
}
