//! Dispatch-capability verdict exposed to Flutter.
//!
//! "Rust decides once, surfaces obey" (`pin_activation.rs` pattern): the
//! honest "can this tool actually run on this surface?" question is
//! answered by [`upeg_core::dispatch_capability_for_tool`], not by the
//! Dart UI. This module joins that pure table to the *current* runtime —
//! `RuntimeHost::current()` is `Wasm` on the Flutter-web / PWA `wasm32`
//! build and `Native` on desktop — and to the tool's live dispatcher
//! registration, so the Flutter layer can render an "unsupported" notice instead
//! of firing a dispatch that is guaranteed to fail.
//!
//! Desktop (native) supports every declared tool, so this verdict is only
//! ever `Unsupported` on the PWA build. The Dart side therefore gates its
//! consultation on the wasm runtime and simply trusts this answer.

use upeg_core::{
    DispatchCapability, Invoker, RuntimeHost, Surface, UnsupportedReason,
    dispatch_capability_for_tool,
};
use upeg_runtime::{has_runtime_dispatcher, toolbox_tool};

/// Dart-mirrored view of [`upeg_core::DispatchCapability`]. Sealed so the
/// Dart side gets an exhaustive switch instead of a stringly-typed flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum DispatchCapabilityDto {
    /// The tool can be dispatched in-process on the current surface/host.
    Supported,
    /// The tool cannot run here; `reason` explains which capability is missing.
    Unsupported { reason: UnsupportedReasonDto },
}

/// Dart-mirrored view of [`upeg_core::UnsupportedReason`]. Each variant maps
/// to a specific, localizable explanation on the Flutter side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum UnsupportedReasonDto {
    NoProcessSpawn,
    NoLoaderRuntime,
    NoWasmHost,
    NativeOnlyTool,
}

impl From<UnsupportedReason> for UnsupportedReasonDto {
    fn from(reason: UnsupportedReason) -> Self {
        match reason {
            UnsupportedReason::NoProcessSpawn => Self::NoProcessSpawn,
            UnsupportedReason::NoLoaderRuntime => Self::NoLoaderRuntime,
            UnsupportedReason::NoWasmHost => Self::NoWasmHost,
            UnsupportedReason::NativeOnlyTool => Self::NativeOnlyTool,
        }
    }
}

impl From<DispatchCapability> for DispatchCapabilityDto {
    fn from(capability: DispatchCapability) -> Self {
        match capability {
            DispatchCapability::Supported => Self::Supported,
            DispatchCapability::Unsupported(reason) => Self::Unsupported {
                reason: reason.into(),
            },
        }
    }
}

/// The surface implied by the current in-process host. The FRB layer only
/// backs GUI surfaces (Desktop on native, PWA on wasm32).
///
/// `pub(crate)` because it is the single answer to "which surface is this
/// build?": the dispatch path stamps the same value into every call's
/// execution context (`api/tools/desktop_context.rs`), and a second
/// hard-coded answer there is how the PWA came to dispatch as `desktop` —
/// an approval surface it is not.
pub(crate) const fn surface_for(host: RuntimeHost) -> Surface {
    match host {
        RuntimeHost::Native => Surface::Desktop,
        RuntimeHost::Wasm => Surface::Pwa,
    }
}

/// A `Function` tool produces its value by running a runtime dispatcher, so
/// if that dispatcher is compiled out on the current host the tool is
/// native-only *here* — regardless of trigger `Source`. This intentionally
/// broadens the earlier `Source::Timer`-only rule: a `Launcher`/`UserInput`
/// Function tool like `eth.gas`, whose native-gated dispatcher is absent on
/// `wasm32`, is just as native-only as a `Timer` one like `net.status`.
///
/// The verdict still hinges on the per-tool [`has_runtime_dispatcher`] probe
/// at the call site: a pure Function tool (`num.hex_to_decimal`, `csv.*`)
/// keeps its dispatcher on every target, so `dispatcher_present` stays true
/// and it remains `Supported`. Non-`Function` invokers are already resolved
/// by the invoker-level table and never consult the probe.
const fn requires_dispatcher(invoker: Invoker) -> bool {
    matches!(invoker, Invoker::Function)
}

/// Capability verdict for `tool_id` on the current runtime.
///
/// Unknown tool ids report `Supported` — there is no meta to prove a
/// missing capability, and dispatch will surface its own "not found"
/// error. Registration is ensured first so the dispatcher probe is accurate.
#[flutter_rust_bridge::frb(sync)]
pub fn dispatch_capability_for(tool_id: String) -> DispatchCapabilityDto {
    if let Err(error) = super::tools::register_toolkit_runtime() {
        tracing::error!(%error, "upeg toolkit runtime registration failed");
    }

    let Some(meta) = toolbox_tool(&tool_id) else {
        return DispatchCapabilityDto::Supported;
    };

    let host = RuntimeHost::current();
    let capability = dispatch_capability_for_tool(
        surface_for(host),
        meta.invoker,
        host,
        requires_dispatcher(meta.invoker),
        has_runtime_dispatcher(meta.id),
    );
    capability.into()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn native_runtime_supports_all_builtin_tools() {
        // On the native test host every declared tool is dispatchable.
        for tool_id in ["net.status", "eth.gas", "num.hex_to_decimal"] {
            assert_eq!(
                dispatch_capability_for(tool_id.to_string()),
                DispatchCapabilityDto::Supported,
                "{tool_id} must be supported on the native runtime",
            );
        }
    }

    #[test]
    fn unregistered_tool_answers_supported() {
        assert_eq!(
            dispatch_capability_for("nonexistent.tool".to_string()),
            DispatchCapabilityDto::Supported,
        );
    }

    #[test]
    fn only_function_tools_require_dispatcher_probe() {
        // The broadened rule: every Function tool runs via a dispatcher, so
        // any Function tool missing its dispatcher on this host is
        // native-only — not just the `Source::Timer` ones. `eth.gas`
        // (UserInput/Launcher source) is the case Phase 1 exposed. Static
        // and loader-backed invokers are already resolved by the
        // invoker-level table and must not consult the per-tool probe.
        assert!(requires_dispatcher(Invoker::Function));
        for invoker in [
            Invoker::Static,
            Invoker::External,
            Invoker::Http,
            Invoker::Chain,
            Invoker::Llm,
            Invoker::Embed,
            Invoker::Wasm,
        ] {
            assert!(
                !requires_dispatcher(invoker),
                "{invoker:?} must not require the per-tool dispatcher probe",
            );
        }
    }

    #[test]
    fn each_runtime_host_maps_to_one_surface() {
        // This table is this build's surface identity. The dispatch
        // path reads the same table, so if this drifts, the call
        // envelope's `_upeg.surface` drifts too.
        assert_eq!(surface_for(RuntimeHost::Native), Surface::Desktop);
        assert_eq!(surface_for(RuntimeHost::Wasm), Surface::Pwa);
    }

    #[test]
    fn unsupported_variant_moves_reason_to_dto() {
        let dto = DispatchCapabilityDto::from(DispatchCapability::Unsupported(
            UnsupportedReason::NoLoaderRuntime,
        ));
        assert_eq!(
            dto,
            DispatchCapabilityDto::Unsupported {
                reason: UnsupportedReasonDto::NoLoaderRuntime,
            }
        );
    }
}
