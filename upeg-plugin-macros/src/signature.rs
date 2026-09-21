//! Guest `inputs = [...]` ⇄ function-signature validation.
//!
//! The guest ergonomic improvement over the built-in macro (design §4):
//! a `required` DSL field must map to a non-`Option` parameter, and an
//! `optional` DSL field must map to an `Option<...>` parameter — giving
//! the Rust type system a real job instead of only checking it at
//! runtime (CLAUDE.md: "use the Rust type system actively").

use syn::{ItemFn, Result};
use upeg_tool_grammar::{InputRequirement, ToolInput, input_type_label};

use crate::params::{collect_guest_params, family_accepts_label, family_expected_labels};

pub(crate) use crate::params::GuestParam;

/// Validate that `inputs` declares exactly one field per fn parameter, in
/// matching order, with matching names, compatible types, and matching
/// required/`Option` shape. Returns the classified parameters on success
/// so the caller (codegen) doesn't need to re-walk the signature.
///
/// # Errors
///
/// Returns a span-pointed `syn::Error` on arity mismatch, name mismatch,
/// type-family mismatch, or required/`Option` mismatch.
pub(crate) fn validate_guest_inputs_match_signature(
    inputs: &Option<Vec<ToolInput>>,
    item_fn: &ItemFn,
) -> Result<Vec<GuestParam>> {
    let declared = inputs.as_deref().unwrap_or(&[]);
    let params = collect_guest_params(item_fn)?;

    if declared.len() != params.len() {
        return Err(syn::Error::new(
            item_fn.sig.ident.span(),
            format!(
                "`inputs` must declare exactly one field per function parameter ({} declared, {} parameters)",
                declared.len(),
                params.len()
            ),
        ));
    }

    for (position, (input, param)) in declared.iter().zip(params.iter()).enumerate() {
        let input_name = input.name.to_string();
        let param_name = param.name.to_string();
        if input_name != param_name {
            return Err(syn::Error::new(
                input.name.span(),
                format!(
                    "`inputs` field #{position} is `{input_name}`, but the function parameter is `{param_name}`"
                ),
            ));
        }

        match input.requirement {
            InputRequirement::Required if param.binding.is_option() => {
                return Err(syn::Error::new(
                    input.name.span(),
                    format!(
                        "`inputs` field `{input_name}` is `required`, but function parameter `{param_name}` is `Option<...>`; use `optional` or drop the `Option`"
                    ),
                ));
            }
            InputRequirement::Optional if !param.binding.is_option() => {
                return Err(syn::Error::new(
                    input.name.span(),
                    format!(
                        "`inputs` field `{input_name}` is `optional`, but function parameter `{param_name}` is not `Option<...>`; wrap it in `Option<...>` or declare the field `required`"
                    ),
                ));
            }
            InputRequirement::Required | InputRequirement::Optional => {}
        }

        let label = input_type_label(&input.ty)?;
        if !family_accepts_label(param.family, label) {
            return Err(syn::Error::new(
                input.ty.span(),
                format!(
                    "`inputs` field `{input_name}` uses `{}`, but function parameter `{param_name}` is compatible with {}",
                    input.ty,
                    family_expected_labels(param.family)
                ),
            ));
        }
    }

    Ok(params)
}
