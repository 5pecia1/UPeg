//! Tweaks (theme / accent / locale / peg-hole visibility) FRB surface.
//!
//! The on-disk record + persistence path lives in
//! `upeg_pegboard_ui::features::tweaks`. This module exposes the Flutter
//! side a Dart-shaped DTO with owned `String` fields — the source enum
//! variants are encoded as their `serde_json` discriminator names
//! (e.g. `"Dark"`, `"Green"`, `"En"`) so a single string mapping
//! survives every Rust → Dart hop.

use upeg_core::prefs::{Accent, Locale, Theme};
use upeg_pegboard_ui::features::tweaks::{
    Tweaks, load_tweaks as load_inner, save_tweaks as save_inner,
};
use upeg_pegboard_ui::i18n::SUPPORTED_LOCALES;

/// Dart-mirrored view of [`Tweaks`].
///
/// Every field is an owned `String` so it crosses the FFI boundary
/// without lifetime acrobatics. The enum-shaped fields (`theme`,
/// `accent`, `locale`) carry the serde-encoded variant name —
/// `LOCALE_VALUES`, `THEME_VALUES`, `ACCENT_VALUES` enumerate the
/// shipping set so Dart can build dropdowns without re-encoding the
/// list.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct TweaksDto {
    pub theme: String,
    pub accent: String,
    pub show_holes: bool,
    pub locale: String,
    /// Desktop "Local HTTP host" switch. When on, the app embeds an
    /// in-process HTTP host at boot; when off it only attaches to a host
    /// somebody else started. Default off (FR-16 explicit activation).
    pub local_http_host: bool,
}

impl From<&Tweaks> for TweaksDto {
    fn from(t: &Tweaks) -> Self {
        Self {
            theme: theme_to_str(t.theme).to_string(),
            accent: accent_to_str(t.accent).to_string(),
            show_holes: t.show_holes,
            locale: locale_to_str(t.locale).to_string(),
            local_http_host: t.local_http_host,
        }
    }
}

impl TryFrom<TweaksDto> for Tweaks {
    type Error = super::boot::FrbError;

    fn try_from(dto: TweaksDto) -> Result<Self, Self::Error> {
        Ok(Self {
            theme: parse_theme(&dto.theme)?,
            accent: parse_accent(&dto.accent)?,
            show_holes: dto.show_holes,
            locale: parse_locale(&dto.locale)?,
            local_http_host: dto.local_http_host,
        })
    }
}

/// Shipping locale set. Returned to Dart for the Settings dropdown so
/// the value list stays in sync with the Rust catalog parity tests.
#[flutter_rust_bridge::frb(sync)]
pub fn supported_locales() -> Vec<String> {
    SUPPORTED_LOCALES
        .iter()
        .copied()
        .map(|l| locale_to_str(l).to_string())
        .collect()
}

/// Shipping theme set.
#[flutter_rust_bridge::frb(sync)]
pub fn supported_themes() -> Vec<String> {
    [Theme::Light, Theme::Dark]
        .into_iter()
        .map(|t| theme_to_str(t).to_string())
        .collect()
}

/// Shipping accent set.
#[flutter_rust_bridge::frb(sync)]
pub fn supported_accents() -> Vec<String> {
    [Accent::Green, Accent::Amber, Accent::Cyan, Accent::Pink]
        .into_iter()
        .map(|a| accent_to_str(a).to_string())
        .collect()
}

/// Load the persisted tweaks, or the default value if none exist.
#[flutter_rust_bridge::frb(sync)]
pub fn load_tweaks() -> TweaksDto {
    let t = load_inner().unwrap_or_default();
    TweaksDto::from(&t)
}

/// Persist the tweaks. Returns `Validation` if any enum-shaped field
/// carries an unknown variant string.
#[flutter_rust_bridge::frb(sync)]
pub fn save_tweaks(tweaks: TweaksDto) -> Result<(), super::boot::FrbError> {
    let value = Tweaks::try_from(tweaks)?;
    save_inner(&value);
    Ok(())
}

// ─── enum ↔ string ─────────────────────────────────────────────

const fn theme_to_str(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "Dark",
        Theme::Light => "Light",
    }
}

const fn accent_to_str(accent: Accent) -> &'static str {
    match accent {
        Accent::Green => "Green",
        Accent::Amber => "Amber",
        Accent::Cyan => "Cyan",
        Accent::Pink => "Pink",
    }
}

const fn locale_to_str(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "En",
        Locale::Ko => "Ko",
    }
}

fn parse_theme(s: &str) -> Result<Theme, super::boot::FrbError> {
    match s {
        "Dark" => Ok(Theme::Dark),
        "Light" => Ok(Theme::Light),
        other => Err(super::boot::FrbError::Validation {
            field: "theme".to_string(),
            reason: format!("unknown theme `{other}`"),
        }),
    }
}

fn parse_accent(s: &str) -> Result<Accent, super::boot::FrbError> {
    match s {
        "Green" => Ok(Accent::Green),
        "Amber" => Ok(Accent::Amber),
        "Cyan" => Ok(Accent::Cyan),
        "Pink" => Ok(Accent::Pink),
        other => Err(super::boot::FrbError::Validation {
            field: "accent".to_string(),
            reason: format!("unknown accent `{other}`"),
        }),
    }
}

fn parse_locale(s: &str) -> Result<Locale, super::boot::FrbError> {
    match s {
        "En" => Ok(Locale::En),
        "Ko" => Ok(Locale::Ko),
        other => Err(super::boot::FrbError::Validation {
            field: "locale".to_string(),
            reason: format!("unknown locale `{other}`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_왕복은_모든_필드를_보존한다() {
        let original = Tweaks {
            theme: Theme::Dark,
            accent: Accent::Cyan,
            show_holes: false,
            locale: Locale::Ko,
            local_http_host: true,
        };
        let dto = TweaksDto::from(&original);
        assert_eq!(dto.theme, "Dark");
        assert_eq!(dto.accent, "Cyan");
        assert!(!dto.show_holes);
        assert_eq!(dto.locale, "Ko");
        assert!(dto.local_http_host);
        let back = Tweaks::try_from(dto).expect("parse");
        assert_eq!(back, original);
    }

    #[test]
    fn 알려지지_않은_테마는_검증_오류로_거부된다() {
        let dto = TweaksDto {
            theme: "Mauve".to_string(),
            accent: "Green".to_string(),
            show_holes: true,
            locale: "En".to_string(),
            local_http_host: false,
        };
        match Tweaks::try_from(dto) {
            Err(super::super::boot::FrbError::Validation { field, .. }) => {
                assert_eq!(field, "theme");
            }
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn 알려지지_않은_로케일은_검증_오류로_거부된다() {
        let dto = TweaksDto {
            theme: "Light".to_string(),
            accent: "Green".to_string(),
            show_holes: true,
            locale: "Fr".to_string(),
            local_http_host: false,
        };
        match Tweaks::try_from(dto) {
            Err(super::super::boot::FrbError::Validation { field, .. }) => {
                assert_eq!(field, "locale");
            }
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn 지원되는_로케일은_영어와_한국어를_정확히_반환한다() {
        assert_eq!(
            supported_locales(),
            vec!["En".to_string(), "Ko".to_string()]
        );
    }

    #[test]
    fn 지원되는_테마와_accent_set_을_반환한다() {
        assert_eq!(
            supported_themes(),
            vec!["Light".to_string(), "Dark".to_string()],
        );
        assert_eq!(
            supported_accents(),
            vec![
                "Green".to_string(),
                "Amber".to_string(),
                "Cyan".to_string(),
                "Pink".to_string(),
            ],
        );
    }
}
