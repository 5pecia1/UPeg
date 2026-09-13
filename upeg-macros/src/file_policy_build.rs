use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Result};

use super::parse::KindParams;

const DEFAULT_FILE_MAX_COUNT: u32 = 1;

pub(super) fn static_file_input_kind_expr(
    ty: &Ident,
    params: Option<&KindParams>,
) -> Result<TokenStream> {
    let (extensions, max_count, max_file_bytes, max_total_bytes) = match params {
        Some(KindParams::FilePolicy {
            extensions,
            max_count,
            max_file_bytes,
            max_total_bytes,
        }) => (
            extensions.as_slice(),
            max_count.unwrap_or(DEFAULT_FILE_MAX_COUNT),
            *max_file_bytes,
            *max_total_bytes,
        ),
        None => (&[][..], DEFAULT_FILE_MAX_COUNT, None, None),
        Some(_) => {
            return Err(syn::Error::new(
                ty.span(),
                "`File` accepts only extensions/max_count/max_file_bytes/max_total_bytes",
            ));
        }
    };
    let max_count = syn::LitInt::new(&max_count.to_string(), proc_macro2::Span::call_site());
    let max_file_bytes = optional_u64_expr(max_file_bytes);
    let max_total_bytes = optional_u64_expr(max_total_bytes);

    Ok(quote! {
        ::upeg_core::StaticInputKind::File(::upeg_core::StaticFileInputPolicy {
            extensions: &[ #( #extensions ),* ],
            max_count: #max_count,
            max_file_bytes: #max_file_bytes,
            max_total_bytes: #max_total_bytes,
        })
    })
}

fn optional_u64_expr(value: Option<u64>) -> TokenStream {
    if let Some(value) = value {
        let literal = syn::LitInt::new(&value.to_string(), proc_macro2::Span::call_site());
        quote! { Some(#literal) }
    } else {
        quote! { None }
    }
}
