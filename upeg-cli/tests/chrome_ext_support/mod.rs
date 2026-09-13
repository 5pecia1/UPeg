#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    dead_code,
    reason = "shared test fixtures: each Chrome-extension test target uses a subset, and integration tests use unwrap/expect/panic idiomatically"
)]

//! Shared fixtures for the Chrome-extension contract tests.
//!
//! `chrome_ext.rs` pins the extension's structure (manifest shape, per-site
//! registration, detector table, selector adapter, worker) and
//! `chrome_ext_i18n.rs` pins the message catalog. Both read the same files
//! off disk and both walk the same `_locales/` layout, so the paths, the
//! catalog readers, and the key extractors live here once.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

pub const CHROME_EXT_DIR: &str = "chrome-ext";
pub const MANIFEST_JSON: &str = "manifest.json";
pub const POPUP_HTML: &str = "popup.html";
pub const POPUP_JS: &str = "popup.js";
pub const BUILD_SH: &str = "build.sh";
pub const CONTENT_JS: &str = "content.js";
pub const BACKGROUND_JS: &str = "background.js";
pub const DETECTORS_JS: &str = "detectors.js";
pub const HOST_API_JS: &str = "host_api.js";
pub const SELECTOR_ADAPTER_JS: &str = "selector_adapter.js";
pub const SITE_ACCESS_JS: &str = "site_access.js";
pub const TOOL_ROUTING_JS: &str = "tool_routing.js";
pub const LOCALES_DIR: &str = "_locales";
pub const MESSAGES_JSON: &str = "messages.json";
pub const DEFAULT_LOCALE: &str = "en";
pub const SHIPPED_LOCALES: [&str; 2] = ["en", "ko"];
pub const MESSAGE_FIELD: &str = "message";
pub const PLACEHOLDERS_FIELD: &str = "placeholders";
pub const PLACEHOLDER_CONTENT_FIELD: &str = "content";
pub const POPUP_I18N_LOOKUP_PREFIX: &str = "i18nMessage('";
pub const CONTENT_I18N_LOOKUP_PREFIX: &str = "i18nMessage('";
pub const DETECTOR_LABEL_KEY_PREFIX: &str = "labelKey: '";
pub const I18N_HTML_ATTRS: [&str; 4] = [
    "data-i18n",
    "data-i18n-title",
    "data-i18n-aria-label",
    "data-i18n-placeholder",
];
pub const MSG_REF_PREFIX: &str = "__MSG_";
pub const MSG_REF_SUFFIX: &str = "__";

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli has a workspace parent")
        .to_path_buf()
}

pub fn ext_path(name: &str) -> PathBuf {
    repo_root().join(CHROME_EXT_DIR).join(name)
}

pub fn read(name: &str) -> String {
    let path = ext_path(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

pub fn manifest() -> Value {
    let raw = read(MANIFEST_JSON);
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("manifest.json invalid JSON: {e}"))
}

pub fn locale_messages_path(locale: &str) -> PathBuf {
    repo_root()
        .join(CHROME_EXT_DIR)
        .join(LOCALES_DIR)
        .join(locale)
        .join(MESSAGES_JSON)
}

pub fn locale_messages(locale: &str) -> serde_json::Map<String, Value> {
    let path = locale_messages_path(locale);
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} invalid JSON: {e}", path.display()));
    v.as_object()
        .unwrap_or_else(|| panic!("{} must be a JSON object", path.display()))
        .clone()
}

pub fn locale_keys(locale: &str) -> BTreeSet<String> {
    locale_messages(locale).keys().cloned().collect()
}

/// Keys passed as a single-quoted first argument right after `prefix`,
/// e.g. `i18nMessage('boardEmpty')` -> `boardEmpty`. Dynamic call sites
/// (ternary key picks) are not captured; existence checks are a subset
/// gate, so that only loosens, never falsely fails.
pub fn quoted_keys_after<'a>(source: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    source
        .split(prefix)
        .skip(1)
        .filter_map(|tail| tail.split('\'').next())
        .collect()
}

/// Keys named by `data-i18n*="<key>"` attributes in popup.html.
pub fn data_i18n_keys(html: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for attr in I18N_HTML_ATTRS {
        let needle = format!("{attr}=\"");
        keys.extend(
            html.split(needle.as_str())
                .skip(1)
                .filter_map(|tail| tail.split('"').next())
                .map(str::to_string),
        );
    }
    keys
}

/// `$NAME$` placeholder tokens inside a message string, uppercased
/// (chrome.i18n matches placeholder names case-insensitively).
pub fn message_placeholder_tokens(message: &str) -> BTreeSet<String> {
    message
        .split('$')
        .enumerate()
        .filter(|(i, seg)| {
            i % 2 == 1
                && !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@')
        })
        .map(|(_, seg)| seg.to_ascii_uppercase())
        .collect()
}

pub fn assert_keys_resolve_in_all_locales(referenced: &BTreeSet<&str>, source_label: &str) {
    assert!(
        !referenced.is_empty(),
        "{source_label}: key extraction found nothing — the scan prefix/attribute list is stale"
    );
    for locale in SHIPPED_LOCALES {
        let catalog = locale_keys(locale);
        for key in referenced {
            assert!(
                catalog.contains(*key),
                "{source_label} references i18n key `{key}` missing from _locales/{locale}/{MESSAGES_JSON}"
            );
        }
    }
}
