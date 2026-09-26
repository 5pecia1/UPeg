//! Pin activation routing exposed to Flutter.
//!
//! Centralises the "what should happen when the user taps a pin?" decision
//! so the Dart UI never has to duplicate the activation policy. The Rust
//! side reads the tool's `PinKind`, the embed URL registry, and the input
//! spec to decide between three outcomes:
//!
//! - `DispatchImmediate { tool_id }` — `PinKind::Launcher`, `Inline`,
//!   `Action`, or `Live` whose input spec has zero required fields.
//!   Dart fires `dispatch_tool` with the args carried in `args_json`
//!   (typically `"{}"`). For a tool pinned on the visible board the
//!   canonical result renders INLINE in the pin body (inline-first
//!   activation, issue 7); the expanded modal stays reachable only via
//!   an explicit gesture (keyboard `o` / pin context menu).
//! - `OpenModal { tool_id }` — `Chain`, `Llm`, `ControlledEmbed`, any
//!   `Inline`/`Action`/`Live`/`Launcher` with required inputs, and Embed
//!   without a URL. Dart pushes the standard `ExpandedModalPage`.
//! - `OpenEmbed { tool_id }` — `PinKind::Embed` (Passive) with a
//!   registered URL. Dart pushes `EmbedPage` (a full-screen webview).
//!
//! Note: `PinKind::ControlledEmbed` does NOT activate via this enum
//! anymore — its pin tile is rendered inline (`bodyOverride` slot) and
//! never requires a click/page navigation. The Rust dispatcher handles
//! headless invocations through `ControlledEmbedBackend`.
//!
//! Result presentation follows the same inline-first rule: a successful
//! run of a pin already on the visible board renders its canonical result
//! in the pin body — no toast. Snackbars are reserved for run failures
//! and for activated tools with no on-board placement to render into
//! (palette hits, deep links, off-board tools). A passive `Embed` already
//! pinned on the visible board focuses its inline body on activation
//! instead of navigating to `EmbedPage`.
//!
//! Honest provider state: a `Live` pin whose `Http` invoker has a
//! `Static` source and no configured provider shows a "setup required"
//! badge instead of a runnable affordance, and activating it explains the
//! missing provider rather than failing generically. The rule keys off
//! invoker/source metadata, so any future same-shaped tool is honest by
//! construction. (`memo.create` is a metadata-declared `Shortcut` action
//! — chord `Cmd+Shift+N` — that creates a memo and focuses the on-board
//! notepad; `memo.scratch` is a `Live` pin whose inline notepad is backed
//! by the same store.)

use upeg_core::PinKind;
use upeg_runtime::{embed_url_for, toolbox_tool};

/// Outcome of activating a pin. Sealed enum so the Dart side gets an
/// exhaustive switch instead of a stringly-typed discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum PinActivationDto {
    /// Dispatch the tool immediately (no modal). `tool_id` is echoed so
    /// the Dart side doesn't need to thread the original argument back
    /// through the activation lookup.
    DispatchImmediate { tool_id: String },
    /// Open the `ExpandedModalPage` for `tool_id`.
    OpenModal { tool_id: String },
    /// Open the dedicated full-screen webview page (`EmbedPage`) for a
    /// Passive Embed pin (`PinKind::Embed`) whose URL is resolvable.
    /// Dart calls `resolve_embed_url` when the page mounts.
    OpenEmbed { tool_id: String },
}

/// Decide how a pin tap on `tool_id` should be handled. Unknown tool
/// ids fall back to `OpenModal` — the modal renders its own
/// "unknown tool" error state, so the activation layer doesn't need to
/// emit a separate validation error.
///
/// `args_json` is reserved for future routing rules (e.g. "Launcher
/// with explicit args_json bypasses required-field check"). Today only
/// the implicit empty-args case dispatches immediately.
#[flutter_rust_bridge::frb(sync)]
pub fn pin_activation_for(tool_id: String, args_json: String) -> PinActivationDto {
    let _ = args_json; // reserved for future dispatch-with-args routing
    let Some(meta) = toolbox_tool(&tool_id) else {
        return PinActivationDto::OpenModal { tool_id };
    };

    match meta.pin {
        PinKind::Embed => {
            if embed_url_for(meta.id).is_some() {
                PinActivationDto::OpenEmbed { tool_id }
            } else {
                PinActivationDto::OpenModal { tool_id }
            }
        }
        // Inline-first activation (issue 7): a runnable pin with no
        // required inputs dispatches immediately and renders its result
        // inline. With required inputs, fall back to the modal so the
        // user can fill them. Launcher keeps this same rule (its prior
        // behaviour).
        PinKind::Launcher | PinKind::Inline | PinKind::Action | PinKind::Live => {
            let needs_input = meta.input_spec.fields.iter().any(|field| field.required);
            if needs_input {
                PinActivationDto::OpenModal { tool_id }
            } else {
                PinActivationDto::DispatchImmediate { tool_id }
            }
        }
        // Chain/Llm always open the modal (they drive a multi-step or
        // conversational UI that has no meaningful inline form). ControlledEmbed
        // is rendered inline (bodyOverride); tapping it has no special
        // activation — fall through to the modal, which is harmless
        // because the inline tile is always visible above it.
        PinKind::ControlledEmbed | PinKind::Chain | PinKind::Llm => {
            PinActivationDto::OpenModal { tool_id }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::{
        ALL_SURFACES, InputFieldSpec, InputKind, InputName, InputSpec, Invoker, OutputSpec,
        PegboardUnits, PinKind, Source, ToolMeta,
    };
    use upeg_runtime::{register_embed_url, toolbox_add_tool_managed};

    fn fixture_meta(local: &'static str, pin: PinKind, required_inputs: bool) -> ToolMeta {
        let input_spec = if required_inputs {
            let field = InputFieldSpec::new(
                InputName::new("x").expect("test input name is canonical"),
                None,
                None,
                true,
                InputKind::String,
            )
            .expect("test field constraints must validate");
            InputSpec::new(vec![field]).expect("test input spec must validate")
        } else {
            InputSpec::empty()
        };
        // Pick a pin/invoker pair that satisfies the embed pairing
        // validator regardless of which `pin` the test passes in.
        let (invoker, surfaces): (Invoker, &'static [upeg_core::Surface]) = match pin {
            PinKind::Embed => (Invoker::Static, ALL_SURFACES),
            PinKind::ControlledEmbed => (Invoker::Embed, upeg_core::EMBED_SURFACES),
            _ => (Invoker::Function, ALL_SURFACES),
        };
        ToolMeta {
            id: Box::leak(format!("frb_pin_activation_test.{local}").into_boxed_str()),
            toolkit: "frb_pin_activation_test",
            local_id: local,
            tags: &[],
            display_label: "frb pin activation test",
            description: "",
            input_spec,
            output_spec: OutputSpec::empty(),
            primary_output_id: None,
            effect: upeg_core::ToolEffect::Unknown,
            presentation: None,
            source: Source::UserInput,
            pin,
            pegboard_units: PegboardUnits::U1,
            invoker,
            surfaces,
            boards: &[],
        }
    }

    #[test]
    fn pin_activation_for_routes_embed_tool_to_open_embed() {
        let guard = toolbox_add_tool_managed(fixture_meta("embed_with_url", PinKind::Embed, false));
        register_embed_url(guard.id(), "https://example.com/embed");
        let result = pin_activation_for(
            "frb_pin_activation_test.embed_with_url".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenEmbed {
                tool_id: "frb_pin_activation_test.embed_with_url".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_demotes_embed_without_url_to_open_modal() {
        let _guard = toolbox_add_tool_managed(fixture_meta("embed_no_url", PinKind::Embed, false));
        let result = pin_activation_for(
            "frb_pin_activation_test.embed_no_url".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.embed_no_url".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_inputless_launcher_to_dispatch_immediate() {
        let _guard =
            toolbox_add_tool_managed(fixture_meta("launcher_empty", PinKind::Launcher, false));
        let result = pin_activation_for(
            "frb_pin_activation_test.launcher_empty".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::DispatchImmediate {
                tool_id: "frb_pin_activation_test.launcher_empty".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_launcher_with_input_to_open_modal() {
        let _guard =
            toolbox_add_tool_managed(fixture_meta("launcher_with_input", PinKind::Launcher, true));
        let result = pin_activation_for(
            "frb_pin_activation_test.launcher_with_input".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.launcher_with_input".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_inputless_live_tool_to_dispatch_immediate() {
        // Inline-first: a Live pin with no required inputs (e.g. a scratch
        // memo or a one-shot ticker) dispatches immediately and renders
        // its result inline instead of forcing the modal.
        let _guard = toolbox_add_tool_managed(fixture_meta("live_one", PinKind::Live, false));
        let result =
            pin_activation_for("frb_pin_activation_test.live_one".to_string(), "{}".into());
        assert_eq!(
            result,
            PinActivationDto::DispatchImmediate {
                tool_id: "frb_pin_activation_test.live_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_live_tool_with_input_to_open_modal() {
        let _guard = toolbox_add_tool_managed(fixture_meta("live_with_input", PinKind::Live, true));
        let result = pin_activation_for(
            "frb_pin_activation_test.live_with_input".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.live_with_input".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_inputless_action_tool_to_dispatch_immediate() {
        let _guard = toolbox_add_tool_managed(fixture_meta("action_one", PinKind::Action, false));
        let result = pin_activation_for(
            "frb_pin_activation_test.action_one".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::DispatchImmediate {
                tool_id: "frb_pin_activation_test.action_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_chain_tool_to_open_modal() {
        // Chain drives a multi-step UI with no inline form — always modal.
        let _guard = toolbox_add_tool_managed(fixture_meta("chain_one", PinKind::Chain, false));
        let result =
            pin_activation_for("frb_pin_activation_test.chain_one".to_string(), "{}".into());
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.chain_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_llm_tool_to_open_modal() {
        let _guard = toolbox_add_tool_managed(fixture_meta("llm_one", PinKind::Llm, false));
        let result = pin_activation_for("frb_pin_activation_test.llm_one".to_string(), "{}".into());
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.llm_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_demotes_controlled_embed_tool_to_open_modal() {
        // ControlledEmbed pins render inline (bodyOverride) and never
        // navigate to a separate page — tapping them is effectively a
        // no-op or a fall-through to the standard expanded modal. The
        // activation enum therefore reports `OpenModal` for these
        // tools; the Dart board canvas branches earlier on PinKind so
        // the modal is rarely reached.
        let guard = toolbox_add_tool_managed(fixture_meta(
            "controlled_one",
            PinKind::ControlledEmbed,
            false,
        ));
        register_embed_url(guard.id(), "https://example.com/controlled");
        upeg_runtime::set_selector_bindings(
            guard.id(),
            vec![upeg_runtime::SelectorBinding {
                role: upeg_runtime::BindingRole::Input,
                field: "q".into(),
                selector: "#q".into(),
                trigger_action: upeg_core::ControlledEmbedTriggerAction::Click,
                wait: None,
            }],
        );
        let result = pin_activation_for(
            "frb_pin_activation_test.controlled_one".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.controlled_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_inputless_inline_tool_to_dispatch_immediate() {
        let _guard = toolbox_add_tool_managed(fixture_meta("inline_one", PinKind::Inline, false));
        let result = pin_activation_for(
            "frb_pin_activation_test.inline_one".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::DispatchImmediate {
                tool_id: "frb_pin_activation_test.inline_one".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_routes_inline_tool_with_input_to_open_modal() {
        let _guard =
            toolbox_add_tool_managed(fixture_meta("inline_with_input", PinKind::Inline, true));
        let result = pin_activation_for(
            "frb_pin_activation_test.inline_with_input".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.inline_with_input".to_string(),
            }
        );
    }

    #[test]
    fn pin_activation_for_demotes_unregistered_tool_to_open_modal() {
        let result = pin_activation_for(
            "frb_pin_activation_test.not_registered".to_string(),
            "{}".into(),
        );
        assert_eq!(
            result,
            PinActivationDto::OpenModal {
                tool_id: "frb_pin_activation_test.not_registered".to_string(),
            }
        );
    }
}
