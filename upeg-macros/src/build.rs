//! Token-stream builders for the `#[tool]` macro. Split out of `lib.rs`
//! so the parser surface stays focused and each file stays below the
//! workspace's file-size budget.

use quote::quote;
use syn::{Ident, LitStr, Result};
use upeg_tool_grammar::reject_file_policy_on_output;

use super::file_policy_build::static_file_input_kind_expr;
use super::parse::{
    InputRequirement, KindParams, SourceArg, ToolInput, ToolOutput, input_type_label,
};

pub(super) fn build_static_input_spec_expr(
    inputs: &Option<Vec<ToolInput>>,
) -> Result<proc_macro2::TokenStream> {
    let Some(inputs) = inputs else {
        return Ok(quote! { ::upeg_core::StaticInputSpec::empty() });
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut fields = Vec::with_capacity(inputs.len());

    for input in inputs {
        let name = input.name.to_string();
        if !seen.insert(name.clone()) {
            return Err(syn::Error::new(
                input.name.span(),
                format!("duplicate input field `{name}`"),
            ));
        }

        let name_lit = LitStr::new(&name, input.name.span());
        let required = syn::LitBool::new(
            input.requirement == InputRequirement::Required,
            input.name.span(),
        );
        let description = if let Some(description) = &input.description {
            quote! { Some(#description) }
        } else {
            quote! { None }
        };
        let kind = static_input_kind_expr(&input.ty, input.params.as_ref())?;
        let constraints = static_field_constraints_expr(input.params.as_ref());
        fields.push(quote! {
            ::upeg_core::StaticInputFieldSpec {
                name: #name_lit,
                label: None,
                description: #description,
                required: #required,
                kind: #kind,
                constraints: #constraints,
            }
        });
    }

    Ok(quote! {
        ::upeg_core::StaticInputSpec {
            fields: &[ #( #fields ),* ],
        }
    })
}

pub(super) fn build_static_output_spec_expr(
    outputs: &Option<Vec<ToolOutput>>,
) -> Result<proc_macro2::TokenStream> {
    let Some(outputs) = outputs else {
        return Ok(quote! { ::upeg_core::StaticOutputSpec::empty() });
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut fields = Vec::with_capacity(outputs.len());

    for output in outputs {
        let name = output.name.to_string();
        if !seen.insert(name.clone()) {
            return Err(syn::Error::new(
                output.name.span(),
                format!("duplicate output field `{name}`"),
            ));
        }
        let name_lit = LitStr::new(&name, output.name.span());
        let description = if let Some(description) = &output.description {
            quote! { Some(#description) }
        } else {
            quote! { None }
        };
        let kind = static_output_kind_expr(&output.ty, output.params.as_ref())?;
        let constraints = static_field_constraints_expr(output.params.as_ref());
        fields.push(quote! {
            ::upeg_core::StaticOutputFieldSpec {
                name: #name_lit,
                label: None,
                description: #description,
                kind: #kind,
                constraints: #constraints,
            }
        });
    }

    Ok(quote! {
        ::upeg_core::StaticOutputSpec {
            fields: &[ #( #fields ),* ],
        }
    })
}

pub(super) fn static_input_kind_expr(
    ty: &Ident,
    params: Option<&KindParams>,
) -> Result<proc_macro2::TokenStream> {
    Ok(match input_type_label(ty)? {
        "string" => quote! { ::upeg_core::StaticInputKind::String },
        "number" => quote! { ::upeg_core::StaticInputKind::Number },
        "integer" => quote! { ::upeg_core::StaticInputKind::Integer },
        "boolean" => quote! { ::upeg_core::StaticInputKind::Boolean },
        "options" => {
            let choices = choices_array_expr(params, ty)?;
            quote! { ::upeg_core::StaticInputKind::Options(#choices) }
        }
        "multi_options" => {
            let choices = choices_array_expr(params, ty)?;
            quote! { ::upeg_core::StaticInputKind::MultiOptions(#choices) }
        }
        "markdown" => quote! { ::upeg_core::StaticInputKind::Markdown },
        "json" => quote! { ::upeg_core::StaticInputKind::Json },
        "datetime" => quote! { ::upeg_core::StaticInputKind::DateTime },
        "file_path" => quote! { ::upeg_core::StaticInputKind::FilePath },
        "url" => quote! { ::upeg_core::StaticInputKind::Url },
        "file" => static_file_input_kind_expr(ty, params)?,
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("unsupported input type `{other}`"),
            ));
        }
    })
}

pub(super) fn static_output_kind_expr(
    ty: &Ident,
    params: Option<&KindParams>,
) -> Result<proc_macro2::TokenStream> {
    let label = output_type_label(ty)?;
    Ok(match label {
        "string" => quote! { ::upeg_core::StaticOutputKind::String },
        "number" => quote! { ::upeg_core::StaticOutputKind::Number },
        "integer" => quote! { ::upeg_core::StaticOutputKind::Integer },
        "boolean" => quote! { ::upeg_core::StaticOutputKind::Boolean },
        "options" => {
            let choices = choices_array_expr(params, ty)?;
            quote! { ::upeg_core::StaticOutputKind::Options(#choices) }
        }
        "multi_options" => {
            let choices = choices_array_expr(params, ty)?;
            quote! { ::upeg_core::StaticOutputKind::MultiOptions(#choices) }
        }
        "markdown" => quote! { ::upeg_core::StaticOutputKind::Markdown },
        "json" => quote! { ::upeg_core::StaticOutputKind::Json },
        "datetime" => quote! { ::upeg_core::StaticOutputKind::DateTime },
        "file_path" => quote! { ::upeg_core::StaticOutputKind::FilePath },
        "url" => quote! { ::upeg_core::StaticOutputKind::Url },
        "file" => {
            reject_file_policy_on_output(ty, params)?;
            quote! { ::upeg_core::StaticOutputKind::File }
        }
        "embedded_view" => {
            let Some(KindParams::EmbedUrl(url)) = params else {
                return Err(syn::Error::new(
                    ty.span(),
                    "`EmbeddedView` requires an inline url, e.g. `EmbeddedView(\"https://...\")`",
                ));
            };
            quote! { ::upeg_core::StaticOutputKind::EmbeddedView { url: #url } }
        }
        other => {
            return Err(syn::Error::new(
                ty.span(),
                format!("unsupported output type `{other}`"),
            ));
        }
    })
}

pub(super) fn choices_array_expr(
    params: Option<&KindParams>,
    ty: &Ident,
) -> Result<proc_macro2::TokenStream> {
    match params {
        Some(KindParams::Choices(choices)) => {
            let entries = choices.iter().map(|value| {
                quote! {
                    ::upeg_core::StaticChoiceOption {
                        value: #value,
                        label: None,
                        description: None,
                    }
                }
            });
            Ok(quote! { &[ #( #entries ),* ] })
        }
        None => Ok(quote! { &[] }),
        Some(_) => Err(syn::Error::new(
            ty.span(),
            "`Options`/`MultiOptions` accept only a choice array — e.g. `Options([\"a\", \"b\"])`",
        )),
    }
}

pub(super) fn static_field_constraints_expr(
    params: Option<&KindParams>,
) -> proc_macro2::TokenStream {
    match params {
        Some(KindParams::Numeric { min, max, default }) => {
            let min_expr = optional_f64_expr(*min);
            let max_expr = optional_f64_expr(*max);
            let default_expr = optional_f64_expr(*default);
            quote! {
                ::upeg_core::StaticFieldConstraints {
                    number: Some(::upeg_core::StaticNumberConstraints {
                        min: #min_expr,
                        max: #max_expr,
                        default: #default_expr,
                    }),
                    string: None,
                }
            }
        }
        Some(KindParams::StringConstraints {
            regex,
            placeholder,
            default,
        }) => {
            let regex_expr = optional_litstr_expr(regex.as_ref());
            let placeholder_expr = optional_litstr_expr(placeholder.as_ref());
            let default_expr = optional_litstr_expr(default.as_ref());
            quote! {
                ::upeg_core::StaticFieldConstraints {
                    number: None,
                    string: Some(::upeg_core::StaticStringConstraints {
                        regex: #regex_expr,
                        placeholder: #placeholder_expr,
                        default: #default_expr,
                    }),
                }
            }
        }
        _ => quote! { ::upeg_core::StaticFieldConstraints::empty() },
    }
}

fn optional_f64_expr(value: Option<f64>) -> proc_macro2::TokenStream {
    if let Some(v) = value {
        quote! { Some(#v) }
    } else {
        quote! { None }
    }
}

fn optional_litstr_expr(value: Option<&LitStr>) -> proc_macro2::TokenStream {
    if let Some(v) = value {
        quote! { Some(#v) }
    } else {
        quote! { None }
    }
}

fn output_type_label(ty: &Ident) -> Result<&'static str> {
    let label = ty.to_string();
    Ok(match label.as_str() {
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

pub(super) fn build_source_expr(source: Option<&SourceArg>) -> Result<proc_macro2::TokenStream> {
    let Some(source) = source else {
        return Ok(quote! { ::upeg_core::StaticSource::UserInput });
    };
    Ok(match source {
        SourceArg::UserInput => quote! { ::upeg_core::StaticSource::UserInput },
        SourceArg::Manual => quote! { ::upeg_core::StaticSource::Manual },
        SourceArg::Static => quote! { ::upeg_core::StaticSource::Static },
        SourceArg::Timer(interval) => {
            let ms = parse_duration_to_ms(interval)?;
            let ms_lit = syn::LitInt::new(&ms.to_string(), interval.span());
            quote! { ::upeg_core::StaticSource::Timer { interval_ms: #ms_lit } }
        }
        SourceArg::Shortcut(keys) => {
            quote! { ::upeg_core::StaticSource::Shortcut { keys: #keys } }
        }
    })
}

/// Parse a human-readable duration literal (`"30s"`, `"500ms"`, `"1m"`,
/// `"1m30s"`, …) and return its length in milliseconds.
///
/// Backed by [`humantime::parse_duration`] so we don't reimplement the
/// number-and-unit grammar ourselves. The compile-time scheduler uses ms,
/// so durations above `u64::MAX` ms are rejected at the macro boundary.
pub(super) fn parse_duration_to_ms(lit: &LitStr) -> Result<u64> {
    let raw = lit.value();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(syn::Error::new(
            lit.span(),
            "duration must not be empty (e.g. \"30s\", \"500ms\", \"1m\", \"1m30s\")",
        ));
    }
    let duration = humantime::parse_duration(trimmed).map_err(|err| {
        syn::Error::new(
            lit.span(),
            format!(
                "duration `{raw}` could not be parsed by humantime: {err} \
                 (expected e.g. \"500ms\", \"30s\", \"1m\", \"1m30s\")"
            ),
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        syn::Error::new(
            lit.span(),
            format!("duration `{raw}` is too large to represent in milliseconds"),
        )
    })
}
