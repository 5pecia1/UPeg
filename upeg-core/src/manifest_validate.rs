//! Manifest pairing validators — type-safe rules for the embed type system.
//!
//! Two embed tool types share `PinKind`/`Invoker` real estate:
//!   * **Passive Embed**: `pin = Embed`, `invoker = Static`. The webview
//!     IS the tool; the user interacts with the page directly.
//!   * **Controlled Embed**: `pin = ControlledEmbed`, `invoker = Embed`.
//!     The app drives the page by writing form values into DOM via
//!     CSS selectors.
//!
//! [`validate_embed_pairing`] is called by every registration path
//! (`#[tool]` macro expansion, TOML loader, manual `register_static_tool`)
//! so authors get a clear error at registration time rather than a
//! silent runtime mismatch.

use crate::types::{BindingRole, Invoker, PinKind, SelectorBinding};

/// Embed-pairing validation failure. Each variant pins down exactly
/// which rule was violated so authors get an actionable message
/// instead of "manifest invalid".
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EmbedPairingError {
    #[error(
        "tool `{tool_id}`: pin = Embed must pair with invoker = Static \
         (passive embed has no invocation), got invoker = {invoker}"
    )]
    PassiveEmbedInvokerMismatch {
        tool_id: String,
        invoker: &'static str,
    },

    #[error(
        "tool `{tool_id}`: pin = ControlledEmbed must pair with invoker = Embed \
         (selector adapter), got invoker = {invoker}"
    )]
    ControlledEmbedInvokerMismatch {
        tool_id: String,
        invoker: &'static str,
    },

    #[error("tool `{tool_id}`: invoker = Static only valid for pin = Embed, got pin = {pin}")]
    StaticInvokerOutsidePassiveEmbed { tool_id: String, pin: &'static str },

    #[error(
        "tool `{tool_id}`: invoker = Embed only valid for pin = ControlledEmbed, got pin = {pin}"
    )]
    EmbedInvokerOutsideControlledEmbed { tool_id: String, pin: &'static str },

    #[error(
        "tool `{tool_id}`: pin = ControlledEmbed requires at least one \
         `[[tools.controlled_embed.bindings]]` row"
    )]
    ControlledEmbedRequiresBindings { tool_id: String },

    #[error(
        "tool `{tool_id}`: pin = ControlledEmbed requires at least one binding \
         with `role = \"trigger\"` (the click that fires the page's action)"
    )]
    ControlledEmbedRequiresTrigger { tool_id: String },

    #[error(
        "tool `{tool_id}`: pin = ControlledEmbed requires at least one binding \
         with `role = \"output\"` (otherwise there is no value to return)"
    )]
    ControlledEmbedRequiresOutput { tool_id: String },

    #[error(
        "tool `{tool_id}`: pin = Embed (passive) must have no \
         `controlled_embed.bindings` rows; the webview IS the tool"
    )]
    PassiveEmbedRejectsBindings { tool_id: String },
}

/// Validate just the `pin`/`invoker` pairing. Callable from any
/// registration entry point that has those two fields but doesn't yet
/// know the selector_bindings list (`ToolMeta::assert_valid`,
/// `StaticToolMeta::assert_valid`).
///
/// SoC: pure data check, no I/O. The binding-shape rules
/// (Trigger/Output presence, PassiveEmbed-rejects-bindings) live in
/// [`validate_embed_binding_shape`] because they need the parsed
/// bindings.
pub fn validate_pin_invoker_pairing(
    tool_id: &str,
    pin: PinKind,
    invoker: Invoker,
) -> Result<(), EmbedPairingError> {
    // Pin → Invoker direction
    match (pin, invoker) {
        (PinKind::Embed, Invoker::Static) => {}
        (PinKind::Embed, other) => {
            return Err(EmbedPairingError::PassiveEmbedInvokerMismatch {
                tool_id: tool_id.to_string(),
                invoker: other.label(),
            });
        }
        (PinKind::ControlledEmbed, Invoker::Embed) => {}
        (PinKind::ControlledEmbed, other) => {
            return Err(EmbedPairingError::ControlledEmbedInvokerMismatch {
                tool_id: tool_id.to_string(),
                invoker: other.label(),
            });
        }
        _ => {}
    }

    // Invoker → Pin direction (catches misuse outside Embed pins)
    match (pin, invoker) {
        (PinKind::Embed | PinKind::ControlledEmbed, _) => {}
        (other, Invoker::Static) => {
            return Err(EmbedPairingError::StaticInvokerOutsidePassiveEmbed {
                tool_id: tool_id.to_string(),
                pin: other.label(),
            });
        }
        (other, Invoker::Embed) => {
            return Err(EmbedPairingError::EmbedInvokerOutsideControlledEmbed {
                tool_id: tool_id.to_string(),
                pin: other.label(),
            });
        }
        _ => {}
    }

    Ok(())
}

/// Validate the binding shape against the pin kind. The loader calls
/// this after parsing `controlled_embed.bindings`:
///   * Passive `Embed` rejects any bindings (webview is the tool).
///   * `ControlledEmbed` requires ≥ 1 Trigger and ≥ 1 Output binding
///     (Input is optional — "press button, get result" is legal).
///
/// Pure data check.
pub fn validate_embed_binding_shape(
    tool_id: &str,
    pin: PinKind,
    bindings: &[SelectorBinding],
) -> Result<(), EmbedPairingError> {
    match pin {
        PinKind::Embed => {
            if !bindings.is_empty() {
                return Err(EmbedPairingError::PassiveEmbedRejectsBindings {
                    tool_id: tool_id.to_string(),
                });
            }
        }
        PinKind::ControlledEmbed => {
            if bindings.is_empty() {
                return Err(EmbedPairingError::ControlledEmbedRequiresBindings {
                    tool_id: tool_id.to_string(),
                });
            }
            let has_trigger = bindings.iter().any(|b| b.role == BindingRole::Trigger);
            let has_output = bindings.iter().any(|b| b.role == BindingRole::Output);
            if !has_trigger {
                return Err(EmbedPairingError::ControlledEmbedRequiresTrigger {
                    tool_id: tool_id.to_string(),
                });
            }
            if !has_output {
                return Err(EmbedPairingError::ControlledEmbedRequiresOutput {
                    tool_id: tool_id.to_string(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

/// Combined pairing + binding-shape validator, for callers (the
/// loader) that have all three pieces. Equivalent to:
///   `validate_pin_invoker_pairing(...)? &&
///    validate_embed_binding_shape(...)`.
pub fn validate_embed_pairing(
    tool_id: &str,
    pin: PinKind,
    invoker: Invoker,
    bindings: &[SelectorBinding],
) -> Result<(), EmbedPairingError> {
    validate_pin_invoker_pairing(tool_id, pin, invoker)?;
    validate_embed_binding_shape(tool_id, pin, bindings)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ControlledEmbedTriggerAction;

    /// `ControlledEmbed`에서 모든 검증을 통과하는 최소 바인딩 세트
    /// (Input + Trigger + Output 한 줄씩). 합법 케이스 테스트의
    /// 공통 입력.
    fn ok_controlled_bindings() -> Vec<SelectorBinding> {
        vec![
            SelectorBinding {
                role: BindingRole::Input,
                field: "amount".into(),
                selector: "#amount".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            SelectorBinding {
                role: BindingRole::Trigger,
                field: String::new(),
                selector: "button[type=submit]".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            SelectorBinding {
                role: BindingRole::Output,
                field: "result".into(),
                selector: "#result".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
        ]
    }

    #[test]
    fn passive_embed_과_static_invoker는_validate된다() {
        assert!(
            validate_embed_pairing("demo.passive", PinKind::Embed, Invoker::Static, &[]).is_ok()
        );
    }

    #[test]
    fn passive_embed_과_embed_invoker는_거부된다() {
        let result = validate_embed_pairing("demo.passive", PinKind::Embed, Invoker::Embed, &[]);
        assert!(matches!(
            result,
            Err(EmbedPairingError::PassiveEmbedInvokerMismatch { .. })
        ));
    }

    #[test]
    fn passive_embed_과_function_invoker는_거부된다() {
        let result = validate_embed_pairing("demo.passive", PinKind::Embed, Invoker::Function, &[]);
        assert!(matches!(
            result,
            Err(EmbedPairingError::PassiveEmbedInvokerMismatch { .. })
        ));
    }

    #[test]
    fn passive_embed에_바인딩이_있으면_거부된다() {
        let bindings = vec![SelectorBinding {
            role: BindingRole::Input,
            field: "x".into(),
            selector: "#x".into(),
            trigger_action: ControlledEmbedTriggerAction::Click,
            wait: None,
        }];
        let result = validate_embed_pairing(
            "demo.passive.bad",
            PinKind::Embed,
            Invoker::Static,
            &bindings,
        );
        assert!(matches!(
            result,
            Err(EmbedPairingError::PassiveEmbedRejectsBindings { .. })
        ));
    }

    #[test]
    fn controlled_embed_과_embed_invoker는_validate된다() {
        assert!(
            validate_embed_pairing(
                "demo.controlled",
                PinKind::ControlledEmbed,
                Invoker::Embed,
                &ok_controlled_bindings(),
            )
            .is_ok()
        );
    }

    #[test]
    fn controlled_embed_과_function_invoker는_거부된다() {
        let result = validate_embed_pairing(
            "demo.controlled",
            PinKind::ControlledEmbed,
            Invoker::Function,
            &ok_controlled_bindings(),
        );
        assert!(matches!(
            result,
            Err(EmbedPairingError::ControlledEmbedInvokerMismatch { .. })
        ));
    }

    #[test]
    fn embed_invoker는_controlled_embed가_아니면_거부된다() {
        let result = validate_embed_pairing("demo.misuse", PinKind::Inline, Invoker::Embed, &[]);
        assert!(matches!(
            result,
            Err(EmbedPairingError::EmbedInvokerOutsideControlledEmbed { .. })
        ));
    }

    #[test]
    fn static_invoker는_passive_embed가_아니면_거부된다() {
        let result = validate_embed_pairing("demo.misuse", PinKind::Action, Invoker::Static, &[]);
        assert!(matches!(
            result,
            Err(EmbedPairingError::StaticInvokerOutsidePassiveEmbed { .. })
        ));
    }

    #[test]
    fn controlled_embed는_바인딩이_없으면_거부된다() {
        let result = validate_embed_pairing(
            "demo.controlled.empty",
            PinKind::ControlledEmbed,
            Invoker::Embed,
            &[],
        );
        assert!(matches!(
            result,
            Err(EmbedPairingError::ControlledEmbedRequiresBindings { .. })
        ));
    }

    #[test]
    fn controlled_embed는_trigger_없이_거부된다() {
        let bindings = vec![
            SelectorBinding {
                role: BindingRole::Input,
                field: "x".into(),
                selector: "#x".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            SelectorBinding {
                role: BindingRole::Output,
                field: "y".into(),
                selector: "#y".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
        ];
        let result = validate_embed_pairing(
            "demo.controlled.no.trigger",
            PinKind::ControlledEmbed,
            Invoker::Embed,
            &bindings,
        );
        assert!(matches!(
            result,
            Err(EmbedPairingError::ControlledEmbedRequiresTrigger { .. })
        ));
    }

    #[test]
    fn controlled_embed는_output_없이_거부된다() {
        let bindings = vec![
            SelectorBinding {
                role: BindingRole::Input,
                field: "x".into(),
                selector: "#x".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            SelectorBinding {
                role: BindingRole::Trigger,
                field: String::new(),
                selector: "button".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
        ];
        let result = validate_embed_pairing(
            "demo.controlled.no.output",
            PinKind::ControlledEmbed,
            Invoker::Embed,
            &bindings,
        );
        assert!(matches!(
            result,
            Err(EmbedPairingError::ControlledEmbedRequiresOutput { .. })
        ));
    }

    #[test]
    fn controlled_embed는_input_없어도_허용된다() {
        // "버튼만 누르면 결과 반환" 케이스 — Input 0 OK.
        let bindings = vec![
            SelectorBinding {
                role: BindingRole::Trigger,
                field: String::new(),
                selector: "button".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
            SelectorBinding {
                role: BindingRole::Output,
                field: "result".into(),
                selector: "#result".into(),
                trigger_action: ControlledEmbedTriggerAction::Click,
                wait: None,
            },
        ];
        assert!(
            validate_embed_pairing(
                "demo.controlled.input.free",
                PinKind::ControlledEmbed,
                Invoker::Embed,
                &bindings,
            )
            .is_ok()
        );
    }

    #[test]
    fn non_embed_tool은_pairing_검사_통과한다() {
        // Any non-embed pin+invoker combination is out of scope.
        assert!(
            validate_embed_pairing("demo.regular", PinKind::Inline, Invoker::Function, &[],)
                .is_ok()
        );
    }
}
