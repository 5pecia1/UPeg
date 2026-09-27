use super::*;

#[test]
fn every_english_key_has_a_korean_translation() {
    // Adding an entry to EN without a matching KO entry would silently
    // serve English under Locale::Ko via the core fallback chain —
    // intentional for a future-language onboarding ramp, but unwanted
    // here because Ko is a shipping locale we want fully translated.
    let missing: Vec<&str> = EN
        .keys()
        .chain(surface_io::EN.keys())
        .chain(board_guidance::EN.keys())
        .chain(media::EN.keys())
        .copied()
        .filter(|key| catalog(Locale::Ko, key).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "Ko catalog is missing translations for: {missing:?}",
    );
}

#[test]
fn there_are_no_orphan_korean_keys() {
    // A Ko-only key would never be reached — the core lookup hits Ko
    // first, but every call site identifies a key by the literal it
    // wrote in EN. An orphan Ko entry means a stale catalog row.
    let orphans: Vec<&str> = KO
        .keys()
        .chain(surface_io::KO.keys())
        .chain(board_guidance::KO.keys())
        .chain(media::KO.keys())
        .copied()
        .filter(|key| catalog(Locale::En, key).is_none())
        .collect();
    assert!(
        orphans.is_empty(),
        "Ko catalog has orphan keys (no matching En entry): {orphans:?}",
    );
}

#[test]
fn all_cheatsheet_label_keys_exist_in_both_locales() {
    // The cheatsheet renders `upeg_core::binding_catalog()` labels
    // through this catalog — a missing key would leak the raw i18n
    // key (`???` marker chain) into the overlay.
    for locale in [Locale::En, Locale::Ko] {
        for scope_bindings in upeg_core::binding_catalog() {
            assert!(
                catalog(locale, scope_bindings.label_key).is_some(),
                "{locale:?} catalog is missing scope label {}",
                scope_bindings.label_key,
            );
            for entry in &scope_bindings.entries {
                assert!(
                    catalog(locale, entry.label_key).is_some(),
                    "{locale:?} catalog is missing entry label {}",
                    entry.label_key,
                );
            }
        }
        // Overlay chrome keys are not part of the shared catalog
        // structure but still must exist in both locales.
        for key in ["keys.title", "keys.footer.close", "keys.requires_focus"] {
            assert!(
                catalog(locale, key).is_some(),
                "{locale:?} catalog is missing cheatsheet chrome key {key}",
            );
        }
    }
}

#[test]
fn korean_locale_returns_korean_catalog_text() {
    assert_eq!(
        catalog(Locale::Ko, "empty.suggestion_header"),
        Some("추천 시작 도구"),
    );
}

#[test]
fn english_locale_returns_english_catalog_text() {
    assert_eq!(
        catalog(Locale::En, "empty.suggestion_header"),
        Some("Suggested starters"),
    );
}

#[test]
fn catalog_returns_none_for_unknown_keys() {
    assert_eq!(catalog(Locale::En, "no.such.key"), None);
    assert_eq!(catalog(Locale::Ko, "no.such.key"), None);
}

#[test]
fn t_respects_the_locale_parameter() {
    assert_eq!(t("settings.theme.dark", Locale::En), "dark");
    assert_eq!(t("settings.theme.dark", Locale::Ko), "다크");
}

#[test]
fn t_args_replaces_named_placeholders() {
    let out = t_args("popup.no_match", &[("needle", "uuid")], Locale::En);
    assert_eq!(out, "no tools match \"uuid\"");
}

#[test]
fn supported_locales_are_exactly_english_and_korean() {
    // Pin the shipping locale set so adding a locale becomes a
    // deliberate change (catalog parity + Settings dropdown +
    // detect_from_str all have to update together).
    assert_eq!(SUPPORTED_LOCALES, &[Locale::En, Locale::Ko]);
}

#[test]
fn pin_accessibility_keys_exist_in_both_locales() {
    // Keys consumed by Flutter Pin Semantics(label/value/hint), context-menu
    // labels, and CustomSemanticsAction labels. A missing key makes screen
    // readers announce the raw key.
    for key in [
        "a11y.pin.label",
        "a11y.pin.label_plain",
        "a11y.pin.hint_run",
        "a11y.pin.running",
        "a11y.pin.stale",
        "a11y.pin.result_ok",
        "a11y.pin.result_ok_empty",
        "a11y.pin.result_error",
        "a11y.pin.result_error_empty",
        "a11y.pin.restored",
        "pin.last_run.just_now",
        "pin.last_run.minutes_ago",
        "pin.last_run.hours_ago",
        "pin.last_run.days_ago",
        "pin.menu.open",
        "pin.menu.edit_color",
        "pin.menu.reset_size",
        "pin.menu.unpin",
        "pin.resize.handle_tooltip",
        "pin.resize.banner",
    ] {
        assert!(catalog(Locale::En, key).is_some(), "En missing {key}");
        assert!(catalog(Locale::Ko, key).is_some(), "Ko missing {key}");
    }
}

// ─── tool meta lookup ─────────────────────────────

#[test]
fn tool_namespace_keys_maintain_locale_parity() {
    let missing: Vec<&str> = EN
        .keys()
        .chain(surface_io::EN.keys())
        .chain(board_guidance::EN.keys())
        .chain(media::EN.keys())
        .copied()
        .filter(|key| key.starts_with("tool.") && catalog(Locale::Ko, key).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "Ko catalog missing tool-meta keys: {missing:?}",
    );
}

#[test]
fn known_tool_keys_resolve_in_their_namespace() {
    for id in [
        "num.hex_to_decimal",
        "id.uuid_v7",
        "convert.json_format",
        "text.regex_match",
    ] {
        let label_key = format!("tool.{id}.label");
        let desc_key = format!("tool.{id}.description");
        assert!(catalog(Locale::En, &label_key).is_some());
        assert!(catalog(Locale::Ko, &label_key).is_some());
        assert!(catalog(Locale::En, &desc_key).is_some());
        assert!(catalog(Locale::Ko, &desc_key).is_some());
    }
}
