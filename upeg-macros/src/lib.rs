//! Procedural macros for upeg.
//!
//! ## `#[upeg_core::tool]`
//!
//! Annotate a Rust function with this attribute to register its `StaticToolMeta`
//! into the global `inventory` registry. PRD v2.1 §5.1: "one manifest,
//! every surface automatically" — every surface (Desktop/CLI/TUI/MCP/HTTP/Ext/PWA)
//! reads the same registry, so a tool defined once shows up everywhere.
//!
//! ```ignore
//! use upeg_core::tool;
//!
//! #[tool(
//!     id = "num.hex_to_decimal",
//!     toolkit = "num",
//!     tags = ["pure", "dev"],
//!     inputs = [
//!         required input: String = "Hex string, e.g. 0xff or DEADBEEF",
//!     ],
//!     pin = Inline,
//!     pegboard_units = U2,
//!     invoker = Function,
//!     boards = ["dev"],
//! )]
//! pub fn hex_to_decimal(input: &str) -> Option<u128> {
//!     u128::from_str_radix(input.trim_start_matches("0x"), 16).ok()
//! }
//! ```

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
use syn::{Ident, Item, ItemFn, LitStr, parse_macro_input};

mod build;
mod file_policy_build;
mod parse;
mod validate;

use build::{build_source_expr, build_static_input_spec_expr, build_static_output_spec_expr};
use parse::{ToolArgs, ToolkitArgs};
use upeg_tool_grammar::{
    ALLOWED_INVOKERS, ALLOWED_PEGBOARD_UNITS, ALLOWED_PIN_KINDS, ALLOWED_SURFACES,
    display_label_from_slug, rustdoc_first_line, validate_enum_ident, validate_tool_identity,
};
use validate::validate_inputs_match_signature;

#[cfg(test)]
use build::{parse_duration_to_ms, static_field_constraints_expr, static_input_kind_expr};
#[cfg(test)]
use parse::{SUPPORTED_INPUT_TYPES, SourceArg, ToolInput, ToolOutput};

/// Attribute macro that registers the annotated function as a Tool.
///
/// Required keys: `id`, `toolkit`, `pegboard_units`. Optional: `inputs`, `tags`,
/// `pin`, `invoker`, `surfaces` (defaults to `ALL_SURFACES`), `boards`
/// (defaults to `[]`).
#[proc_macro_attribute]
pub fn tool(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ToolArgs);
    let item_fn = parse_macro_input!(item as ItemFn);

    let local_id = match validate_tool_identity(&args.id, &args.toolkit) {
        Ok(local_id) => local_id,
        Err(err) => return err.to_compile_error().into(),
    };
    if args.invoker == "Function"
        && let Err(err) = validate_inputs_match_signature(&args.inputs, &item_fn)
    {
        return err.to_compile_error().into();
    }
    for tag in &args.tags {
        if tag.value().trim().is_empty() {
            return syn::Error::new(tag.span(), "`tags` entries must be non-empty")
                .to_compile_error()
                .into();
        }
        if tag.value() != tag.value().trim() {
            return syn::Error::new(tag.span(), "`tags` entries must be canonical and unpadded")
                .to_compile_error()
                .into();
        }
    }
    for board in &args.boards {
        if board.value().trim().is_empty() {
            return syn::Error::new(board.span(), "`boards` entries must be non-empty")
                .to_compile_error()
                .into();
        }
        if board.value() != board.value().trim() {
            return syn::Error::new(
                board.span(),
                "`boards` entries must be canonical and unpadded",
            )
            .to_compile_error()
            .into();
        }
    }
    if let Err(err) = validate_enum_ident(&args.pin, "pin", ALLOWED_PIN_KINDS) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_enum_ident(
        &args.pegboard_units,
        "pegboard_units",
        ALLOWED_PEGBOARD_UNITS,
    ) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_enum_ident(&args.invoker, "invoker", ALLOWED_INVOKERS) {
        return err.to_compile_error().into();
    }
    if let Some(surface_idents) = &args.surfaces {
        for surface_ident in surface_idents {
            if let Err(err) = validate_enum_ident(surface_ident, "surfaces", ALLOWED_SURFACES) {
                return err.to_compile_error().into();
            }
        }
    }

    let id_lit = LitStr::new(&args.id.value(), args.id.span());
    let toolkit_lit = LitStr::new(&args.toolkit.value(), args.toolkit.span());
    let local_id_lit = LitStr::new(&local_id, args.id.span());
    let id_const = Ident::new(
        &format!("{}_TOOL_ID", item_fn.sig.ident.to_string().to_uppercase()),
        item_fn.sig.ident.span(),
    );
    let tag_lits = &args.tags;
    let wk = &args.pin;
    let units = &args.pegboard_units;
    let inv = &args.invoker;
    let board_lits = &args.boards;

    let surfaces_expr = match &args.surfaces {
        std::option::Option::None => quote! { ::upeg_core::ALL_SURFACES },
        std::option::Option::Some(idents) => quote! { &[ #( ::upeg_core::Surface::#idents ),* ] },
    };

    let display_label_value = args
        .display_label
        .as_ref()
        .map(LitStr::value)
        .or_else(|| rustdoc_first_line(&item_fn))
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| display_label_from_slug(&local_id));
    let display_label_lit = LitStr::new(display_label_value.trim(), args.id.span());

    let description_expr = if let Some(lit) = &args.description {
        quote! { #lit }
    } else {
        quote! { "" }
    };

    let input_spec_expr = match build_static_input_spec_expr(&args.inputs) {
        Ok(expr) => expr,
        Err(err) => return err.to_compile_error().into(),
    };
    let output_spec_expr = match build_static_output_spec_expr(&args.outputs) {
        Ok(expr) => expr,
        Err(err) => return err.to_compile_error().into(),
    };
    let primary_output_id_expr = args
        .outputs
        .as_ref()
        .and_then(|outputs| outputs.first())
        .map(|output| {
            let name = LitStr::new(&output.name.to_string(), output.name.span());
            quote! { Some(#name) }
        })
        .unwrap_or_else(|| quote! { None });
    let source_expr = match build_source_expr(args.source.as_ref()) {
        Ok(expr) => expr,
        Err(err) => return err.to_compile_error().into(),
    };

    let expanded = quote! {
        #item_fn

        pub(crate) const #id_const: &str = #id_lit;

        ::upeg_core::inventory::submit! {
            ::upeg_core::StaticToolMeta {
                id: #id_const,
                toolkit: #toolkit_lit,
                local_id: #local_id_lit,
                tags: &[ #( #tag_lits ),* ],
                display_label: #display_label_lit,
                description: #description_expr,
                input_spec: #input_spec_expr,
                output_spec: #output_spec_expr,
                primary_output_id: #primary_output_id_expr,
                effect: ::upeg_core::ToolEffect::Unknown,
                source: #source_expr,
                pin: ::upeg_core::PinKind::#wk,
                pegboard_units: ::upeg_core::PegboardUnits::#units,
                invoker: ::upeg_core::Invoker::#inv,
                surfaces: #surfaces_expr,
                boards: &[ #( #board_lits ),* ],
            }
        }
    };

    expanded.into()
}

/// Attribute macro that registers a static Toolkit as a distribution/grouping
/// unit. It can annotate any item; the item is emitted unchanged plus a
/// `ToolkitMeta` inventory entry.
#[proc_macro_attribute]
pub fn toolkit(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ToolkitArgs);
    let item = parse_macro_input!(item as Item);

    if args.id.value().trim().is_empty() {
        return syn::Error::new(args.id.span(), "`id` must be a non-empty Toolkit id")
            .to_compile_error()
            .into();
    }
    if args.id.value() != args.id.value().trim() {
        return syn::Error::new(
            args.id.span(),
            "Toolkit `id` must be canonical and unpadded",
        )
        .to_compile_error()
        .into();
    }
    for tag in &args.tags {
        if tag.value().trim().is_empty() {
            return syn::Error::new(tag.span(), "`tags` entries must be non-empty")
                .to_compile_error()
                .into();
        }
        if tag.value() != tag.value().trim() {
            return syn::Error::new(tag.span(), "`tags` entries must be canonical and unpadded")
                .to_compile_error()
                .into();
        }
    }

    let id_lit = LitStr::new(&args.id.value(), args.id.span());
    let tag_lits = &args.tags;
    let description_expr = if let Some(lit) = &args.description {
        quote! { #lit }
    } else {
        quote! { "" }
    };

    let expanded = quote! {
        #item

        ::upeg_core::inventory::submit! {
            ::upeg_core::ToolkitMeta {
                id: #id_lit,
                tags: &[ #( #tag_lits ),* ],
                description: #description_expr,
            }
        }
    };

    expanded.into()
}

#[cfg(test)]
mod file_policy_tests;
#[cfg(test)]
mod tests;
