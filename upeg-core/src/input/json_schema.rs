use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::{
    ChoiceOption, ChoiceSpec, InputAdapterError, InputFieldSpec, InputKind, InputName, InputSpec,
    file_policy_schema::{
        UPEG_FILE_POLICY_SCHEMA_KEY, file_policy_from_schema, file_policy_schema_value,
    },
    file_wire_schema::{FileWireContract, UPEG_FILE_WIRE_SCHEMA_KEY},
    numeric_constraint_schema::{numeric_constraints_from_schema, write_numeric_constraints},
    string_constraint_schema::{string_constraints_from_schema, write_string_constraints},
};

const UNSUPPORTED_SCHEMA_KEYWORDS: &[&str] = &["$ref", "oneOf", "anyOf", "allOf"];
const JSON_SCHEMA_FORMAT_KEY: &str = "format";
const UPEG_KIND_SCHEMA_KEY: &str = "x-upeg-kind";

impl InputSpec {
    /// Export the typed input spec as the JSON-Schema-like subset used at
    /// external documentation and protocol boundaries.
    #[must_use]
    pub fn to_json_schema_value(&self) -> Value {
        let mut properties = Map::with_capacity(self.fields.len());
        let mut required = Vec::new();

        for field in &self.fields {
            if field.required {
                required.push(Value::String(field.name.as_str().to_string()));
            }
            properties.insert(field.name.as_str().to_string(), field_schema_value(field));
        }

        Value::Object(Map::from_iter([
            ("type".to_string(), Value::String("object".to_string())),
            ("properties".to_string(), Value::Object(properties)),
            ("required".to_string(), Value::Array(required)),
            ("additionalProperties".to_string(), Value::Bool(false)),
        ]))
    }
}

impl TryFrom<&Value> for InputSpec {
    type Error = InputAdapterError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let root = value
            .as_object()
            .ok_or(InputAdapterError::JsonSchemaRootNotObject {
                kind: json_value_kind(value),
            })?;
        reject_unsupported_keywords("root", root)?;
        validate_root_type(root)?;
        validate_additional_properties(root)?;

        let required = required_names(root)?;
        let Some(properties) = properties_object(root)? else {
            if let Some(property) = required.iter().next() {
                return Err(InputAdapterError::JsonSchemaRequiredPropertyNotDeclared {
                    property: property.clone(),
                });
            }
            return Ok(Self::empty());
        };

        for property in &required {
            if !properties.contains_key(property) {
                return Err(InputAdapterError::JsonSchemaRequiredPropertyNotDeclared {
                    property: property.clone(),
                });
            }
        }

        let mut fields = Vec::with_capacity(properties.len());
        for (name, schema_value) in properties {
            let schema = schema_value.as_object().ok_or_else(|| {
                InputAdapterError::JsonSchemaPropertyNotObject {
                    property: name.clone(),
                }
            })?;
            fields.push(field_from_json_schema(
                name,
                required.contains(name),
                schema,
            )?);
        }

        Ok(Self::new(fields)?)
    }
}

fn field_schema_value(field: &InputFieldSpec) -> Value {
    let mut schema = kind_schema_value(&field.kind);
    write_numeric_constraints(&mut schema, &field.kind, &field.constraints);
    write_string_constraints(&mut schema, &field.kind, &field.constraints);
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

fn kind_schema_value(kind: &InputKind) -> Map<String, Value> {
    match kind {
        InputKind::String => scalar_schema("string"),
        InputKind::Number => scalar_schema("number"),
        InputKind::Integer => scalar_schema("integer"),
        InputKind::Boolean => scalar_schema("boolean"),
        InputKind::Options(choices) => {
            let mut schema = scalar_schema("string");
            schema.insert("enum".to_string(), choice_values(choices));
            schema
        }
        InputKind::MultiOptions(choices) => {
            let mut items = scalar_schema("string");
            items.insert("enum".to_string(), choice_values(choices));
            Map::from_iter([
                ("type".to_string(), Value::String("array".to_string())),
                ("items".to_string(), Value::Object(items)),
            ])
        }
        InputKind::Markdown => string_format_schema("markdown", InputKind::Markdown.label()),
        InputKind::Json => upeg_kind_schema(InputKind::Json.label()),
        InputKind::DateTime => string_format_schema("date-time", InputKind::DateTime.label()),
        InputKind::FilePath => string_upeg_kind_schema(InputKind::FilePath.label()),
        InputKind::Url => string_format_schema("uri", InputKind::Url.label()),
        InputKind::File(policy) => {
            let mut schema =
                Map::from_iter([("type".to_string(), Value::String("object".to_string()))]);
            schema.insert(
                UPEG_KIND_SCHEMA_KEY.to_string(),
                Value::String(kind.label().to_string()),
            );
            schema.insert(
                UPEG_FILE_POLICY_SCHEMA_KEY.to_string(),
                file_policy_schema_value(policy),
            );
            schema.insert(
                UPEG_FILE_WIRE_SCHEMA_KEY.to_string(),
                FileWireContract::schema_value(),
            );
            schema
        }
    }
}

fn scalar_schema(ty: &str) -> Map<String, Value> {
    Map::from_iter([("type".to_string(), Value::String(ty.to_string()))])
}

fn string_format_schema(format: &str, upeg_kind: &str) -> Map<String, Value> {
    let mut schema = string_upeg_kind_schema(upeg_kind);
    schema.insert(
        JSON_SCHEMA_FORMAT_KEY.to_string(),
        Value::String(format.to_string()),
    );
    schema
}

fn string_upeg_kind_schema(upeg_kind: &str) -> Map<String, Value> {
    let mut schema = scalar_schema("string");
    schema.insert(
        UPEG_KIND_SCHEMA_KEY.to_string(),
        Value::String(upeg_kind.to_string()),
    );
    schema
}

fn upeg_kind_schema(upeg_kind: &str) -> Map<String, Value> {
    Map::from_iter([(
        UPEG_KIND_SCHEMA_KEY.to_string(),
        Value::String(upeg_kind.to_string()),
    )])
}

fn choice_values(choices: &ChoiceSpec) -> Value {
    Value::Array(
        choices
            .options
            .iter()
            .map(|option| Value::String(option.value.clone()))
            .collect(),
    )
}

fn validate_root_type(root: &Map<String, Value>) -> Result<(), InputAdapterError> {
    let Some(ty_value) = root.get("type") else {
        return Ok(());
    };
    let Some(ty) = ty_value.as_str() else {
        return Err(InputAdapterError::JsonSchemaRootTypeNotString {
            kind: json_value_kind(ty_value),
        });
    };
    if ty == "object" {
        Ok(())
    } else {
        Err(InputAdapterError::JsonSchemaRootTypeNotObject { ty: ty.into() })
    }
}

fn validate_additional_properties(root: &Map<String, Value>) -> Result<(), InputAdapterError> {
    let Some(value) = root.get("additionalProperties") else {
        return Ok(());
    };
    if matches!(value, Value::Bool(false)) {
        Ok(())
    } else {
        Err(InputAdapterError::JsonSchemaAdditionalPropertiesNotFalse {
            kind: json_value_kind(value),
        })
    }
}

fn properties_object(
    root: &Map<String, Value>,
) -> Result<Option<&Map<String, Value>>, InputAdapterError> {
    let Some(properties) = root.get("properties") else {
        return Ok(None);
    };
    properties
        .as_object()
        .ok_or(InputAdapterError::JsonSchemaPropertiesNotObject {
            kind: json_value_kind(properties),
        })
        .map(Some)
}

fn required_names(root: &Map<String, Value>) -> Result<BTreeSet<String>, InputAdapterError> {
    let Some(required) = root.get("required") else {
        return Ok(BTreeSet::new());
    };
    let required = required
        .as_array()
        .ok_or(InputAdapterError::JsonSchemaRequiredNotArray {
            kind: json_value_kind(required),
        })?;
    let mut names = BTreeSet::new();
    for value in required {
        let Some(name) = value.as_str() else {
            return Err(InputAdapterError::JsonSchemaRequiredEntryNotString {
                kind: json_value_kind(value),
            });
        };
        names.insert(name.to_string());
    }
    Ok(names)
}

fn field_from_json_schema(
    name: &str,
    required: bool,
    schema: &Map<String, Value>,
) -> Result<InputFieldSpec, InputAdapterError> {
    reject_unsupported_keywords(&format!("property `{name}`"), schema)?;
    let label = optional_string(schema, name, "title")?;
    let description = optional_string(schema, name, "description")?;
    let kind = input_kind_from_field_schema(name, schema)?;
    let mut constraints = numeric_constraints_from_schema(name, &kind, schema)?;
    constraints.string = string_constraints_from_schema(name, &kind, schema)?;

    Ok(InputFieldSpec::with_constraints(
        InputName::new(name.to_string())?,
        label,
        description,
        required,
        kind,
        constraints,
    )?)
}

fn optional_string(
    schema: &Map<String, Value>,
    property: &str,
    key: &str,
) -> Result<Option<String>, InputAdapterError> {
    let Some(value) = schema.get(key) else {
        return Ok(None);
    };
    let Some(text) = value.as_str() else {
        return if key == "description" {
            Err(InputAdapterError::JsonSchemaDescriptionNotString {
                property: property.to_string(),
                kind: json_value_kind(value),
            })
        } else {
            Err(InputAdapterError::JsonSchemaTitleNotString {
                property: property.to_string(),
                kind: json_value_kind(value),
            })
        };
    };
    Ok(Some(text.to_string()))
}

fn input_kind_from_field_schema(
    property: &str,
    schema: &Map<String, Value>,
) -> Result<InputKind, InputAdapterError> {
    if schema.contains_key("properties") {
        return Err(InputAdapterError::JsonSchemaNestedObject {
            property: property.to_string(),
        });
    }

    if let Some(upeg_kind) = upeg_kind(schema) {
        return input_kind_from_upeg_kind(property, schema, upeg_kind);
    }

    let ty = schema_type(property, schema)?;
    let enum_values = enum_values(property, schema)?;
    let ty = ty.unwrap_or("string");

    match ty {
        "string" => enum_values.map_or(Ok(InputKind::String), |values| {
            Ok(InputKind::Options(choice_spec_from_values(values)?))
        }),
        "number" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Number),
        "integer" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Integer),
        "boolean" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Boolean),
        "markdown" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Markdown),
        "json" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Json),
        "datetime" => scalar_kind_without_enum(property, ty, enum_values, InputKind::DateTime),
        "file_path" => scalar_kind_without_enum(property, ty, enum_values, InputKind::FilePath),
        "url" => scalar_kind_without_enum(property, ty, enum_values, InputKind::Url),
        "options" => {
            let values =
                enum_values.ok_or_else(|| InputAdapterError::JsonSchemaOptionsMissingEnum {
                    property: property.to_string(),
                })?;
            Ok(InputKind::Options(choice_spec_from_values(values)?))
        }
        "multi_options" => multi_options_from_legacy_schema(property, schema, enum_values),
        "array" => {
            reject_top_level_enum(property, ty, enum_values)?;
            multi_options_from_array_schema(property, schema)
        }
        "object" => Err(InputAdapterError::JsonSchemaNestedObject {
            property: property.to_string(),
        }),
        other => Err(InputAdapterError::JsonSchemaUnsupportedType {
            property: property.to_string(),
            ty: other.to_string(),
        }),
    }
}

fn upeg_kind(schema: &Map<String, Value>) -> Option<&str> {
    schema.get(UPEG_KIND_SCHEMA_KEY).and_then(Value::as_str)
}

fn input_kind_from_upeg_kind(
    property: &str,
    schema: &Map<String, Value>,
    upeg_kind: &str,
) -> Result<InputKind, InputAdapterError> {
    let enum_values = enum_values(property, schema)?;
    match upeg_kind {
        "string" => enum_values.map_or(Ok(InputKind::String), |values| {
            Ok(InputKind::Options(choice_spec_from_values(values)?))
        }),
        "number" => scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Number),
        "integer" => scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Integer),
        "boolean" => scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Boolean),
        "options" => {
            let values =
                enum_values.ok_or_else(|| InputAdapterError::JsonSchemaOptionsMissingEnum {
                    property: property.to_string(),
                })?;
            Ok(InputKind::Options(choice_spec_from_values(values)?))
        }
        "multi_options" => multi_options_from_legacy_schema(property, schema, enum_values),
        "markdown" => {
            scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Markdown)
        }
        "json" => scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Json),
        "datetime" => {
            scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::DateTime)
        }
        "file_path" => {
            scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::FilePath)
        }
        "url" => scalar_kind_without_enum(property, upeg_kind, enum_values, InputKind::Url),
        "file" => {
            reject_top_level_enum(property, upeg_kind, enum_values)?;
            FileWireContract::from_schema(schema.get(UPEG_FILE_WIRE_SCHEMA_KEY)).map_err(
                |source| InputAdapterError::JsonSchemaFileWire {
                    property: property.to_string(),
                    source,
                },
            )?;
            let policy = file_policy_from_schema(schema.get(UPEG_FILE_POLICY_SCHEMA_KEY)).map_err(
                |source| InputAdapterError::JsonSchemaFilePolicy {
                    property: property.to_string(),
                    source,
                },
            )?;
            Ok(InputKind::File(policy))
        }
        other => Err(InputAdapterError::JsonSchemaUnsupportedType {
            property: property.to_string(),
            ty: other.to_string(),
        }),
    }
}

fn scalar_kind_without_enum(
    property: &str,
    ty: &str,
    enum_values: Option<Vec<String>>,
    kind: InputKind,
) -> Result<InputKind, InputAdapterError> {
    reject_top_level_enum(property, ty, enum_values)?;
    Ok(kind)
}

fn reject_top_level_enum(
    property: &str,
    ty: &str,
    enum_values: Option<Vec<String>>,
) -> Result<(), InputAdapterError> {
    if enum_values.is_some() {
        Err(InputAdapterError::JsonSchemaEnumUnsupportedType {
            property: property.to_string(),
            ty: ty.to_string(),
        })
    } else {
        Ok(())
    }
}

fn multi_options_from_legacy_schema(
    property: &str,
    schema: &Map<String, Value>,
    enum_values: Option<Vec<String>>,
) -> Result<InputKind, InputAdapterError> {
    if let Some(values) = enum_values {
        return Ok(InputKind::MultiOptions(choice_spec_from_values(values)?));
    }
    if schema.contains_key("items") {
        return multi_options_from_array_schema(property, schema);
    }
    Err(InputAdapterError::JsonSchemaOptionsMissingEnum {
        property: property.to_string(),
    })
}

fn multi_options_from_array_schema(
    property: &str,
    schema: &Map<String, Value>,
) -> Result<InputKind, InputAdapterError> {
    let Some(items_value) = schema.get("items") else {
        return Err(InputAdapterError::JsonSchemaArrayItemsMissingEnum {
            property: property.to_string(),
        });
    };
    if items_value.is_array() {
        return Err(InputAdapterError::JsonSchemaTupleArray {
            property: property.to_string(),
        });
    }
    let items = items_value.as_object().ok_or_else(|| {
        InputAdapterError::JsonSchemaArrayItemsNotObject {
            property: property.to_string(),
            kind: json_value_kind(items_value),
        }
    })?;
    reject_unsupported_keywords(&format!("property `{property}` array items"), items)?;
    if items.contains_key("properties") {
        return Err(InputAdapterError::JsonSchemaNestedObject {
            property: property.to_string(),
        });
    }

    let item_ty = array_item_type(property, items)?.unwrap_or("string");
    if item_ty != "string" {
        return Err(InputAdapterError::JsonSchemaArrayItemsUnsupportedType {
            property: property.to_string(),
            ty: item_ty.to_string(),
        });
    }

    let values = enum_values(property, items)?.ok_or_else(|| {
        InputAdapterError::JsonSchemaArrayItemsMissingEnum {
            property: property.to_string(),
        }
    })?;
    Ok(InputKind::MultiOptions(choice_spec_from_values(values)?))
}

fn schema_type<'a>(
    property: &str,
    schema: &'a Map<String, Value>,
) -> Result<Option<&'a str>, InputAdapterError> {
    let Some(value) = schema.get("type") else {
        return Ok(None);
    };
    value
        .as_str()
        .ok_or(InputAdapterError::JsonSchemaTypeNotString {
            property: property.to_string(),
            kind: json_value_kind(value),
        })
        .map(Some)
}

fn array_item_type<'a>(
    property: &str,
    items: &'a Map<String, Value>,
) -> Result<Option<&'a str>, InputAdapterError> {
    let Some(value) = items.get("type") else {
        return Ok(None);
    };
    value
        .as_str()
        .ok_or(InputAdapterError::JsonSchemaArrayItemsTypeNotString {
            property: property.to_string(),
            kind: json_value_kind(value),
        })
        .map(Some)
}

fn enum_values(
    property: &str,
    schema: &Map<String, Value>,
) -> Result<Option<Vec<String>>, InputAdapterError> {
    let Some(value) = schema.get("enum") else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or(InputAdapterError::JsonSchemaEnumNotArray {
            property: property.to_string(),
            kind: json_value_kind(value),
        })?;
    if values.is_empty() {
        return Err(InputAdapterError::JsonSchemaEmptyEnum {
            property: property.to_string(),
        });
    }

    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let Some(choice) = value.as_str() else {
            return Err(InputAdapterError::JsonSchemaEnumValueNotString {
                property: property.to_string(),
                kind: json_value_kind(value),
            });
        };
        parsed.push(choice.to_string());
    }
    Ok(Some(parsed))
}

fn choice_spec_from_values(values: Vec<String>) -> Result<ChoiceSpec, InputAdapterError> {
    let mut options = Vec::with_capacity(values.len());
    for value in values {
        options.push(ChoiceOption::new(value, None, None)?);
    }
    Ok(ChoiceSpec::new(options)?)
}

fn reject_unsupported_keywords(
    location: &str,
    schema: &Map<String, Value>,
) -> Result<(), InputAdapterError> {
    for &keyword in UNSUPPORTED_SCHEMA_KEYWORDS {
        if schema.contains_key(keyword) {
            return Err(InputAdapterError::JsonSchemaUnsupportedKeyword {
                location: location.to_string(),
                keyword,
            });
        }
    }
    Ok(())
}

fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn input_name(raw: &str) -> InputName {
        InputName::new(raw).expect("test input name should be valid")
    }

    fn choice(value: &str) -> ChoiceOption {
        ChoiceOption::new(value, None, None).expect("test choice should be valid")
    }

    fn choices(values: &[&str]) -> ChoiceSpec {
        ChoiceSpec::new(values.iter().map(|value| choice(value)).collect())
            .expect("test choice spec should be valid")
    }

    fn field(name: &str, required: bool, kind: InputKind) -> InputFieldSpec {
        InputFieldSpec::new(
            input_name(name),
            Some(format!("{name} title")),
            Some(format!("{name} description")),
            required,
            kind,
        )
        .expect("test field should be valid")
    }

    fn all_kind_spec() -> InputSpec {
        InputSpec::new(vec![
            field("string", true, InputKind::String),
            field("number", true, InputKind::Number),
            field("integer", true, InputKind::Integer),
            field("boolean", true, InputKind::Boolean),
            field(
                "options",
                true,
                InputKind::Options(choices(&["fast", "safe"])),
            ),
            field(
                "multi_options",
                true,
                InputKind::MultiOptions(choices(&["fast", "safe"])),
            ),
            field("markdown", false, InputKind::Markdown),
            field("json", false, InputKind::Json),
            field("datetime", false, InputKind::DateTime),
            field("file_path", false, InputKind::FilePath),
            field("url", false, InputKind::Url),
        ])
        .expect("all kind spec should be valid")
    }

    #[test]
    fn json_schema_어댑터는_모든_입력_종류를_왕복한다() {
        let spec = all_kind_spec();

        let schema = spec.to_json_schema_value();
        assert_eq!(schema["type"], json!("object"));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["required"],
            json!([
                "string",
                "number",
                "integer",
                "boolean",
                "options",
                "multi_options"
            ]),
        );
        assert_eq!(schema["properties"]["string"]["type"], json!("string"));
        assert_eq!(schema["properties"]["number"]["type"], json!("number"));
        assert_eq!(schema["properties"]["integer"]["type"], json!("integer"));
        assert_eq!(schema["properties"]["boolean"]["type"], json!("boolean"));
        assert_eq!(schema["properties"]["markdown"]["type"], json!("string"));
        assert_eq!(
            schema["properties"]["markdown"]["format"],
            json!("markdown")
        );
        assert_eq!(
            schema["properties"]["markdown"]["x-upeg-kind"],
            json!("markdown")
        );
        assert!(schema["properties"]["json"].get("type").is_none());
        assert_eq!(schema["properties"]["json"]["x-upeg-kind"], json!("json"));
        assert_eq!(schema["properties"]["datetime"]["type"], json!("string"));
        assert_eq!(
            schema["properties"]["datetime"]["format"],
            json!("date-time")
        );
        assert_eq!(
            schema["properties"]["datetime"]["x-upeg-kind"],
            json!("datetime")
        );
        assert_eq!(schema["properties"]["file_path"]["type"], json!("string"));
        assert_eq!(
            schema["properties"]["file_path"]["x-upeg-kind"],
            json!("file_path")
        );
        assert_eq!(schema["properties"]["url"]["type"], json!("string"));
        assert_eq!(schema["properties"]["url"]["format"], json!("uri"));
        assert_eq!(schema["properties"]["url"]["x-upeg-kind"], json!("url"));
        assert_eq!(schema["properties"]["options"]["type"], json!("string"));
        assert_eq!(
            schema["properties"]["options"]["enum"],
            json!(["fast", "safe"]),
        );
        assert_eq!(
            schema["properties"]["multi_options"],
            json!({
                "type": "array",
                "items": { "type": "string", "enum": ["fast", "safe"] },
                "title": "multi_options title",
                "description": "multi_options description",
            }),
        );

        let imported = InputSpec::try_from(&schema).expect("exported schema should import");
        assert_eq!(imported, spec);
    }

    #[test]
    fn json_schema_어댑터는_열거형이_있는_레거시_선택_타입_라벨을_가져온다() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "options", "enum": ["fast", "safe"] },
                "flags": { "type": "multi_options", "enum": ["dry", "verbose"] }
            },
            "required": ["mode"]
        });

        let spec = InputSpec::try_from(&schema).expect("legacy choice labels should import");

        assert!(spec.fields[0].required);
        assert!(matches!(spec.fields[0].kind, InputKind::Options(_)));
        assert!(!spec.fields[1].required);
        assert!(matches!(spec.fields[1].kind, InputKind::MultiOptions(_)));
    }

    #[test]
    fn json_schema_어댑터는_x_upeg_kind로_특수_입력을_가져온다() {
        let schema = json!({
            "type": "object",
            "properties": {
                "body": { "type": "string", "format": "markdown", "x-upeg-kind": "markdown" },
                "payload": { "x-upeg-kind": "json" },
                "when": { "type": "string", "format": "date-time", "x-upeg-kind": "datetime" },
                "path": { "type": "string", "x-upeg-kind": "file_path" },
                "link": { "type": "string", "format": "uri", "x-upeg-kind": "url" },
                "upload": { "type": "object", "x-upeg-kind": "file" }
            }
        });

        let spec = InputSpec::try_from(&schema).expect("x-upeg-kind schema should import");

        let kind_for = |name: &str| {
            &spec
                .fields
                .iter()
                .find(|field| field.name.as_str() == name)
                .unwrap_or_else(|| panic!("field `{name}` should exist"))
                .kind
        };
        assert!(matches!(kind_for("body"), InputKind::Markdown));
        assert!(matches!(kind_for("payload"), InputKind::Json));
        assert!(matches!(kind_for("when"), InputKind::DateTime));
        assert!(matches!(kind_for("path"), InputKind::FilePath));
        assert!(matches!(kind_for("link"), InputKind::Url));
        assert!(matches!(kind_for("upload"), InputKind::File(_)));
    }

    #[test]
    fn json_schema_어댑터는_지원되지_않은_구성을_거부한다() {
        let nested = json!({
            "type": "object",
            "properties": {
                "nested": { "type": "object", "properties": { "name": { "type": "string" } } }
            }
        });
        assert!(matches!(
            InputSpec::try_from(&nested),
            Err(InputAdapterError::JsonSchemaNestedObject { property }) if property == "nested"
        ));

        let reference = json!({ "$ref": "#/components/schemas/Input" });
        assert!(matches!(
            InputSpec::try_from(&reference),
            Err(InputAdapterError::JsonSchemaUnsupportedKeyword {
                keyword: "$ref",
                ..
            })
        ));

        let composition = json!({
            "type": "object",
            "properties": { "mode": { "oneOf": [{ "type": "string" }] } }
        });
        assert!(matches!(
            InputSpec::try_from(&composition),
            Err(InputAdapterError::JsonSchemaUnsupportedKeyword {
                keyword: "oneOf",
                ..
            })
        ));

        let tuple_array = json!({
            "type": "object",
            "properties": { "flags": { "type": "array", "items": [{ "type": "string" }] } }
        });
        assert!(matches!(
            InputSpec::try_from(&tuple_array),
            Err(InputAdapterError::JsonSchemaTupleArray { property }) if property == "flags"
        ));

        let non_string_enum = json!({
            "type": "object",
            "properties": { "mode": { "type": "string", "enum": ["fast", 1] } }
        });
        assert!(matches!(
            InputSpec::try_from(&non_string_enum),
            Err(InputAdapterError::JsonSchemaEnumValueNotString { property, .. })
                if property == "mode"
        ));

        let empty_enum = json!({
            "type": "object",
            "properties": { "mode": { "type": "string", "enum": [] } }
        });
        assert!(matches!(
            InputSpec::try_from(&empty_enum),
            Err(InputAdapterError::JsonSchemaEmptyEnum { property }) if property == "mode"
        ));

        let unknown_type = json!({
            "type": "object",
            "properties": { "blob": { "type": "bytes" } }
        });
        assert!(matches!(
            InputSpec::try_from(&unknown_type),
            Err(InputAdapterError::JsonSchemaUnsupportedType { property, ty })
                if property == "blob" && ty == "bytes"
        ));
    }
}
