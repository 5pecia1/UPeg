/// Documentation row extracted from one JSON Schema object property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldRow {
    /// TOML field name.
    pub(super) field: String,
    /// Whether the schema lists this field in `required`.
    pub(super) required: bool,
    /// Rendered JSON Schema type.
    pub(super) type_name: String,
    /// User-facing schema description.
    pub(super) description: String,
}

pub(super) fn extract_fields_from_schema(
    value: &serde_json::Value,
    schema_name: &str,
) -> Vec<FieldRow> {
    let Some(schema) = schema_for_name(value, schema_name) else {
        return Vec::new();
    };
    let Some(properties) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let required = required_fields(schema);

    let mut rows = Vec::with_capacity(properties.len());
    for (field, property_schema) in properties {
        rows.push(FieldRow {
            field: field.clone(),
            required: required.contains(&field.as_str()),
            type_name: schema_type_label(property_schema),
            description: property_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("—")
                .to_string(),
        });
    }
    rows
}

pub(super) fn schema_for_name<'a>(
    value: &'a serde_json::Value,
    schema_name: &str,
) -> Option<&'a serde_json::Value> {
    if value.get("title").and_then(serde_json::Value::as_str) == Some(schema_name) {
        return Some(value);
    }

    value
        .get("$defs")
        .and_then(serde_json::Value::as_object)
        .and_then(|defs| defs.get(schema_name))
}

pub(super) fn required_fields(schema: &serde_json::Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn schema_type_label(schema: &serde_json::Value) -> String {
    if let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str) {
        return schema_ref_label(reference).to_string();
    }

    if let Some(type_name) = schema.get("type").and_then(serde_json::Value::as_str) {
        return schema_type_token(schema, type_name);
    }

    if let Some(type_names) = schema.get("type").and_then(serde_json::Value::as_array) {
        let labels = type_names
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(|type_name| schema_type_token(schema, type_name))
            .collect::<Vec<_>>();
        if !labels.is_empty() {
            return labels.join(" or ");
        }
    }

    // `Option<SomeNamedType>` lands here: schemars emits
    // `anyOf: [{"$ref": …}, {"type": "null"}]` rather than the
    // `"type": [..., "null"]` union it uses for inline scalars.
    if let Some(variants) = schema.get("anyOf").and_then(serde_json::Value::as_array) {
        let labels = variants
            .iter()
            .map(schema_type_label)
            .filter(|label| label != "unknown")
            .collect::<Vec<_>>();
        if !labels.is_empty() {
            return labels.join(" or ");
        }
    }

    "unknown".to_string()
}

pub(super) fn schema_type_token(schema: &serde_json::Value, type_name: &str) -> String {
    match type_name {
        "array" => schema
            .get("items")
            .map(|items| format!("array<{}>", schema_type_label(items)))
            .unwrap_or_else(|| "array".to_string()),
        other => other.to_string(),
    }
}

pub(super) fn schema_ref_label(reference: &str) -> &str {
    reference.strip_prefix("#/$defs/").unwrap_or(reference)
}

pub(super) fn markdown_cell(value: &str) -> String {
    let normalized = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("<br>");

    if normalized.is_empty() {
        "—".to_string()
    } else {
        normalized.replace('|', "\\|")
    }
}
