#![allow(
    clippy::expect_used,
    reason = "infallible at construction (compile-time static ids) or fatal on failure (poisoned mutex)"
)]

use std::sync::atomic::{AtomicU64, Ordering};
use upeg_core::{
    FileValue, OutputEntry, OutputKind, OutputValue, ToolError, ToolFailure, ToolId, ToolResult,
    ToolSuccess, preflight_file_output_json,
};

use crate::output_text::tool_success_primary_text;
use crate::toolbox_tool;
use output_budget::{enforce_untrusted_output_budget, invoker_requires_output_budget};

const DEFAULT_TEXT_OUTPUT_ID: &str = "result";
const TOOL_ERROR_CODE: &str = "tool_error";
/// Canonical error code for "the caller's arguments are wrong", as
/// opposed to "the tool ran and failed". Public so invoker dispatchers
/// outside this crate classify their own argument rejections the same
/// way the shared validation path does.
pub const INVALID_ARGS_CODE: &str = "invalid_args";
const OUTPUT_CONVERSION_ERROR_CODE: &str = "output_conversion_error";

mod output_budget;

/// Borrowed, validated Tool invocation arguments.
#[derive(Clone, Copy, Debug)]
pub struct DispatchArgs<'a> {
    value: &'a serde_json::Value,
    object: &'a serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DispatchArgsError {
    #[error("dispatch args must be a JSON object, got {kind}")]
    NotObject { kind: &'static str },
}

impl<'a> DispatchArgs<'a> {
    pub fn parse(value: &'a serde_json::Value) -> Result<Self, DispatchArgsError> {
        let Some(object) = value.as_object() else {
            return Err(DispatchArgsError::NotObject {
                kind: json_value_kind(value),
            });
        };
        Ok(Self { value, object })
    }

    pub fn get(&self, key: &str) -> Option<&'a serde_json::Value> {
        self.object.get(key)
    }

    pub const fn as_object(&self) -> &'a serde_json::Map<String, serde_json::Value> {
        self.object
    }

    pub const fn as_value(&self) -> &'a serde_json::Value {
        self.value
    }
}

const fn json_value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

impl std::fmt::Display for DispatchArgs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

type ToolDispatcher = std::sync::Arc<dyn for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync>;

#[derive(Clone)]
struct RuntimeDispatcherEntry {
    dispatcher: ToolDispatcher,
    generation: u64,
}

static RUNTIME_DISPATCHERS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, RuntimeDispatcherEntry>>,
> = std::sync::OnceLock::new();

fn dispatchers_lock()
-> &'static std::sync::Mutex<std::collections::HashMap<&'static str, RuntimeDispatcherEntry>> {
    RUNTIME_DISPATCHERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

static NEXT_RUNTIME_DISPATCHER_GENERATION: AtomicU64 = AtomicU64::new(1);

fn canonical_tool_id(id: &str) {
    ToolId::parse_canonical(id).expect("tool id must be canonical and unpadded");
}

pub fn register_runtime_dispatcher<F>(id: &'static str, f: F)
where
    F: for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static,
{
    let _ = insert_runtime_dispatcher(id, f);
}

pub fn register_single_text_runtime_dispatcher<F>(id: &'static str, f: F)
where
    F: for<'a> Fn(DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    register_runtime_dispatcher(id, single_text_dispatcher(id, f));
}

#[derive(Debug)]
pub struct RuntimeDispatcherRegistration {
    id: &'static str,
    generation: u64,
}

impl Drop for RuntimeDispatcherRegistration {
    fn drop(&mut self) {
        let Ok(mut guard) = dispatchers_lock().lock() else {
            return;
        };
        if guard
            .get(self.id)
            .is_some_and(|entry| entry.generation == self.generation)
        {
            guard.remove(self.id);
        }
    }
}

pub fn register_runtime_dispatcher_managed<F>(
    id: &'static str,
    f: F,
) -> RuntimeDispatcherRegistration
where
    F: for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static,
{
    let generation = insert_runtime_dispatcher(id, f);
    RuntimeDispatcherRegistration { id, generation }
}

pub fn register_single_text_runtime_dispatcher_managed<F>(
    id: &'static str,
    f: F,
) -> RuntimeDispatcherRegistration
where
    F: for<'a> Fn(DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    register_runtime_dispatcher_managed(id, single_text_dispatcher(id, f))
}

fn insert_runtime_dispatcher<F>(id: &'static str, f: F) -> u64
where
    F: for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static,
{
    canonical_tool_id(id);
    let generation = NEXT_RUNTIME_DISPATCHER_GENERATION.fetch_add(1, Ordering::Relaxed);
    dispatchers_lock()
        .lock()
        .expect("runtime dispatcher registry poisoned")
        .insert(
            id,
            RuntimeDispatcherEntry {
                dispatcher: std::sync::Arc::new(f),
                generation,
            },
        );
    generation
}

/// Whether a runtime dispatcher is currently registered for `id`, without
/// running it. Used by the capability layer to distinguish a `Function`
/// tool whose dispatcher is compiled out on this host (native-only) from
/// one that is genuinely runnable here.
pub fn has_runtime_dispatcher(id: &str) -> bool {
    dispatchers_lock()
        .lock()
        .expect("runtime dispatcher registry poisoned")
        .contains_key(id)
}

pub fn try_runtime_dispatch(id: &str, args: &serde_json::Value) -> Option<ToolResult> {
    let f = {
        let guard = dispatchers_lock()
            .lock()
            .expect("runtime dispatcher registry poisoned");
        guard
            .get(id)
            .map(|entry| std::sync::Arc::clone(&entry.dispatcher))
    };
    f.map(|f| match DispatchArgs::parse(args) {
        Ok(args) => {
            let meta = toolbox_tool(id);
            if let Some(meta) = meta
                && let Err(error) = meta.input_spec.validate_json_args(args.as_object())
            {
                return tool_failure(INVALID_ARGS_CODE, error.to_string());
            }
            enforce_untrusted_output_budget(meta, f(args))
        }
        Err(error) => tool_failure(INVALID_ARGS_CODE, error.to_string()),
    })
}

pub fn single_text_dispatcher<F>(
    id: &'static str,
    f: F,
) -> impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static
where
    F: for<'a> Fn(DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    move |args| single_text_result(id, f(args))
}

pub fn single_text_result(id: &str, result: Result<String, String>) -> ToolResult {
    match result {
        Ok(text) => single_text_success(id, text)
            .unwrap_or_else(|error| tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error)),
        Err(message) => tool_failure(TOOL_ERROR_CODE, message),
    }
}

pub fn tool_result_text(result: ToolResult) -> Result<String, String> {
    match result {
        ToolResult::Success(success) => Ok(tool_success_primary_text(&success)),
        ToolResult::Failure(failure) => Err(failure.error.message),
    }
}

#[must_use]
pub fn tool_failure(code: impl Into<String>, message: impl Into<String>) -> ToolResult {
    ToolResult::Failure(ToolFailure {
        error: ToolError {
            code: code.into(),
            message: message.into(),
            details: None,
        },
    })
}

/// [`tool_failure`] plus a structured `details` payload.
///
/// `details` is the canonical envelope's extension point: the External
/// invoker uses it to ship the child's exit code and both captured
/// streams so agents and operators get the diagnostics instead of a
/// one-line summary.
#[must_use]
pub fn tool_failure_with_details(
    code: impl Into<String>,
    message: impl Into<String>,
    details: serde_json::Value,
) -> ToolResult {
    ToolResult::Failure(ToolFailure {
        error: ToolError {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        },
    })
}

fn single_text_success(id: &str, text: String) -> Result<ToolResult, String> {
    let Some(meta) = toolbox_tool(id) else {
        return Ok(ToolResult::Success(fallback_text_success(text)));
    };

    match meta.output_spec.fields.as_slice() {
        [] => Ok(ToolResult::Success(fallback_text_success(text))),
        [field] => {
            let value = output_value_from_text(
                &field.kind,
                &text,
                invoker_requires_output_budget(meta.invoker),
            )?;
            let entry = OutputEntry {
                id: field.name.clone(),
                label: field.label.clone(),
                kind: field.kind.clone(),
                value,
            };
            ToolSuccess::new(
                meta.primary_output_id
                    .map(str::to_string)
                    .or_else(|| Some(field.name.clone())),
                vec![entry],
            )
            .map(ToolResult::Success)
            .map_err(|error| error.to_string())
        }
        fields => Err(format!(
            "single text dispatcher for `{id}` cannot fill {} output fields",
            fields.len()
        )),
    }
}

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

fn output_value_from_text(
    kind: &OutputKind,
    text: &str,
    enforce_file_budget: bool,
) -> Result<OutputValue, String> {
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
            let value = serde_json::from_str::<serde_json::Value>(text)
                .map_err(|error| format!("output text is not a file JSON object: {error}"))?;
            if enforce_file_budget {
                preflight_file_output_json(&value)
                    .map_err(|error| format!("output file failed resource preflight: {error}"))?;
            }
            serde_json::from_value::<FileValue>(value)
                .map(OutputValue::File)
                .map_err(|error| format!("output text is not a file JSON object: {error}"))
        }
        OutputKind::EmbeddedView { .. } => Ok(OutputValue::EmbeddedView(text.to_string())),
    }
}

#[cfg(test)]
#[path = "dispatch_output_budget_tests.rs"]
mod output_budget_tests;

#[cfg(test)]
#[path = "dispatch_text_tests.rs"]
mod text_tests;
