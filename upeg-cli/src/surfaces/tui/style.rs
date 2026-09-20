//! `Tweaks` → ratatui color mapping for the TUI surface.
//!
//! The desktop UI uses CSS variables (`--accent`, `oklch(…)`) to project
//! the shared `Tweaks` record onto its visual treatment. The TUI's
//! analogue is this module: pure functions from `(Theme, Accent)` to
//! `ratatui::style::Color`. Keeping the mapping surface-local mirrors
//! the desktop's `display_vars_for` — `upeg-core` stays UI-agnostic, and
//! each surface decides which CSS / ratatui palette renders the same
//! preference record.
//!
//! Light vs Dark theme maps the same accent family to a darker / lighter
//! ratatui colour so the foreground stands out from the terminal's
//! ambient background. The mapping is exhaustive (no `_ =>` arm) so
//! adding a new `Accent` variant fails to compile until the palette
//! ships a matching tone.

use ratatui::style::Color;
use upeg_core::PinColorHex;
use upeg_core::prefs::{Accent, Theme};

/// Project the shared `(Theme, Accent)` pair onto a ratatui foreground
/// `Color` for highlighted text and focused borders.
///
/// Mirrors `upeg_pegboard_ui::features::tweaks::accent_color_for` in role —
/// both project the same enum pair onto a surface-local palette.
pub const fn accent_color(theme: Theme, accent: Accent) -> Color {
    match (theme, accent) {
        (Theme::Light, Accent::Green) => Color::Green,
        (Theme::Light, Accent::Amber) => Color::Yellow,
        (Theme::Light, Accent::Cyan) => Color::Cyan,
        (Theme::Light, Accent::Pink) => Color::Magenta,
        (Theme::Dark, Accent::Green) => Color::LightGreen,
        (Theme::Dark, Accent::Amber) => Color::LightYellow,
        (Theme::Dark, Accent::Cyan) => Color::LightCyan,
        (Theme::Dark, Accent::Pink) => Color::LightMagenta,
    }
}

/// Project an optional pin color hex (`#RRGGBB`) onto a ratatui `Color`.
///
/// If `pin_color` is `None` or parsing fails, returns the `fallback` color.
/// Parsing uses `u8::from_str_radix` for each RGB component.
pub fn pin_color_or_fallback(pin_color: Option<&PinColorHex>, fallback: Color) -> Color {
    let Some(hex) = pin_color.and_then(|p| p.as_str().get(1..)) else {
        return fallback;
    };

    let Ok(r) = u8::from_str_radix(&hex[0..2], 16) else {
        return fallback;
    };
    let Ok(g) = u8::from_str_radix(&hex[2..4], 16) else {
        return fallback;
    };
    let Ok(b) = u8::from_str_radix(&hex[4..6], 16) else {
        return fallback;
    };

    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::PinColorHex;

    #[test]
    fn light_theme_green_accent_maps_to_green() {
        assert_eq!(accent_color(Theme::Light, Accent::Green), Color::Green);
    }

    #[test]
    fn dark_theme_green_accent_maps_to_light_green() {
        assert_eq!(accent_color(Theme::Dark, Accent::Green), Color::LightGreen);
    }

    #[test]
    fn accent_color_table_covers_every_theme_accent_pair() {
        // Complete table of all 8 cases. Adding a new Accent variant
        // fails to compile until its tone lands in `accent_color`.
        let cases = [
            ((Theme::Light, Accent::Green), Color::Green),
            ((Theme::Light, Accent::Amber), Color::Yellow),
            ((Theme::Light, Accent::Cyan), Color::Cyan),
            ((Theme::Light, Accent::Pink), Color::Magenta),
            ((Theme::Dark, Accent::Green), Color::LightGreen),
            ((Theme::Dark, Accent::Amber), Color::LightYellow),
            ((Theme::Dark, Accent::Cyan), Color::LightCyan),
            ((Theme::Dark, Accent::Pink), Color::LightMagenta),
        ];
        for ((theme, accent), expected) in cases {
            assert_eq!(
                accent_color(theme, accent),
                expected,
                "{theme:?} {accent:?}"
            );
        }
    }

    #[test]
    fn each_theme_picks_mutually_distinct_accent_colors() {
        // Within one theme the four accents must resolve to different
        // colors; otherwise the user cannot see an accent change.
        for theme in [Theme::Light, Theme::Dark] {
            let colors = [
                accent_color(theme, Accent::Green),
                accent_color(theme, Accent::Amber),
                accent_color(theme, Accent::Cyan),
                accent_color(theme, Accent::Pink),
            ];
            for i in 0..colors.len() {
                for j in (i + 1)..colors.len() {
                    assert_ne!(
                        colors[i], colors[j],
                        "{theme:?}: accents #{i} and #{j} resolved to the same Color"
                    );
                }
            }
        }
    }

    #[test]
    fn pin_color_hex_maps_to_rgb_color() {
        let pin_color = PinColorHex::parse("#FF8040").unwrap();
        let result = pin_color_or_fallback(Some(&pin_color), Color::Reset);
        assert_eq!(result, Color::Rgb(255, 128, 64));
    }

    #[test]
    fn missing_pin_color_uses_fallback_accent() {
        let fallback = Color::Cyan;
        let result = pin_color_or_fallback(None, fallback);
        assert_eq!(result, fallback);
    }
}
