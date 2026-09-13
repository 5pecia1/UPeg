use syn::{
    Ident, LitInt, LitStr, Result, Token, bracketed, parse::ParseStream, punctuated::Punctuated,
};

use crate::parse::KindParams;

const EXTENSIONS_PARAMETER: &str = "extensions";
const MAX_COUNT_PARAMETER: &str = "max_count";
const MAX_FILE_BYTES_PARAMETER: &str = "max_file_bytes";
const MAX_TOTAL_BYTES_PARAMETER: &str = "max_total_bytes";
const EXPECTED_PARAMETERS: &str = "extensions/max_count/max_file_bytes/max_total_bytes";

pub(crate) struct ParsedFilePolicyParams {
    pub(crate) extensions: Vec<LitStr>,
    pub(crate) max_count: Option<u32>,
    pub(crate) max_file_bytes: Option<u64>,
    pub(crate) max_total_bytes: Option<u64>,
}

pub(crate) fn parse_file_policy_params(content: ParseStream<'_>) -> Result<ParsedFilePolicyParams> {
    let mut extensions: Option<Vec<LitStr>> = None;
    let mut max_count: Option<u32> = None;
    let mut max_file_bytes: Option<u64> = None;
    let mut max_total_bytes: Option<u64> = None;

    while !content.is_empty() {
        let key: Ident = content.parse()?;
        content.parse::<Token![=]>()?;

        match key.to_string().as_str() {
            EXTENSIONS_PARAMETER => {
                reject_duplicate(&extensions, &key)?;
                extensions = Some(parse_extensions(content)?);
            }
            MAX_COUNT_PARAMETER => {
                reject_duplicate(&max_count, &key)?;
                max_count = Some(parse_u32(content, &key)?);
            }
            MAX_FILE_BYTES_PARAMETER => {
                reject_duplicate(&max_file_bytes, &key)?;
                max_file_bytes = Some(parse_u64(content, &key)?);
            }
            MAX_TOTAL_BYTES_PARAMETER => {
                reject_duplicate(&max_total_bytes, &key)?;
                max_total_bytes = Some(parse_u64(content, &key)?);
            }
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unknown file parameter `{other}` (expected {EXPECTED_PARAMETERS})"),
                ));
            }
        }

        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }

    Ok(ParsedFilePolicyParams {
        extensions: extensions.unwrap_or_default(),
        max_count,
        max_file_bytes,
        max_total_bytes,
    })
}

pub fn reject_file_policy_on_output(ty: &Ident, params: Option<&KindParams>) -> Result<()> {
    if matches!(params, Some(KindParams::FilePolicy { .. })) {
        return Err(syn::Error::new(
            ty.span(),
            "`File(...)` policy is input-only; output `File` does not accept inline parameters",
        ));
    }
    Ok(())
}

fn reject_duplicate<T>(value: &Option<T>, key: &Ident) -> Result<()> {
    if value.is_some() {
        return Err(syn::Error::new(
            key.span(),
            format!("duplicate file parameter `{key}`"),
        ));
    }
    Ok(())
}

fn parse_extensions(content: ParseStream<'_>) -> Result<Vec<LitStr>> {
    if !content.peek(syn::token::Bracket) {
        return Err(content
            .error("`extensions` expects an array of string literals, e.g. `[\"png\", \"jpg\"]`"));
    }

    let entries;
    bracketed!(entries in content);
    let values: Punctuated<LitStr, Token![,]> = Punctuated::parse_terminated(&entries)?;
    Ok(values.into_iter().collect())
}

fn parse_u32(content: ParseStream<'_>, key: &Ident) -> Result<u32> {
    let literal = parse_unsigned_literal(content, key)?;
    literal.base10_parse::<u32>()
}

fn parse_u64(content: ParseStream<'_>, key: &Ident) -> Result<u64> {
    let literal = parse_unsigned_literal(content, key)?;
    literal.base10_parse::<u64>()
}

fn parse_unsigned_literal(content: ParseStream<'_>, key: &Ident) -> Result<LitInt> {
    if !content.peek(LitInt) {
        return Err(content.error(format!("`{key}` expects an unsigned integer literal")));
    }
    content.parse()
}
