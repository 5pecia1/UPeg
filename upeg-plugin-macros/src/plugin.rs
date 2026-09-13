//! `upeg_plugin! { toolkit: "...", tags: [...], description: "...", tools: [f1, f2] }`
//! aggregator (design §1/§7): expands to the plugin's `manifest` extism
//! export, calling each listed tool fn's hidden `__upeg_tool_decl_*`
//! accessor.
//!
//! No collection magic — `inventory`/link-section registries don't run
//! reliably on `wasm32-unknown-unknown` reactor guests (§1 of the design
//! doc). Every listed tool fn must exist in scope, so a typo in `tools`
//! surfaces as a plain "cannot find function" compile error pointing at
//! the exact token — the one-line-per-tool re-listing is the entire cost.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Ident, LitStr, Result, Token, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::build::decl_symbol_ident;

pub(crate) struct PluginArgs {
    pub(crate) toolkit: LitStr,
    pub(crate) tags: Vec<LitStr>,
    pub(crate) description: Option<LitStr>,
    pub(crate) tools: Vec<Ident>,
}

impl Parse for PluginArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut toolkit: Option<LitStr> = None;
        let mut tags: Option<Vec<LitStr>> = None;
        let mut description: Option<LitStr> = None;
        let mut tools: Option<Vec<Ident>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "toolkit" => toolkit = Some(input.parse()?),
                "tags" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<LitStr, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    tags = Some(items.into_iter().collect());
                }
                "description" => description = Some(input.parse()?),
                "tools" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    tools = Some(items.into_iter().collect());
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown upeg_plugin! argument `{other}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let toolkit = toolkit.ok_or_else(|| input.error("missing `toolkit: \"...\"`"))?;
        let tools = tools.ok_or_else(|| input.error("missing `tools: [...]`"))?;
        if tools.is_empty() {
            return Err(input.error("`tools` must list at least one #[tool]-annotated fn"));
        }

        let mut seen = HashSet::new();
        for tool in &tools {
            if !seen.insert(tool.to_string()) {
                return Err(syn::Error::new(
                    tool.span(),
                    format!("duplicate tool `{tool}` in `tools`"),
                ));
            }
        }

        Ok(Self {
            toolkit,
            tags: tags.unwrap_or_default(),
            description,
            tools,
        })
    }
}

/// Build the `manifest` extism export from a parsed `upeg_plugin! {...}`
/// invocation (design §7's "after" expansion).
pub(crate) fn build_plugin_manifest(args: &PluginArgs) -> TokenStream {
    let toolkit_lit = &args.toolkit;
    let mut chain = quote! { ::upeg_plugin_api::PluginManifest::new(#toolkit_lit) };
    if !args.tags.is_empty() {
        let tag_lits = &args.tags;
        chain = quote! { #chain.with_tags([ #( #tag_lits ),* ]) };
    }
    if let Some(description) = &args.description {
        chain = quote! { #chain.with_description(#description) };
    }
    for tool in &args.tools {
        let decl_ident = decl_symbol_ident(tool);
        chain = quote! { #chain.with_tool(#decl_ident()) };
    }

    quote! {
        #[::extism_pdk::plugin_fn]
        pub fn manifest(_: ()) -> ::extism_pdk::FnResult<::std::string::String> {
            let __m = #chain;
            ::std::result::Result::Ok(__m.to_json()?)
        }
    }
}
