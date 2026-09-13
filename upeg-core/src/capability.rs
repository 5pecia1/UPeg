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
    fn native_호스트는_모든_surface_invoker_조합을_지원한다() {
        for &surface in ALL_SURFACES {
            for &invoker in ALL_INVOKERS {
                assert_eq!(
                    dispatch_capability(surface, invoker, RuntimeHost::Native),
                    DispatchCapability::Supported,
                    "native 호스트에서 {surface:?}×{invoker:?} 는 지원되어야 한다",
                );
            }
        }
    }

    #[test]
    fn wasm_호스트_판정은_surface에_무관하게_invoker로만_결정된다() {
        for &surface in ALL_SURFACES {
            for &invoker in ALL_INVOKERS {
                assert_eq!(
                    dispatch_capability(surface, invoker, RuntimeHost::Wasm),
                    expected_wasm_capability(invoker),
                    "wasm 호스트에서 {surface:?}×{invoker:?} 판정이 고정 표와 다르다",
                );
            }
        }
    }

    #[test]
    fn wasm에서_function과_static은_지원된다() {
        assert!(
            dispatch_capability(Surface::Pwa, Invoker::Function, RuntimeHost::Wasm).is_supported()
        );
        assert!(
            dispatch_capability(Surface::Pwa, Invoker::Static, RuntimeHost::Wasm).is_supported()
        );
    }

    #[test]
    fn wasm에서_external은_프로세스_스폰_부재로_미지원이다() {
        assert_eq!(
            dispatch_capability(Surface::Pwa, Invoker::External, RuntimeHost::Wasm).reason(),
            Some(UnsupportedReason::NoProcessSpawn),
        );
    }

    #[test]
    fn wasm에서_http_chain_llm_embed은_로더_부재로_미지원이다() {
        for invoker in [Invoker::Http, Invoker::Chain, Invoker::Llm, Invoker::Embed] {
            assert_eq!(
                dispatch_capability(Surface::Pwa, invoker, RuntimeHost::Wasm).reason(),
                Some(UnsupportedReason::NoLoaderRuntime),
                "{invoker:?} 는 wasm 로더 부재로 미지원이어야 한다",
            );
        }
    }

    #[test]
    fn wasm에서_wasm_invoker는_extism_호스트_부재로_미지원이다() {
        assert_eq!(
            dispatch_capability(Surface::Pwa, Invoker::Wasm, RuntimeHost::Wasm).reason(),
            Some(UnsupportedReason::NoWasmHost),
        );
    }

    #[test]
    fn dispatcher_없는_function_타이머_도구는_wasm에서_native_only로_낮춰진다() {
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
    fn dispatcher_없는_function_런처_도구도_wasm에서_native_only다() {
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
    fn dispatcher_있는_function_타이머_도구는_wasm에서_지원된다() {
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
    fn dispatcher_불필요한_function_도구는_없어도_wasm에서_지원된다() {
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
    fn native에서는_dispatcher가_없어도_native_only로_낮추지_않는다() {
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
    fn 로더_invoker의_미지원_판정은_dispatcher_probe로_덮이지_않는다() {
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
    fn runtime_host_라벨은_왕복_파싱한다() {
        for host in [RuntimeHost::Native, RuntimeHost::Wasm] {
            assert_eq!(RuntimeHost::parse(host.label()), Some(host));
        }
    }

    #[test]
    fn unsupported_reason_라벨은_왕복_파싱한다() {
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
    fn current_host는_컴파일_타깃과_일치한다() {
        let expected = if cfg!(target_arch = "wasm32") {
            RuntimeHost::Wasm
        } else {
            RuntimeHost::Native
        };
        assert_eq!(RuntimeHost::current(), expected);
    }
}
