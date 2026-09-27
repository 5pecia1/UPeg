//! A Desktop WebView provider for the common Controlled Embed dispatcher.
//!
//! Registration is reserved synchronously before subscribing so disposing a
//! Dart service before its stream attaches cannot resurrect that service.
//! Execution waits only on a Rust worker; never call synchronous tool dispatch
//! from the Dart isolate that services this stream.
//!
//! Session contract — the app-level WebView service owns one page per board
//! pin, or per Tool id for calls without a pin. Cards and debug modals never
//! own browser lifetimes:
//! - Normal Run and debug Run/Re-run take the same Rust dispatcher path;
//!   the resolved inputs/bindings/settings arrive over this request
//!   stream, and a CLI attached to the app's embedded HTTP host uses the
//!   same path. Debug additionally observes the step events and selector
//!   checks of that same run.
//! - Raw DOM strings are normalized in Rust to the declared output type,
//!   label, and primary output; debug's final result gets the same
//!   normalization and errors.
//! - Opening Debug alone never runs the Tool or reloads the page, and
//!   closing the debug screen does not end the session. One controller
//!   attaches to at most one WebViewWidget at a time.
//! - The hidden host keeps the configured viewport — a debug window's
//!   size never changes the run viewport (large pages scroll into view).
//! - Requests for the same session run in order. Cancellation blocks
//!   follow-up operations that have not started; it cannot undo a click
//!   that already ran, and a call is never re-run in another browser when
//!   the provider exits.
//! - Sessions live for the app process and are recreated when the URL or
//!   browser settings change; a settings change never swaps a page while
//!   a run or debug is in flight. Pages are not cookie/account
//!   isolation.
//! - Requires the Flutter engine and a native platform host — no WebView
//!   inside a PWA, no display-less server. The native-CLI headless path
//!   stays separate: a Trigger navigation is never replayed there, and
//!   only an idempotent first Output read retries inside the Trigger's
//!   existing settle budget when it lands in Chrome's destroyed-context
//!   window; other CDP failures return immediately and a context that
//!   stays stale past the bound fails the run.

use crate::frb_generated::StreamSink;

use super::boot::FrbError;
use super::embed::{
    BindingRoleDto, BindingWaitConditionDto, ControlledEmbedSettingsDto, SelectorBindingDto,
};
use super::tools::CanonicalToolResult;
use upeg_runtime::controlled_embed::{ControlledEmbedError, ControlledEmbedResponse};

/// One invocation, already resolved by the Rust dispatcher.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct WebViewExecutionRequestDto {
    pub request_id: u64,
    pub tool_id: String,
    /// Present only when this call came from an actual board placement.
    pub board_key: Option<String>,
    pub pin_id: Option<String>,
    pub url: String,
    pub bindings: Vec<SelectorBindingDto>,
    pub settings: ControlledEmbedSettingsDto,
    pub inputs: Vec<(String, String)>,
}

/// Ordered provider events. Cancellation stops later steps; it cannot undo a
/// click or another page action which already happened.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum WebViewExecutionEventDto {
    Ready,
    Execute { request: WebViewExecutionRequestDto },
    Cancel { request_id: u64 },
}

/// Raw DOM values return to Rust for canonical output type and primary-field
/// interpretation. A binding timeout retains its structured runtime identity.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum WebViewExecutionCompletionDto {
    Success {
        outputs: Vec<(String, String)>,
    },
    Failed {
        message: String,
    },
    Cancelled,
    WaitTimeout {
        role: BindingRoleDto,
        selector: String,
        for_selector: String,
        condition: BindingWaitConditionDto,
        timeout_ms: u64,
    },
}

/// Apply the registered output schema without executing page actions. Safe to
/// call synchronously before storing a session's normal or debugger result.
#[flutter_rust_bridge::frb(sync)]
pub fn normalize_webview_result(
    tool_id: String,
    completion: WebViewExecutionCompletionDto,
) -> CanonicalToolResult {
    #[cfg(not(target_arch = "wasm32"))]
    {
        upeg_loader::normalize_controlled_embed_result(&tool_id, completion.into()).into()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (tool_id, completion);
        let error = ControlledEmbedError::FeatureDisabled;
        upeg_runtime::tool_failure(error.code(), error.to_string()).into()
    }
}

impl From<WebViewExecutionCompletionDto> for Result<ControlledEmbedResponse, ControlledEmbedError> {
    fn from(completion: WebViewExecutionCompletionDto) -> Self {
        match completion {
            WebViewExecutionCompletionDto::Success { outputs } => {
                Ok(ControlledEmbedResponse { outputs })
            }
            WebViewExecutionCompletionDto::Failed { message } => {
                Err(ControlledEmbedError::BackendFailed(message))
            }
            WebViewExecutionCompletionDto::Cancelled => Err(ControlledEmbedError::Cancelled),
            WebViewExecutionCompletionDto::WaitTimeout {
                role,
                selector,
                for_selector,
                condition,
                timeout_ms,
            } => Err(ControlledEmbedError::WaitTimeout {
                role: role.into(),
                selector,
                for_selector,
                condition: condition.into(),
                timeout_ms,
            }),
        }
    }
}

/// Reserve a new provider generation and retire any previous provider.
/// The backend remains unavailable until its stream sends `Ready`.
#[flutter_rust_bridge::frb(sync)]
pub fn create_webview_provider() -> Result<u64, FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::install_provider()
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(FrbError::HostUnavailable)
    }
}

/// Attach the reserved provider. An old or disposed generation is rejected.
pub fn webview_execution_stream(
    provider_id: u64,
    sink: StreamSink<WebViewExecutionEventDto>,
) -> Result<(), FrbError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::bridge().attach(
            provider_id,
            std::sync::Arc::new(move |event| sink.add(event).is_ok()),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (provider_id, sink);
        Err(FrbError::HostUnavailable)
    }
}

/// Complete only the matching live invocation. Late or duplicate results are
/// discarded, including results from a replaced provider.
#[flutter_rust_bridge::frb(sync)]
pub fn complete_webview_execution(
    provider_id: u64,
    request_id: u64,
    completion: WebViewExecutionCompletionDto,
) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::bridge().complete(provider_id, request_id, completion)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (provider_id, request_id, completion);
        false
    }
}

/// Detach this generation and wake all its waiting callers. Dispose should
/// invoke this before cancelling the Dart stream subscription.
#[flutter_rust_bridge::frb(sync)]
pub fn unregister_webview_provider(provider_id: u64) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::bridge().unregister(provider_id)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = provider_id;
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
