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
    fn 밝은_테마의_초록_강조색은_초록으로_매핑된다() {
        assert_eq!(accent_color(Theme::Light, Accent::Green), Color::Green);
    }

    #[test]
    fn 어두운_테마의_초록_강조색은_밝은_초록으로_매핑된다() {
        assert_eq!(accent_color(Theme::Dark, Accent::Green), Color::LightGreen);
    }

    #[test]
    fn 각_테마에서_모든_강조색은_서로_다른_색을_가진다() {
        // 8개 경우의 완전한 표다. 새 Accent 변형을 추가하면 해당 톤이
        // `accent_color`에 들어오기 전까지 컴파일이 실패한다.
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
    fn 각_테마는_서로_구분되는_강조색을_고른다() {
        // 같은 테마 안에서 네 강조색은 서로 다른 색으로 해석되어야 한다.
        // 그렇지 않으면 사용자가 강조색 변경을 볼 수 없다.
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
                        "{theme:?}: 강조색 #{i}와 #{j}가 같은 Color로 해석되었다"
                    );
                }
            }
        }
    }

    #[test]
    fn 핀_색상_hex는_rgb_색으로_매핑된다() {
        let pin_color = PinColorHex::parse("#FF8040").unwrap();
        let result = pin_color_or_fallback(Some(&pin_color), Color::Reset);
        assert_eq!(result, Color::Rgb(255, 128, 64));
    }

    #[test]
    fn 핀_색상이_없으면_fallback_강조색을_사용한다() {
        let fallback = Color::Cyan;
        let result = pin_color_or_fallback(None, fallback);
        assert_eq!(result, fallback);
    }
}
