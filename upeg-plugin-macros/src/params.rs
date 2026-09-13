//! Classifies a guest tool fn's parameter types into the closed set
//! `#[tool]` supports, producing both the type-label family used for
//! `inputs = [...]` compatibility checking ([`signature`](super::signature))
//! and the concrete extraction/call strategy used for wrapper codegen
//! ([`build`](super::build)).
//!
//! Kept crate-local by design (§3 of the plugin-macro design doc): the
//! built-in `upeg-macros` classifies signatures against `StaticInputKind`
//! and rejects `Option<T>`; the guest classifies against
//! `::upeg_plugin_api::FromPluginArg` impls and *requires* `Option<T>` for
//! optional inputs. Only the closed type-label family names are shared,
//! via `upeg_tool_grammar::input_type_label`.

use syn::{FnArg, ItemFn, Pat, Type, spanned::Spanned};

/// The closed set of scalar type-label families a DSL `inputs`/`outputs`
/// field can declare, mirroring `upeg-macros`' `ParamKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamFamily {
    StringLike,
    Integer,
    Number,
    Boolean,
    JsonValue,
}

/// How the wrapper extracts and calls a single guest fn parameter.
pub(crate) enum ParamBinding {
    /// A plain owned scalar/JSON param (`String`, an integer type, `f32`/
    /// `f64`, `bool`, `serde_json::Value`): extracted via
    /// `<Ty as FromPluginArg>::from_plugin_arg` and passed by value.
    Owned(Type),
    /// `&str` (or `&String`): `FromPluginArg` cannot bind a borrow, so the
    /// wrapper extracts an owned `String` local and passes `&local`.
    StrRef,
    /// `Option<T>` over one of the [`Self::Owned`] scalar types.
    OptionOwned(Type),
    /// `Option<&str>`: extracted as `Option<String>`, passed via
    /// `.as_deref()`.
    OptionStrRef,
}

impl ParamBinding {
    pub(crate) const fn is_option(&self) -> bool {
        matches!(self, Self::OptionOwned(_) | Self::OptionStrRef)
    }
}

/// One classified fn parameter: its declared name, DSL-compatible type
/// family, and wrapper binding strategy.
pub(crate) struct GuestParam {
    pub(crate) name: syn::Ident,
    pub(crate) family: ParamFamily,
    pub(crate) binding: ParamBinding,
}

/// Collect and classify every parameter of the annotated fn, in order.
///
/// # Errors
///
/// Returns a span-pointed error for a `self` receiver, a non-identifier
/// pattern (destructuring), or a parameter type outside the closed set
/// this attribute supports.
pub(crate) fn collect_guest_params(item_fn: &ItemFn) -> syn::Result<Vec<GuestParam>> {
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
                let (family, binding) =
                    classify_guest_param_type(pat_type.ty.as_ref()).ok_or_else(|| {
                        let ty = pat_type.ty.as_ref();
                        syn::Error::new(
                            pat_type.ty.span(),
                            format!(
                                "unsupported `#[tool]` parameter type `{}`; supported: &str/String, integers, floats, bool, serde_json::Value, and Option<...> over any of those",
                                quote::quote! { #ty }
                            ),
                        )
                    })?;
                Ok(GuestParam {
                    name: pat_ident.ident.clone(),
                    family,
                    binding,
                })
            }
        })
        .collect()
}

fn classify_guest_param_type(ty: &Type) -> Option<(ParamFamily, ParamBinding)> {
    if let Some(inner) = option_inner_type(ty) {
        return classify_option_inner(inner);
    }
    match ty {
        Type::Reference(reference) => {
            let family = classify_str_like_reference(reference.elem.as_ref())?;
            Some((family, ParamBinding::StrRef))
        }
        Type::Path(path) => {
            let family = classify_owned_path(path)?;
            Some((family, ParamBinding::Owned(ty.clone())))
        }
        _ => None,
    }
}

fn classify_option_inner(inner: &Type) -> Option<(ParamFamily, ParamBinding)> {
    match inner {
        Type::Reference(reference) => {
            let family = classify_str_like_reference(reference.elem.as_ref())?;
            Some((family, ParamBinding::OptionStrRef))
        }
        Type::Path(path) => {
            let family = classify_owned_path(path)?;
            Some((family, ParamBinding::OptionOwned(inner.clone())))
        }
        _ => None,
    }
}

/// `Option<T>`'s inner `T`, or `None` if `ty` isn't an `Option<...>` path.
fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(generics) = &segment.arguments else {
        return None;
    };
    match generics.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn classify_str_like_reference(ty: &Type) -> Option<ParamFamily> {
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?.ident.to_string();
    matches!(last.as_str(), "str" | "String").then_some(ParamFamily::StringLike)
}

fn classify_owned_path(path: &syn::TypePath) -> Option<ParamFamily> {
    let last = path.path.segments.last()?.ident.to_string();
    match last.as_str() {
        "String" => Some(ParamFamily::StringLike),
        "usize" | "u8" | "u16" | "u32" | "u64" | "u128" | "isize" | "i8" | "i16" | "i32"
        | "i64" | "i128" => Some(ParamFamily::Integer),
        "f32" | "f64" => Some(ParamFamily::Number),
        "bool" => Some(ParamFamily::Boolean),
        "Value" => Some(ParamFamily::JsonValue),
        _ => None,
    }
}

/// Whether an `inputs = [...]` field's closed type-label (`"string"`,
/// `"integer"`, ...) is compatible with a classified parameter family.
///
/// Mirrors `upeg-macros`' `param_kind_accepts_label`: string-typed DSL
/// kinds (`Options`, `Markdown`, `Json`, `Datetime`, `FilePath`, `Url`, in
/// addition to plain `String`) all bind to a string-like Rust parameter,
/// since the guest only ever sees their JSON-encoded string form.
pub(crate) fn family_accepts_label(family: ParamFamily, label: &str) -> bool {
    match family {
        ParamFamily::StringLike => matches!(
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
        ParamFamily::Integer => label == "integer",
        ParamFamily::Number => label == "number",
        ParamFamily::Boolean => label == "boolean",
        ParamFamily::JsonValue => label == "json",
    }
}

/// Human-readable expected-label list for a family, used in mismatch
/// error messages.
pub(crate) const fn family_expected_labels(family: ParamFamily) -> &'static str {
    match family {
        ParamFamily::StringLike => {
            "String, Options, MultiOptions, Markdown, Json, Datetime, FilePath, or Url"
        }
        ParamFamily::Integer => "Integer",
        ParamFamily::Number => "Number",
        ParamFamily::Boolean => "Boolean",
        ParamFamily::JsonValue => "Json",
    }
}
