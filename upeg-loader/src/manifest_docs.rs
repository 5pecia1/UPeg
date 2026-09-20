mod render;
mod schema_extract;
pub(crate) mod sections;

use render::{json_schema_summary, render_section};
use sections::GUIDE_SECTIONS;

/// OKF v0.2 frontmatter for the generated guide. The guide ships inside the
/// `docs/` knowledge bundle, where every non-reserved markdown file needs a
/// `type` field. Deliberately timestamp-free: the drift check compares the
/// rendered file byte-for-byte against the committed baseline, so a
/// regeneration-time `generated.at` would fail every run.
const GUIDE_FRONTMATTER: &str = "\
---
type: Generated Reference
title: External Tool Manifest Guide
description: Full Toolkit TOML field reference generated from the Rust manifest types.
tags: [manifest, toml, reference, generated]
status: stable
generated: { by: process:upeg-interface-toolkit-schema }
---

";

pub fn toolkit_manifest_docs_markdown() -> Result<String, serde_json::Error> {
    let schema = crate::schema::toolkit_schema_value()?;
    let schema_summary = json_schema_summary(&schema);
    let mut markdown = String::from(GUIDE_FRONTMATTER);
    markdown.push_str(
        "# External Tool Manifest Guide\n\nGenerated from upeg-loader manifest documentation metadata. Do not edit by hand.\n\n",
    );

    for section in GUIDE_SECTIONS {
        render_section(&mut markdown, section, &schema, &schema_summary);
    }
    let trimmed_len = markdown.trim_end_matches('\n').len();
    markdown.truncate(trimmed_len);
    markdown.push('\n');

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::toolkit_manifest_docs_markdown;
    use crate::schema::toolkit_schema_value;
    use serde_json::Value;

    const FIELD_REFERENCE_SCHEMAS: &[(&str, u64)] = &[
        ("ToolkitToml", 1),
        ("ToolEntryToml", 2),
        ("InputFieldToml", 3),
        ("InputChoiceToml", 4),
        ("OutputFieldToml", 5),
        ("ChainStepToml", 6),
        ("ChainConnectionToml", 7),
        ("KeyValueToml", 8),
        ("CredentialRefToml", 9),
        ("SelectorBindingToml", 10),
        ("TriggerToml", 11),
        ("ControlledEmbedToml", 12),
        ("ControlledEmbedBrowserToml", 12),
        ("BindingWaitToml", 13),
        ("BoardEntryToml", 14),
        ("PresentationToml", 15),
        ("PresentationColumnToml", 16),
        ("PresentationActionToml", 17),
        ("PresentationBindingToml", 18),
    ];

    fn discover_field_reference_schemas() -> Vec<String> {
        let schema = toolkit_schema_value().expect("schema");
        let mut names = Vec::new();

        let mut entries: Vec<(&str, u64)> = Vec::new();

        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            for (name, def_schema) in defs {
                if def_schema.get("x-doc-reference").and_then(Value::as_bool) == Some(true) {
                    let order = def_schema
                        .get("x-doc-order")
                        .and_then(Value::as_u64)
                        .unwrap_or(999);
                    entries.push((name, order));
                }
            }
        }

        if schema.get("x-doc-reference").and_then(Value::as_bool) == Some(true) {
            let order = schema
                .get("x-doc-order")
                .and_then(Value::as_u64)
                .unwrap_or(999);
            if let Some(title) = schema.get("title").and_then(Value::as_str) {
                entries.push((title, order));
            }
        }

        entries.sort_by_key(|&(_, order)| order);
        for (name, _) in entries {
            names.push(name.to_string());
        }

        names
    }

    #[test]
    fn manifest_docs_markdown_contains_required_content() {
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");

        assert!(markdown.contains("# External Tool Manifest Guide"));
        assert!(markdown.contains(
            "Generated from upeg-loader manifest documentation metadata. Do not edit by hand."
        ));

        for heading in [
            "Who this is for",
            "Quick start: one External tool in TOML",
            "Where upeg loads external manifests from",
            "Toolkit TOML structure",
            "Field reference",
            "Tool output contracts",
            "Invoker-specific fields",
            "Input rules",
            "Credentials and secret references",
            "JSON Schema and editor validation",
            "WASM plugin manifests",
            "MCP upstream configs",
            "Validation commands",
            "Troubleshooting common errors",
        ] {
            assert!(
                markdown.contains(&format!("## {heading}")),
                "missing section heading: {heading}"
            );
        }

        assert_eq!(
            markdown
                .matches("| Field | Required | Type | Description |")
                .count(),
            FIELD_REFERENCE_SCHEMAS.len(),
            "field reference should render one table per object schema"
        );

        let discovered = discover_field_reference_schemas();
        assert_eq!(
            discovered.len(),
            FIELD_REFERENCE_SCHEMAS.len(),
            "discovered schemas must match expected count"
        );
        for schema_name in &discovered {
            assert!(
                markdown.contains(&format!("### {schema_name}")),
                "missing field table heading for {schema_name}"
            );
        }
        assert!(markdown.contains(
            "| `id` | yes | string | Stable toolkit namespace. Tool entries are registered as `<toolkit>.<tool>`. |"
        ));
        assert!(markdown.contains("| `tool` | yes | string | Canonical Tool id to invoke. |"));
        assert!(markdown.contains(
            "| `when` | no | string or null | Boolean expression. False skips the step. |"
        ));
        assert!(markdown.contains("Tool outputs are canonical structured `ToolResult` values"));
        assert!(markdown.contains("`--json` prints the canonical success/error envelope"));
        assert!(markdown.contains(
            "MCP `tools/call` uses the same canonical success envelope as `structuredContent`"
        ));
        assert!(markdown.contains("render labels and values from canonical output entries"));
        assert!(!markdown.contains("|  |"));
        assert!(!markdown.contains("| no |  |"));

        for field in [
            "id",
            "tags",
            "display_label",
            "description",
            "tools",
            "inputs",
            "outputs",
            "pin",
            "pegboard_units",
            "invoker",
            "surfaces",
            "boards",
            "command",
            "args_template",
            "steps",
            "connections",
            "output",
            "url",
            "method",
            "headers",
            "body",
            "prompt",
            "provider",
            "model",
            "credential",
            "credentials",
            "wasm_path",
            "triggers",
            "embed_url",
            "controlled_embed.bindings",
            "action",
            "user_agent",
            "custom_user_agent",
            "viewport",
            "viewport_width",
            "viewport_height",
            "name",
            "type",
            "label",
            "required",
            "options",
            "value",
        ] {
            assert!(
                markdown.contains(&format!("`{field}`")),
                "missing field: {field}"
            );
        }
        assert!(markdown.contains("upeg tool validate ~/.upeg/toolkits/demo.toml"));
        assert!(markdown.contains("fixtures/toolkit.schema.json"));
        assert!(markdown.contains("examples/plugins/greet/"));
        assert!(markdown.contains("examples/mcp-imports/local.toml"));
        assert!(markdown.contains("retired `category` or `cat` field"));
        assert!(!markdown.contains("RuntimeToolManifest"));
        assert!(markdown.ends_with('\n'));
    }

    #[test]
    fn field_reference_uses_schema_discovery() {
        let schema = toolkit_schema_value().expect("schema");
        let mut names = Vec::new();

        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            let mut entries: Vec<(&str, u64)> = Vec::new();
            for (name, def_schema) in defs {
                let has_ref = def_schema.get("x-doc-reference").and_then(Value::as_bool);
                assert!(
                    has_ref.is_some(),
                    "$defs.{name} missing x-doc-reference (has {:?})",
                    def_schema.get("x-doc-reference")
                );
                if has_ref == Some(true) {
                    let order = def_schema
                        .get("x-doc-order")
                        .and_then(Value::as_u64)
                        .unwrap_or(999);
                    entries.push((name, order));
                }
            }
            entries.sort_by_key(|&(_, order)| order);
            for (name, _) in entries {
                names.push(name.to_string());
            }
        }

        let root_has_ref = schema.get("x-doc-reference").and_then(Value::as_bool);
        assert!(
            root_has_ref == Some(true),
            "root missing x-doc-reference (has {:?})",
            schema.get("x-doc-reference")
        );
        if root_has_ref == Some(true)
            && let Some(title) = schema.get("title").and_then(Value::as_str)
        {
            names.push(title.to_string());
        }

        assert_eq!(
            names.len(),
            FIELD_REFERENCE_SCHEMAS.len(),
            "discovered {names:?}"
        );
    }

    #[test]
    fn section_branching_uses_typed_enum() {
        use super::sections::{GUIDE_SECTIONS, SectionKind};
        let kinds: Vec<_> = GUIDE_SECTIONS.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SectionKind::FieldReference));
        assert!(kinds.contains(&SectionKind::InvokerFields));
        assert!(kinds.contains(&SectionKind::Troubleshooting));
    }

    #[test]
    fn manifest_docs_defines_no_content_constants() {
        let source = include_str!("manifest_docs.rs");
        let render_constants = [
            "const SECTIONS",
            "const INVOKERS",
            "const TROUBLESHOOTING",
            "const FIELD_REFERENCE_SCHEMAS",
            "const QUICK_START_EXAMPLES",
            "const TOOLKIT_STRUCTURE_EXAMPLES",
            "const INPUT_EXAMPLES",
            "const CREDENTIAL_EXAMPLES",
            "const VALIDATION_EXAMPLES",
        ];
        let mut in_tests = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.contains("#[cfg(test)]") {
                in_tests = true;
            }
            if in_tests {
                continue;
            }
            for name in &render_constants {
                assert!(
                    !trimmed.contains(name),
                    "manifest_docs.rs should not define render constant: {name}\nline: {trimmed}"
                );
            }
        }
    }

    #[test]
    fn invoker_table_uses_shared_metadata() {
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");
        for (name, purpose) in &[
            ("External", "spawn a local process"),
            ("Http", "call an HTTP endpoint"),
            (
                "Embed",
                "open a GUI sidecar runtime adapter in a WebView or iframe",
            ),
            (
                "Chain",
                "compose other tools with ordered steps and connections",
            ),
            ("Llm", "render a prompt into an LLM/provider adapter"),
            ("Wasm", "load a WASM module adapter"),
        ] {
            assert!(
                markdown.contains(&"| `".to_string())
                    && markdown.contains(name)
                    && markdown.contains(purpose),
                "invoker table missing {name} with purpose: {purpose}",
            );
        }
    }

    #[test]
    fn troubleshooting_uses_shared_metadata() {
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");
        for problem in &[
            "retired `category` or `cat` field",
            "`pegboard_units` is required",
            "`invoker` is required",
            "missing invoker-specific field",
            "inline secret field",
            "unknown surface or invoker",
            "chain cycle",
            "invalid `inputs`",
        ] {
            assert!(
                markdown.contains(problem),
                "troubleshooting table missing: {problem}"
            );
        }
    }

    #[test]
    fn invoker_names_match_runtime_invokers() {
        use crate::invoker_metadata::RUNTIME_INVOKERS;
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");

        for spec in RUNTIME_INVOKERS {
            assert!(
                markdown.contains(&format!("| `{}`", spec.name)),
                "generated docs missing invoker `{}`",
                spec.name,
            );
        }

        let rendered_count = markdown.matches("| `").filter(|_| true).count();
        let expected_min = RUNTIME_INVOKERS.len();
        assert!(
            rendered_count >= expected_min,
            "expected at least {expected_min} invoker rows, found {rendered_count}",
        );
    }

    #[test]
    fn troubleshooting_entries_match_shared_metadata() {
        use crate::error_metadata::TOOLKIT_TROUBLESHOOTING;
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");

        for entry in TOOLKIT_TROUBLESHOOTING {
            assert!(
                markdown.contains(entry.problem),
                "generated docs missing troubleshooting problem: `{}`",
                entry.problem,
            );
        }
    }

    #[test]
    fn generated_docs_include_file_based_examples() {
        let markdown = toolkit_manifest_docs_markdown().expect("docs should render");
        for expected in [
            "invoker = \"External\"",
            "invoker = \"Chain\"",
            "invoker = \"Llm\"",
            "invoker = \"Http\"",
            "printf",
            "connections = [{ from = \"hash\", to = \"uppercase\" }]",
            "provider = \"echo\"",
            "store = \"env\"",
            "upeg tool validate",
            "just toolkit-schema",
        ] {
            assert!(
                markdown.contains(expected),
                "generated docs missing file-backed example content: `{expected}`",
            );
        }
    }

    #[test]
    fn discovered_schema_metadata_count_matches_doc_tables() {
        let discovered = discover_field_reference_schemas();
        let docs = toolkit_manifest_docs_markdown().expect("docs should render");
        let table_sections = docs.matches("### ").count();
        assert!(
            table_sections >= discovered.len(),
            "docs has {table_sections} ### subheadings, schema has {} doc schemas",
            discovered.len(),
        );
    }
}
