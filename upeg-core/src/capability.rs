//! Surface × Invoker × runtime → dispatch capability, decided in one place.
//!
//! "Define once, obey everywhere" (PRD v2.1 §5.1, `pin_activation.rs`
//! pattern): a Tool is authored once, and whether it can actually *run*
//! in-process on a given surface is a physical fact about the runtime the
//! code was compiled for — not a per-surface UI guess. This module is that
//! single fact table. Surfaces (Flutter Desktop/PWA, …) consult it and
//! degrade honestly ("unsupported on this surface") instead of firing a
//! dispatch that is guaranteed to fail.
//!
//! Physical constraints encoded here (verified against the tree):
//! - The native build links `upeg-loader`, which owns subprocess spawning
//!   (`External`), the HTTP client (`Http`), the chain executor (`Chain`),
//!   the LLM caller (`Llm`), the controlled-embed webview driver (`Embed`),
//!   and the extism WASM host (`Wasm`).
//! - The `wasm32` build (Flutter web / PWA) links *none* of those, so every
//!   loader-backed invoker is unsupported there.
//! - `Function` and `Static` invokers run in-process with no loader, so they
//!   are supported on both hosts — *except* a `Function` tool whose
//!   dispatcher is compiled out on this host because it needs native-only
//!   services (raw sockets / filesystem / SQLite), e.g. `net.status` on
//!   wasm32. That last case can only be seen with a per-tool dispatcher
//!   probe, so it is layered on by [`dispatch_capability_for_tool`].
//!
//! Rendering contract for surfaces:
//! - The verdict is a closed enum, never a string — `Supported` or
//!   `Unsupported(UnsupportedReason)` with exactly four reasons
//!   (`NoProcessSpawn` / `NoLoaderRuntime` / `NoWasmHost` /
//!   `NativeOnlyTool`).
//! - `Unsupported` renders as an honest notice, never a runnable
//!   affordance: `SurfaceUnsupportedBody` replaces only the pin's
//!   rendered body — the pin's declaration stays visible and
//!   tappable-to-inspect, never hidden and never wired to dispatch. The
//!   same pattern backs `ProviderNotConfiguredBody` and the
//!   controlled-embed inline notice.
//! - `gui_meta` / built-in `surfaces` declarations are audited against
//!   this table, so a tool never claims a surface it cannot run on
//!   without explanation. Browser surfaces (PWA, ext) pair with a native
//!   host to run what they cannot — see the host-attach module in
//!   `flutter_app`.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::types::{Invoker, Surface};

// ─── RuntimeHost ────────────────────────────────────────────────
// Which build of the in-process runtime is asking. Kept as a closed enum
// (not a bare `is_wasm: bool`) so call sites read `RuntimeHost::Wasm`
// instead of a boolean whose polarity you have to remember.

/// The compilation target of the in-process runtime consulting the table.
///
/// `Native` links `upeg-loader` (subprocess/http/chain/llm/embed/wasm-host);
/// `Wasm` (the Flutter-web / PWA `wasm32` build) links none of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RuntimeHost {
    Native,
    Wasm,
}

impl RuntimeHost {
    /// The host the current crate is being compiled for. `Wasm` on
    /// `wasm32`, `Native` everywhere else — the same `cfg` split
    /// `upeg-loader` linkage follows.
    pub const fn current() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::Wasm
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::Native
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wasm => "wasm",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "native" => Self::Native,
            "wasm" => Self::Wasm,
            _ => return None,
        })
    }
}

// ─── UnsupportedReason ──────────────────────────────────────────
// Closed set of *why* a dispatch cannot run here. Every variant maps to a
// concrete missing capability so surfaces can render a specific,
// non-magic-string explanation. Labels are the stable ids surfaces key
// their localized copy off of.

/// Why a Tool cannot be dispatched in-process on the current surface+host.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UnsupportedReason {
    /// `External` invoker needs to spawn a subprocess. The `wasm32`
    /// sandbox (Flutter web) has no process model.
    NoProcessSpawn,
    /// `Http` / `Chain` / `Llm` / controlled-`Embed` invokers execute
    /// inside `upeg-loader`, which is not linked into the `wasm32` build.
    NoLoaderRuntime,
    /// `Wasm` invoker needs the extism host runtime (`wasm-plugin`),
    /// which is absent from the `wasm32` build.
    NoWasmHost,
    /// A `Function` tool whose runtime dispatcher is compiled out on this
    /// host because it depends on native-only services (raw sockets,
    /// filesystem, SQLite). e.g. `net.status` on `wasm32`.
    NativeOnlyTool,
}

impl UnsupportedReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoProcessSpawn => "no_process_spawn",
            Self::NoLoaderRuntime => "no_loader_runtime",
            Self::NoWasmHost => "no_wasm_host",
            Self::NativeOnlyTool => "native_only_tool",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "no_process_spawn" => Self::NoProcessSpawn,
            "no_loader_runtime" => Self::NoLoaderRuntime,
            "no_wasm_host" => Self::NoWasmHost,
            "native_only_tool" => Self::NativeOnlyTool,
            _ => return None,
        })
    }
}

// ─── DispatchCapability ─────────────────────────────────────────

/// The verdict: can this Tool be dispatched in-process here, and if not, why.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DispatchCapability {
    Supported,
    Unsupported(UnsupportedReason),
}

impl DispatchCapability {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// The reason when unsupported; `None` when supported.
    pub const fn reason(self) -> Option<UnsupportedReason> {
        match self {
            Self::Supported => None,
            Self::Unsupported(reason) => Some(reason),
        }
    }
}

/// Invoker-level dispatch capability for `surface` on `host`.
///
/// This is the pure physical table: it depends only on the invoker's
/// runtime requirements and the host's linked capabilities. `surface` is
/// part of the signature so the matrix is complete and future
/// surface-specific rules have a home, but it imposes no constraint today
/// (a tool's *listing* per surface is already gated by `ToolMeta::is_on_surface`).
///
/// Per-tool nuance (a `Function` tool whose dispatcher is compiled out on
/// this host) is not visible at the invoker level — layer it on with
/// [`dispatch_capability_for_tool`].
pub const fn dispatch_capability(
    surface: Surface,
    invoker: Invoker,
    host: RuntimeHost,
) -> DispatchCapability {
    // Reserved: no surface currently narrows dispatch capability beyond
    // what the host already decides. Bind it so the matrix stays explicit.
    let _ = surface;
    match host {
        // Native links the full loader — every invoker can run.
        RuntimeHost::Native => DispatchCapability::Supported,
        RuntimeHost::Wasm => wasm_invoker_capability(invoker),
    }
}

const fn wasm_invoker_capability(invoker: Invoker) -> DispatchCapability {
    match invoker {
        // Pure in-process: no loader, no host services required.
        Invoker::Function | Invoker::Static => DispatchCapability::Supported,
        // Subprocess — no process model in the browser sandbox.
        Invoker::External => DispatchCapability::Unsupported(UnsupportedReason::NoProcessSpawn),
        // Loader-backed runtimes, all unlinked on wasm32.
        Invoker::Http | Invoker::Chain | Invoker::Llm | Invoker::Embed => {
            DispatchCapability::Unsupported(UnsupportedReason::NoLoaderRuntime)
        }
        // extism host is part of the loader's `wasm-plugin` feature.
        Invoker::Wasm => DispatchCapability::Unsupported(UnsupportedReason::NoWasmHost),
    }
}

/// Dispatch capability for a concrete Tool, refining [`dispatch_capability`]
/// with a per-tool dispatcher probe.
///
/// `requires_dispatcher` is `true` when the tool can only produce a value by
/// running its runtime dispatcher (an auto-refreshing `Source::Timer` Live
/// pin is the canonical case). `dispatcher_present` is whether the current
/// host actually has that dispatcher registered.
///
/// A `Function` tool that the invoker table calls `Supported`, yet which
/// needs a dispatcher that is compiled out on this host, is really
/// [`UnsupportedReason::NativeOnlyTool`] — e.g. `net.status`, whose
/// `std::net`-backed dispatcher is `#[cfg(not(wasm32))]`. Self-presenting
/// (`Static`) and user-edited (no dispatcher needed) tools are unaffected.
pub const fn dispatch_capability_for_tool(
    surface: Surface,
    invoker: Invoker,
    host: RuntimeHost,
    requires_dispatcher: bool,
    dispatcher_present: bool,
) -> DispatchCapability {
    match dispatch_capability(surface, invoker, host) {
        DispatchCapability::Unsupported(reason) => DispatchCapability::Unsupported(reason),
        // The dispatcher probe only distinguishes native-only Function
        // tools on the wasm host. On native every declared dispatcher is
        // linked, so a `false` probe there is not a surface-capability
        // fact and must not downgrade the verdict.
        DispatchCapability::Supported => match host {
            RuntimeHost::Wasm if requires_dispatcher && !dispatcher_present => {
                DispatchCapability::Unsupported(UnsupportedReason::NativeOnlyTool)
            }
            _ => DispatchCapability::Supported,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ALL_SURFACES;

    const ALL_INVOKERS: &[Invoker] = &[
        Invoker::Function,
        Invoker::External,
        Invoker::Http,
        Invoker::Static,
        Invoker::Embed,
        Invoker::Chain,
        Invoker::Llm,
        Invoker::Wasm,
    ];

    /// The pinned truth on the wasm32 (PWA) host, keyed by invoker. Any
    /// change to the physical constraints must update this table.
    fn expected_wasm_capability(invoker: Invoker) -> DispatchCapability {
        match invoker {
            Invoker::Function | Invoker::Static => DispatchCapability::Supported,
            Invoker::External => DispatchCapability::Unsupported(UnsupportedReason::NoProcessSpawn),
            Invoker::Http | Invoker::Chain | Invoker::Llm | Invoker::Embed => {
                DispatchCapability::Unsupported(UnsupportedReason::NoLoaderRuntime)
            }
            Invoker::Wasm => DispatchCapability::Unsupported(UnsupportedReason::NoWasmHost),
        }
    }

    #[test]
    fn native_host_supports_every_surface_invoker_combination() {
        for &surface in ALL_SURFACES {
            for &invoker in ALL_INVOKERS {
                assert_eq!(
                    dispatch_capability(surface, invoker, RuntimeHost::Native),
                    DispatchCapability::Supported,
                    "{surface:?}×{invoker:?} must be supported on a native host",
                );
            }
        }
    }

    #[test]
    fn wasm_host_verdict_is_decided_by_invoker_regardless_of_surface() {
        for &surface in ALL_SURFACES {
            for &invoker in ALL_INVOKERS {
                assert_eq!(
                    dispatch_capability(surface, invoker, RuntimeHost::Wasm),
                    expected_wasm_capability(invoker),
                    "{surface:?}×{invoker:?} verdict on wasm differs from the fixed table",
                );
            }
        }
    }

    #[test]
    fn function_and_static_invokers_are_supported_on_wasm() {
        assert!(
            dispatch_capability(Surface::Pwa, Invoker::Function, RuntimeHost::Wasm).is_supported()
        );
        assert!(
            dispatch_capability(Surface::Pwa, Invoker::Static, RuntimeHost::Wasm).is_supported()
        );
    }

    #[test]
    fn external_invoker_is_unsupported_on_wasm_without_process_spawn() {
        assert_eq!(
            dispatch_capability(Surface::Pwa, Invoker::External, RuntimeHost::Wasm).reason(),
            Some(UnsupportedReason::NoProcessSpawn),
        );
    }

    #[test]
    fn http_chain_llm_and_embed_invokers_are_unsupported_on_wasm_without_a_loader() {
        for invoker in [Invoker::Http, Invoker::Chain, Invoker::Llm, Invoker::Embed] {
            assert_eq!(
                dispatch_capability(Surface::Pwa, invoker, RuntimeHost::Wasm).reason(),
                Some(UnsupportedReason::NoLoaderRuntime),
                "{invoker:?} must be unsupported on wasm due to the missing loader",
            );
        }
    }

    #[test]
    fn wasm_invoker_is_unsupported_on_wasm_without_an_extism_host() {
        assert_eq!(
            dispatch_capability(Surface::Pwa, Invoker::Wasm, RuntimeHost::Wasm).reason(),
            Some(UnsupportedReason::NoWasmHost),
        );
    }

    #[test]
    fn function_timer_tool_without_dispatcher_is_lowered_to_native_only_on_wasm() {
        // net.status: Function invoker, Timer source, dispatcher compiled
        // out on wasm32. requires_dispatcher=true, dispatcher_present=false.
        let cap = dispatch_capability_for_tool(
            Surface::Pwa,
            Invoker::Function,
            RuntimeHost::Wasm,
            true,
            false,
        );
        assert_eq!(cap.reason(), Some(UnsupportedReason::NativeOnlyTool));
    }

    #[test]
    fn function_launcher_tool_without_dispatcher_is_also_native_only_on_wasm() {
        // eth.gas: Function invoker, Launcher/UserInput source (NOT Timer),
        // native-gated JSON-RPC dispatcher absent on wasm32. The verdict
        // must not depend on the trigger source — a Function tool that
        // requires a dispatcher and lacks one on this host is native-only.
        let cap = dispatch_capability_for_tool(
            Surface::Pwa,
            Invoker::Function,
            RuntimeHost::Wasm,
            true,
            false,
        );
        assert_eq!(cap.reason(), Some(UnsupportedReason::NativeOnlyTool));
    }

    #[test]
    fn function_timer_tool_with_dispatcher_is_supported_on_wasm() {
        // time.epoch: Function + Timer, dispatcher IS present on wasm.
        let cap = dispatch_capability_for_tool(
            Surface::Pwa,
            Invoker::Function,
            RuntimeHost::Wasm,
            true,
            true,
        );
        assert!(cap.is_supported());
    }

    #[test]
    fn function_tool_needing_no_dispatcher_is_supported_on_wasm() {
        // memo.scratch: Function, user-edited (no dispatcher needed).
        let cap = dispatch_capability_for_tool(
            Surface::Pwa,
            Invoker::Function,
            RuntimeHost::Wasm,
            false,
            false,
        );
        assert!(cap.is_supported());
    }

    #[test]
    fn native_host_does_not_lower_to_native_only_when_dispatcher_is_absent() {
        let cap = dispatch_capability_for_tool(
            Surface::Desktop,
            Invoker::Function,
            RuntimeHost::Native,
            true,
            false,
        );
        assert!(cap.is_supported());
    }

    #[test]
    fn loader_invoker_unsupported_verdict_is_not_overridden_by_dispatcher_probe() {
        // Http on wasm stays NoLoaderRuntime even if a probe says
        // "no dispatcher" — the invoker-level reason wins.
        let cap = dispatch_capability_for_tool(
            Surface::Pwa,
            Invoker::Http,
            RuntimeHost::Wasm,
            true,
            false,
        );
        assert_eq!(cap.reason(), Some(UnsupportedReason::NoLoaderRuntime));
    }

    #[test]
    fn runtime_host_labels_round_trip_through_parse() {
        for host in [RuntimeHost::Native, RuntimeHost::Wasm] {
            assert_eq!(RuntimeHost::parse(host.label()), Some(host));
        }
    }

    #[test]
    fn unsupported_reason_labels_round_trip_through_parse() {
        for reason in [
            UnsupportedReason::NoProcessSpawn,
            UnsupportedReason::NoLoaderRuntime,
            UnsupportedReason::NoWasmHost,
            UnsupportedReason::NativeOnlyTool,
        ] {
            assert_eq!(UnsupportedReason::parse(reason.label()), Some(reason));
        }
    }

    #[test]
    fn current_host_matches_the_compile_target() {
        let expected = if cfg!(target_arch = "wasm32") {
            RuntimeHost::Wasm
        } else {
            RuntimeHost::Native
        };
        assert_eq!(RuntimeHost::current(), expected);
    }
}
