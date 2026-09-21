//! Display-language selector and a pure detection helper.
//!
//! `Locale` is the key fed into `crate::i18n::t` / `crate::i18n::t_args`. The
//! catalog itself lives next to each surface — this enum is just the
//! discriminator, kept in `upeg-core` so every surface (desktop UI, TUI, …)
//! resolves the same value when reading the shared `Tweaks` record.
//!
//! Platform-specific sources (POSIX `$LANG`, browser `navigator.language`,
//! a `--locale` CLI flag) belong to the caller — `detect_from_str` is a
//! pure string→enum lookup so it stays unit-testable without env or DOM.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Display language for upeg surfaces.
///
/// English is the canonical fallback: any missing translation key, any
/// unsupported detection hint, and the default value when a new install
/// has no persisted preference all resolve to [`Locale::En`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Locale {
    /// English. Default and ultimate fallback for any missing translation.
    #[default]
    En,
    /// Korean.
    Ko,
}

impl Locale {
    /// Parse a locale hint string into a [`Locale`]. Pure — platform-specific
    /// sources (POSIX `$LANG`, browser `navigator.language`, CLI flag) are
    /// the caller's job to fetch.
    ///
    /// Accepted shapes (case-insensitive): BCP 47 (`ko`, `ko-KR`) and POSIX
    /// (`ko_KR.UTF-8`). Everything else — including empty, `"C"`, `"POSIX"`,
    /// and unsupported tags such as `"ja_JP.UTF-8"` — falls back to
    /// [`Locale::En`].
    ///
    /// A two-letter language match also requires either end-of-string or one
    /// of the BCP 47 / POSIX separators (`-`, `_`, `.`) immediately after, so
    /// e.g. `"kos"` or `"korean"` does **not** parse as Korean.
    pub fn detect_from_str(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Self::En;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("ko")
            && (rest.is_empty() || rest.starts_with(['-', '_', '.']))
        {
            return Self::Ko;
        }
        Self::En
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_english() {
        assert_eq!(Locale::default(), Locale::En);
    }

    #[test]
    fn detects_posix_korean() {
        assert_eq!(Locale::detect_from_str("ko_KR.UTF-8"), Locale::Ko);
    }

    #[test]
    fn detects_bcp_47_korean() {
        assert_eq!(Locale::detect_from_str("ko-KR"), Locale::Ko);
        assert_eq!(Locale::detect_from_str("ko"), Locale::Ko);
        // Case-insensitive — uppercase tags must parse too.
        assert_eq!(Locale::detect_from_str("KO"), Locale::Ko);
    }

    #[test]
    fn english_locales_detect_as_english() {
        assert_eq!(Locale::detect_from_str("en_US.UTF-8"), Locale::En);
        assert_eq!(Locale::detect_from_str("en-GB"), Locale::En);
    }

    #[test]
    fn empty_or_blank_values_fall_back_to_english() {
        assert_eq!(Locale::detect_from_str(""), Locale::En);
        assert_eq!(Locale::detect_from_str("   "), Locale::En);
    }

    #[test]
    fn unsupported_values_fall_back_to_english() {
        // ja / fr / zh are recognised language tags, just not supported yet.
        // Pin the fallback policy so adding support is a deliberate change.
        assert_eq!(Locale::detect_from_str("ja_JP.UTF-8"), Locale::En);
        assert_eq!(Locale::detect_from_str("fr"), Locale::En);
        // C / POSIX locales mean "no human language preference".
        assert_eq!(Locale::detect_from_str("C"), Locale::En);
        assert_eq!(Locale::detect_from_str("POSIX"), Locale::En);
    }

    #[test]
    fn korean_prefix_requires_a_separator() {
        // Without the separator guard, anything starting with the letters
        // k-o (kos, korean, kotlin) would falsely classify as Korean.
        assert_eq!(Locale::detect_from_str("kos"), Locale::En);
        assert_eq!(Locale::detect_from_str("korean"), Locale::En);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_korean() {
        let s = serde_json::to_string(&Locale::Ko).expect("serialize");
        let parsed: Locale = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(parsed, Locale::Ko);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips_english() {
        let s = serde_json::to_string(&Locale::En).expect("serialize");
        let parsed: Locale = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(parsed, Locale::En);
    }
}
