use std::collections::BTreeSet;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use upeg_core::ToolMeta;

use crate::i18n::t;

use super::super::model::{State, ToolPickerMode};

pub(super) struct ToolPickerRender<'a> {
    pub(super) state: &'a State,
    pub(super) tools: &'a [&'static ToolMeta],
    pub(super) mode: ToolPickerMode,
    pub(super) query: &'a str,
    pub(super) cursor: usize,
    pub(super) accent: Color,
}

/// Full-body tool-picker: query input on top, filtered toolbox list below.
/// Each row has a `★ ` prefix when already pinned to the active board so the
/// user reads pin-state at a glance.
pub(super) fn render_tool_picker(frame: &mut Frame, area: Rect, render: ToolPickerRender<'_>) {
    let locale = render.state.tweaks.locale;
    let title = match render.mode {
        ToolPickerMode::Pin => t(locale, "tui.toolpicker.title"),
        ToolPickerMode::Search => t(locale, "tui.toolsearch.title"),
    };
    let query_label = t(locale, "tui.toolpicker.query");
    let pinned_marker = t(locale, "tui.toolpicker.pinned_marker");
    let results = super::super::update::tool_picker_results(
        render.state,
        render.mode,
        render.query,
        render.tools,
    );
    let pinned_ids: BTreeSet<&str> = render
        .state
        .filters
        .board
        .as_deref()
        .and_then(|b| render.state.layouts.get(b))
        .map(|placements| placements.iter().map(|p| p.tool_id.as_str()).collect())
        .unwrap_or_default();
    let mut body: Vec<Line> = Vec::with_capacity(results.len() + 3);
    body.push(Line::from(""));
    body.push(Line::from(vec![
        Span::styled(query_label, Style::default().fg(Color::Gray)),
        Span::styled(
            render.query.to_string(),
            Style::default()
                .fg(render.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(render.accent)),
    ]));
    body.push(Line::from(""));
    if results.is_empty() {
        body.push(Line::from(Span::styled(
            t(locale, "tui.toolpicker.empty"),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (idx, tool) in results.iter().enumerate() {
            let is_pinned = pinned_ids.contains(tool.id);
            let is_focused = idx == render.cursor;
            let marker = if is_pinned { pinned_marker } else { "  " };
            let row_style = if is_focused {
                Style::default()
                    .fg(render.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let cursor_chevron = if is_focused { "▶ " } else { "  " };
            body.push(Line::from(vec![
                Span::styled(cursor_chevron.to_string(), row_style),
                Span::styled(marker.to_string(), row_style),
                Span::styled(tool.id.to_string(), row_style),
                Span::raw("  "),
                Span::styled(
                    tool.display_label.to_string(),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }
    }
    let panel = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(render.accent)),
    );
    frame.render_widget(panel, area);
}
