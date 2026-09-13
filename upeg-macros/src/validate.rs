//! Validation for the `#[tool]` macro: the `inputs = [...]` ↔
//! function-signature match. Split out of `lib.rs` so the macro entry
//! points stay focused and each file stays below the workspace's
//! file-size budget.
//!
//! Tool identity rules (`validate_tool_identity`) and the enum-ident
//! allow-lists (`ALLOWED_PIN_KINDS` etc. + `validate_enum_ident`) live in
//! `upeg-tool-grammar` — shared with the WASM-guest macro crate — and are
//! re-exported from `lib.rs`. Only `validate_inputs_match_signature` stays
//! here: it rejects `Option<T>` parameters, which is specific to the
//! built-in macro (the guest variant's signature check is the opposite).

use quote::quote;
use syn::{FnArg, ItemFn, Pat, Result, Type, spanned::Spanned};

use super::parse::{ToolInput, input_type_label};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParamKind {
    StringLike,
    Integer,
    Number,
    Boolean,
    JsonValue,
    File,
}

struct FunctionParam {
    name: String,
    kind: ParamKind,
}

pub(super) fn validate_inputs_match_signature(
    inputs: &Option<Vec<ToolInput>>,
    item_fn: &ItemFn,
) -> Result<()> {
    let declared = inputs.as_deref().unwrap_or(&[]);
    let params = collect_function_params(item_fn)?;

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
        if input_name != param.name {
            return Err(syn::Error::new(
                input.name.span(),
                format!(
                    "`inputs` field #{position} is `{input_name}`, but the function parameter is `{}`",
                    param.name
                ),
            ));
        }
        let label = input_type_label(&input.ty)?;
        if !param_kind_accepts_label(param.kind, label) {
            return Err(syn::Error::new(
                input.ty.span(),
                format!(
                    "`inputs` field `{input_name}` uses `{}`, but function parameter `{}` is compatible with {}",
                    input.ty,
                    param.name,
                    param_kind_expected_labels(param.kind)
                ),
            ));
        }
    }

    Ok(())
}

fn collect_function_params(item_fn: &ItemFn) -> Result<Vec<FunctionParam>> {
    item_fn
        .sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(receiver) => Err(syn::Error::new(
                receiver.self_token.span,
                "`#[tool]` can only annotate free functions",
            )),
            FnArg::Typed(pat_type) => {
                let Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                    return Err(syn::Error::new(
                        pat_type.pat.span(),
                        "`#[tool]` function parameters must be plain identifiers",
                    ));
                };
                let kind = classify_rust_param_type(pat_type.ty.as_ref()).ok_or_else(|| {
                    let ty = pat_type.ty.as_ref();
                    syn::Error::new(
                        pat_type.ty.span(),
                        format!(
                            "unsupported `#[tool]` parameter type `{}`; supported: &str/String, integers, floats, bool, serde_json::Value, &FileValue/FileValue",
                            quote! { #ty }
                        ),
                    )
                })?;
                Ok(FunctionParam {
                    name: pat_ident.ident.to_string(),
                    kind,
                })
            }
        })
        .collect()
}

fn classify_rust_param_type(ty: &Type) -> Option<ParamKind> {
    match ty {
        Type::Reference(reference) => classify_referenced_param_type(reference.elem.as_ref()),
        Type::Path(path) => classify_path_param_type(path),
        _ => None,
    }
}

fn classify_referenced_param_type(ty: &Type) -> Option<ParamKind> {
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?.ident.to_string();
    match last.as_str() {
        "str" | "String" => Some(ParamKind::StringLike),
        "Value" => Some(ParamKind::JsonValue),
        "FileValue" => Some(ParamKind::File),
        _ => None,
    }
}

fn classify_path_param_type(path: &syn::TypePath) -> Option<ParamKind> {
    let last = path.path.segments.last()?.ident.to_string();
    match last.as_str() {
        "String" => Some(ParamKind::StringLike),
        "usize" | "u8" | "u16" | "u32" | "u64" | "u128" | "isize" | "i8" | "i16" | "i32"
        | "i64" | "i128" => Some(ParamKind::Integer),
        "f32" | "f64" => Some(ParamKind::Number),
        "bool" => Some(ParamKind::Boolean),
        "Value" => Some(ParamKind::JsonValue),
        "FileValue" => Some(ParamKind::File),
        _ => None,
    }
}

fn param_kind_accepts_label(kind: ParamKind, label: &str) -> bool {
    match kind {
        ParamKind::StringLike => matches!(
            label,
            "string"
                | "options"
                | "multi_options"
                | "markdown"
                | "json"
                | "datetime"
                | "file_path"
                | "url"
        ),
        ParamKind::Integer => label == "integer",
        ParamKind::Number => label == "number",
        ParamKind::Boolean => label == "boolean",
        ParamKind::JsonValue => label == "json",
        ParamKind::File => label == "file",
    }
}

const fn param_kind_expected_labels(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::StringLike => {
            "String, Options, MultiOptions, Markdown, Json, Datetime, FilePath, or Url"
        }
        ParamKind::Integer => "Integer",
        ParamKind::Number => "Number",
        ParamKind::Boolean => "Boolean",
        ParamKind::JsonValue => "Json",
        ParamKind::File => "File",
    }
}
