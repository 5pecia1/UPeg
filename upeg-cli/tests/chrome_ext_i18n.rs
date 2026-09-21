#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::get_unwrap,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

//! Message-catalog contract for the Chrome extension
//! (chrome.i18n + `_locales/{en,ko}/messages.json`).
//!
//! popup.js/content.js carry no user-facing literals; they reference
//! messages.json keys, popup.html carries `data-i18n*` attributes that
//! `localizeStaticDom` resolves at startup, and detectors.js names its
//! label keys as table data. These tests pin the catalog contract: valid
//! JSON, En/Ko key parity, placeholder consistency, that every referenced
//! key resolves in every shipped locale (chrome.i18n.getMessage returns ""
//! for a missing key — a silent runtime blank), and that no key is orphaned.

use serde_json::Value;
use std::collections::BTreeSet;

mod chrome_ext_support;
use chrome_ext_support::{
    BACKGROUND_JS, CONTENT_I18N_LOOKUP_PREFIX, CONTENT_JS, DEFAULT_LOCALE,
    DETECTOR_LABEL_KEY_PREFIX, DETECTORS_JS, HOST_API_JS, I18N_HTML_ATTRS, MANIFEST_JSON,
    MESSAGE_FIELD, MESSAGES_JSON, MSG_REF_PREFIX, MSG_REF_SUFFIX, PLACEHOLDER_CONTENT_FIELD,
    PLACEHOLDERS_FIELD, POPUP_HTML, POPUP_I18N_LOOKUP_PREFIX, POPUP_JS, SHIPPED_LOCALES,
    SITE_ACCESS_JS, TOOL_ROUTING_JS, assert_keys_resolve_in_all_locales, data_i18n_keys,
    locale_keys, locale_messages, manifest, message_placeholder_tokens, quoted_keys_after, read,
};

// The 503 fallback message key. 503 is transport-level ("the host answered
// but is not serving requests"), so the key names the host — there is no
// `rest-api` service to be disabled any more.
const HOST_UNAVAILABLE_KEY: &str = "hostUnavailable";

// === i18n migration (chrome.i18n + _locales/{en,ko}/messages.json) ===
//
// popup.js/content.js no longer carry user-facing literals; they reference
// messages.json keys and popup.html carries `data-i18n*` attributes that
// `localizeStaticDom` resolves at startup. These tests pin the catalog
// contract: valid JSON, En/Ko key parity, placeholder consistency, and
// that every key referenced from JS/HTML/manifest resolves in every
// shipped locale (chrome.i18n.getMessage returns "" for a missing key —
// a silent runtime blank, so the gate lives here).

#[test]
fn locale_catalogs_are_valid_json_with_nonempty_messages() {
    for locale in SHIPPED_LOCALES {
        let messages = locale_messages(locale);
        assert!(
            !messages.is_empty(),
            "_locales/{locale}/{MESSAGES_JSON} must not be empty"
        );
        for (key, entry) in &messages {
            let text = entry
                .get(MESSAGE_FIELD)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("[{locale}] `{key}` needs a string `{MESSAGE_FIELD}`"));
            assert!(
                !text.is_empty(),
                "[{locale}] `{key}` message must not be empty"
            );
        }
    }
}

#[test]
fn locale_catalogs_keep_en_ko_key_parity() {
    let baseline = locale_keys(DEFAULT_LOCALE);
    for locale in SHIPPED_LOCALES {
        let keys = locale_keys(locale);
        let missing: Vec<&String> = baseline.difference(&keys).collect();
        let extra: Vec<&String> = keys.difference(&baseline).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "locale `{locale}` diverges from `{DEFAULT_LOCALE}`: missing {missing:?}, extra {extra:?}"
        );
    }
}

#[test]
fn locale_catalog_placeholders_match_declaration_and_use_and_keep_en_ko_parity() {
    let baseline = locale_messages(DEFAULT_LOCALE);
    for locale in SHIPPED_LOCALES {
        let messages = locale_messages(locale);
        for (key, entry) in &messages {
            let text = entry[MESSAGE_FIELD].as_str().unwrap();
            let used = message_placeholder_tokens(text);
            let declared: BTreeSet<String> = entry
                .get(PLACEHOLDERS_FIELD)
                .and_then(Value::as_object)
                .map(|m| m.keys().map(|k| k.to_ascii_uppercase()).collect())
                .unwrap_or_default();
            assert_eq!(
                used, declared,
                "[{locale}] `{key}`: $TOKEN$ usage in the message must exactly match the `{PLACEHOLDERS_FIELD}` declaration"
            );
            // Substitution wiring ($1, $2, …) must be identical across
            // locales — call sites pass one positional array for all.
            let baseline_entry = &baseline[key];
            let content_of = |e: &Value, name: &str| {
                e[PLACEHOLDERS_FIELD][name][PLACEHOLDER_CONTENT_FIELD].clone()
            };
            if let Some(placeholders) = entry.get(PLACEHOLDERS_FIELD).and_then(Value::as_object) {
                for name in placeholders.keys() {
                    assert_eq!(
                        content_of(entry, name),
                        content_of(baseline_entry, name),
                        "[{locale}] `{key}` placeholder `{name}` substitution index diverges from `{DEFAULT_LOCALE}`"
                    );
                }
            }
        }
    }
}

#[test]
fn i18n_keys_referenced_by_popup_js_exist_in_every_locale() {
    let js = read(POPUP_JS);
    let referenced = quoted_keys_after(&js, POPUP_I18N_LOOKUP_PREFIX);
    assert_keys_resolve_in_all_locales(&referenced, POPUP_JS);
    // The migrated 503 fallback must stay keyed (not a re-hardcoded literal).
    assert!(
        referenced.contains(HOST_UNAVAILABLE_KEY),
        "popup.js must reference `{HOST_UNAVAILABLE_KEY}` for the 503 fallback"
    );
}

#[test]
fn i18n_keys_referenced_by_content_js_exist_in_every_locale() {
    let js = read(CONTENT_JS);
    let referenced = quoted_keys_after(&js, CONTENT_I18N_LOOKUP_PREFIX);
    assert_keys_resolve_in_all_locales(&referenced, CONTENT_JS);
}

#[test]
fn detector_table_label_keys_exist_in_every_locale() {
    // content.js resolves a row's label through `labelKeyFor`, so the key
    // never appears as a literal at the chrome.i18n call site — the table
    // is where it has to be checked.
    let js = read(DETECTORS_JS);
    let referenced = quoted_keys_after(&js, DETECTOR_LABEL_KEY_PREFIX);
    assert_keys_resolve_in_all_locales(&referenced, DETECTORS_JS);
}

#[test]
fn popup_html_data_i18n_attributes_reference_keys_in_every_locale() {
    let html = read(POPUP_HTML);
    let keys = data_i18n_keys(&html);
    let referenced: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    assert_keys_resolve_in_all_locales(&referenced, POPUP_HTML);
}

#[test]
fn manifest_declares_a_default_locale_and_a_localized_description() {
    let v = manifest();
    assert_eq!(
        v["default_locale"], DEFAULT_LOCALE,
        "manifest must declare default_locale `{DEFAULT_LOCALE}` for chrome.i18n fallback"
    );
    let description = v["description"].as_str().expect("description string");
    let key = description
        .strip_prefix(MSG_REF_PREFIX)
        .and_then(|rest| rest.strip_suffix(MSG_REF_SUFFIX))
        .unwrap_or_else(|| {
            panic!("manifest description must be a `{MSG_REF_PREFIX}<key>{MSG_REF_SUFFIX}` catalog reference, got `{description}`")
        });
    let referenced: BTreeSet<&str> = BTreeSet::from([key]);
    assert_keys_resolve_in_all_locales(&referenced, MANIFEST_JSON);
}

#[test]
fn the_locale_catalog_keeps_no_orphaned_keys() {
    // Every catalog entry must be reachable from JS, popup.html, or the
    // manifest. A key nobody references is dead weight that survives every
    // other gate here (they are all subset checks).
    // Deliberately looser than the subset gates above: a key picked by a
    // ternary (`state.token ? 'tokenRejected' : 'tokenRequired'`) is a real
    // reference the call-site scan cannot see, so this one asks only
    // "does the key appear as a quoted literal anywhere the extension ships".
    let sources: Vec<String> = [
        POPUP_JS,
        CONTENT_JS,
        DETECTORS_JS,
        POPUP_HTML,
        MANIFEST_JSON,
        SITE_ACCESS_JS,
        TOOL_ROUTING_JS,
        HOST_API_JS,
        BACKGROUND_JS,
    ]
    .iter()
    .map(|name| read(name))
    .collect();

    let orphans: Vec<String> = locale_keys(DEFAULT_LOCALE)
        .into_iter()
        .filter(|key| {
            let single = format!("'{key}'");
            let double = format!("\"{key}\"");
            let msg_ref = format!("{MSG_REF_PREFIX}{key}{MSG_REF_SUFFIX}");
            !sources.iter().any(|source| {
                source.contains(&single) || source.contains(&double) || source.contains(&msg_ref)
            })
        })
        .collect();
    assert!(
        orphans.is_empty(),
        "_locales/{DEFAULT_LOCALE}/{MESSAGES_JSON} carries keys nothing references: {orphans:?}"
    );
}

#[test]
fn popup_js_keeps_the_static_dom_localization_wiring() {
    let js = read(POPUP_JS);
    for marker in [
        // Single lookup point — user-facing strings never re-inline.
        "const i18nMessage = (key, substitutions) => chrome.i18n.getMessage(key, substitutions);",
        // Static popup.html nodes are localized once at startup.
        "function localizeStaticDom()",
        "localizeStaticDom();",
    ] {
        assert!(
            js.contains(marker),
            "popup.js must keep i18n wiring marker `{marker}`"
        );
    }
    for attr in I18N_HTML_ATTRS {
        assert!(
            js.contains(attr),
            "popup.js localizeStaticDom must keep handling `{attr}`"
        );
    }
}
