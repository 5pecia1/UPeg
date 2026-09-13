//! Shared user preferences record.
//!
//! The data shape is shared so every surface — desktop UI, TUI, future
//! native menubar — reads and writes the same on-disk record. Surface-
//! specific presentation (CSS variables for the WebView, ratatui colors
//! for the terminal) is layered on top in each surface crate.

use super::Locale;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Light/dark mode selector.
///
/// Maps to a `data-theme` attribute on the desktop's `<html>` and to a
/// foreground-tone palette in the TUI. The enum stays data-only here; the
/// CSS / ratatui mappings live in each surface crate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Theme {
    Dark,
    Light,
}

/// Accent color family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Accent {
    Green,
    Amber,
    Cyan,
    Pink,
}

/// Persistent user-preference record shared across surfaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Tweaks {
    pub theme: Theme,
    pub accent: Accent,
    pub show_holes: bool,
    pub locale: Locale,
    /// Desktop only: run an in-process HTTP host at app start.
    ///
    /// Default OFF — a network listener must be activated explicitly
    /// (FR-16). When OFF the desktop still attaches to a host somebody
    /// else started; it just never becomes one. Surfaces that are not
    /// the desktop ignore this field.
    ///
    /// `#[serde(default)]` because the switch is the newest field on a
    /// record users already have on disk; a preferences file without it
    /// must keep its theme instead of resetting to defaults.
    #[cfg_attr(feature = "serde", serde(default))]
    pub local_http_host: bool,
}

impl Tweaks {
    /// Default `Tweaks` value usable from `const` contexts (e.g. as the
    /// initial value of a `GlobalSignal`).
    pub const fn default_const() -> Self {
        Self {
            theme: Theme::Light,
            accent: Accent::Green,
            show_holes: true,
            locale: Locale::En,
            local_http_host: false,
        }
    }
}

impl Default for Tweaks {
    fn default() -> Self {
        Self::default_const()
    }
}

/// Parse a serialized [`Tweaks`] record. Returns `None` for invalid JSON or
/// missing required fields — callers fall back to [`Tweaks::default_const`]
/// in that case rather than panicking on a corrupted preferences file.
#[cfg(feature = "serde")]
pub fn parse_tweaks(json: &str) -> Option<Tweaks> {
    serde_json::from_str(json).ok()
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn 기본값_상수는_기본값_트레이트와_일치한다() {
        assert_eq!(Tweaks::default(), Tweaks::default_const());
    }

    #[test]
    fn 기본값은_밝은_초록_영어와_빈칸이다() {
        let t = Tweaks::default_const();
        assert_eq!(t.theme, Theme::Light);
        assert_eq!(t.accent, Accent::Green);
        assert!(t.show_holes);
        assert_eq!(t.locale, Locale::En);
        assert!(
            !t.local_http_host,
            "네트워크 리스너는 명시적으로 켜야 한다 (FR-16)"
        );
    }

    #[test]
    fn json_왕복은_로케일을_포함한_모든_필드를_보존한다() {
        let original = Tweaks {
            theme: Theme::Dark,
            accent: Accent::Cyan,
            show_holes: false,
            locale: Locale::Ko,
            local_http_host: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let back = parse_tweaks(&json).expect("parse");
        assert_eq!(back, original);
    }

    #[test]
    fn 설정_파싱은_쓰레기값을_거부한다() {
        assert!(parse_tweaks("not json").is_none());
        // Empty object is missing every required field.
        assert!(parse_tweaks("{}").is_none());
        // Unknown variant on an existing field.
        assert!(parse_tweaks(r#"{"theme":"Mauve","accent":"Green","show_holes":true}"#).is_none());
    }

    #[test]
    fn 로컬_http_host_필드가_없는_기록은_꺼짐으로_파싱된다() {
        let json = r#"{"theme":"Dark","accent":"Green","show_holes":true,"locale":"Ko"}"#;
        let t = parse_tweaks(json).expect("parse");
        assert!(!t.local_http_host);
        assert_eq!(t.theme, Theme::Dark, "기존 필드는 그대로 유지된다");
    }

    #[test]
    fn 설정_파싱은_명시적_로케일을_존중한다() {
        let json = r#"{"theme":"Light","accent":"Green","show_holes":true,"locale":"Ko"}"#;
        let t = parse_tweaks(json).expect("parse");
        assert_eq!(t.locale, Locale::Ko);
    }

    #[test]
    fn 설정_파싱은_낯선_필드를_거부한다() {
        let json = r#"{"theme":"Dark","accent":"Pink","density":"Compact","show_holes":true,"locale":"Ko"}"#;
        assert!(parse_tweaks(json).is_none());
    }
}
