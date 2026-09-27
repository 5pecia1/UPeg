//! Shared dispatch implementation for browser and native toolkit guests.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        reason = "tests use fixture assertions"
    )
)]

use serde::Deserialize;
use upeg_core::{ToolError, ToolFailure, ToolResult};

pub const TOOLKIT_ID: &str = env!("UPEG_TOOLKIT_ID");

#[derive(Deserialize)]
pub struct Request {
    pub abi_digest: String,
    pub tool_id: String,
    pub args: serde_json::Value,
}

fn error(code: &str, message: impl Into<String>) -> ToolResult {
    ToolResult::Failure(ToolFailure {
        error: ToolError {
            code: code.to_owned(),
            message: message.into(),
            details: None,
        },
    })
}

pub fn embedded_abi_digest() -> Result<String, String> {
    Ok(env!("UPEG_GUEST_ABI_DIGEST").to_owned())
}

pub fn run_result(request: Request) -> ToolResult {
    let expected_abi = match embedded_abi_digest() {
        Ok(value) => value,
        Err(message) => return error("guest_metadata", message),
    };
    if request.abi_digest != expected_abi {
        return error("abi_mismatch", "toolkit ABI does not match the shell");
    }
    if !request.tool_id.starts_with(TOOLKIT_ID)
        || request.tool_id.as_bytes().get(TOOLKIT_ID.len()) != Some(&b'.')
    {
        return error("wrong_toolkit", "tool id belongs to a different toolkit");
    }
    match upeg_tools::dispatch_registered(&request.tool_id, &request.args) {
        upeg_tools::RegisteredDispatch::Ran(result) => result,
        upeg_tools::RegisteredDispatch::NotFound => error(
            "tool_not_found",
            format!("tool `{}` is not registered", request.tool_id),
        ),
        upeg_tools::RegisteredDispatch::Unimplemented => error(
            "dispatch_unimplemented",
            format!("tool `{}` has no dispatcher", request.tool_id),
        ),
    }
}

pub fn run(request: Request) -> String {
    run_result(request).to_canonical_json().to_string()
}

pub fn dispatch_json(tool_id: &str, args_json: &str, abi: &str) -> String {
    let args = match serde_json::from_str(args_json) {
        Ok(value) => value,
        Err(parse_error) => {
            return error("invalid_args_json", parse_error.to_string())
                .to_canonical_json()
                .to_string();
        }
    };
    run(Request {
        abi_digest: abi.to_owned(),
        tool_id: tool_id.to_owned(),
        args,
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn abi_digest() -> String {
    embedded_abi_digest().unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn dispatch(tool_id: &str, args_json: &str) -> String {
    let abi = abi_digest();
    dispatch_json(tool_id, args_json, &abi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_cross_toolkit_call() {
        let abi = embedded_abi_digest().expect("metadata digest");
        let result: serde_json::Value =
            serde_json::from_str(&dispatch_json("text.trim", "{}", &abi)).expect("JSON");
        assert_eq!(result["error"]["code"], "wrong_toolkit");
    }

    #[test]
    fn compiled_abi_matches_the_host_catalog() {
        let snapshot =
            serde_json::from_str(upeg_toolkit_catalog::EMBEDDED_METADATA).expect("snapshot parses");
        let expected = upeg_toolkit_catalog::abi_digest(&snapshot).expect("digest");
        assert_eq!(embedded_abi_digest().expect("guest ABI"), expected);
    }
}
