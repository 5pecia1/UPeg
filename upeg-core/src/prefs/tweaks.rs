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
    fn default_const_matches_the_default_trait() {
        assert_eq!(Tweaks::default(), Tweaks::default_const());
    }

    #[test]
    fn defaults_are_light_green_english_and_holes() {
        let t = Tweaks::default_const();
        assert_eq!(t.theme, Theme::Light);
        assert_eq!(t.accent, Accent::Green);
        assert!(t.show_holes);
        assert_eq!(t.locale, Locale::En);
        assert!(
            !t.local_http_host,
            "the network listener must be enabled explicitly (FR-16)"
        );
    }

    #[test]
    fn json_round_trip_preserves_every_field_including_locale() {
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
    fn settings_parse_rejects_garbage() {
        assert!(parse_tweaks("not json").is_none());
        // Empty object is missing every required field.
        assert!(parse_tweaks("{}").is_none());
        // Unknown variant on an existing field.
        assert!(parse_tweaks(r#"{"theme":"Mauve","accent":"Green","show_holes":true}"#).is_none());
    }

    #[test]
    fn records_without_the_local_http_host_field_parse_as_off() {
        let json = r#"{"theme":"Dark","accent":"Green","show_holes":true,"locale":"Ko"}"#;
        let t = parse_tweaks(json).expect("parse");
        assert!(!t.local_http_host);
        assert_eq!(t.theme, Theme::Dark, "existing fields stay intact");
    }

    #[test]
    fn settings_parse_respects_an_explicit_locale() {
        let json = r#"{"theme":"Light","accent":"Green","show_holes":true,"locale":"Ko"}"#;
        let t = parse_tweaks(json).expect("parse");
        assert_eq!(t.locale, Locale::Ko);
    }

    #[test]
    fn settings_parse_rejects_unfamiliar_fields() {
        let json = r#"{"theme":"Dark","accent":"Pink","density":"Compact","show_holes":true,"locale":"Ko"}"#;
        assert!(parse_tweaks(json).is_none());
    }
}
