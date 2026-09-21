use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use upeg_core::prefs::Locale;
use upeg_core::ux::result_status_label;
use upeg_core::{DraftInputValue, InputFieldSpec, InputKind, OutputEntry, PinColorHex, ToolMeta};
use upeg_runtime::{ToolMetaRuntimeExt, output_value_text};

use crate::i18n::{t, t_args};

use super::grid::{
    BOARD_FILTER_PREFIX, BodyLayout, TAG_FILTER_PREFIX, TuiLayout, filter_bar_content_width,
    filter_bar_max_scroll, filter_bar_scrollbar_area, filter_bar_viewport_width, grid_content_area,
    grid_h_scrollbar_area, grid_max_h_scroll, grid_max_scroll, grid_scrollbar_area,
    grid_visible_rect, pegboard_cells, rect_contains, tui_layout,
};
use super::model::{BodyDominant, FocusArea, State, ToolPickerMode, TuiFormState, View};
use super::scroll::{Horizontal, ScrollOffset};
use super::style::accent_color;

mod confirm;
mod field_hint;
mod pin_color_editor;
mod presentation;
mod running;
mod tool_card;
mod tool_picker;

use confirm::{render_confirm_approval, render_confirm_delete_board, render_confirm_quit};
use field_hint::constraint_hint;
use pin_color_editor::render_pin_color_editor;
use presentation::append_presentation;
use running::running_body;
use tool_card::render_tool_card;
use tool_picker::{ToolPickerRender, render_tool_picker};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderContext {
    pub(crate) footer_status: String,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            footer_status: "MCP off · HTTP off".to_string(),
        }
    }
}

/// Render one frame given current state and tool list.
#[cfg(test)]
pub fn render(frame: &mut Frame, state: &State, tools: &[&'static ToolMeta]) {
    render_with_context(frame, state, tools, &RenderContext::default());
}

#[cfg(test)]
pub fn render_with_filters(
    frame: &mut Frame,
    state: &State,
    tools: &[&'static ToolMeta],
    filters: &super::model::TuiFilters,
) {
    let mut state = state.clone();
    state.filters = filters.clone();
    state.sync_filter_cursors();
    render_with_context(frame, &state, tools, &RenderContext::default());
}

#[cfg(test)]
pub(crate) fn render_with_context(
    frame: &mut Frame,
    state: &State,
    tools: &[&'static ToolMeta],
    context: &RenderContext,
) {
    let pin_colors = vec![None; tools.len()];
    render_with_context_and_pin_colors(frame, state, tools, &pin_colors, context);
}

#[cfg(test)]
pub fn render_with_pin_colors(
    frame: &mut Frame,
    state: &State,
    tools: &[&'static ToolMeta],
    pin_colors: &[Option<PinColorHex>],
) {
    render_with_context_and_pin_colors(frame, state, tools, pin_colors, &RenderContext::default());
}

pub(crate) fn render_with_context_and_pin_colors(
    frame: &mut Frame,
    state: &State,
    tools: &[&'static ToolMeta],
    pin_colors: &[Option<PinColorHex>],
    context: &RenderContext,
) {
    let layout = tui_layout(frame.area());
    let accent = accent_color(state.tweaks.theme, state.tweaks.accent);

    render_header(frame, layout.header, state, accent);
    render_board_bar(frame, layout.boards, state, accent);
    render_tag_bar(frame, layout.tags, state, accent);
    let body = layout.body();
    match &state.view {
        View::Settings { focused_field } => {
            render_settings_panel(frame, body, state, *focused_field, accent);
        }
        View::BoardEditor { mode, buffer } => {
            render_board_editor(frame, body, state, mode, buffer, accent);
        }
        View::ConfirmDeleteBoard { title, .. } => {
            render_confirm_delete_board(frame, body, state, title, accent);
        }
        View::ConfirmQuit => {
            render_confirm_quit(frame, body, state, accent);
        }
        View::ConfirmApproval { tool_id, .. } => {
            render_confirm_approval(frame, body, state, tool_id, accent);
        }
        View::PinColorEditor(editor) => {
            render_pin_color_editor(frame, body, state, editor, accent);
        }
        View::ToolPicker {
            mode,
            query,
            cursor,
            ..
        } => {
            render_tool_picker(
                frame,
                body,
                ToolPickerRender {
                    state,
                    tools,
                    mode: *mode,
                    query,
                    cursor: *cursor,
                    accent,
                },
            );
        }
        View::List | View::Detail => {
            render_body_surfaces_with_pin_colors(
                frame,
                &layout,
                BodyDominant::Board,
                state,
                tools,
                accent,
                pin_colors,
            );
        }
        View::Form { .. } | View::Result { .. } | View::Running { .. } => {
            render_body_surfaces_with_pin_colors(
                frame,
                &layout,
                BodyDominant::RightPane,
                state,
                tools,
                accent,
                pin_colors,
            );
        }
    }
    render_footer(
        frame,
        layout.footer,
        &context.footer_status,
        state.status_message.as_deref(),
    );
}

/// Render the pegboard grid and/or right pane according to the body
/// layout. Each `BodyLayout` arm matches what mouse routing and focus
/// reconciliation see, so a click never lands on a surface that
/// wasn't drawn.
fn render_body_surfaces_with_pin_colors(
    frame: &mut Frame,
    layout: &TuiLayout,
    dom: BodyDominant,
    state: &State,
    tools: &[&'static ToolMeta],
    accent: Color,
    pin_colors: &[Option<PinColorHex>],
) {
    match layout.body_layout(dom) {
        BodyLayout::Dual { grid, right } => {
            render_pegboard_grid_with_pin_colors(frame, grid, state, tools, accent, pin_colors);
            render_right_pane(frame, right, state, tools, accent);
        }
        BodyLayout::BoardOnly(rect) => {
            render_pegboard_grid_with_pin_colors(frame, rect, state, tools, accent, pin_colors);
        }
        BodyLayout::RightPaneOnly(rect) => {
            render_right_pane(frame, rect, state, tools, accent);
        }
    }
}

/// Full-body Settings panel: a bordered block titled with the catalog
/// `tui.settings.title`, accent-coloured focus border, and one styled
/// line per field. Focused field gets the accent foreground; the
/// current value sits to the right of the field label after a `:`.
fn render_settings_panel(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    focused: super::model::SettingsField,
    accent: Color,
) {
    use super::model::SettingsField;
    let locale = state.tweaks.locale;
    let title = t(locale, "tui.settings.title");
    let value_locale = match state.tweaks.locale {
        upeg_core::prefs::Locale::En => t(locale, "tui.settings.locale.en"),
        upeg_core::prefs::Locale::Ko => t(locale, "tui.settings.locale.ko"),
    };
    let value_theme = match state.tweaks.theme {
        upeg_core::prefs::Theme::Dark => t(locale, "tui.settings.theme.dark"),
        upeg_core::prefs::Theme::Light => t(locale, "tui.settings.theme.light"),
    };
    let value_accent = match state.tweaks.accent {
        upeg_core::prefs::Accent::Green => t(locale, "tui.settings.accent.green"),
        upeg_core::prefs::Accent::Amber => t(locale, "tui.settings.accent.amber"),
        upeg_core::prefs::Accent::Cyan => t(locale, "tui.settings.accent.cyan"),
        upeg_core::prefs::Accent::Pink => t(locale, "tui.settings.accent.pink"),
    };
    let row = |field: SettingsField, label: &str, value: &str| -> Line<'_> {
        let is_focused = field == focused;
        let marker = if is_focused { "▶ " } else { "  " };
        let label_style = if is_focused {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let value_style = if is_focused {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(vec![
            Span::styled(marker.to_string(), label_style),
            Span::styled(format!("{label:<10}"), label_style),
            Span::raw("  "),
            Span::styled(value.to_string(), value_style),
        ])
    };
    let lines = vec![
        Line::from(""),
        row(
            SettingsField::Locale,
            t(locale, "tui.settings.field.locale"),
            value_locale,
        ),
        row(
            SettingsField::Theme,
            t(locale, "tui.settings.field.theme"),
            value_theme,
        ),
        row(
            SettingsField::Accent,
            t(locale, "tui.settings.field.accent"),
            value_accent,
        ),
        Line::from(""),
        Line::from(Span::styled(
            t(locale, "tui.settings.hint"),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let panel = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(accent)),
    );
    frame.render_widget(panel, area);
}

/// Full-body single-line editor for new / rename board. Renders the
/// current `buffer` after a mode-specific prompt; an empty buffer
/// shows a dim hint instead of stranding the user.
fn render_board_editor(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    mode: &super::model::BoardEditMode,
    buffer: &str,
    accent: Color,
) {
    use super::model::BoardEditMode;
    let locale = state.tweaks.locale;
    let title = t(locale, "tui.board.editor.title");
    let prompt = match mode {
        BoardEditMode::AddBoard => t(locale, "tui.board.editor.add_prompt").to_string(),
        BoardEditMode::Rename { original_title, .. } => crate::i18n::t_args(
            locale,
            "tui.board.editor.rename_prompt",
            &[("title", original_title.as_str())],
        ),
    };
    let body = if buffer.is_empty() {
        vec![
            Line::from(""),
            Line::from(Span::styled(prompt, Style::default().fg(Color::Gray))),
            Line::from(Span::styled(
                t(locale, "tui.board.editor.empty_hint"),
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(prompt, Style::default().fg(Color::Gray)),
                Span::styled(
                    buffer.to_string(),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "█",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
            ]),
        ]
    };
    let panel = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(accent)),
    );
    frame.render_widget(panel, area);
}

fn render_header(frame: &mut Frame, area: Rect, state: &State, accent: Color) {
    let locale = state.tweaks.locale;
    let mut spans = vec![
        Span::styled(
            t(locale, "tui.header.title"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];
    // Modeless: one hint set. Board-management (n / R / D / a) and
    // per-pin (p / c / m / [ ]) keys are always live, so the hint bar
    // advertises them alongside navigation. All routed through i18n so
    // the Ko catalog can rephrase them.
    spans.extend([
        Span::raw(t(locale, "tui.header.hint.navigate")),
        Span::raw(t(locale, "tui.header.hint.open")),
        Span::raw(t(locale, "tui.header.hint.run")),
        Span::raw(t(locale, "tui.header.hint.filters")),
        Span::raw(t(locale, "tui.header.hint.settings")),
        Span::raw(t(locale, "tui.header.edit.hint.new")),
        Span::raw(t(locale, "tui.header.edit.hint.rename")),
        Span::raw(t(locale, "tui.header.edit.hint.delete")),
        Span::raw(t(locale, "tui.header.edit.hint.add")),
        Span::raw(t(locale, "tui.header.edit.hint.pin")),
        Span::raw(t(locale, "tui.header.edit.hint.color")),
        Span::raw(t(locale, "tui.header.edit.hint.move")),
        Span::raw(t(locale, "tui.header.hint.quit")),
    ]);
    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(t(locale, "tui.header.frame_title")),
    );
    frame.render_widget(header, area);
}

fn render_board_bar(frame: &mut Frame, area: Rect, state: &State, accent: Color) {
    let options = state.board_filter_options();
    render_filter_bar(
        frame,
        FilterBarView {
            area,
            title: t(state.tweaks.locale, "tui.boards.title"),
            prefix: BOARD_FILTER_PREFIX,
            options: &options,
            active: state.filters.board.as_deref(),
            scroll: state.board_scroll,
            focused_index: state.board_cursor,
            has_focus: state.focus == FocusArea::Boards,
            accent,
        },
    );
}

fn render_tag_bar(frame: &mut Frame, area: Rect, state: &State, accent: Color) {
    let options = state.tag_filter_options();
    render_filter_bar(
        frame,
        FilterBarView {
            area,
            title: t(state.tweaks.locale, "tui.tags.title"),
            prefix: TAG_FILTER_PREFIX,
            options: &options,
            active: state.filters.tag.as_deref(),
            scroll: state.tag_scroll,
            focused_index: state.tag_cursor,
            has_focus: state.focus == FocusArea::Tags,
            accent,
        },
    );
}

struct FilterBarView<'a> {
    area: Rect,
    title: &'static str,
    prefix: &'static str,
    options: &'a [String],
    active: Option<&'a str>,
    scroll: ScrollOffset<Horizontal>,
    focused_index: usize,
    has_focus: bool,
    accent: Color,
}

fn render_filter_bar(frame: &mut Frame, view: FilterBarView<'_>) {
    let FilterBarView {
        area,
        title,
        prefix,
        options,
        active,
        scroll,
        focused_index,
        has_focus,
        accent,
    } = view;
    let mut spans = vec![Span::styled(
        prefix,
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )];
    for (index, option) in options.iter().enumerate() {
        push_filter_span(
            &mut spans,
            option,
            active,
            has_focus && index == focused_index,
            accent,
        );
    }
    let scroll = scroll
        .clamp_to(filter_bar_max_scroll(area, prefix, options))
        .get();
    let bar = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(focus_border_style(has_focus, accent)),
        )
        .scroll((0, scroll));
    frame.render_widget(bar, area);

    if let Some(scrollbar_area) = filter_bar_scrollbar_area(area)
        && filter_bar_max_scroll(area, prefix, options) > 0
    {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#")
            .thumb_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .track_style(Style::default().fg(Color::DarkGray))
            .begin_style(Style::default().fg(Color::DarkGray))
            .end_style(Style::default().fg(Color::DarkGray));
        let mut scrollbar_state =
            ScrollbarState::new(filter_bar_content_width(prefix, options) as usize)
                .position(scroll as usize)
                .viewport_content_length(filter_bar_viewport_width(area, prefix, options) as usize);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn push_filter_span(
    spans: &mut Vec<Span<'static>>,
    label: &str,
    active: Option<&str>,
    focused: bool,
    accent: Color,
) {
    let is_all_active = label == "all" && active.is_none();
    let is_active = is_all_active || active == Some(label);
    if is_active && focused {
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
    } else if is_active {
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ));
    } else if focused {
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
    } else {
        spans.push(Span::raw(format!(" {label} ")));
    }
}

fn focus_border_style(has_focus: bool, accent: Color) -> Style {
    if has_focus {
        Style::default().fg(accent)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_pegboard_grid_with_pin_colors(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    tools: &[&'static ToolMeta],
    accent: Color,
    pin_colors: &[Option<PinColorHex>],
) {
    let count_str = tools.len().to_string();
    let title = if let Some(move_mode) = &state.move_mode {
        t_args(
            state.tweaks.locale,
            "tui.grid.moving_title",
            &[("tool", move_mode.tool_id), ("count", count_str.as_str())],
        )
    } else if let Some(resize_mode) = &state.resize_mode {
        let cols_str = resize_mode.cols.to_string();
        let rows_str = resize_mode.rows.to_string();
        t_args(
            state.tweaks.locale,
            "tui.grid.resize_title",
            &[
                ("tool", resize_mode.tool_id),
                ("cols", cols_str.as_str()),
                ("rows", rows_str.as_str()),
                ("count", count_str.as_str()),
            ],
        )
    } else {
        t_args(
            state.tweaks.locale,
            "tui.grid.title",
            &[("count", count_str.as_str())],
        )
    };
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
            .border_style(focus_border_style(state.focus == FocusArea::Grid, accent)),
        area,
    );

    let content = grid_content_area(area);
    if tools.is_empty() {
        frame.render_widget(
            Paragraph::new(t(state.tweaks.locale, "tui.grid.empty")),
            content,
        );
        return;
    }

    for cell in pegboard_cells(tools, area) {
        let Some(tool) = tools.get(cell.index) else {
            continue;
        };
        let Some(rect) = grid_visible_rect(cell, area, state.grid_scroll, state.grid_h_scroll)
        else {
            continue;
        };
        if !rect_contains(content, rect.x, rect.y) || rect.height < 3 {
            continue;
        }
        let pin_color = pin_colors.get(cell.index).and_then(Option::as_ref);
        render_tool_card(
            frame,
            rect,
            tool,
            cell.index == state.cursor,
            state.tweaks.locale,
            accent,
            pin_color,
        );
    }

    render_grid_scrollbar(frame, area, state, tools, accent);
}

fn render_grid_scrollbar(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    tools: &[&'static ToolMeta],
    accent: Color,
) {
    let max_v = grid_max_scroll(tools, area);
    if max_v > 0
        && let Some(bar_area) = grid_scrollbar_area(area)
    {
        let content_height = grid_content_area(area).height.max(1) as usize;
        let scroll = state.grid_scroll.clamp_to(max_v).get();
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"))
            .track_symbol(Some("|"))
            .thumb_symbol("#")
            .thumb_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .track_style(Style::default().fg(Color::DarkGray))
            .begin_style(Style::default().fg(Color::DarkGray))
            .end_style(Style::default().fg(Color::DarkGray));
        let mut scrollbar_state = ScrollbarState::new((max_v as usize).saturating_add(1))
            .position(scroll as usize)
            .viewport_content_length(content_height);
        frame.render_stateful_widget(scrollbar, bar_area, &mut scrollbar_state);
    }

    let max_h = grid_max_h_scroll(area);
    if max_h > 0
        && let Some(bar_area) = grid_h_scrollbar_area(area)
    {
        let content_width = grid_content_area(area).width.max(1) as usize;
        let scroll = state.grid_h_scroll.clamp_to(max_h).get();
        let scrollbar = Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#")
            .thumb_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .track_style(Style::default().fg(Color::DarkGray))
            .begin_style(Style::default().fg(Color::DarkGray))
            .end_style(Style::default().fg(Color::DarkGray));
        let mut scrollbar_state = ScrollbarState::new((max_h as usize).saturating_add(1))
            .position(scroll as usize)
            .viewport_content_length(content_width);
        frame.render_stateful_widget(scrollbar, bar_area, &mut scrollbar_state);
    }
}

fn render_right_pane(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    tools: &[&'static ToolMeta],
    accent: Color,
) {
    let (title, body) = right_pane_content(state, tools);
    let scroll = state
        .right_scroll
        .clamp_to(right_pane_max_scroll(&body, area))
        .get();
    let detail = Paragraph::new(body).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(focus_border_style(
                state.focus == FocusArea::RightPane,
                accent,
            )),
    );
    frame.render_widget(detail.scroll((scroll, 0)), area);
}

fn right_pane_max_scroll(body: &str, area: Rect) -> u16 {
    let width = area.width.saturating_sub(2).max(1) as usize;
    let height = area.height.saturating_sub(2) as usize;
    let lines = body
        .lines()
        .map(|line| line.chars().count().max(1).div_ceil(width))
        .sum::<usize>();
    lines.saturating_sub(height).min(u16::MAX as usize) as u16
}

fn right_pane_content(state: &State, tools: &[&'static ToolMeta]) -> (&'static str, String) {
    let locale = state.tweaks.locale;
    match &state.view {
        View::List => (
            t(locale, "tui.right_pane.list"),
            selected_tool_manifest(tools, state.cursor, locale)
                .unwrap_or_else(|| t(locale, "tui.empty.list").to_string()),
        ),
        View::Detail => (
            t(locale, "tui.right_pane.detail"),
            selected_tool_manifest(tools, state.cursor, locale)
                .unwrap_or_else(|| t(locale, "tui.empty.detail").to_string()),
        ),
        View::Form { tool_id, form } => (
            t(locale, "tui.right_pane.form"),
            form_body(tool_id, form, locale),
        ),
        View::Result {
            tool_id,
            outputs,
            text,
            is_error,
        } => (
            t(locale, "tui.right_pane.result"),
            result_body(
                tool_id,
                outputs,
                text,
                *is_error,
                state.result_row,
                state.result_action,
            ),
        ),
        View::Running {
            tool_id,
            tail,
            cancelling,
        } => (
            t(locale, "tui.right_pane.running"),
            running_body(tool_id, tail, *cancelling, locale),
        ),
        // Full-body overlays (Settings, BoardEditor, ConfirmDelete,
        // ToolPicker) take over the entire content area via dedicated
        // render passes; render_with_context branches before calling
        // this function so these arms are dead at runtime. Empty
        // fallback (vs `unreachable!`) keeps direct tests safe; the
        // exhaustive match ensures a new View variant has to opt in.
        View::Settings { .. } => (t(locale, "tui.settings.title"), String::new()),
        View::BoardEditor { .. } => (t(locale, "tui.board.editor.title"), String::new()),
        View::ConfirmDeleteBoard { .. } => {
            (t(locale, "tui.board.delete.confirm.title"), String::new())
        }
        View::ConfirmQuit => (t(locale, "tui.quit.confirm.title"), String::new()),
        View::ConfirmApproval { .. } => (t(locale, "tui.approval.confirm.title"), String::new()),
        View::PinColorEditor(_) => (t(locale, "tui.pin_color.editor.title"), String::new()),
        View::ToolPicker { mode, .. } => {
            let title = match mode {
                ToolPickerMode::Pin => t(locale, "tui.toolpicker.title"),
                ToolPickerMode::Search => t(locale, "tui.toolsearch.title"),
            };
            (title, String::new())
        }
    }
}

fn result_body(
    tool_id: &'static str,
    outputs: &[OutputEntry],
    text: &str,
    is_error: bool,
    selected_row: usize,
    selected_action: usize,
) -> String {
    let mut body = format!("{tool_id} → {}", result_status_label(is_error));
    let detail = if !is_error && !outputs.is_empty() {
        output_rows(outputs)
    } else {
        text.to_string()
    };
    let rendered_presentation = append_presentation(
        &mut body,
        tool_id,
        outputs,
        is_error,
        selected_row,
        selected_action,
    );
    // Keep the canonical output available, but put the actionable table first
    // so a long JSON value cannot push the presentation below the viewport.
    if !detail.is_empty() {
        body.push_str(if rendered_presentation {
            "\n\nRaw output:\n"
        } else {
            "\n\n"
        });
        body.push_str(&detail);
    }
    body
}

fn output_rows(outputs: &[OutputEntry]) -> String {
    outputs
        .iter()
        .map(|entry| {
            let label = entry.label.as_deref().unwrap_or(entry.id.as_str());
            format!("{label}\t{}", output_value_text(&entry.value))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn selected_tool_manifest(
    tools: &[&'static ToolMeta],
    cursor: usize,
    locale: Locale,
) -> Option<String> {
    tools.get(cursor).map(|tool| {
        format!(
            "{}\n{}\n\n{}\n\n{}",
            tool.display_label,
            tool.id,
            manifest_text(tool, locale),
            t(locale, "tui.manifest.run_hint"),
        )
    })
}

fn manifest_text(tool: &'static ToolMeta, locale: Locale) -> String {
    let mut s = format!(
        "id            {}\ntoolkit       {}\ntags          {}\npin   {}\ninvoker       {}\nsurfaces      {}\nboards {}\n\n{}",
        tool.id,
        tool.toolkit,
        tool.tag_labels().join(", "),
        tool.pin.label(),
        tool.invoker.label(),
        tool.surfaces_label(),
        if tool.boards.is_empty() {
            t(locale, "tui.manifest.no_boards").to_string()
        } else {
            tool.boards.join(", ")
        },
        if tool.description.is_empty() {
            t(locale, "tui.manifest.no_description")
        } else {
            tool.description
        },
    );
    if let Some(url) = upeg_runtime::embed_url_for(tool.id) {
        s.push_str(&t_args(locale, "tui.manifest.embed_url", &[("url", url)]));
    }
    let fields = &tool.input_spec.fields;
    if fields.is_empty() {
        s.push_str(t(locale, "tui.manifest.inputs_none"));
    } else {
        let count_str = fields.len().to_string();
        s.push_str(&t_args(
            locale,
            "tui.manifest.inputs_header",
            &[("count", count_str.as_str())],
        ));
        for f in fields {
            let req = if f.required { ", required" } else { "" };
            let desc = if f.description.as_deref().unwrap_or_default().is_empty() {
                String::new()
            } else {
                format!(" - {}", f.description.as_deref().unwrap_or_default())
            };
            s.push_str(&format!(
                "\n  {:<12} ({}{}){}",
                f.name,
                input_kind_summary(&f.kind),
                req,
                desc
            ));
        }
    }
    let bindings = upeg_runtime::selector_bindings_for(tool.id);
    if !bindings.is_empty() {
        let count_str = bindings.len().to_string();
        s.push_str(&t_args(
            locale,
            "tui.manifest.bindings_header",
            &[("count", count_str.as_str())],
        ));
        for b in &bindings {
            s.push_str(&format!("\n  {:<12} -> {}", b.field, b.selector));
        }
    }
    s
}

fn form_body(tool_id: &'static str, form: &TuiFormState, locale: Locale) -> String {
    let mut body = t_args(locale, "tui.run.header", &[("tool_id", tool_id)]);
    for (i, field) in form.fields.iter().enumerate() {
        let Some(spec) = form.spec_for_field(field) else {
            continue;
        };
        let marker = if i == form.focused { "▶ " } else { "  " };
        let req = if spec.required { " *" } else { "" };
        let cursor_indicator = if i == form.focused { "▌" } else { "" };
        body.push_str(&format!(
            "{marker}{name}{req} ({ty}): {value}{cur}\n",
            name = field_label(spec),
            ty = input_kind_summary(&spec.kind),
            value = draft_display(&field.draft),
            cur = cursor_indicator,
        ));
        for line in [
            spec.description
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            constraint_hint(spec),
        ]
        .into_iter()
        .flatten()
        {
            body.push_str(&format!("    {line}\n"));
        }
    }
    body.push_str(
        "\n[↵/F1] run\n[Tab] next · [←/→] cycle bool/options · [Ctrl+U] clear\n[Esc] back",
    );
    body
}

fn field_label(field: &InputFieldSpec) -> &str {
    field
        .label
        .as_deref()
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| field.name.as_str())
}

fn input_kind_summary(kind: &InputKind) -> String {
    match kind {
        InputKind::String => "string".to_string(),
        InputKind::Number => "number".to_string(),
        InputKind::Integer => "integer".to_string(),
        InputKind::Boolean => "boolean".to_string(),
        InputKind::Options(choices) => format!("options [{}]", choice_values(choices)),
        InputKind::MultiOptions(choices) => format!("multi_options [{}]", choice_values(choices)),
        InputKind::Markdown => "markdown".to_string(),
        InputKind::Json => "json".to_string(),
        InputKind::DateTime => "datetime".to_string(),
        InputKind::FilePath => "file_path".to_string(),
        InputKind::Url => "url".to_string(),
        InputKind::File(_) => "file".to_string(),
    }
}

fn choice_values(choices: &upeg_core::ChoiceSpec) -> String {
    choices
        .options
        .iter()
        .map(|option| option.value.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn draft_display(draft: &DraftInputValue) -> String {
    match draft {
        DraftInputValue::Empty => String::new(),
        DraftInputValue::Text(value) => value.clone(),
        DraftInputValue::Boolean(value) => value.to_string(),
        DraftInputValue::Options(value) => value.clone().unwrap_or_default(),
        DraftInputValue::MultiOptions(values) => values.join(", "),
        DraftInputValue::File(file) => file.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
    }
}

fn render_footer(frame: &mut Frame, area: Rect, footer_status: &str, status_message: Option<&str>) {
    let text = match status_message {
        // An ephemeral toast (F2 clipboard result) briefly replaces the
        // permanent footer status until the next keystroke clears it —
        // see `update.rs`'s `state.status_message = None` at the top of
        // every key handler.
        Some(message) => format!(" {message} "),
        None => format!(
            " {footer_status} · v{} · same Toolbox as CLI/MCP/HTTP ",
            env!("CARGO_PKG_VERSION"),
        ),
    };
    let footer = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}
