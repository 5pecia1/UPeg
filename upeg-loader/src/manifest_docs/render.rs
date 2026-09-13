use serde_json::Value;

use super::schema_extract::{extract_fields_from_schema, markdown_cell};
use super::sections::{self, ManifestDocSection, ManifestExample, SectionKind};

pub(super) fn render_section(
    markdown: &mut String,
    section: &ManifestDocSection,
    schema: &Value,
    schema_summary: &str,
) {
    markdown.push_str("## ");
    markdown.push_str(section.title);
    markdown.push_str("\n\n");
    markdown.push_str(section.body);
    markdown.push_str("\n\n");

    match section.kind {
        SectionKind::FieldReference => render_field_reference(markdown, schema),
        SectionKind::InvokerFields => sections::render_invoker_table(markdown),
        SectionKind::JsonSchemaValidation => {
            markdown.push_str(schema_summary);
            markdown.push_str("\n\n");
        }
        SectionKind::Troubleshooting => sections::render_troubleshooting_table(markdown),
        _ => {}
    }

    render_examples(markdown, section.examples);
}

pub(super) fn render_examples(markdown: &mut String, examples: &[ManifestExample]) {
    for example in examples {
        markdown.push_str("### ");
        markdown.push_str(example.title);
        markdown.push_str("\n\n```");
        markdown.push_str(example.language);
        markdown.push('\n');
        markdown.push_str(example.body);
        markdown.push_str("\n```\n\n");
    }
}

pub(super) fn render_field_table(
    markdown: &mut String,
    fields: &[super::schema_extract::FieldRow],
) {
    markdown.push_str("| Field | Required | Type | Description |\n");
    markdown.push_str("| --- | --- | --- | --- |\n");
    for field in fields {
        markdown.push_str("| `");
        markdown.push_str(&field.field);
        markdown.push_str("` | ");
        markdown.push_str(if field.required { "yes" } else { "no" });
        markdown.push_str(" | ");
        markdown.push_str(&markdown_cell(&field.type_name));
        markdown.push_str(" | ");
        markdown.push_str(&markdown_cell(&field.description));
        markdown.push_str(" |\n");
    }
    markdown.push('\n');
}

pub(super) fn render_field_reference(markdown: &mut String, schema: &Value) {
    let mut schemas: Vec<(&str, u64)> = Vec::new();

    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, def_schema) in defs {
            if def_schema.get("x-doc-reference").and_then(Value::as_bool) == Some(true) {
                let order = def_schema
                    .get("x-doc-order")
                    .and_then(Value::as_u64)
                    .unwrap_or(999);
                schemas.push((name, order));
            }
        }
    }

    if schema.get("x-doc-reference").and_then(Value::as_bool) == Some(true) {
        let order = schema
            .get("x-doc-order")
            .and_then(Value::as_u64)
            .unwrap_or(999);
        if let Some(title) = schema.get("title").and_then(Value::as_str) {
            schemas.push((title, order));
        }
    }

    schemas.sort_by_key(|&(_, order)| order);

    for (schema_name, _) in schemas {
        markdown.push_str("### ");
        markdown.push_str(schema_name);
        markdown.push_str("\n\n");

        let rows = extract_fields_from_schema(schema, schema_name);
        render_field_table(markdown, &rows);
    }
}

pub(super) fn join_code(fields: &[&str]) -> String {
    if fields.is_empty() {
        return "—".to_string();
    }

    fields
        .iter()
        .map(|field| format!("`{field}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn json_schema_summary(schema: &Value) -> String {
    let title = schema
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("ToolkitToml");
    let top_level_fields = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .map(|field| format!("`{field}`"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "no top-level properties found".to_string());

    format!("Generated schema title: `{title}`. Top-level schema fields: {top_level_fields}.")
}
