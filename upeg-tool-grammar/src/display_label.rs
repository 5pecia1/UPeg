//! Display-label precedence shared by both `#[tool]`-style attribute
//! macros: an explicit `display_label = "..."` wins, then the annotated
//! function's first rustdoc line, then its prettified local-id slug. Moved
//! from `upeg-macros` so `upeg-plugin-macros` shares this exact precedence
//! (see this crate's top-level docs).

use syn::{Expr, ItemFn, Meta};

/// Extract the first non-empty line of an item's outer `///` rustdoc
/// comment, if any.
pub fn rustdoc_first_line(item_fn: &ItemFn) -> Option<String> {
    item_fn.attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("doc") {
            return None;
        }
        match &attr.meta {
            Meta::NameValue(name_value) => match &name_value.value {
                Expr::Lit(expr_lit) => match &expr_lit.lit {
                    syn::Lit::Str(lit) => lit
                        .value()
                        .lines()
                        .map(str::trim)
                        .find(|line| !line.is_empty())
                        .map(ToOwned::to_owned),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    })
}

/// Prettify a `{toolkit}.{local_id}`-style slug's local component into a
/// human-readable display label, e.g. `hex_to_decimal` -> `Hex To Decimal`.
#[must_use]
pub fn display_label_from_slug(value: &str) -> String {
    value
        .rsplit_once('.')
        .map_or(value, |(_, local)| local)
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(display_label_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_label_word(word: &str) -> String {
    match word.to_ascii_lowercase().as_str() {
        "api" => "API".to_string(),
        "base32" => "Base32".to_string(),
        "base64" => "Base64".to_string(),
        "crc32" => "CRC32".to_string(),
        "csv" => "CSV".to_string(),
        "html" => "HTML".to_string(),
        "id" => "ID".to_string(),
        "iso" => "ISO".to_string(),
        "json" => "JSON".to_string(),
        "md5" => "MD5".to_string(),
        "nanoid" => "NanoID".to_string(),
        "qr" => "QR".to_string(),
        "rgb" => "RGB".to_string(),
        "sha1" => "SHA-1".to_string(),
        "sha256" => "SHA-256".to_string(),
        "sha512" => "SHA-512".to_string(),
        "url" => "URL".to_string(),
        "uuid" => "UUID".to_string(),
        lower => {
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    #[test]
    fn extracts_first_rustdoc_line() {
        let item_fn: ItemFn = parse_str(
            "
            /// Greet a person by name.
            ///
            /// More detail here.
            pub fn greet_hello(name: &str) -> String { String::new() }
            ",
        )
        .unwrap();

        assert_eq!(
            rustdoc_first_line(&item_fn).as_deref(),
            Some("Greet a person by name.")
        );
    }

    #[test]
    fn returns_none_without_rustdoc() {
        let item_fn: ItemFn =
            parse_str("pub fn greet_hello(name: &str) -> String { String::new() }").unwrap();

        assert_eq!(rustdoc_first_line(&item_fn), None);
    }

    #[test]
    fn slug_converts_separators_to_spaces_and_capitalizes_words() {
        assert_eq!(display_label_from_slug("hex_to_decimal"), "Hex To Decimal");
        assert_eq!(
            display_label_from_slug("num.hex_to_decimal"),
            "Hex To Decimal"
        );
    }

    #[test]
    fn slug_preserves_known_acronyms() {
        assert_eq!(display_label_from_slug("uuid_generate"), "UUID Generate");
        assert_eq!(display_label_from_slug("json_to_csv"), "JSON To CSV");
    }
}
