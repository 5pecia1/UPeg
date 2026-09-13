use upeg_core::{Invoker, OutputValue, ToolMeta, ToolResult, validate_file_output_tree};

use super::{OUTPUT_CONVERSION_ERROR_CODE, tool_failure};

pub(super) const fn invoker_requires_output_budget(invoker: Invoker) -> bool {
    match invoker {
        Invoker::Function | Invoker::Static => false,
        Invoker::External
        | Invoker::Http
        | Invoker::Embed
        | Invoker::Chain
        | Invoker::Llm
        | Invoker::Wasm => true,
    }
}

pub(super) fn enforce_untrusted_output_budget(
    meta: Option<&ToolMeta>,
    result: ToolResult,
) -> ToolResult {
    if !meta.is_some_and(|meta| invoker_requires_output_budget(meta.invoker)) {
        return result;
    }
    let ToolResult::Success(success) = &result else {
        return result;
    };
    for entry in &success.outputs {
        if let OutputValue::File(file) = &entry.value
            && let Err(error) = validate_file_output_tree(file)
        {
            return tool_failure(OUTPUT_CONVERSION_ERROR_CODE, error.to_string());
        }
    }
    result
}
