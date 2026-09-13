//! Tweaks data model + persistence.
//!
//! The on-disk `Tweaks` record (theme / accent / locale / show_holes)
//! lives in `upeg-core::prefs` so every surface — Flutter desktop/PWA,
//! TUI, future native menubar — reads and writes the same shape. This
//! module owns the *persistence* path (read from / write to the shared
//! `platform::storage` adapter) and the surface-agnostic
//! display-variable helpers that map a `Tweaks` value to CSS-flavoured
//! strings.

#[cfg(test)]
mod tests;

pub use upeg_core::prefs::{Accent, Theme, Tweaks, parse_tweaks};

use crate::platform::storage;

/// Snapshot of the CSS-flavoured variables a `Tweaks` value maps to.
///
/// Kept in this crate so the same mapping serves Flutter (web/desktop)
/// and any future WebView callers. Field types are `&'static str`
/// because every value is an internal compile-time constant.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DisplayVars {
    pub data_theme: &'static str,
    pub accent: &'static str,
    pub hole: &'static str,
    pub hole_deep: &'static str,
}

/// HTML `data-theme` attribute string for the given `Theme`.
pub const fn theme_data_attr(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    }
}

/// CSS `oklch(...)` literal for the given accent/theme pair.
pub const fn accent_color_for(accent: Accent, theme: Theme) -> &'static str {
    match (accent, theme) {
        (Accent::Green, Theme::Dark) => "oklch(0.78 0.16 145)",
        (Accent::Green, Theme::Light) => "oklch(0.55 0.16 145)",
        (Accent::Amber, Theme::Dark) => "oklch(0.82 0.16 75)",
        (Accent::Amber, Theme::Light) => "oklch(0.58 0.16 60)",
        (Accent::Cyan, Theme::Dark) => "oklch(0.80 0.13 200)",
        (Accent::Cyan, Theme::Light) => "oklch(0.58 0.13 210)",
        (Accent::Pink, Theme::Dark) => "oklch(0.78 0.16 0)",
        (Accent::Pink, Theme::Light) => "oklch(0.58 0.16 0)",
    }
}

/// Compute the four display variables a `Tweaks` value implies.
pub fn display_vars_for(t: Tweaks) -> DisplayVars {
    let (hole, hole_deep) = if !t.show_holes {
        ("transparent", "transparent")
    } else if matches!(t.theme, Theme::Light) {
        ("rgba(0,0,0,0.07)", "rgba(255,255,255,0.6)")
    } else {
        ("rgba(255,255,255,0.05)", "rgba(0,0,0,0.45)")
    };
    DisplayVars {
        data_theme: theme_data_attr(t.theme),
        accent: accent_color_for(t.accent, t.theme),
        hole,
        hole_deep,
    }
}

/// Build the JS payload that applies a `Tweaks` value to
/// `document.documentElement`. Pure string builder so:
///   * the same code runs on wasm (browser) and on any future
///     WebView surface;
///   * tests can assert that every CSS variable name is present
///     without a DOM.
pub fn apply_tweaks_script(t: Tweaks) -> String {
    let vars = display_vars_for(t);
    // String values are all internal constants (data_attr / color_for /
    // hole / hole_deep), so we can interpolate them directly — no
    // untrusted input ever enters this script.
    format!(
        "(() => {{ \
            const r = document.documentElement; \
            if (!r) return; \
            r.setAttribute('data-theme', '{theme}'); \
            r.style.setProperty('--accent', '{accent}'); \
            r.style.setProperty('--hole', '{hole}'); \
            r.style.setProperty('--hole-deep', '{hole_deep}'); \
        }})();",
        theme = vars.data_theme,
        accent = vars.accent,
        hole = vars.hole,
        hole_deep = vars.hole_deep,
    )
}

/// Persist a `Tweaks` value via the shared platform storage adapter.
/// Silently swallows serialisation failures (same policy as
/// `boards`/`layouts`: a corrupted write is logged via the platform
/// layer's tracing target, never panicked over).
pub fn save_tweaks(t: &Tweaks) {
    let Ok(json) = serde_json::to_string(t) else {
        return;
    };
    let _ = storage::set_item(storage::TWEAKS_KEY, &json);
}

/// Load the persisted `Tweaks` value. Returns `None` when no record
/// exists or when it fails to parse — callers fall back to
/// `Tweaks::default_const`.
pub fn load_tweaks() -> Option<Tweaks> {
    let json = storage::get_item(storage::TWEAKS_KEY)?;
    parse_tweaks(&json)
}
