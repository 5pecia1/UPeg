//! The Chain invoker's output layer: turning a step's text or canonical
//! success into the shape the *chain* promised.
//!
//! Split out of `chain.rs` so the execution engine (ordering, approval,
//! step metadata) and the output contract (declared `outputs`, primary
//! selection, typed conversion) each read on their own. The other
//! invokers in `dispatcher.rs` reuse [`OutputAdapter`] through the
//! parent module's re-export, which is why its items stay
//! `pub(super)`-visible from `chain`.

use crate::ToolToml;
use crate::dispatcher::{
    DEFAULT_TEXT_OUTPUT_ID, OUTPUT_CONVERSION_ERROR_CODE, STDERR_OUTPUT_ID, TOOL_ERROR_CODE,
};
use serde_json::Value;
use std::collections::BTreeMap;
use upeg_core::{
    FileValue, OutputEntry, OutputFieldSpec, OutputKind, OutputValue, ToolResult, ToolSuccess,
    preflight_file_output_json,
};
use upeg_runtime::tool_success_primary_canonical_wire_text;

#[derive(Clone, Debug)]
pub(crate) struct OutputAdapter {
    tool_id: String,
    fields: Result<Vec<OutputFieldSpec>, String>,
    primary_output_id: Option<String>,
}

impl OutputAdapter {
    pub(crate) fn from_meta(meta: &upeg_core::ToolMeta) -> Self {
        Self {
            tool_id: meta.id.to_string(),
            fields: Ok(meta.output_spec.fields.clone()),
            primary_output_id: meta.primary_output_id.map(str::to_string),
        }
    }

    pub(crate) fn from_tool(parsed: &ToolToml) -> Self {
        let fields = parsed
            .outputs
            .clone()
            .into_iter()
            .map(OutputFieldSpec::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string());
        Self {
            tool_id: parsed.id.trim().to_string(),
            fields,
            primary_output_id: parsed.primary_output_id.clone(),
        }
    }

    pub(crate) fn text_result(&self, result: Result<String, String>) -> ToolResult {
        match result {
            Ok(text) => self.success_from_text(text).unwrap_or_else(|error| {
                upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error)
            }),
            Err(message) => upeg_runtime::tool_failure(TOOL_ERROR_CODE, message),
        }
    }

    /// External success shape: `text` is the primary output, and a
    /// non-empty `stderr` rides along as a secondary `stderr` output.
    ///
    /// Only tools that declare no `outputs` of their own get the extra
    /// entry — a tool with a declared output contract keeps exactly the
    /// shape it promised, so nothing downstream sees a surprise field.
    pub(crate) fn text_result_with_stderr(&self, text: String, stderr: &str) -> ToolResult {
        if stderr.is_empty() || self.has_declared_outputs() {
            return self.text_result(Ok(text));
        }
        ToolSuccess::new(
            Some(DEFAULT_TEXT_OUTPUT_ID.to_string()),
            vec![
                text_output_entry(DEFAULT_TEXT_OUTPUT_ID, text),
                text_output_entry(STDERR_OUTPUT_ID, stderr.to_string()),
            ],
        )
        .map(ToolResult::Success)
        .unwrap_or_else(|error| {
            upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error.to_string())
        })
    }

    pub(crate) fn named_text_result(
        &self,
        result: Result<Vec<(String, String)>, String>,
    ) -> ToolResult {
        match result {
            Ok(outputs) => self
                .success_from_named_text(outputs)
                .unwrap_or_else(|error| {
                    upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error)
                }),
            Err(message) => upeg_runtime::tool_failure(TOOL_ERROR_CODE, message),
        }
    }

    pub(crate) fn adapt_delegate_result(&self, result: ToolResult) -> ToolResult {
        if !self.has_declared_outputs() {
            return result;
        }
        match result {
            ToolResult::Success(success) => self.adapt_final_success(success),
            failure @ ToolResult::Failure(_) => failure,
        }
    }

    pub(crate) fn adapt_final_success(&self, mut success: ToolSuccess) -> ToolResult {
        match self.take_primary_file_value(&mut success) {
            Ok(Some((field, value))) => {
                let entry = OutputEntry {
                    id: field.name.clone(),
                    label: field.label.clone(),
                    kind: field.kind,
                    value,
                };
                ToolSuccess::new(Some(field.name), vec![entry])
                    .map(ToolResult::Success)
                    .unwrap_or_else(|error| {
                        upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error.to_string())
                    })
            }
            Ok(None) => self.text_result(
                tool_success_primary_canonical_wire_text(&success)
                    .map_err(|error| error.to_string()),
            ),
            Err(error) => upeg_runtime::tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error),
        }
    }

    fn take_primary_file_value(
        &self,
        success: &mut ToolSuccess,
    ) -> Result<Option<(OutputFieldSpec, OutputValue)>, String> {
        let fields = self.fields.as_ref().map_err(Clone::clone)?;
        let Some(field) = self.primary_field(fields)?.cloned() else {
            return Ok(None);
        };
        if field.kind != OutputKind::File {
            return Ok(None);
        }
        let selected = success
            .primary_output_id
            .as_deref()
            .and_then(|primary| success.outputs.iter().position(|entry| entry.id == primary))
            .or_else(|| (!success.outputs.is_empty()).then_some(0));
        let Some(index) = selected else {
            return Ok(None);
        };
        if !matches!(success.outputs[index].value, OutputValue::File(_)) {
            return Ok(None);
        }
        let entry = success.outputs.swap_remove(index);
        Ok(Some((field, entry.value)))
    }

    fn success_from_text(&self, text: String) -> Result<ToolResult, String> {
        let fields = self.fields.as_ref().map_err(Clone::clone)?;
        let Some(field) = self.primary_field(fields)? else {
            return Ok(ToolResult::Success(fallback_text_success(text)));
        };
        let entry = entry_from_text(field, text)?;
        ToolSuccess::new(Some(entry.id.clone()), vec![entry])
            .map(ToolResult::Success)
            .map_err(|error| error.to_string())
    }

    fn success_from_named_text(
        &self,
        outputs: Vec<(String, String)>,
    ) -> Result<ToolResult, String> {
        let fields = self.fields.as_ref().map_err(Clone::clone)?;
        if fields.is_empty() {
            let value = named_outputs_json_text(outputs)?;
            return Ok(ToolResult::Success(fallback_text_success(value)));
        }

        let by_id = outputs.into_iter().collect::<BTreeMap<_, _>>();
        let entries = fields
            .iter()
            .filter_map(|field| {
                by_id
                    .get(&field.name)
                    .map(|text| entry_from_text(field, text.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let primary = self
            .primary_output_id
            .clone()
            .ok_or_else(|| format!("primary_output_id required for `{}`", self.tool_id))?;
        ToolSuccess::new(Some(primary), entries)
            .map(ToolResult::Success)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn has_declared_outputs(&self) -> bool {
        self.fields.as_ref().is_ok_and(|fields| !fields.is_empty())
    }

    fn primary_field<'a>(
        &self,
        fields: &'a [OutputFieldSpec],
    ) -> Result<Option<&'a OutputFieldSpec>, String> {
        if fields.is_empty() {
            return Ok(None);
        }
        let primary = self
            .primary_output_id
            .as_deref()
            .or_else(|| (fields.len() == 1).then_some(fields[0].name.as_str()))
            .ok_or_else(|| format!("primary_output_id required for `{}`", self.tool_id))?;
        fields
            .iter()
            .find(|field| field.name == primary)
            .ok_or_else(|| format!("primary_output_id `{primary}` does not reference an output"))
            .map(Some)
    }
}

fn entry_from_text(field: &OutputFieldSpec, text: String) -> Result<OutputEntry, String> {
    Ok(OutputEntry {
        id: field.name.clone(),
        label: field.label.clone(),
        kind: field.kind.clone(),
        value: output_value_from_text(&field.kind, &text)?,
    })
}

fn text_output_entry(id: &str, text: String) -> OutputEntry {
    OutputEntry {
        id: id.to_string(),
        label: None,
        kind: OutputKind::String,
        value: OutputValue::String(text),
    }
}

#[allow(
    clippy::expect_used,
    reason = "DEFAULT_TEXT_OUTPUT_ID always present in outputs; panic is the contract"
)]
fn fallback_text_success(text: String) -> ToolSuccess {
    ToolSuccess::new(
        Some(DEFAULT_TEXT_OUTPUT_ID.to_string()),
        vec![OutputEntry {
            id: DEFAULT_TEXT_OUTPUT_ID.to_string(),
            label: None,
            kind: OutputKind::String,
            value: OutputValue::String(text),
        }],
    )
    .expect("fallback text success has matching primary output id")
}

fn named_outputs_json_text(outputs: Vec<(String, String)>) -> Result<String, String> {
    let outputs_map = outputs
        .into_iter()
        .map(|(k, v): (String, String)| (k, serde_json::Value::String(v)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    serde_json::to_string(&serde_json::Value::Object(outputs_map)).map_err(|e| e.to_string())
}

pub(crate) fn output_value_from_text(kind: &OutputKind, text: &str) -> Result<OutputValue, String> {
    let trimmed = text.trim();
    match kind {
        OutputKind::String => Ok(OutputValue::String(text.to_string())),
        OutputKind::Number => match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(serde_json::Value::Number(number)) => Ok(OutputValue::Number(number)),
            _ => Err(format!("output text `{text}` is not a JSON number")),
        },
        OutputKind::Integer => trimmed
            .parse::<i64>()
            .map(OutputValue::Integer)
            .map_err(|_| format!("output text `{text}` is not an integer")),
        OutputKind::Boolean => trimmed
            .parse::<bool>()
            .map(OutputValue::Boolean)
            .map_err(|_| format!("output text `{text}` is not a boolean")),
        OutputKind::Options(_) => Ok(OutputValue::Options(text.to_string())),
        OutputKind::MultiOptions(_) => serde_json::from_str::<Vec<String>>(trimmed)
            .map(OutputValue::MultiOptions)
            .map_err(|_| format!("output text `{text}` is not a JSON string array")),
        OutputKind::Markdown => Ok(OutputValue::Markdown(text.to_string())),
        OutputKind::Json => serde_json::from_str::<serde_json::Value>(text)
            .map(OutputValue::Json)
            .map_err(|error| format!("output text is not valid JSON: {error}")),
        OutputKind::DateTime => Ok(OutputValue::DateTime(text.to_string())),
        OutputKind::FilePath => Ok(OutputValue::FilePath(text.to_string())),
        OutputKind::Url => Ok(OutputValue::Url(text.to_string())),
        OutputKind::File => {
            let value = serde_json::from_str::<Value>(text)
                .map_err(|error| format!("output text is not a file JSON object: {error}"))?;
            preflight_file_output_json(&value)
                .map_err(|error| format!("output file failed resource preflight: {error}"))?;
            serde_json::from_value::<FileValue>(value)
                .map(OutputValue::File)
                .map_err(|error| format!("output text is not a file JSON object: {error}"))
        }
        OutputKind::EmbeddedView { .. } => Ok(OutputValue::EmbeddedView(text.to_string())),
    }
}
