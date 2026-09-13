//! JSON Schema generation for Toolkit TOML manifests.

use crate::model::ToolkitToml;

/// Generate the Toolkit TOML JSON Schema as a `serde_json::Value`.
pub fn toolkit_schema_value() -> Result<serde_json::Value, serde_json::Error> {
    let mut schema = serde_json::to_value(schemars::schema_for!(ToolkitToml))?;
    normalize_toolkit_schema_contract(&mut schema);
    Ok(schema)
}

fn normalize_toolkit_schema_contract(schema: &mut serde_json::Value) {
    let Some(root) = schema.as_object_mut() else {
        return;
    };

    let required = root
        .entry("required")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let Some(required) = required.as_array_mut() {
        for field in ["id", "tools"] {
            if !required.iter().any(|value| value.as_str() == Some(field)) {
                required.push(serde_json::Value::String(field.to_string()));
            }
        }
        required.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    }

    if let Some(tools_schema) = schema
        .pointer_mut("/properties/tools")
        .and_then(serde_json::Value::as_object_mut)
    {
        tools_schema.insert("minItems".to_string(), serde_json::Value::from(1));
    }
}

/// Generate the Toolkit TOML JSON Schema as pretty JSON with a trailing newline.
pub fn toolkit_schema_json() -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(&toolkit_schema_value()?)?;
    json.push('\n');
    Ok(json)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{toolkit_schema_json, toolkit_schema_value};

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
    ];

    #[test]
    fn 도구킷_schema_json은_예상한_형태를_가진다() {
        let schema = toolkit_schema_value().expect("schema should serialize to JSON value");

        assert!(schema.get("$schema").and_then(Value::as_str).is_some());
        assert_eq!(
            schema.get("title").and_then(Value::as_str),
            Some("ToolkitToml")
        );
        assert!(schema.pointer("/properties/tools").is_some());

        let json = toolkit_schema_json().expect("schema should serialize to pretty JSON");
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn schema는_문서_참조와_순서를_담는다() {
        let schema = toolkit_schema_value().expect("schema");
        let defs = schema
            .get("$defs")
            .and_then(Value::as_object)
            .expect("$defs present");

        for (name, expected_order) in FIELD_REFERENCE_SCHEMAS {
            let def = if *name == "ToolkitToml" {
                &schema
            } else {
                defs.get(*name)
                    .unwrap_or_else(|| panic!("$defs should contain {name}"))
            };

            assert_eq!(
                def.get("x-doc-reference").and_then(Value::as_bool),
                Some(true),
                "{name} should have x-doc-reference = true"
            );
            assert_eq!(
                def.get("x-doc-order").and_then(Value::as_u64),
                Some(*expected_order),
                "{name} should have x-doc-order = {expected_order}"
            );
        }
    }

    #[test]
    fn 문서_참조_schema는_알수없는_필드에_닫혀_있다() {
        let schema = toolkit_schema_value().expect("schema");
        let defs = schema
            .get("$defs")
            .and_then(Value::as_object)
            .expect("$defs present");

        for (name, _order) in FIELD_REFERENCE_SCHEMAS {
            let def = if *name == "ToolkitToml" {
                &schema
            } else {
                defs.get(*name)
                    .unwrap_or_else(|| panic!("$defs should contain {name}"))
            };

            assert_eq!(
                def.get("additionalProperties").and_then(Value::as_bool),
                Some(false),
                "{name} should reject unknown fields in generated schema"
            );
        }
    }

    #[test]
    fn 도구킷_schema는_id와_비어있지_않은_도구를_요구한다() {
        let schema = toolkit_schema_value().expect("schema");

        assert_eq!(
            schema
                .pointer("/properties/tools/minItems")
                .and_then(Value::as_u64),
            Some(1),
            "tools should require at least one entry"
        );

        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("root required array present");
        for field in ["id", "tools"] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "root required should contain {field}"
            );
        }
    }

    #[test]
    fn 생성된_schema와_문서는_명확한_manifest_표현을_사용한다() {
        let schema = toolkit_schema_value().expect("schema");
        let tool_properties = schema
            .pointer("/$defs/ToolEntryToml/properties")
            .and_then(Value::as_object)
            .expect("ToolEntryToml properties present");

        let description_for = |field: &str| {
            tool_properties
                .get(field)
                .and_then(|prop| prop.get("description"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{field} should have a description"))
        };
        let normalized_description_for = |field: &str| description_for(field).replace('\n', " ");

        let credential_description = normalized_description_for("credential");
        assert!(credential_description.contains("Primary credential name/key"));
        assert!(credential_description.contains("UPEG_CREDENTIAL_<NAME>"));
        assert!(credential_description.contains("Never inline a secret"));
        let credentials_description = normalized_description_for("credentials");
        assert!(credentials_description.contains("no secret values are persisted"));
        let pin_description = normalized_description_for("pin");
        assert!(pin_description.contains("UI rendering hint only"));
        assert!(pin_description.contains("runtime adapter selection"));
        assert!(normalized_description_for("invoker").contains("canonical runtime declaration"));
        let primary_output_description = normalized_description_for("primary_output_id");
        assert!(primary_output_description.contains("Canonical output field id"));
        assert!(primary_output_description.contains("MCP text fallback"));
        assert!(primary_output_description.contains("Required when `outputs`"));
        assert!(primary_output_description.contains("omitted when `outputs` is empty"));
        let embed_url_description = normalized_description_for("embed_url");
        assert!(embed_url_description.contains("GUI sidecar URL registered when present"));
        assert!(embed_url_description.contains("not gated only by `pin`"));
        let controlled_embed_description = normalized_description_for("controlled_embed");
        assert!(controlled_embed_description.contains("Controlled Embed browser settings"));

        let docs = crate::toolkit_manifest_docs_markdown().expect("docs should render");
        assert!(docs.contains("Primary credential name/key for HTTP/LLM templates"));
        assert!(docs.contains("primary_output_id"));
        assert!(docs.contains("Canonical output field id used for the default CLI result"));
        assert!(docs.contains("Required when `outputs` is non-empty"));
        assert!(docs.contains("Tool outputs are canonical structured `ToolResult` values"));
        assert!(docs.contains("Never inline a secret"));
        assert!(docs.contains("open a GUI sidecar runtime adapter in a WebView or iframe"));
        assert!(docs.contains("controlled_embed"));
        assert!(docs.contains("custom_user_agent"));
    }

    #[test]
    fn 필드_설명은_빈_폴백이_아니다() {
        let schema = toolkit_schema_value().expect("schema");
        let defs = schema
            .get("$defs")
            .and_then(Value::as_object)
            .expect("$defs present");

        for (name, _order) in FIELD_REFERENCE_SCHEMAS {
            let def = if *name == "ToolkitToml" {
                &schema
            } else {
                defs.get(*name)
                    .unwrap_or_else(|| panic!("$defs should contain {name}"))
            };

            let properties = def
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{name} should have properties"));

            for (field, prop) in properties {
                let desc = prop
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                assert!(
                    !desc.is_empty(),
                    "{name}.{field} should have a non-empty description"
                );
            }
        }
    }
}
