//! Argument parsing for the guest `#[tool(...)]` attribute.
//!
//! Accepts the same `inputs`/`outputs` DSL as the built-in macro (shared
//! via `upeg_tool_grammar`), but a different top-level key set: `invoker`,
//! `boards`, and `source` are forbidden here — the host always runs plugin
//! tools with `invoker = Wasm`, never schedules them onto boards, and has
//! no plugin-side scheduling `source` concept. See design §5.

use syn::{
    Ident, LitStr, Result, Token, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use upeg_tool_grammar::{ToolInput, ToolOutput};

/// `invoker` is not allowed on `#[upeg::tool]`: the host always runs
/// plugin tools with `invoker = Wasm`, so there is nothing for the author
/// to choose.
const FORBIDDEN_INVOKER_MESSAGE: &str = "`invoker` is not allowed on `#[upeg::tool]`: the host always runs plugin tools with `invoker = Wasm`. Remove this key.";
/// `boards` is not allowed: the host always registers plugin tools with an
/// empty board list, the built-in-only scheduling grouping concept.
const FORBIDDEN_BOARDS_MESSAGE: &str = "`boards` is not allowed on `#[upeg::tool]`: the host does not use scheduling boards for wasm plugin tools. Remove this key.";
/// `source` is not allowed: it is a built-in-only compile-time scheduling
/// concept (Timer/Shortcut/...) absent from the plugin manifest contract.
const FORBIDDEN_SOURCE_MESSAGE: &str = "`source` is not allowed on `#[upeg::tool]`: `source` is a built-in-only scheduling concept absent from the plugin contract. Remove this key.";

pub(crate) struct ToolArgs {
    pub(crate) id: LitStr,
    pub(crate) toolkit: LitStr,
    pub(crate) display_label: Option<LitStr>,
    pub(crate) description: Option<LitStr>,
    pub(crate) inputs: Option<Vec<ToolInput>>,
    pub(crate) outputs: Option<Vec<ToolOutput>>,
    pub(crate) tags: Vec<LitStr>,
    pub(crate) pin: Option<Ident>,
    pub(crate) pegboard_units: Ident,
    pub(crate) surfaces: Option<Vec<Ident>>,
}

impl Parse for ToolArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id: Option<LitStr> = None;
        let mut toolkit: Option<LitStr> = None;
        let mut display_label: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut inputs: Option<Vec<ToolInput>> = None;
        let mut outputs: Option<Vec<ToolOutput>> = None;
        let mut tags: Option<Vec<LitStr>> = None;
        let mut pin: Option<Ident> = None;
        let mut pegboard_units: Option<Ident> = None;
        let mut surfaces: Option<Vec<Ident>> = None;

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
                "tags" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<LitStr, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    tags = Some(items.into_iter().collect());
                }
                "pin" => pin = Some(input.parse()?),
                "pegboard_units" => pegboard_units = Some(input.parse()?),
                "surfaces" => {
                    let content;
                    bracketed!(content in input);
                    let items: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    surfaces = Some(items.into_iter().collect());
                }
                "invoker" => return Err(syn::Error::new(key.span(), FORBIDDEN_INVOKER_MESSAGE)),
                "boards" => return Err(syn::Error::new(key.span(), FORBIDDEN_BOARDS_MESSAGE)),
                "source" => return Err(syn::Error::new(key.span(), FORBIDDEN_SOURCE_MESSAGE)),
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
            tags: tags.unwrap_or_default(),
            pin,
            pegboard_units: pegboard_units
                .ok_or_else(|| input.error("missing `pegboard_units = U1|U2|U2T`"))?,
            surfaces,
        })
    }
}
