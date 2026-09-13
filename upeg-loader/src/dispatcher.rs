use crate::ToolToml;
use crate::model::KeyValueToml;
use serde_json::json;
use upeg_core::ToolResult;
use upeg_runtime::{DispatchArgs, tool_success_primary_text};

/// Environment variable that carries the active Board key into External
/// tool subprocesses. Surfaced as a const so both this dispatcher and any
/// downstream consumer (CLI/desktop env catalog) reference one name.
pub const BOARD_ENV: &str = "UPEG_BOARD";

/// Environment variable that carries the auto-detected project manifest
/// path into External tool subprocesses.
pub const PROJECT_MANIFEST_ENV: &str = "UPEG_PROJECT_MANIFEST";

const TOOL_ERROR_CODE: &str = "tool_error";
/// Canonical `error.code` for a run that was stopped on request — the
/// ambient [`upeg_runtime::CancellationToken`] fired while a child was
/// still running. Distinct from [`TOOL_ERROR_CODE`] because "you asked
/// me to stop" is not a diagnosis of the tool.
const CANCELLED_ERROR_CODE: &str = "cancelled";
const OUTPUT_CONVERSION_ERROR_CODE: &str = "output_conversion_error";
const DEFAULT_TEXT_OUTPUT_ID: &str = "result";
/// Secondary output id used to surface a successful command's stderr
/// when the tool declares no outputs of its own. Progress and warning
/// text (`cargo`'s compile log, `git`'s hints) is real output; dropping
/// it on the floor made successful runs look emptier than they were.
const STDERR_OUTPUT_ID: &str = "stderr";

/// Rebuild pointer appended to the `Wasm` invoker's stub-build error
/// (see `wasm_dispatcher_for`'s `#[cfg(not(feature = "wasm"))]` arm).
/// upeg-cli exposes this crate's `wasm` feature as its own `wasm-plugin`
/// feature, so the pointer names the CLI-facing flag users actually pass.
/// `cfg`-gated with its only call site so builds with `wasm` enabled
/// don't warn on dead code.
#[cfg(not(feature = "wasm"))]
const WASM_STUB_REBUILD_HINT: &str = "rebuild with --features wasm-plugin (see README)";

mod chain;
mod credentials;
pub(crate) mod external;
mod http;
mod llm;

// `chain` owns the Chain invoker plus the shared output/template helpers the
// other invokers in this module reuse. Re-export the surface loader.rs and the
// test modules reference as `crate::dispatcher::{...}`, and pull the helpers
// this module's other dispatchers call into local scope.
pub(crate) use chain::chain_dispatcher_for;
pub(crate) use chain::{
    ApprovalSurfaces, ApprovalSurfacesError, CHAIN_STEPS_OUTPUT_ID, DEFAULT_APPROVAL_SURFACES,
    surface_label_list,
};
pub(crate) use external::external_dispatcher_for;
// `MAX_CHAIN_DEPTH` is referenced only by the cycle regression tests
// (`crate::dispatcher::MAX_CHAIN_DEPTH`); gate the re-export so non-test
// builds don't warn on an unused import.
#[cfg(test)]
pub(crate) use chain::MAX_CHAIN_DEPTH;
use chain::{OutputAdapter, render_template};

pub(crate) fn http_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Http") {
        return None;
    }
    let url = parsed.url.clone()?.trim().to_string();
    if url.is_empty() {
        return None;
    }
    let method = parsed.method.clone();
    let headers = parsed.headers.clone().unwrap_or_default();
    let body = parsed.body.clone();
    let credentials = parsed.credentials.clone().unwrap_or_default();
    let primary_credential = parsed.credential.clone();
    let output_adapter = OutputAdapter::from_tool(parsed);

    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        let run = || -> Result<String, String> {
            let creds = credentials::credential_map(&credentials, primary_credential.as_deref())?;
            let url = render_template(&url, args.as_value(), None, &creds)?;
            let body = body
                .as_deref()
                .map(|template| render_template(template, args.as_value(), None, &creds))
                .transpose()?;
            let method = method
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_uppercase)
                .unwrap_or_else(|| {
                    if body.is_some() {
                        "POST".to_string()
                    } else {
                        "GET".to_string()
                    }
                });
            let mut rendered_headers = Vec::new();
            for KeyValueToml { name, value } in &headers {
                rendered_headers.push((
                    name.trim().to_string(),
                    render_template(value, args.as_value(), None, &creds)?,
                ));
            }
            http::run_http_request(&method, &url, rendered_headers, body)
        };
        output_adapter.text_result(run())
    })
}

pub(crate) fn llm_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Llm") {
        return None;
    }
    let prompt = parsed.prompt.clone()?.trim().to_string();
    if prompt.is_empty() {
        return None;
    }
    let provider_raw = parsed.provider.clone().unwrap_or_default();
    let model = parsed.model.clone().unwrap_or_default();
    let base_url = parsed
        .base_url
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| llm::OPENAI_DEFAULT_BASE_URL.to_string());
    let credentials = parsed.credentials.clone().unwrap_or_default();
    let primary_credential = parsed.credential.clone();
    let output_adapter = OutputAdapter::from_tool(parsed);

    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        let run_text = || -> Result<String, String> {
            let creds = credentials::credential_map(&credentials, primary_credential.as_deref())?;
            let rendered = render_template(&prompt, args.as_value(), None, &creds)?;
            let provider: llm::ProviderKind = provider_raw
                .parse()
                .map_err(|error: llm::ProviderKindParseError| error.to_string())?;
            match provider {
                llm::ProviderKind::Echo => Ok(rendered),
                llm::ProviderKind::ToolDelegate(tool_id) => {
                    dispatch_llm_delegate_tool(tool_id.as_str(), &model, &rendered, &output_adapter)
                }
                llm::ProviderKind::OpenAiCompatible => {
                    let api_key = primary_credential
                        .as_deref()
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .and_then(|name| creds.get(name))
                        .cloned()
                        .ok_or_else(|| llm::OpenAiProviderError::MissingCredential.to_string())?;
                    llm::run_openai_chat_completion(llm::OpenAiRequest {
                        base_url: &base_url,
                        model: &model,
                        api_key: &api_key,
                        prompt: &rendered,
                    })
                    .map_err(|error| error.to_string())
                }
            }
        };
        output_adapter.text_result(run_text())
    })
}

/// Forward the rendered LLM prompt into another registered Tool
/// (`provider = "tool:<id>"`) and normalize its result back to text.
fn dispatch_llm_delegate_tool(
    tool_id: &str,
    model: &str,
    rendered_prompt: &str,
    output_adapter: &OutputAdapter,
) -> Result<String, String> {
    match upeg_runtime::try_runtime_dispatch(
        tool_id,
        &json!({
            "input": rendered_prompt,
            "model": model,
        }),
    ) {
        Some(result) => match output_adapter.adapt_delegate_result(result) {
            ToolResult::Success(success) => Ok(tool_success_primary_text(&success)),
            ToolResult::Failure(failure) => Err(format!(
                "llm provider tool `{tool_id}`: {}",
                failure.error.message
            )),
        },
        None => Err(format!("llm provider tool `{tool_id}` not found")),
    }
}

/// Lazy-load state for a `Wasm` invoker's dispatcher closure.
///
/// Hazard this guards against: the lazy closure below is registered as
/// *the* runtime dispatcher for `tool_id` before the wasm module is ever
/// loaded. If the loaded module registers a *different* id than
/// `tool_id` (toolkit TOML id vs. wasm manifest id mismatch), the
/// registry entry for `tool_id` is never overwritten — it still holds
/// this very closure. Re-entering `try_runtime_dispatch(&tool_id, ..)`
/// after loading would therefore call this closure again, forever
/// (infinite recursion / stack overflow), since `should_load` would be
/// false on every subsequent turn and nothing else short-circuits the
/// loop. See the hazard note in `upeg-frb/src/api/boot.rs`
/// (`write_toml_wasm_path_toolkit`).
///
/// The fix: check, once, which ids `upeg_wasm::load_and_register`
/// actually registered. If `tool_id` isn't among them, cache a
/// permanent error (`IdMismatch`) instead of ever calling
/// `try_runtime_dispatch(&tool_id, ..)` — so neither the first
/// dispatch nor any later one can recurse into this closure.
#[cfg(feature = "wasm")]
enum WasmLoadState {
    /// Module not loaded yet.
    Pending,
    /// Module loaded and it registered `tool_id` itself — the registry
    /// entry for `tool_id` is now the module's own dispatcher, so
    /// re-dispatching by id is safe.
    Ready,
    /// Module loaded but registered different id(s); this is the
    /// pre-formatted, permanent error for every call from here on.
    IdMismatch(String),
}

/// Error message for a `Wasm` tool whose declared id isn't among the ids
/// its module actually registered. Centralized so the wording is
/// identical however many times a mismatched dispatcher is invoked.
#[cfg(feature = "wasm")]
fn wasm_tool_id_mismatch_message(wasm_path: &str, tool_id: &str, declared: &[&str]) -> String {
    format!(
        "wasm module `{wasm_path}` did not register tool `{tool_id}`; it declares: {}",
        declared.join(", ")
    )
}

#[cfg(feature = "wasm")]
pub(crate) fn wasm_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Wasm") {
        return None;
    }
    let wasm_path = parsed.wasm_path.clone()?.trim().to_string();
    if wasm_path.is_empty() {
        return None;
    }
    let tool_id = parsed.id.trim().to_string();
    let state = std::sync::Arc::new(std::sync::Mutex::new(WasmLoadState::Pending));
    let output_adapter = OutputAdapter::from_tool(parsed);
    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        let run = || -> Result<ToolResult, String> {
            {
                let mut guard = state
                    .lock()
                    .map_err(|_| "wasm loader state poisoned".to_string())?;
                if matches!(*guard, WasmLoadState::Pending) {
                    let registered_ids =
                        upeg_wasm::load_and_register(std::path::Path::new(&wasm_path))
                            .map_err(|e| format!("wasm `{wasm_path}`: {e}"))?;
                    *guard = if registered_ids.iter().any(|id| *id == tool_id) {
                        WasmLoadState::Ready
                    } else {
                        WasmLoadState::IdMismatch(wasm_tool_id_mismatch_message(
                            &wasm_path,
                            &tool_id,
                            &registered_ids,
                        ))
                    };
                }
                // Never fall through to `try_runtime_dispatch(&tool_id, ..)`
                // on a mismatch — see `WasmLoadState`'s doc comment for why
                // that would recurse into this same closure.
                if let WasmLoadState::IdMismatch(message) = &*guard {
                    return Err(message.clone());
                }
            }
            match upeg_runtime::try_runtime_dispatch(&tool_id, args.as_value()) {
                Some(result) => Ok(output_adapter.adapt_delegate_result(result)),
                None => Err(format!(
                    "wasm module `{wasm_path}` loaded but did not register `{tool_id}`"
                )),
            }
        };
        match run() {
            Ok(result) => result,
            Err(message) => upeg_runtime::tool_failure(TOOL_ERROR_CODE, message),
        }
    })
}

#[cfg(not(feature = "wasm"))]
pub(crate) fn wasm_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Wasm") {
        return None;
    }
    let tool_id = parsed.id.trim().to_string();
    let wasm_path = parsed.wasm_path.clone().unwrap_or_default();
    Some(move |_args: DispatchArgs<'_>| -> ToolResult {
        upeg_runtime::tool_failure(
            TOOL_ERROR_CODE,
            format!(
                "wasm invoker for `{tool_id}` requires the `wasm` cargo feature \
             (configured wasm_path: `{}`) — {WASM_STUB_REBUILD_HINT}",
                wasm_path.trim()
            ),
        )
    })
}

/// Normalize raw DOM output without dispatching or touching a browser.
/// Desktop session state and ordinary dispatch use this same output contract.
pub fn normalize_controlled_embed_result(
    tool_id: &str,
    result: Result<
        upeg_runtime::controlled_embed::ControlledEmbedResponse,
        upeg_runtime::controlled_embed::ControlledEmbedError,
    >,
) -> ToolResult {
    const TOOL_NOT_FOUND_CODE: &str = "tool_not_found";
    let Some(meta) = upeg_runtime::toolbox_tool(tool_id) else {
        return upeg_runtime::tool_failure(
            TOOL_NOT_FOUND_CODE,
            format!("tool `{tool_id}` is not registered"),
        );
    };
    match result {
        Ok(response) => OutputAdapter::from_meta(meta).named_text_result(Ok(response.outputs)),
        Err(error) => upeg_runtime::tool_failure(error.code(), error.to_string()),
    }
}

/// Build a Controlled Embed dispatcher for the registered tool metadata.
/// The installed backend selects the Desktop WebView or headless browser;
/// raw DOM values use the same normalization as the Desktop session debugger.
/// An unavailable backend returns its error without selecting another browser.
pub(crate) fn controlled_embed_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Embed") {
        return None;
    }
    // Keep the canonical id owned by the `'static` dispatcher closure.
    // Runtime lookups accept `&str`, so each invocation can borrow from this
    // `String`; no `Box::leak` is needed to manufacture a static string.
    let tool_id = parsed.id.trim().to_string();
    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        // Look up runtime side-cars by canonical id — these were
        // registered by `register_parsed_toolkit` before the
        // dispatcher chain ran.
        let Some(url) = upeg_runtime::embed_url_for(&tool_id) else {
            return upeg_runtime::tool_failure(
                TOOL_ERROR_CODE,
                format!(
                    "controlled embed `{tool_id}` has no embed_url — \
                  declare it in the manifest"
                ),
            );
        };
        // Owned-aware lookup: GUI/FRB-authored bindings (owned map)
        // must win over the compile-time static bindings on every
        // headless surface (CLI/MCP/HTTP), mirroring the FRB read path
        // (`selector_bindings_owned_for`). Falls back to the static
        // registry when no owned entry exists.
        let bindings = upeg_runtime::selector_bindings_owned_for(&tool_id);
        let settings = upeg_runtime::controlled_embed_settings_owned_for(&tool_id);

        // Convert the dispatch args (JSON object) into the
        // `&[(&str, &str)]` shape the backend expects. JSON values
        // are stringified — number/bool inputs go in as their JSON
        // representation, which is what the page's DOM would see if
        // a user typed them anyway.
        let args_obj = args.as_object();
        let owned_pairs: Vec<(String, String)> = args_obj
            .iter()
            .map(|(k, v): (&String, &serde_json::Value)| {
                let stringified = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (k.clone(), stringified)
            })
            .collect();
        let pair_refs: Vec<(&str, &str)> = owned_pairs
            .iter()
            .map(|(k, v): &(String, String)| (k.as_str(), v.as_str()))
            .collect();

        let backend = upeg_runtime::controlled_embed::controlled_embed_backend();
        let response = backend.run(upeg_runtime::controlled_embed::ControlledEmbedRequest {
            tool_id: &tool_id,
            url,
            bindings: &bindings,
            inputs: &pair_refs,
            settings,
        });
        normalize_controlled_embed_result(&tool_id, response)
    })
}

/// Build a runtime dispatcher for Passive Embed Tools (`pin = Embed`,
/// `invoker = Static`).
///
/// Passive Embed has no app-driven invocation — the webview is the
/// tool. Headless callers get a friendly explanation instead of a
/// generic "dispatch not implemented" so authors and operators can
/// see *why* their CLI/MCP/HTTP call didn't run.
pub(crate) fn static_dispatcher_for(
    parsed: &ToolToml,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static> {
    if parsed.invoker.as_deref().map(str::trim) != Some("Static") {
        return None;
    }
    let tool_id = parsed.id.trim().to_string();
    let embed_url = parsed
        .embed_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("<not configured>")
        .to_string();

    Some(move |_args: DispatchArgs<'_>| -> Result<String, String> {
        Err(format!(
            "static invoker for `{tool_id}` has no headless execution path — \
             Passive Embed renders its webview in a GUI surface and is not \
             callable from CLI/MCP/HTTP (embed_url: `{embed_url}`). \
             Open it in Desktop/PWA/Extension."
        ))
    })
}
