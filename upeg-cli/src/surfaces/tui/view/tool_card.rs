use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use upeg_core::prefs::Locale;
use upeg_core::{PinColorHex, Surface, ToolMeta};
use upeg_runtime::ToolMetaRuntimeExt;

use crate::i18n::t;

use super::super::style::pin_color_or_fallback;

pub(super) fn render_tool_card(
    frame: &mut Frame,
    area: Rect,
    tool: &'static ToolMeta,
    selected: bool,
    locale: Locale,
    accent: Color,
    pin_color: Option<&PinColorHex>,
) {
    let effective_pin_color = pin_color_or_fallback(pin_color, accent);
    let border_color = if pin_color.is_some() {
        effective_pin_color
    } else if selected {
        accent
    } else {
        Color::DarkGray
    };
    let border_style = Style::default().fg(border_color);
    let header_style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let marker = if selected { "▶ " } else { "  " };
    let inner_width = area.width.saturating_sub(2);
    let label_width = inner_width.saturating_sub(marker.len() as u16);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(marker.to_string(), header_style),
            Span::styled(clip(tool.display_label, label_width), header_style),
        ]),
        Line::from(Span::styled(
            clip(tool.id, inner_width),
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            clip(tool.description, inner_width),
            Style::default().fg(Color::White),
        )),
    ];
    if area.height > 5 {
        lines.push(Line::from(Span::styled(
            clip(&tool.tag_labels().join(" "), inner_width),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(vec![
        Span::styled(
            clip(tool.pin.label(), inner_width / 2),
            Style::default().fg(effective_pin_color),
        ),
        Span::raw(" "),
        Span::styled(
            clip(
                if tool.is_on_surface(Surface::Tui) {
                    tool.pegboard_units.label()
                } else {
                    t(locale, "tui.detail.not_on_surface")
                },
                inner_width / 2,
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let card = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    frame.render_widget(card, area);
}

fn clip(value: &str, width: u16) -> String {
    value.chars().take(width as usize).collect()
}
