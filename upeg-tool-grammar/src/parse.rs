//! The `inputs = [...]` / `outputs = [...]` DSL: the typed field syntax used
//! by both the built-in `#[upeg_core::tool]` macro and the WASM-guest
//! `#[upeg::tool]` macro to describe a tool's parameters. Moved out of
//! `upeg-macros` so the two macro crates parse identical grammar instead of
//! hand-kept copies (see this crate's top-level docs).

use syn::{
    Ident, LitFloat, LitInt, LitStr, Result, Token, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::file_policy::parse_file_policy_params;

include!("input_types.inc.rs");

/// One `required`/`optional` field inside a `#[tool(inputs = [...])]` list.
pub struct ToolInput {
    pub requirement: InputRequirement,
    pub name: Ident,
    pub ty: Ident,
    pub params: Option<KindParams>,
    pub description: Option<LitStr>,
}

/// One field inside a `#[tool(outputs = [...])]` list.
pub struct ToolOutput {
    pub name: Ident,
    pub ty: Ident,
    pub params: Option<KindParams>,
    pub description: Option<LitStr>,
}

/// Inline parameters for an input/output type, e.g.
/// `Number(min=1, max=255, default=128)` or `Options(["a", "b"])`.
pub enum KindParams {
    /// `Options(["a","b"])` / `MultiOptions(["a","b"])` — choice value list.
    Choices(Vec<LitStr>),
    /// `Number(min=…, max=…, default=…)` / `Integer(...)` — numeric range.
    Numeric {
        min: Option<f64>,
        max: Option<f64>,
        default: Option<f64>,
    },
    /// `String(regex=…, placeholder=…, default=…)` — string constraints.
    StringConstraints {
        regex: Option<LitStr>,
        placeholder: Option<LitStr>,
        default: Option<LitStr>,
    },
    /// `EmbeddedView("https://...")` — output-only iframe URL.
    EmbedUrl(LitStr),
    FilePolicy {
        extensions: Vec<LitStr>,
        max_count: Option<u32>,
        max_file_bytes: Option<u64>,
        max_total_bytes: Option<u64>,
    },
}

/// Whether an `inputs = [...]` field is `required` or `optional`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputRequirement {
    Required,
    Optional,
}

impl Parse for ToolInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let requirement_ident: Ident = input.parse()?;
        let requirement = match requirement_ident.to_string().as_str() {
            "required" => InputRequirement::Required,
            "optional" => InputRequirement::Optional,
            other => {
                return Err(syn::Error::new(
                    requirement_ident.span(),
                    format!("input field must start with `required` or `optional`, got `{other}`"),
                ));
            }
        };
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Ident = input.parse()?;
        let params = parse_optional_kind_params(input, &ty)?;
        let description = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self {
            requirement,
            name,
            ty,
            params,
            description,
        })
    }
}

impl Parse for ToolOutput {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Ident = input.parse()?;
        let params = parse_optional_kind_params(input, &ty)?;
        let description = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self {
            name,
            ty,
            params,
            description,
        })
    }
}

fn parse_optional_kind_params(input: ParseStream, ty: &Ident) -> Result<Option<KindParams>> {
    if !input.peek(syn::token::Paren) {
        return Ok(None);
    }
    let content;
    parenthesized!(content in input);
    let ty_label = ty.to_string();
    let params = match ty_label.as_str() {
        "Options" | "MultiOptions" => parse_choices_params(&content, ty)?,
        "Number" | "Integer" => parse_numeric_params(&content)?,
        "String" => parse_string_constraints(&content)?,
        "File" => {
            let policy = parse_file_policy_params(&content)?;
            KindParams::FilePolicy {
                extensions: policy.extensions,
                max_count: policy.max_count,
                max_file_bytes: policy.max_file_bytes,
                max_total_bytes: policy.max_total_bytes,
            }
        }
        "EmbeddedView" => {
            let url: LitStr = content.parse()?;
            // Reject extra tokens so typos surface early.
            if !content.is_empty() {
                return Err(content.error("`EmbeddedView(\"url\")` accepts a single string"));
            }
            KindParams::EmbedUrl(url)
        }
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("type `{other}` does not accept inline parameters"),
            ));
        }
    };
    Ok(Some(params))
}

fn parse_choices_params(content: ParseStream, ty: &Ident) -> Result<KindParams> {
    // Either `["a","b"]` (preferred) or comma-separated literals for legacy.
    if content.peek(syn::token::Bracket) {
        let inner;
        bracketed!(inner in content);
        let items: Punctuated<LitStr, Token![,]> = Punctuated::parse_terminated(&inner)?;
        if !content.is_empty() {
            return Err(content.error(format!(
                "`{}` takes a single choice-value array",
                ty.to_string().as_str()
            )));
        }
        Ok(KindParams::Choices(items.into_iter().collect()))
    } else {
        Err(content.error(format!(
            "`{}(...)` expects an array of string literals, e.g. `[\"a\", \"b\"]`",
            ty.to_string().as_str()
        )))
    }
}

fn parse_numeric_params(content: ParseStream) -> Result<KindParams> {
    let mut min: Option<f64> = None;
    let mut max: Option<f64> = None;
    let mut default: Option<f64> = None;
    while !content.is_empty() {
        let key: Ident = content.parse()?;
        content.parse::<Token![=]>()?;
        let value = parse_numeric_literal(content)?;
        match key.to_string().as_str() {
            "min" => min = Some(value),
            "max" => max = Some(value),
            "default" => default = Some(value),
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unknown numeric parameter `{other}` (expected min/max/default)"),
                ));
            }
        }
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    Ok(KindParams::Numeric { min, max, default })
}

fn parse_numeric_literal(content: ParseStream) -> Result<f64> {
    // Accept integer or float literals; negative values via leading `-`.
    let negative = content.peek(Token![-]);
    if negative {
        content.parse::<Token![-]>()?;
    }
    let value: f64 = if content.peek(LitFloat) {
        let lit: LitFloat = content.parse()?;
        lit.base10_parse::<f64>()?
    } else {
        let lit: LitInt = content.parse()?;
        lit.base10_parse::<i64>()? as f64
    };
    Ok(if negative { -value } else { value })
}

fn parse_string_constraints(content: ParseStream) -> Result<KindParams> {
    let mut regex: Option<LitStr> = None;
    let mut placeholder: Option<LitStr> = None;
    let mut default: Option<LitStr> = None;
    while !content.is_empty() {
        let key: Ident = content.parse()?;
        content.parse::<Token![=]>()?;
        let value: LitStr = content.parse()?;
        match key.to_string().as_str() {
            "regex" => regex = Some(value),
            "placeholder" => placeholder = Some(value),
            "default" => default = Some(value),
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "unknown string parameter `{other}` (expected regex/placeholder/default)"
                    ),
                ));
            }
        }
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    Ok(KindParams::StringConstraints {
        regex,
        placeholder,
        default,
    })
}

/// Maps a DSL type ident (`String`, `Integer`, `Options`, ...) to its
/// closed-set label (`"string"`, `"integer"`, `"options"`, ...), or a
/// compile error naming the supported set.
pub fn input_type_label(ty: &Ident) -> Result<&'static str> {
    let ty_name = ty.to_string();
    if let Some((_, label)) = SUPPORTED_INPUT_TYPES
        .iter()
        .find(|(ident, _)| *ident == ty_name)
    {
        return Ok(*label);
    }
    let supported = SUPPORTED_INPUT_TYPES
        .iter()
        .map(|(ident, _)| *ident)
        .collect::<Vec<_>>()
        .join(", ");
    Err(syn::Error::new(
        ty.span(),
        format!("unknown input type `{ty_name}` (use one of: {supported})"),
    ))
}
