//! Argument parsing for the `#[tool]` / `#[toolkit]` macros. Split out of
//! `lib.rs` so the macro entry points stay focused and each file stays
//! below the workspace's file-size budget.
//!
//! The shared `inputs`/`outputs` field grammar (`ToolInput`, `ToolOutput`,
//! `KindParams`, `InputRequirement`, `input_type_label`, ...) lives in
//! `upeg-tool-grammar` and is re-exported here so the rest of this crate
//! keeps using unqualified names. Only the top-level `#[tool(...)]` /
//! `#[toolkit(...)]` argument parsers stay local — they accept a different
//! key set (`invoker`/`boards`/`source`) than the WASM-guest macro that
//! also consumes the shared grammar crate.

use syn::{
    Ident, LitStr, Result, Token, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

#[cfg(test)]
pub(super) use upeg_tool_grammar::SUPPORTED_INPUT_TYPES;
pub(super) use upeg_tool_grammar::{
    InputRequirement, KindParams, ToolInput, ToolOutput, input_type_label,
};

pub(super) struct ToolArgs {
    pub(super) id: LitStr,
    pub(super) toolkit: LitStr,
    pub(super) display_label: Option<LitStr>,
    pub(super) description: Option<LitStr>,
    pub(super) inputs: Option<Vec<ToolInput>>,
    pub(super) outputs: Option<Vec<ToolOutput>>,
    pub(super) source: Option<SourceArg>,
    pub(super) tags: Vec<LitStr>,
    pub(super) pin: Ident,
    pub(super) pegboard_units: Ident,
    pub(super) invoker: Ident,
    pub(super) surfaces: Option<Vec<Ident>>,
    pub(super) boards: Vec<LitStr>,
}

pub(super) enum SourceArg {
    UserInput,
    Manual,
    Static,
    Timer(LitStr),
    Shortcut(LitStr),
}

pub(super) struct ToolkitArgs {
    pub(super) id: LitStr,
    pub(super) tags: Vec<LitStr>,
    pub(super) description: Option<LitStr>,
}

impl Parse for ToolkitArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id: Option<LitStr> = None;
        let mut tags: Option<Vec<LitStr>> = None;
        let mut description: Option<LitStr> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let key_s = key.to_string();
            match key_s.as_str() {
                "id" => id = Some(input.parse()?),
                "tags" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<LitStr, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    tags = Some(items.into_iter().collect());
                }
                "description" => description = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[toolkit] argument `{other}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self {
            id: id.ok_or_else(|| input.error("missing `id = \"...\"`"))?,
            tags: tags.unwrap_or_default(),
            description,
        })
    }
}

impl Parse for ToolArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id: Option<LitStr> = None;
        let mut toolkit: Option<LitStr> = None;
        let mut display_label: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut inputs: Option<Vec<ToolInput>> = None;
        let mut outputs: Option<Vec<ToolOutput>> = None;
        let mut source: Option<SourceArg> = None;
        let mut tags: Option<Vec<LitStr>> = None;
        let mut pin: Option<Ident> = None;
        let mut pegboard_units: Option<Ident> = None;
        let mut invoker: Option<Ident> = None;
        let mut surfaces: Option<Vec<Ident>> = None;
        let mut boards: Option<Vec<LitStr>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let key_s = key.to_string();
            match key_s.as_str() {
                "id" => id = Some(input.parse()?),
                "toolkit" => toolkit = Some(input.parse()?),
                "display_label" => display_label = Some(input.parse()?),
                "description" => description = Some(input.parse()?),
                "inputs" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<ToolInput, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    inputs = Some(items.into_iter().collect());
                }
                "outputs" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<ToolOutput, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    outputs = Some(items.into_iter().collect());
                }
                "source" => {
                    source = Some(input.parse()?);
                }
                "tags" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<LitStr, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    tags = Some(items.into_iter().collect());
                }
                "pin" => pin = Some(input.parse()?),
                "pegboard_units" => pegboard_units = Some(input.parse()?),
                "invoker" => invoker = Some(input.parse()?),
                "surfaces" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    surfaces = Some(items.into_iter().collect());
                }
                "boards" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<LitStr, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    boards = Some(items.into_iter().collect());
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[tool] argument `{other}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self {
            id: id.ok_or_else(|| input.error("missing `id = \"...\"`"))?,
            toolkit: toolkit.ok_or_else(|| input.error("missing `toolkit = \"...\"`"))?,
            display_label,
            description,
            inputs,
            outputs,
            source,
            tags: tags.unwrap_or_default(),
            pin: pin.unwrap_or_else(|| Ident::new("Inline", proc_macro2::Span::call_site())),
            pegboard_units: pegboard_units
                .ok_or_else(|| input.error("missing `pegboard_units = U1|U2|U2T`"))?,
            invoker: invoker
                .unwrap_or_else(|| Ident::new("Function", proc_macro2::Span::call_site())),
            surfaces,
            boards: boards.unwrap_or_default(),
        })
    }
}

impl Parse for SourceArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let label = ident.to_string();
        match label.as_str() {
            "UserInput" => Ok(Self::UserInput),
            "Manual" => Ok(Self::Manual),
            "Static" => Ok(Self::Static),
            "Timer" => {
                let content;
                parenthesized!(content in input);
                let interval: LitStr = content.parse()?;
                if !content.is_empty() {
                    return Err(content.error("`Timer(\"30s\")` accepts a single string literal"));
                }
                Ok(Self::Timer(interval))
            }
            "Shortcut" => {
                let content;
                parenthesized!(content in input);
                let keys: LitStr = content.parse()?;
                if !content.is_empty() {
                    return Err(content
                        .error("`Shortcut(\"Cmd+Shift+N\")` accepts a single string literal"));
                }
                Ok(Self::Shortcut(keys))
            }
            other => Err(syn::Error::new(
                ident.span(),
                format!(
                    "unknown source variant `{other}` (expected UserInput/Manual/Static/Timer(\"…\")/Shortcut(\"…\"))"
                ),
            )),
        }
    }
}
