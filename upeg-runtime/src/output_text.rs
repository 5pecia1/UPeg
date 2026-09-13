//! Human-readable output summaries and explicit text-transport serialization.

use std::fmt::Write as _;

use upeg_core::{FileContent, FileValue, OutputValue, ToolSuccess};

const FILE_SUMMARY_COMPONENT_MAX_BYTES: usize = 92;
const TRUNCATION_MARKER: &str = "…";
const SUMMARY_SEPARATOR: &str = " · ";

/// Return the primary output formatted for bounded human display.
#[must_use]
pub fn tool_success_primary_text(success: &ToolSuccess) -> String {
    primary_output_value(success).map_or_else(String::new, output_value_text)
}

/// Return the primary output serialized for a text-only transport.
///
/// Unlike [`tool_success_primary_text`], a `File` value uses its canonical
/// JSON wire representation, including base64 content.
pub fn tool_success_primary_canonical_wire_text(
    success: &ToolSuccess,
) -> Result<String, serde_json::Error> {
    primary_output_value(success)
        .map_or_else(|| Ok(String::new()), output_value_canonical_wire_text)
}

/// Format one output value for human display.
#[must_use]
pub fn output_value_text(value: &OutputValue) -> String {
    match value {
        OutputValue::String(value)
        | OutputValue::Options(value)
        | OutputValue::Markdown(value)
        | OutputValue::DateTime(value)
        | OutputValue::FilePath(value)
        | OutputValue::Url(value)
        | OutputValue::EmbeddedView(value) => value.clone(),
        OutputValue::Number(value) => value.to_string(),
        OutputValue::Integer(value) => value.to_string(),
        OutputValue::Boolean(value) => value.to_string(),
        OutputValue::MultiOptions(values) => serde_json::to_string(values).unwrap_or_default(),
        OutputValue::Json(value) => value.to_string(),
        OutputValue::File(value) => file_value_summary(value),
    }
}

/// Serialize one output value for a text-only transport.
///
/// Callers must opt into this API because a `File` value materializes its
/// base64 canonical wire representation.
pub fn output_value_canonical_wire_text(value: &OutputValue) -> Result<String, serde_json::Error> {
    match value {
        OutputValue::File(value) => serde_json::to_string(value),
        OutputValue::String(_)
        | OutputValue::Number(_)
        | OutputValue::Integer(_)
        | OutputValue::Boolean(_)
        | OutputValue::Options(_)
        | OutputValue::MultiOptions(_)
        | OutputValue::Markdown(_)
        | OutputValue::Json(_)
        | OutputValue::DateTime(_)
        | OutputValue::FilePath(_)
        | OutputValue::Url(_)
        | OutputValue::EmbeddedView(_) => Ok(output_value_text(value)),
    }
}

fn primary_output_value(success: &ToolSuccess) -> Option<&OutputValue> {
    success
        .primary_output_id
        .as_deref()
        .and_then(|primary| success.outputs.iter().find(|entry| entry.id == primary))
        .or_else(|| success.outputs.first())
        .map(|entry| &entry.value)
}

fn file_value_summary(file: &FileValue) -> String {
    let mut summary = String::new();
    push_bounded_component(
        &mut summary,
        if file.name.is_empty() {
            "file"
        } else {
            &file.name
        },
    );
    summary.push_str(SUMMARY_SEPARATOR);

    match &file.content {
        FileContent::Bytes(bytes) => {
            if let Some(mime) = file.mime.as_deref().filter(|mime| !mime.is_empty()) {
                push_bounded_component(&mut summary, mime);
                summary.push_str(SUMMARY_SEPARATOR);
            }
            let _ = write!(summary, "{} bytes", bytes.len());
        }
        FileContent::Directory(entries) => {
            let _ = write!(
                summary,
                "directory{SUMMARY_SEPARATOR}{} entries",
                entries.len()
            );
        }
    }
    summary
}

fn push_bounded_component(output: &mut String, component: &str) {
    if component.len() <= FILE_SUMMARY_COMPONENT_MAX_BYTES {
        output.push_str(component);
        return;
    }

    let byte_budget = FILE_SUMMARY_COMPONENT_MAX_BYTES - TRUNCATION_MARKER.len();
    let mut end = 0;
    for (index, character) in component.char_indices() {
        let next = index + character.len_utf8();
        if next > byte_budget {
            break;
        }
        end = next;
    }
    output.push_str(component.get(..end).unwrap_or_default());
    output.push_str(TRUNCATION_MARKER);
}
