use super::*;

#[test]
fn 기본값은_명시적_생성자와_일치한다() {
    let t = Tweaks::default_const();
    assert_eq!(t.theme, Theme::Light);
    assert_eq!(t.accent, Accent::Green);
    assert!(t.show_holes);
    assert_eq!(t.locale, upeg_core::prefs::Locale::En);
}

#[test]
fn json_왕복은_전체_필드를_보존한다() {
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
fn 조정값_파싱은_쓰레기값을_처리한다() {
    assert!(parse_tweaks("not json").is_none());
    assert!(parse_tweaks("{}").is_none());
    assert!(parse_tweaks(r#"{"theme":"Mauve"}"#).is_none());
}

#[test]
fn 조정값_파싱은_알려진_형태를_허용한다() {
    let json = r#"{"theme":"Dark","accent":"Pink","show_holes":true,"locale":"En"}"#;
    let t = parse_tweaks(json).expect("parse");
    assert_eq!(t.theme, Theme::Dark);
    assert_eq!(t.accent, Accent::Pink);
    assert!(t.show_holes);
    assert_eq!(t.locale, upeg_core::prefs::Locale::En);
}

#[test]
fn 저장소_키는_버전이다() {
    assert_eq!(crate::platform::storage::TWEAKS_KEY, "upeg.tweaks.v1");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn 네이티브에서_저장_후_로드는_디스크를_거쳐_왕복된다() {
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
fn 테마_데이터_속성은_html과_호환된다() {
    assert_eq!(theme_data_attr(Theme::Dark), "dark");
    assert_eq!(theme_data_attr(Theme::Light), "light");
}

#[test]
fn 강조색_색상_항상은_css를_반환한다() {
    for accent in [Accent::Green, Accent::Amber, Accent::Cyan, Accent::Pink] {
        for theme in [Theme::Dark, Theme::Light] {
            let c = accent_color_for(accent, theme);
            assert!(!c.is_empty());
            assert!(c.starts_with("oklch("));
        }
    }
}

#[test]
fn 표시_변수는_show_holes가_거짓이면_구멍을_숨긴다() {
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
fn 어두운_테마에서_표시_변수는_어두운_palette의_구멍을_고른다() {
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
fn 조정값_적용_스크립트는_세_개의_css_변수와_테마_속성을_모두_설정한다() {
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
