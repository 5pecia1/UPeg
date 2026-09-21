use super::*;

#[test]
fn defaults_match_explicit_constructor() {
    let t = Tweaks::default_const();
    assert_eq!(t.theme, Theme::Light);
    assert_eq!(t.accent, Accent::Green);
    assert!(t.show_holes);
    assert_eq!(t.locale, upeg_core::prefs::Locale::En);
}

#[test]
fn json_round_trip_preserves_every_field() {
    let original = Tweaks {
        theme: Theme::Dark,
        accent: Accent::Cyan,
        show_holes: false,
        local_http_host: false,
        locale: upeg_core::prefs::Locale::Ko,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let back = parse_tweaks(&json).expect("parse");
    assert_eq!(back, original);
}

#[test]
fn tweaks_parsing_handles_garbage() {
    assert!(parse_tweaks("not json").is_none());
    assert!(parse_tweaks("{}").is_none());
    assert!(parse_tweaks(r#"{"theme":"Mauve"}"#).is_none());
}

#[test]
fn tweaks_parsing_accepts_known_shape() {
    let json = r#"{"theme":"Dark","accent":"Pink","show_holes":true,"locale":"En"}"#;
    let t = parse_tweaks(json).expect("parse");
    assert_eq!(t.theme, Theme::Dark);
    assert_eq!(t.accent, Accent::Pink);
    assert!(t.show_holes);
    assert_eq!(t.locale, upeg_core::prefs::Locale::En);
}

#[test]
fn storage_key_is_versioned() {
    assert_eq!(crate::platform::storage::TWEAKS_KEY, "upeg.tweaks.v1");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_save_then_load_round_trips_through_disk() {
    use upeg_runtime::persistence::io;
    let dir = std::env::temp_dir().join(format!(
        "upeg-tweaks-roundtrip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    let path = dir.join(".upeg-tweaks.json");

    let probe = Tweaks {
        theme: Theme::Dark,
        accent: Accent::Pink,
        show_holes: false,
        local_http_host: false,
        locale: upeg_core::prefs::Locale::En,
    };
    let json = serde_json::to_string(&probe).expect("serialize");

    io::save_to_path(&path, &json).expect("write tweaks file");
    let loaded = io::load_from_path(&path).expect("read tweaks file");
    let parsed = parse_tweaks(&loaded).expect("parse round-tripped tweaks");

    assert_eq!(parsed, probe);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn theme_data_attribute_is_html_compatible() {
    assert_eq!(theme_data_attr(Theme::Dark), "dark");
    assert_eq!(theme_data_attr(Theme::Light), "light");
}

#[test]
fn accent_color_always_returns_css() {
    for accent in [Accent::Green, Accent::Amber, Accent::Cyan, Accent::Pink] {
        for theme in [Theme::Dark, Theme::Light] {
            let c = accent_color_for(accent, theme);
            assert!(!c.is_empty());
            assert!(c.starts_with("oklch("));
        }
    }
}

#[test]
fn display_vars_hide_holes_when_show_holes_is_false() {
    let t = Tweaks {
        theme: Theme::Light,
        accent: Accent::Green,
        show_holes: false,
        local_http_host: false,
        locale: upeg_core::prefs::Locale::En,
    };
    let vars = display_vars_for(t);
    assert_eq!(vars.hole, "transparent");
    assert_eq!(vars.hole_deep, "transparent");
}

#[test]
fn display_vars_pick_dark_palette_holes_in_dark_theme() {
    let t = Tweaks {
        theme: Theme::Dark,
        accent: Accent::Cyan,
        show_holes: true,
        local_http_host: false,
        locale: upeg_core::prefs::Locale::En,
    };
    let vars = display_vars_for(t);
    assert_eq!(vars.data_theme, "dark");
    assert_eq!(vars.hole, "rgba(255,255,255,0.05)");
    assert_eq!(vars.hole_deep, "rgba(0,0,0,0.45)");
}

#[test]
fn apply_tweaks_script_sets_all_three_css_vars_and_theme_attribute() {
    let t = Tweaks {
        theme: Theme::Dark,
        accent: Accent::Amber,
        show_holes: true,
        local_http_host: false,
        locale: upeg_core::prefs::Locale::En,
    };
    let script = apply_tweaks_script(t);
    // Every CSS variable + the data-theme attribute must appear in the
    // payload — if any drops out, the WebView side silently no-ops that
    // setter and the UI keeps the prior value.
    assert!(script.contains("data-theme"));
    assert!(script.contains("'dark'"));
    assert!(script.contains("--accent"));
    assert!(script.contains("--hole"));
    assert!(script.contains("--hole-deep"));
    assert!(script.contains("document.documentElement"));
    // Negative pin: density was removed; nothing should re-introduce the
    // dead `--upeg-density` setter without an updated CSS rule consuming it.
    assert!(!script.contains("--upeg-density"));
}
