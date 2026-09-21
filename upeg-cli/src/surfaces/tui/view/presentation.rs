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
    let resolved = upeg_core::resolve_rows(presentation, &output_values);
    let mut rendered = false;
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
            body.push_str(&action.label);
        }
        body.push_str("\n[↑/↓] row · [←/→] action · [a] open");
    }
    for diagnostic in resolved.diagnostics {
        body.push_str(&format!("\n! {diagnostic}"));
    }
    rendered
}
