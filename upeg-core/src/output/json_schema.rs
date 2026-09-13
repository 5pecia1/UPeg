use serde_json::{Map, Value};

use crate::input::{FileWireContract, UPEG_FILE_WIRE_SCHEMA_KEY};

use super::{ChoiceSpec, OutputFieldSpec, OutputKind, OutputSpec};

const JSON_SCHEMA_FORMAT_KEY: &str = "format";
const UPEG_KIND_SCHEMA_KEY: &str = "x-upeg-kind";
const UPEG_URL_SCHEMA_KEY: &str = "x-upeg-url";
const UPEG_EMBEDDED_VIEW_KIND: &str = "embedded_view";

impl OutputSpec {
    #[must_use]
    pub fn to_json_schema_value(&self) -> Value {
        let mut properties = Map::with_capacity(self.fields.len());

        for field in &self.fields {
            properties.insert(field.name.clone(), output_field_schema_value(field));
        }

        Value::Object(Map::from_iter([
            ("type".to_string(), Value::String("object".to_string())),
            ("properties".to_string(), Value::Object(properties)),
            ("required".to_string(), Value::Array(Vec::new())),
            ("additionalProperties".to_string(), Value::Bool(false)),
        ]))
    }
}

fn output_field_schema_value(field: &OutputFieldSpec) -> Value {
    let mut schema = output_kind_schema_value(&field.kind);
    if let Some(label) = &field.label {
        schema.insert("title".to_string(), Value::String(label.clone()));
    }
    if let Some(description) = &field.description {
        schema.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }
    Value::Object(schema)
}

fn output_kind_schema_value(kind: &OutputKind) -> Map<String, Value> {
    match kind {
        OutputKind::String => output_scalar_schema("string"),
        OutputKind::Number => output_scalar_schema("number"),
        OutputKind::Integer => output_scalar_schema("integer"),
        OutputKind::Boolean => output_scalar_schema("boolean"),
        OutputKind::Options(choices) => {
            let mut schema = output_scalar_schema("string");
            schema.insert("enum".to_string(), output_choice_values(choices));
            schema
        }
        OutputKind::MultiOptions(choices) => {
            let mut items = output_scalar_schema("string");
            items.insert("enum".to_string(), output_choice_values(choices));
            Map::from_iter([
                ("type".to_string(), Value::String("array".to_string())),
                ("items".to_string(), Value::Object(items)),
            ])
        }
        OutputKind::Markdown => {
            output_string_format_schema("markdown", OutputKind::Markdown.label())
        }
        OutputKind::Json => output_upeg_kind_schema(OutputKind::Json.label()),
        OutputKind::DateTime => {
            output_string_format_schema("date-time", OutputKind::DateTime.label())
        }
        OutputKind::FilePath => output_string_upeg_kind_schema(OutputKind::FilePath.label()),
        OutputKind::Url => output_string_format_schema("uri", OutputKind::Url.label()),
        OutputKind::File => {
            let mut schema =
                Map::from_iter([("type".to_string(), Value::String("object".to_string()))]);
            schema.insert(
                UPEG_KIND_SCHEMA_KEY.to_string(),
                Value::String(OutputKind::File.label().to_string()),
            );
            schema.insert(
                UPEG_FILE_WIRE_SCHEMA_KEY.to_string(),
                FileWireContract::schema_value(),
            );
            schema
        }
        OutputKind::EmbeddedView { url } => {
            let mut schema = output_string_format_schema("uri", UPEG_EMBEDDED_VIEW_KIND);
            schema.insert(UPEG_URL_SCHEMA_KEY.to_string(), Value::String(url.clone()));
            schema
        }
    }
}

fn output_scalar_schema(ty: &str) -> Map<String, Value> {
    Map::from_iter([("type".to_string(), Value::String(ty.to_string()))])
}

fn output_string_format_schema(format: &str, upeg_kind: &str) -> Map<String, Value> {
    let mut schema = output_string_upeg_kind_schema(upeg_kind);
    schema.insert(
        JSON_SCHEMA_FORMAT_KEY.to_string(),
        Value::String(format.to_string()),
    );
    schema
}

fn output_string_upeg_kind_schema(upeg_kind: &str) -> Map<String, Value> {
    let mut schema = output_scalar_schema("string");
    schema.insert(
        UPEG_KIND_SCHEMA_KEY.to_string(),
        Value::String(upeg_kind.to_string()),
    );
    schema
}

fn output_upeg_kind_schema(upeg_kind: &str) -> Map<String, Value> {
    Map::from_iter([(
        UPEG_KIND_SCHEMA_KEY.to_string(),
        Value::String(upeg_kind.to_string()),
    )])
}

fn output_choice_values(choices: &ChoiceSpec) -> Value {
    Value::Array(
        choices
            .options
            .iter()
            .map(|option| Value::String(option.value.clone()))
            .collect(),
    )
}
