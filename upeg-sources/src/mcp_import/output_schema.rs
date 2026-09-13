use std::collections::BTreeMap;

use serde_json::Value;
use upeg_core::{
    InputAdapterError, InputKind, InputSpec, OutputFieldSpec, OutputKind, OutputSpec,
    OutputSpecError,
};

use super::display_value;

const JSON_SCHEMA_PROPERTIES_KEY: &str = "properties";
const UPEG_FILE_POLICY_SCHEMA_KEY: &str = "x-upeg-file-policy";
const UPEG_KIND_SCHEMA_KEY: &str = "x-upeg-kind";
const UPEG_URL_SCHEMA_KEY: &str = "x-upeg-url";
const UPEG_EMBEDDED_VIEW_KIND: &str = "embedded_view";
const UPEG_URL_KIND: &str = "url";

/// Why an upstream `outputSchema` failed conversion to a typed
/// [`OutputSpec`]. Feeds [`super::SkipReason::OutputSchema`]: under
/// tool-level partial success a conversion failure skips the one
/// tool instead of failing the whole server import.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OutputSchemaError {
    /// The JSON-Schema subset importer rejected the shape (unsupported
    /// keyword such as `$ref`/`oneOf`/`anyOf`/`allOf`, nested object,
    /// non-string enum, ...). The wrapped error names the offending
    /// keyword or construct.
    #[error(transparent)]
    Adapter(#[from] InputAdapterError),
    /// The converted fields failed [`OutputSpec`] validation.
    #[error(transparent)]
    Spec(#[from] OutputSpecError),
    /// Malformed `x-upeg-kind: embedded_view` extension metadata.
    #[error("{0}")]
    EmbeddedView(String),
    /// An output property declares an input-only File policy.
    #[error("property `{property}` declares input-only `x-upeg-file-policy`")]
    FilePolicy { property: String },
}

pub(super) fn output_spec_from_json_schema(value: &Value) -> Result<OutputSpec, OutputSchemaError> {
    if let Some(property) = value
        .as_object()
        .and_then(|root| root.get(JSON_SCHEMA_PROPERTIES_KEY))
        .and_then(Value::as_object)
        .and_then(|properties| {
            properties.iter().find_map(|(name, schema)| {
                schema
                    .as_object()
                    .is_some_and(|schema| schema.contains_key(UPEG_FILE_POLICY_SCHEMA_KEY))
                    .then(|| name.clone())
            })
        })
    {
        return Err(OutputSchemaError::FilePolicy { property });
    }
    let embedded_view_urls = embedded_view_urls_from_output_schema(value)?;
    let input_compatible_value = output_schema_value_for_input_import(value);
    let input_spec = InputSpec::try_from(&input_compatible_value)?;

    Ok(OutputSpec::new(
        input_spec
            .fields
            .into_iter()
            .map(|field| OutputFieldSpec {
                name: field.name.as_str().to_string(),
                label: field.label,
                description: field.description,
                kind: embedded_view_urls
                    .get(field.name.as_str())
                    .map(|url| OutputKind::EmbeddedView { url: url.clone() })
                    .unwrap_or_else(|| output_kind_from_input_kind(field.kind)),
                constraints: field.constraints,
            })
            .collect(),
    )?)
}

fn embedded_view_urls_from_output_schema(
    value: &Value,
) -> Result<BTreeMap<String, String>, OutputSchemaError> {
    let Some(properties) = value
        .as_object()
        .and_then(|root| root.get(JSON_SCHEMA_PROPERTIES_KEY))
        .and_then(Value::as_object)
    else {
        return Ok(BTreeMap::new());
    };

    let mut urls = BTreeMap::new();
    for (name, schema_value) in properties {
        let Some(schema) = schema_value.as_object() else {
            continue;
        };
        if schema.get(UPEG_KIND_SCHEMA_KEY).and_then(Value::as_str) != Some(UPEG_EMBEDDED_VIEW_KIND)
        {
            continue;
        }
        let Some(url_value) = schema.get(UPEG_URL_SCHEMA_KEY) else {
            return Err(OutputSchemaError::EmbeddedView(format!(
                "property `{name}` with `embedded_view` must declare `x-upeg-url`"
            )));
        };
        let Some(url) = url_value.as_str() else {
            return Err(OutputSchemaError::EmbeddedView(format!(
                "property `{name}` `x-upeg-url` must be a string, got {}",
                display_value(url_value)
            )));
        };
        if url.trim().is_empty() {
            return Err(OutputSchemaError::EmbeddedView(format!(
                "property `{name}` `x-upeg-url` must be non-empty"
            )));
        }
        urls.insert(name.clone(), url.to_string());
    }
    Ok(urls)
}

fn output_schema_value_for_input_import(value: &Value) -> Value {
    let mut value = value.clone();
    let Some(properties) = value
        .as_object_mut()
        .and_then(|root| root.get_mut(JSON_SCHEMA_PROPERTIES_KEY))
        .and_then(Value::as_object_mut)
    else {
        return value;
    };

    for schema_value in properties.values_mut() {
        let Some(schema) = schema_value.as_object_mut() else {
            continue;
        };
        if schema.get(UPEG_KIND_SCHEMA_KEY).and_then(Value::as_str) == Some(UPEG_EMBEDDED_VIEW_KIND)
        {
            schema.insert(
                UPEG_KIND_SCHEMA_KEY.to_string(),
                Value::String(UPEG_URL_KIND.to_string()),
            );
        }
    }

    value
}

fn output_kind_from_input_kind(kind: InputKind) -> OutputKind {
    match kind {
        InputKind::String => OutputKind::String,
        InputKind::Number => OutputKind::Number,
        InputKind::Integer => OutputKind::Integer,
        InputKind::Boolean => OutputKind::Boolean,
        InputKind::Options(choices) => OutputKind::Options(choices),
        InputKind::MultiOptions(choices) => OutputKind::MultiOptions(choices),
        InputKind::Markdown => OutputKind::Markdown,
        InputKind::Json => OutputKind::Json,
        InputKind::DateTime => OutputKind::DateTime,
        InputKind::FilePath => OutputKind::FilePath,
        InputKind::Url => OutputKind::Url,
        InputKind::File(_) => OutputKind::File,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OutputSchemaError, SkipReason, parse_tools_list_entries};
    use serde_json::json;
    use upeg_core::OutputKind;

    #[test]
    fn 출력_파일_정책이_있는_mcp_도구는_건너뛴다() {
        // Given
        let tools = [json!({
            "name": "policy_output",
            "outputSchema": {
                "type": "object",
                "properties": {
                    "file": {
                        "type": "object",
                        "x-upeg-kind": "file",
                        "x-upeg-file-policy": {
                            "maxCount": 2,
                            "extensions": []
                        }
                    }
                }
            }
        })];

        // When
        let parsed =
            parse_tools_list_entries(&tools, "test").expect("MCP 도구 목록을 파싱해야 한다");

        // Then
        assert!(parsed.decls.is_empty());
        assert!(matches!(
            parsed.skipped.as_slice(),
            [skipped]
                if matches!(
                    &skipped.reason,
                    SkipReason::UnsupportedOutputSchema(OutputSchemaError::FilePolicy {
                        property
                    }) if property == "file"
                )
        ));
    }

    #[test]
    fn 정책이_없는_파일_출력은_mcp_도구로_가져온다() {
        // Given
        let tools = [json!({
            "name": "plain_file_output",
            "outputSchema": {
                "type": "object",
                "properties": {
                    "file": {
                        "type": "object",
                        "x-upeg-kind": "file"
                    }
                }
            }
        })];

        // When
        let parsed =
            parse_tools_list_entries(&tools, "test").expect("MCP 도구 목록을 파싱해야 한다");

        // Then
        assert!(parsed.skipped.is_empty());
        assert!(matches!(
            parsed.decls[0].output_spec.fields[0].kind,
            OutputKind::File
        ));
    }
}
