//! Codegen for the guest `#[tool]` attribute: the extism export wrapper
//! (design §7 step 2) and the hidden `PluginToolDecl` accessor (§7 step 3).
//!
//! Emits only `::extism_pdk` / `::serde_json` / `::upeg_plugin_api` paths —
//! this crate itself depends on none of them (§0/§2 of the design doc).

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemFn, LitStr, Result};

use upeg_tool_grammar::{
    InputRequirement, KindParams, ToolInput, ToolOutput, input_type_label,
    reject_file_policy_on_output,
};

use crate::args::ToolArgs;
use crate::file_input_policy::plugin_file_policy_expr;
use crate::params::{GuestParam, ParamBinding};
use crate::{DECL_FN_PREFIX, EXPORT_FN_PREFIX};

/// Build the `#[::extism_pdk::plugin_fn]` wrapper that bridges the host's
/// JSON-stringified args string to the user fn's typed parameters and
/// funnels its return value through `ToPluginOutput`.
pub(crate) fn build_export_wrapper(item_fn: &ItemFn, params: &[GuestParam]) -> TokenStream {
    let fn_ident = &item_fn.sig.ident;
    let wrapper_ident = export_symbol_ident(fn_ident);

    let mut let_stmts = Vec::with_capacity(params.len());
    let mut call_args = Vec::with_capacity(params.len());
    for param in params {
        let name = &param.name;
        let name_lit = LitStr::new(&name.to_string(), name.span());
        match &param.binding {
            ParamBinding::Owned(ty) => {
                let_stmts.push(quote! {
                    let #name: #ty =
                        <#ty as ::upeg_plugin_api::FromPluginArg>::from_plugin_arg(&__args, #name_lit, true)
                            .map_err(::extism_pdk::Error::msg)?;
                });
                call_args.push(quote! { #name });
            }
            ParamBinding::StrRef => {
                let_stmts.push(quote! {
                    let #name: ::std::string::String =
                        <::std::string::String as ::upeg_plugin_api::FromPluginArg>::from_plugin_arg(&__args, #name_lit, true)
                            .map_err(::extism_pdk::Error::msg)?;
                });
                call_args.push(quote! { &#name });
            }
            ParamBinding::OptionOwned(ty) => {
                let_stmts.push(quote! {
                    let #name: ::std::option::Option<#ty> =
                        <::std::option::Option<#ty> as ::upeg_plugin_api::FromPluginArg>::from_plugin_arg(&__args, #name_lit, false)
                            .map_err(::extism_pdk::Error::msg)?;
                });
                call_args.push(quote! { #name });
            }
            ParamBinding::OptionStrRef => {
                let_stmts.push(quote! {
                    let #name: ::std::option::Option<::std::string::String> =
                        <::std::option::Option<::std::string::String> as ::upeg_plugin_api::FromPluginArg>::from_plugin_arg(&__args, #name_lit, false)
                            .map_err(::extism_pdk::Error::msg)?;
                });
                call_args.push(quote! { #name.as_deref() });
            }
        }
    }

    quote! {
        #[::extism_pdk::plugin_fn]
        pub fn #wrapper_ident(__input: ::std::string::String) -> ::extism_pdk::FnResult<::std::string::String> {
            let __args: ::serde_json::Value =
                ::serde_json::from_str(&__input).unwrap_or(::serde_json::Value::Null);
            #( #let_stmts )*
            let __ret = #fn_ident( #( #call_args ),* );
            ::std::result::Result::Ok(
                ::upeg_plugin_api::ToPluginOutput::to_plugin_output(__ret)
                    .map_err(::extism_pdk::Error::msg)?
            )
        }
    }
}

/// Build the hidden `#[doc(hidden)] pub fn __upeg_tool_decl_<fn>()` that
/// `upeg_plugin!` calls to assemble the manifest. `display_label_value` and
/// `description_value` are already resolved through the arg → rustdoc →
/// slug precedence (display label) / arg → rustdoc precedence (description)
/// by the caller.
pub(crate) fn build_tool_decl_fn(
    args: &ToolArgs,
    item_fn: &ItemFn,
    display_label_value: &str,
    description_value: Option<&str>,
) -> Result<TokenStream> {
    let fn_ident = &item_fn.sig.ident;
    let decl_ident = decl_symbol_ident(fn_ident);
    let export_symbol_lit = LitStr::new(&export_symbol_name(fn_ident), fn_ident.span());
    let id_lit = LitStr::new(&args.id.value(), args.id.span());
    let toolkit_lit = LitStr::new(&args.toolkit.value(), args.toolkit.span());
    let pegboard_units_lit =
        LitStr::new(&args.pegboard_units.to_string(), args.pegboard_units.span());
    let display_label_lit = LitStr::new(display_label_value.trim(), args.id.span());

    let mut chain = quote! {
        ::upeg_plugin_api::PluginToolDecl::new(#toolkit_lit, #id_lit, #export_symbol_lit)
            .with_pegboard_units(#pegboard_units_lit)
            .with_display_label(#display_label_lit)
    };

    if let Some(description) = description_value {
        let description_lit = LitStr::new(description.trim(), args.id.span());
        chain = quote! { #chain.with_description(#description_lit) };
    }
    if !args.tags.is_empty() {
        let tag_lits = &args.tags;
        chain = quote! { #chain.with_tags([ #( #tag_lits ),* ]) };
    }
    if let Some(input_spec) = plugin_input_spec_expr(&args.inputs)? {
        chain = quote! { #chain.with_input_spec(#input_spec) };
    }
    if let Some(output_spec) = plugin_output_spec_expr(&args.outputs)? {
        chain = quote! { #chain.with_output_spec(#output_spec) };
    }

    let mut extra_stmts = Vec::new();
    if let Some(pin) = &args.pin {
        let pin_lit = LitStr::new(&pin.to_string(), pin.span());
        extra_stmts.push(quote! {
            __decl.pin = ::std::option::Option::Some(#pin_lit.to_string());
        });
    }
    if let Some(surfaces) = &args.surfaces {
        let surface_lits = surfaces
            .iter()
            .map(|surface| LitStr::new(&surface.to_string().to_ascii_lowercase(), surface.span()));
        extra_stmts.push(quote! {
            __decl.surfaces = ::std::option::Option::Some(
                vec![ #( #surface_lits.to_string() ),* ]
            );
        });
    }

    Ok(quote! {
        #[doc(hidden)]
        pub fn #decl_ident() -> ::upeg_plugin_api::PluginToolDecl {
            let mut __decl = #chain;
            #( #extra_stmts )*
            __decl
        }
    })
}

/// The wasm export symbol name for a `#[tool]`-annotated fn (`export`
/// field value in its `PluginToolDecl`).
pub(crate) fn export_symbol_name(fn_ident: &Ident) -> String {
    format!("{EXPORT_FN_PREFIX}{fn_ident}")
}

fn export_symbol_ident(fn_ident: &Ident) -> Ident {
    Ident::new(&export_symbol_name(fn_ident), fn_ident.span())
}

/// The hidden decl-accessor fn name for a `#[tool]`-annotated fn, consumed
/// by `upeg_plugin!`.
pub(crate) fn decl_symbol_ident(fn_ident: &Ident) -> Ident {
    Ident::new(&format!("{DECL_FN_PREFIX}{fn_ident}"), fn_ident.span())
}

fn plugin_input_spec_expr(inputs: &Option<Vec<ToolInput>>) -> Result<Option<TokenStream>> {
    let Some(inputs) = inputs else {
        return Ok(None);
    };
    let mut seen = BTreeSet::new();
    let mut fields = Vec::with_capacity(inputs.len());
    for input in inputs {
        let name = input.name.to_string();
        if !seen.insert(name.clone()) {
            return Err(syn::Error::new(
                input.name.span(),
                format!("duplicate input field `{name}`"),
            ));
        }
        fields.push(plugin_input_field_expr(input)?);
    }
    Ok(Some(quote! {
        ::upeg_plugin_api::PluginInputSpec::new([ #( #fields ),* ])
    }))
}

fn plugin_input_field_expr(input: &ToolInput) -> Result<TokenStream> {
    let name = LitStr::new(&input.name.to_string(), input.name.span());
    let kind = plugin_input_kind_expr(&input.ty, input.params.as_ref())?;
    let ctor = if input.requirement == InputRequirement::Required {
        quote! { ::upeg_plugin_api::PluginInputField::required(#name, #kind) }
    } else {
        quote! { ::upeg_plugin_api::PluginInputField::optional(#name, #kind) }
    };
    let mut field = match &input.description {
        Some(description) => quote! { #ctor.with_description(#description) },
        None => ctor,
    };
    if let Some(policy) = plugin_file_policy_expr(input.params.as_ref()) {
        field = quote! { #field.with_file_policy(#policy) };
    }
    Ok(field)
}

fn plugin_output_spec_expr(outputs: &Option<Vec<ToolOutput>>) -> Result<Option<TokenStream>> {
    let Some(outputs) = outputs else {
        return Ok(None);
    };
    let mut seen = BTreeSet::new();
    let mut fields = Vec::with_capacity(outputs.len());
    for output in outputs {
        let name = output.name.to_string();
        if !seen.insert(name.clone()) {
            return Err(syn::Error::new(
                output.name.span(),
                format!("duplicate output field `{name}`"),
            ));
        }
        fields.push(plugin_output_field_expr(output)?);
    }
    Ok(Some(quote! {
        ::upeg_plugin_api::PluginOutputSpec::new([ #( #fields ),* ])
    }))
}

fn plugin_output_field_expr(output: &ToolOutput) -> Result<TokenStream> {
    let name = LitStr::new(&output.name.to_string(), output.name.span());
    let kind = plugin_output_kind_expr(&output.ty, output.params.as_ref())?;
    let ctor = quote! { ::upeg_plugin_api::PluginOutputField::new(#name, #kind) };
    Ok(match &output.description {
        Some(description) => quote! { #ctor.with_description(#description) },
        None => ctor,
    })
}

fn plugin_input_kind_expr(ty: &Ident, params: Option<&KindParams>) -> Result<TokenStream> {
    Ok(match input_type_label(ty)? {
        "string" => {
            reject_inline_string_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginInputKind::String }
        }
        "number" => {
            reject_inline_numeric_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginInputKind::Number }
        }
        "integer" => {
            reject_inline_numeric_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginInputKind::Integer }
        }
        "boolean" => quote! { ::upeg_plugin_api::PluginInputKind::Boolean },
        "options" => {
            let choices = choices_vec_expr(params, ty)?;
            quote! { ::upeg_plugin_api::PluginInputKind::Options(#choices) }
        }
        "multi_options" => {
            let choices = choices_vec_expr(params, ty)?;
            quote! { ::upeg_plugin_api::PluginInputKind::MultiOptions(#choices) }
        }
        "markdown" => quote! { ::upeg_plugin_api::PluginInputKind::Markdown },
        "json" => quote! { ::upeg_plugin_api::PluginInputKind::Json },
        "datetime" => quote! { ::upeg_plugin_api::PluginInputKind::DateTime },
        "file_path" => quote! { ::upeg_plugin_api::PluginInputKind::FilePath },
        "url" => quote! { ::upeg_plugin_api::PluginInputKind::Url },
        "file" => quote! { ::upeg_plugin_api::PluginInputKind::File },
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("unsupported input type `{other}`"),
            ));
        }
    })
}

fn plugin_output_kind_expr(ty: &Ident, params: Option<&KindParams>) -> Result<TokenStream> {
    Ok(match plugin_output_type_label(ty)? {
        "string" => {
            reject_inline_string_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::String }
        }
        "number" => {
            reject_inline_numeric_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::Number }
        }
        "integer" => {
            reject_inline_numeric_constraints(ty, params)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::Integer }
        }
        "boolean" => quote! { ::upeg_plugin_api::PluginOutputKind::Boolean },
        "options" => {
            let choices = choices_vec_expr(params, ty)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::Options(#choices) }
        }
        "multi_options" => {
            let choices = choices_vec_expr(params, ty)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::MultiOptions(#choices) }
        }
        "markdown" => quote! { ::upeg_plugin_api::PluginOutputKind::Markdown },
        "json" => quote! { ::upeg_plugin_api::PluginOutputKind::Json },
        "datetime" => quote! { ::upeg_plugin_api::PluginOutputKind::DateTime },
        "file_path" => quote! { ::upeg_plugin_api::PluginOutputKind::FilePath },
        "url" => quote! { ::upeg_plugin_api::PluginOutputKind::Url },
        "file" => {
            reject_file_policy_on_output(ty, params)?;
            quote! { ::upeg_plugin_api::PluginOutputKind::File }
        }
        "embedded_view" => {
            let Some(KindParams::EmbedUrl(url)) = params else {
                return Err(syn::Error::new(
                    ty.span(),
                    "`EmbeddedView` requires an inline url, e.g. `EmbeddedView(\"https://...\")`",
                ));
            };
            quote! { ::upeg_plugin_api::PluginOutputKind::EmbeddedView { url: #url.to_string() } }
        }
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("unsupported output type `{other}`"),
            ));
        }
    })
}

/// Output-only type-ident → label mapping (adds `EmbeddedView`, absent
/// from the shared input-only `SUPPORTED_INPUT_TYPES` table). Mirrors
/// `upeg-macros`' local `output_type_label` — kept crate-local per design
/// §3 ("codegen: nothing shared").
fn plugin_output_type_label(ty: &Ident) -> Result<&'static str> {
    Ok(match ty.to_string().as_str() {
        "String" => "string",
        "Number" => "number",
        "Integer" => "integer",
        "Boolean" => "boolean",
        "Options" => "options",
        "MultiOptions" => "multi_options",
        "Markdown" => "markdown",
        "Json" => "json",
        "Datetime" => "datetime",
        "FilePath" => "file_path",
        "Url" => "url",
        "File" => "file",
        "EmbeddedView" => "embedded_view",
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("unsupported output type `{other}`"),
            ));
        }
    })
}

fn choices_vec_expr(params: Option<&KindParams>, ty: &Ident) -> Result<TokenStream> {
    match params {
        Some(KindParams::Choices(choices)) => {
            let entries = choices
                .iter()
                .map(|value| quote! { ::upeg_plugin_api::PluginChoiceOption::new(#value) });
            Ok(quote! { vec![ #( #entries ),* ] })
        }
        None => Ok(quote! { ::std::vec::Vec::new() }),
        Some(_) => Err(syn::Error::new(
            ty.span(),
            "`Options`/`MultiOptions` accept only a choice array — e.g. `Options([\"a\", \"b\"])`",
        )),
    }
}

/// `Number`/`Integer` carry no per-field constraints in the WASM plugin
/// input contract (`PluginInputKind`/`PluginOutputKind` have no min/max/
/// default slot) — reject inline params early with a message naming the
/// gap, instead of silently dropping them.
fn reject_inline_numeric_constraints(ty: &Ident, params: Option<&KindParams>) -> Result<()> {
    if matches!(params, Some(KindParams::Numeric { .. })) {
        return Err(syn::Error::new(
            ty.span(),
            format!(
                "`{ty}` inline constraints (min/max/default) aren't supported by the WASM plugin input contract; declare `{ty}` without inline parameters"
            ),
        ));
    }
    Ok(())
}

/// `String` carries no per-field constraints in the WASM plugin input
/// contract — see [`reject_inline_numeric_constraints`].
fn reject_inline_string_constraints(ty: &Ident, params: Option<&KindParams>) -> Result<()> {
    if matches!(params, Some(KindParams::StringConstraints { .. })) {
        return Err(syn::Error::new(
            ty.span(),
            "`String` inline constraints (regex/placeholder/default) aren't supported by the WASM plugin input contract; declare `String` without inline parameters",
        ));
    }
    Ok(())
}
