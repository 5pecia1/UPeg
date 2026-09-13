//! Tool/toolkit identity rules shared by both macro crates: `id` must be a
//! canonical, unpadded `{toolkit}.{local_name}` string literally prefixed
//! by `toolkit`.

use syn::{LitStr, Result};

/// Validates that `id` is a non-empty, canonical `{toolkit}.{tool}`
/// identifier prefixed by `toolkit`, and returns the local tool name (the
/// suffix after `toolkit.`).
pub fn validate_tool_identity(id: &LitStr, toolkit: &LitStr) -> Result<String> {
    let id_value = id.value();
    let toolkit_value = toolkit.value();
    let id_trimmed = id_value.trim();
    let toolkit_trimmed = toolkit_value.trim();
    if id_trimmed.is_empty() {
        return Err(syn::Error::new(
            id.span(),
            "`id` must be a non-empty `{toolkit}.{tool}` identifier",
        ));
    }
    if id_value != id_trimmed {
        return Err(syn::Error::new(
            id.span(),
            "`id` must be canonical and unpadded",
        ));
    }
    if toolkit_trimmed.is_empty() {
        return Err(syn::Error::new(
            toolkit.span(),
            "`toolkit` must be a non-empty Toolkit id",
        ));
    }
    if toolkit_value != toolkit_trimmed {
        return Err(syn::Error::new(
            toolkit.span(),
            "`toolkit` must be canonical and unpadded",
        ));
    }
    if has_padded_dot_component(toolkit_trimmed) {
        return Err(syn::Error::new(
            toolkit.span(),
            "`toolkit` must be canonical and contain unpadded dot components",
        ));
    }
    if has_padded_dot_component(id_trimmed) {
        return Err(syn::Error::new(
            id.span(),
            "`id` must be canonical and contain unpadded dot components",
        ));
    }
    let Some(rest) = id_trimmed.strip_prefix(toolkit_trimmed) else {
        return Err(syn::Error::new(
            id.span(),
            format!("`id` must start with its owning Toolkit prefix `{toolkit_trimmed}.`"),
        ));
    };
    let Some(local) = rest.strip_prefix('.') else {
        return Err(syn::Error::new(
            id.span(),
            format!("`id` must start with its owning Toolkit prefix `{toolkit_trimmed}.`"),
        ));
    };
    if local.is_empty() {
        return Err(syn::Error::new(
            id.span(),
            "`id` must include a non-empty local tool name after the Toolkit prefix",
        ));
    }
    Ok(local.to_string())
}

fn has_padded_dot_component(value: &str) -> bool {
    value.split('.').any(|part| part != part.trim())
}
