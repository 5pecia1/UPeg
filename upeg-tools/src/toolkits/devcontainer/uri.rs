//! Pure URI / payload parsing for `devcontainer`, extracted from the parent to
//! keep every file under the 1000-line workspace budget.
//!
//! Nothing here touches the filesystem — these are string/JSON transforms over
//! `vscode-remote://dev-container+<hex>` URIs, whose authority carries a
//! hex-encoded JSON payload describing the host workspace. The whole module is
//! native-only (mounted behind `#[cfg(not(target_arch = "wasm32"))]` in the
//! parent) because only the native build has an implementation to feed.

use super::{
    AUTHORITY_SEPARATOR, DEV_AUTH_PREFIX, DEVCONTAINER_URI_MARKER, FILE_URI_PREFIX,
    HOST_WORKSPACE_KEYS, KEY_CONFIG_FILE, KEY_DEVCONTAINER_PATH, KEY_HOST, KEY_HOST_NAME,
    KEY_SETTINGS, SCHEME_SEPARATOR, SSH_REMOTE_PREFIX, SSH_SCHEME_PREFIX, URI_OBJECT_PATH_KEYS,
    VSCODE_REMOTE_SCHEME,
};

/// Radix of the hex-encoded authority payload.
const HEX_RADIX: u32 = 16;
/// Bytes of hex text per decoded byte.
const HEX_DIGITS_PER_BYTE: usize = 2;

/// Character class terminating a Dev Container URI embedded in a larger blob:
/// whitespace, quotes, a backslash, or any JSON structural closer.
const URI_TERMINATOR_CLASS: &str = r#"[^\s"'\\\]\},]+"#;

/// The parsed pieces of a `vscode-remote://dev-container+...` URI, shared by
/// the `workspace.json` and `state.vscdb` entry loaders.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ParsedUri {
    pub(super) container_path: String,
    pub(super) host_workspace_path: String,
    pub(super) devcontainer_config_path: String,
    pub(super) remote_authority: String,
    pub(super) remote_host: String,
    pub(super) devcontainer_authority: String,
    pub(super) decoded: serde_json::Value,
}

/// Percent-decode leniently, keeping the original text on malformed input.
pub(super) fn url_decode(value: &str) -> String {
    urlencoding::decode(value).map_or_else(|_| value.to_string(), std::borrow::Cow::into_owned)
}

/// Decode an ASCII-hex string. Returns `None` on any non-hex byte or on an odd
/// length — a trailing half-byte means the input is not hex-encoded data, so
/// truncating it would silently accept corrupt input.
pub(super) fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(HEX_DIGITS_PER_BYTE) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / HEX_DIGITS_PER_BYTE);
    for pair in bytes.chunks_exact(HEX_DIGITS_PER_BYTE) {
        let hi = char::from(pair[0]).to_digit(HEX_RADIX)?;
        let lo = char::from(pair[1]).to_digit(HEX_RADIX)?;
        out.push(u8::try_from(hi * HEX_RADIX + lo).ok()?);
    }
    Some(out)
}

/// Decode a hex-encoded UTF-8 JSON *object*: non-hex, odd-length, empty,
/// non-UTF-8, non-JSON, or non-object inputs all yield `None`.
pub(super) fn decode_hex_json(value: &str) -> Option<serde_json::Value> {
    if value.is_empty() {
        return None;
    }
    let text = String::from_utf8(decode_hex(value)?).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
    parsed.is_object().then_some(parsed)
}

/// Split a URI into `(scheme, authority, path)`, dropping any query/fragment.
/// A URI without a `://` authority yields an empty scheme/authority and the
/// whole input as the path.
pub(super) fn split_uri(uri: &str) -> (String, String, String) {
    let Some((scheme, rest)) = uri.split_once(SCHEME_SEPARATOR) else {
        return (String::new(), String::new(), uri.to_string());
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..authority_end].to_string();
    let after = &rest[authority_end..];
    let path_end = after.find(['?', '#']).unwrap_or(after.len());
    (scheme.to_string(), authority, after[..path_end].to_string())
}

/// Convert a `file://` URI to a plain filesystem path (percent-decoded),
/// returning non-`file://` input unchanged. On Windows a leading slash before
/// a drive letter (`/C:/…`) is stripped.
pub(super) fn file_uri_to_path(value: &str) -> String {
    if !value.starts_with(FILE_URI_PREFIX) {
        return value.to_string();
    }
    let (_, _, path) = split_uri(value);
    let decoded = url_decode(&path);
    if cfg!(target_os = "windows") {
        let bytes = decoded.as_bytes();
        if bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
        {
            return decoded[1..].to_string();
        }
    }
    decoded
}

/// First non-empty string value among `keys`, in order.
pub(super) fn first_nonempty_str(value: &serde_json::Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(found) = value.get(*key).and_then(serde_json::Value::as_str)
            && !found.is_empty()
        {
            return found.to_string();
        }
    }
    String::new()
}

/// Extract a filesystem path from a decoded URI value that may be a plain
/// `file://` string or an object with `fsPath`/`path`/`external`.
pub(super) fn uri_obj_path(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => file_uri_to_path(text),
        Some(object @ serde_json::Value::Object(_)) => {
            let raw = first_nonempty_str(object, URI_OBJECT_PATH_KEYS);
            if raw.is_empty() {
                String::new()
            } else {
                file_uri_to_path(&raw)
            }
        }
        _ => String::new(),
    }
}

/// Decode an outer remote authority into `(authority, host)`. An
/// `ssh-remote+<hex>` authority has its host name extracted from the decoded
/// payload where possible.
pub(super) fn decode_remote_authority(authority: &str) -> (String, String) {
    if authority.is_empty() {
        return (String::new(), String::new());
    }
    let authority = url_decode(authority);
    if let Some(raw_host) = authority.strip_prefix(SSH_REMOTE_PREFIX) {
        let host = match decode_hex_json(raw_host) {
            Some(decoded) => {
                let name = first_nonempty_str(&decoded, &[KEY_HOST_NAME, KEY_HOST]);
                if name.is_empty() {
                    raw_host.to_string()
                } else {
                    name
                }
            }
            None => raw_host.to_string(),
        };
        return (authority, host);
    }
    (authority.clone(), authority)
}

/// Parse a `vscode-remote://dev-container+...` URI into its component paths.
/// Non-Dev-Container URIs yield `None`.
pub(super) fn parse_devcontainer_uri(uri: &str) -> Option<ParsedUri> {
    let (scheme, raw_authority, path) = split_uri(uri);
    if scheme != VSCODE_REMOTE_SCHEME {
        return None;
    }
    let authority = url_decode(&raw_authority);
    let (dev_authority, outer_authority) = match authority.split_once(AUTHORITY_SEPARATOR) {
        Some((dev, outer)) => (dev.to_string(), outer.to_string()),
        None => (authority, String::new()),
    };
    let encoded = dev_authority.strip_prefix(DEV_AUTH_PREFIX)?;
    let decoded = decode_hex_json(encoded).unwrap_or_else(|| serde_json::json!({}));

    let (mut remote_authority, mut remote_host) = decode_remote_authority(&outer_authority);
    let settings = decoded.get(KEY_SETTINGS).filter(|value| value.is_object());
    if remote_host.is_empty()
        && let Some(host) = settings
            .and_then(|value| value.get(KEY_HOST))
            .and_then(serde_json::Value::as_str)
    {
        remote_authority = host.to_string();
        remote_host = host
            .strip_prefix(SSH_SCHEME_PREFIX)
            .unwrap_or(host)
            .to_string();
    }

    let host_workspace_path = first_nonempty_str(&decoded, HOST_WORKSPACE_KEYS);

    let mut config_path = decoded
        .get(KEY_DEVCONTAINER_PATH)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if config_path.is_empty() {
        config_path = uri_obj_path(decoded.get(KEY_CONFIG_FILE));
    }

    Some(ParsedUri {
        container_path: url_decode(&path),
        host_workspace_path,
        devcontainer_config_path: config_path,
        remote_authority,
        remote_host,
        devcontainer_authority: dev_authority,
        decoded,
    })
}

/// Find the first embedded Dev Container URI in a blob (as stored in
/// `debug.selectedroot` / `history.entries`), or an empty string when the blob
/// holds none. The pattern is built from [`DEVCONTAINER_URI_MARKER`] so the
/// const stays the single source of truth.
pub(super) fn extract_first_devcontainer_uri(value: &str) -> String {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let regex = PATTERN.get_or_init(|| {
        #[allow(
            clippy::expect_used,
            reason = "the pattern is built from a constant marker plus a constant character class, so it is always valid"
        )]
        regex::Regex::new(&format!(
            "{}{URI_TERMINATOR_CLASS}",
            regex::escape(DEVCONTAINER_URI_MARKER)
        ))
        .expect("devcontainer URI regex built from constants is valid")
    });
    regex
        .find(value)
        .map_or_else(String::new, |matched| matched.as_str().to_string())
}

/// Normalize a URI for rank matching: percent-decode, then drop trailing `/`.
pub(super) fn norm_uri(uri: &str) -> String {
    url_decode(uri).trim_end_matches('/').to_string()
}
