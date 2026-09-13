use proc_macro2::TokenStream;
use quote::quote;
use upeg_tool_grammar::KindParams;

pub(crate) fn plugin_file_policy_expr(params: Option<&KindParams>) -> Option<TokenStream> {
    let Some(KindParams::FilePolicy {
        extensions,
        max_count,
        max_file_bytes,
        max_total_bytes,
    }) = params
    else {
        return None;
    };

    let max_count = if let Some(value) = max_count {
        quote! { #value }
    } else {
        quote! { ::upeg_plugin_api::DEFAULT_PLUGIN_FILE_MAX_COUNT }
    };
    let max_file_bytes = option_u64_expr(*max_file_bytes);
    let max_total_bytes = option_u64_expr(*max_total_bytes);

    Some(quote! {
        ::upeg_plugin_api::PluginFileInputPolicy {
            max_count: #max_count,
            extensions: vec![ #( #extensions.to_string() ),* ],
            max_file_bytes: #max_file_bytes,
            max_total_bytes: #max_total_bytes,
        }
    })
}

fn option_u64_expr(value: Option<u64>) -> TokenStream {
    if let Some(value) = value {
        quote! { ::std::option::Option::Some(#value) }
    } else {
        quote! { ::std::option::Option::None }
    }
}
