//! Procedural macros for upeg WASM-guest plugins.
//!
//! ## `#[upeg::tool]`
//!
//! Annotate a plain typed Rust fn to generate the extism/serde/manifest
//! boilerplate a WASM plugin export needs — the guest-side counterpart of
//! `upeg_core::tool`. See the design doc (plugin-macro-design.md) §0/§5/§7
//! for the full rationale; summary:
//!
//! ```ignore
//! use upeg_plugin_macros::{tool, upeg_plugin};
//!
//! /// Greet a person by name.
//! #[tool(
//!     id = "greet.hello",
//!     toolkit = "greet",
//!     pegboard_units = U1,
//!     inputs = [ required name: String = "Person to greet" ],
//! )]
//! pub fn greet_hello(name: &str) -> String {
//!     format!("Hello, {name}!")
//! }
//!
//! upeg_plugin! {
//!     toolkit: "greet",
//!     tools: [greet_hello],
//! }
//! ```
//!
//! Unlike the built-in macro, `#[tool]` never depends on `upeg-core` /
//! `upeg-runtime` / `inventory` — it only emits `::extism_pdk` /
//! `::serde_json` / `::upeg_plugin_api` paths, so the guest crate's
//! dependency surface stays `extism-pdk` + `serde_json` +
//! `upeg-plugin-api` + `upeg-plugin-macros`.
//!
//! `invoker` / `boards` / `source` are forbidden keys here: the host
//! always runs plugin tools with `invoker = Wasm`, never schedules them
//! onto boards, and has no plugin-side `source` concept (design §5).
//!
//! ## `upeg_plugin! { toolkit: "...", tools: [f1, f2] }`
//!
//! Aggregates the `PluginToolDecl` accessors `#[tool]` generates into the
//! plugin's `manifest` extism export. No `inventory`/link-section
//! collection magic — see the design doc §1 for why that doesn't work
//! reliably on `wasm32-unknown-unknown` reactor guests.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

mod args;
mod build;
mod file_input_policy;
mod params;
mod plugin;
mod signature;

use args::ToolArgs;
use build::{build_export_wrapper, build_tool_decl_fn};
use plugin::{PluginArgs, build_plugin_manifest};
use signature::validate_guest_inputs_match_signature;
use upeg_tool_grammar::{
    ALLOWED_PEGBOARD_UNITS, ALLOWED_PIN_KINDS, ALLOWED_SURFACES, display_label_from_slug,
    rustdoc_first_line, validate_enum_ident, validate_tool_identity,
};

#[cfg(test)]
use build::export_symbol_name;
#[cfg(test)]
use params::collect_guest_params;

/// Symbol prefix for the extism export wrapper generated per `#[tool]` fn
/// (design §7 step 2). A shared const so the attribute (which emits the
/// wrapper) and `upeg_plugin!` (which never references the wrapper
/// directly, only the decl accessor) can't drift.
pub(crate) const EXPORT_FN_PREFIX: &str = "__upeg_export_";
/// Symbol prefix for the hidden `PluginToolDecl` accessor generated per
/// `#[tool]` fn (design §7 step 3), consumed by `upeg_plugin!`.
pub(crate) const DECL_FN_PREFIX: &str = "__upeg_tool_decl_";

/// Attribute macro that turns a plain typed Rust fn into a WASM plugin
/// tool export plus a hidden manifest-decl accessor.
///
/// Required keys: `id`, `toolkit`, `pegboard_units` (`U1`/`U2`/`U2T`).
/// Optional: `inputs`, `outputs`, `tags`, `pin`, `surfaces`, `description`,
/// `display_label`. Forbidden: `invoker`, `boards`, `source` (see the
/// module-level docs for why).
#[proc_macro_attribute]
pub fn tool(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ToolArgs);
    let item_fn = parse_macro_input!(item as ItemFn);

    let local_id = match validate_tool_identity(&args.id, &args.toolkit) {
        Ok(local_id) => local_id,
        Err(err) => return err.to_compile_error().into(),
    };
    let params = match validate_guest_inputs_match_signature(&args.inputs, &item_fn) {
        Ok(params) => params,
        Err(err) => return err.to_compile_error().into(),
    };
    if let Err(err) = validate_tags(&args.tags) {
        return err.to_compile_error().into();
    }
    if let Some(pin) = &args.pin
        && let Err(err) = validate_enum_ident(pin, "pin", ALLOWED_PIN_KINDS)
    {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_enum_ident(
        &args.pegboard_units,
        "pegboard_units",
        ALLOWED_PEGBOARD_UNITS,
    ) {
        return err.to_compile_error().into();
    }
    if let Some(surface_idents) = &args.surfaces {
        for surface_ident in surface_idents {
            if let Err(err) = validate_enum_ident(surface_ident, "surfaces", ALLOWED_SURFACES) {
                return err.to_compile_error().into();
            }
        }
    }

    let display_label_value = args
        .display_label
        .as_ref()
        .map(LitStr::value)
        .or_else(|| rustdoc_first_line(&item_fn))
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| display_label_from_slug(&local_id));

    let description_value = args
        .description
        .as_ref()
        .map(LitStr::value)
        .or_else(|| rustdoc_first_line(&item_fn))
        .filter(|description| !description.trim().is_empty());

    let wrapper = build_export_wrapper(&item_fn, &params);
    let decl_fn = match build_tool_decl_fn(
        &args,
        &item_fn,
        &display_label_value,
        description_value.as_deref(),
    ) {
        Ok(tokens) => tokens,
        Err(err) => return err.to_compile_error().into(),
    };

    let expanded = quote! {
        #item_fn

        #wrapper

        #decl_fn
    };

    expanded.into()
}

/// Function-like macro that aggregates `#[tool]`-annotated fns into the
/// plugin's `manifest` extism export.
///
/// `toolkit` and a non-empty `tools` list are required; `tags` and
/// `description` are optional Toolkit-level metadata.
#[proc_macro]
pub fn upeg_plugin(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as PluginArgs);
    if let Err(err) = validate_tags(&args.tags) {
        return err.to_compile_error().into();
    }
    build_plugin_manifest(&args).into()
}

fn validate_tags(tags: &[LitStr]) -> syn::Result<()> {
    for tag in tags {
        if tag.value().trim().is_empty() {
            return Err(syn::Error::new(
                tag.span(),
                "`tags` entries must be non-empty",
            ));
        }
        if tag.value() != tag.value().trim() {
            return Err(syn::Error::new(
                tag.span(),
                "`tags` entries must be canonical and unpadded",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod file_output_policy_tests;
#[cfg(test)]
mod tests;
