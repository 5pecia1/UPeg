mod dispatcher;
mod dispatcher_chain;
mod dispatcher_chain_nested;
mod dispatcher_cycles;
mod execution_requirements;
mod file_input_policy;
mod loader;
mod parse_boards;
mod parse_core;
mod parse_validation;
mod presentation;

fn controlled_embed_backend_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn single_tool_toml(flat_tool: &str) -> Result<String, crate::LoadError> {
    let mut tool: toml::Table = toml::from_str(flat_tool).map_err(crate::LoadError::Toml)?;
    if tool.contains_key("tools") {
        return Ok(flat_tool.to_string());
    }

    let id = tool
        .get("id")
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let toolkit = tool
        .remove("toolkit")
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();

    if let Some(full_id) = id {
        let local_id = upeg_core::ToolId::parse_canonical_in_toolkit(&full_id, &toolkit)
            .map(|identity| identity.local().to_string())
            .unwrap_or(full_id);
        tool.insert("id".into(), toml::Value::String(local_id));
    }
    tool.entry("pegboard_units")
        .or_insert_with(|| toml::Value::String("U1".into()));

    let mut root = toml::Table::new();
    root.insert("id".into(), toml::Value::String(toolkit));
    root.insert(
        "tools".into(),
        toml::Value::Array(vec![toml::Value::Table(tool)]),
    );
    Ok(render_single_tool_toml(root))
}

fn render_single_tool_toml(mut root: toml::Table) -> String {
    use std::fmt::Write as _;

    let tools = root.remove("tools").expect("test helper always sets tools");
    let mut out = String::new();
    for (key, value) in root {
        writeln!(&mut out, "{key} = {value}").expect("write to string");
    }
    out.push_str("\n[[tools]]\n");
    let toml::Value::Array(mut tools) = tools else {
        unreachable!("test helper always sets tools array");
    };
    let toml::Value::Table(tool) = tools.remove(0) else {
        unreachable!("test helper always sets one tool table");
    };
    for (key, value) in tool {
        writeln!(&mut out, "{key} = {value}").expect("write to string");
    }
    out
}

fn single_tool_toml_str(flat_tool: &str) -> String {
    single_tool_toml(flat_tool).expect("valid v2.1 test fixture")
}

fn call_dispatcher_result<F>(f: &F, value: serde_json::Value) -> upeg_core::ToolResult
where
    F: for<'a> Fn(upeg_runtime::DispatchArgs<'a>) -> upeg_core::ToolResult,
{
    f(upeg_runtime::DispatchArgs::parse(&value).expect("test args must be an object"))
}

fn call_dispatcher<F>(f: &F, value: serde_json::Value) -> Result<String, String>
where
    F: for<'a> Fn(upeg_runtime::DispatchArgs<'a>) -> upeg_core::ToolResult,
{
    upeg_runtime::tool_result_text(call_dispatcher_result(f, value))
}

fn runtime_success_text(result: Option<upeg_core::ToolResult>) -> String {
    match result {
        Some(upeg_core::ToolResult::Success(success)) => {
            upeg_runtime::tool_success_primary_text(&success)
        }
        Some(upeg_core::ToolResult::Failure(failure)) => {
            panic!("expected success, got failure: {:?}", failure.error)
        }
        None => panic!("expected registered dispatcher"),
    }
}

fn tool_success(result: upeg_core::ToolResult) -> upeg_core::ToolSuccess {
    match result {
        upeg_core::ToolResult::Success(success) => success,
        upeg_core::ToolResult::Failure(failure) => {
            panic!("expected success, got failure: {:?}", failure.error)
        }
    }
}

fn runtime_failure_message(result: Option<upeg_core::ToolResult>) -> String {
    match result {
        Some(upeg_core::ToolResult::Failure(failure)) => failure.error.message,
        Some(upeg_core::ToolResult::Success(success)) => {
            panic!("expected failure, got success: {success:?}")
        }
        None => panic!("expected registered dispatcher"),
    }
}

fn parse_single_tool(flat_tool: &str) -> Result<upeg_core::ToolMeta, crate::LoadError> {
    let (_toolkit, tools) = crate::parse_toolkit_full(&single_tool_toml(flat_tool)?)?;
    let [(meta, _toml)] = tools.as_slice() else {
        panic!("single-tool test fixture produced {} tools", tools.len());
    };
    Ok(meta.clone())
}

fn parse_full_single_tool(
    flat_tool: &str,
) -> Result<(upeg_core::ToolMeta, crate::ToolToml), crate::LoadError> {
    let (_toolkit, mut tools) = crate::parse_toolkit_full(&single_tool_toml(flat_tool)?)?;
    assert!(
        tools.len() == 1,
        "single-tool test fixture produced {} tools",
        tools.len()
    );
    Ok(tools.pop().expect("one tool"))
}
