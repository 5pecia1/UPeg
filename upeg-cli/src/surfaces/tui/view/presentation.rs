use upeg_core::OutputEntry;

pub(super) fn append_presentation(
    body: &mut String,
    tool_id: &str,
    outputs: &[OutputEntry],
    is_error: bool,
    selected_row: usize,
    selected_action: usize,
) -> bool {
    if is_error {
        return false;
    }
    let Some(tool) = upeg_runtime::toolbox_tool(tool_id) else {
        return false;
    };
    let Some(presentation) = tool.presentation.as_ref() else {
        return false;
    };
    let output_values = serde_json::Value::Object(
        outputs
            .iter()
            .map(|entry| (entry.id.clone(), entry.value.to_json_value()))
            .collect(),
    );
    let view = upeg_core::resolve_view(presentation, &output_values);
    let resolved = &view.rows;
    let mut rendered = view.title.is_some();
    if let Some(title) = &view.title {
        body.push_str(&format!("\n\n{title}"));
    }
    if let Some(subtitle) = &view.subtitle {
        body.push_str(&format!("\n{subtitle}"));
        rendered = true;
    }
    if let Some(status) = &view.status {
        body.push_str(&format!("\n[{:?}] {}", status.tone, status.label));
        rendered = true;
    }
    for field in &view.summary {
        let value = match &field.value {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        body.push_str(&format!("\n{}: {value}", field.label));
        rendered = true;
    }
    for notice in &view.notices {
        body.push_str(&format!("\n! [{:?}] {}", notice.severity, notice.text));
        rendered = true;
    }
    if !resolved.rows.is_empty() {
        rendered = true;
        let selected_row = selected_row.min(resolved.rows.len() - 1);
        body.push_str("\n\n");
        body.push_str(
            &presentation
                .columns
                .iter()
                .map(|column| column.label.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        for (index, row) in resolved.rows.iter().enumerate() {
            let marker = if index == selected_row { "▶" } else { " " };
            let cells = row
                .cells
                .iter()
                .map(|value| match value {
                    serde_json::Value::String(value) => value.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\t");
            body.push_str(&format!("\n{marker} {cells}"));
        }
    }
    if resolved.rows.is_empty()
        && let Some(message) = &view.empty_message
    {
        body.push_str(&format!("\n\n{message}"));
        rendered = true;
    }
    if let Some(detail) = &view.detail {
        for field in &detail.fields {
            body.push_str(&format!("\n{}: {}", field.label, field.value));
            rendered = true;
        }
        for content in [&detail.markdown, &detail.diff].into_iter().flatten() {
            if !content.is_empty() {
                body.push_str(&format!("\n\n{content}"));
                rendered = true;
            }
        }
    }
    if !resolved.rows.is_empty()
        && let Some(detail) = view
            .row_details
            .get(selected_row.min(resolved.rows.len() - 1))
    {
        for field in &detail.fields {
            body.push_str(&format!("\n{}: {}", field.label, field.value));
            rendered = true;
        }
        for content in [&detail.markdown, &detail.diff].into_iter().flatten() {
            if !content.is_empty() {
                body.push_str(&format!("\n\n{content}"));
                rendered = true;
            }
        }
    }
    let row_available = resolved.row_actions_enabled && !resolved.rows.is_empty();
    let actions = presentation
        .actions
        .iter()
        .filter(|action| matches!(action.scope, upeg_core::ActionScope::Result) || row_available)
        .collect::<Vec<_>>();
    if !actions.is_empty() {
        rendered = true;
        let selected_action = selected_action.min(actions.len() - 1);
        body.push_str("\n\nActions: ");
        for (index, action) in actions.iter().enumerate() {
            if index > 0 {
                body.push_str(" · ");
            }
            if index == selected_action {
                body.push_str("▶ ");
            }
            let availability =
                upeg_core::resolve_action_availability(presentation, action, &output_values);
            body.push_str(&action.label);
            if !availability.enabled {
                body.push_str(&format!(" ({})", availability.reason.unwrap_or_default()));
            }
        }
        body.push_str("\n[↑/↓] row · [←/→] action · [a] open");
    }
    for diagnostic in resolved.diagnostics.iter().chain(view.diagnostics.iter()) {
        body.push_str(&format!("\n! {diagnostic}"));
    }
    rendered
}
