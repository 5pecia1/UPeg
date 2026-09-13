//! i18n FRB surface.
//!
//! Exposes the `upeg-pegboard-ui` translation catalog to Flutter as
//! two sync FRB calls: [`translate`] for plain key lookup and
//! [`translate_args`] for `{name}` placeholder substitution.
//!
//! The locale is a parameter on each call — there is no thread-local
//! global — so Dart can drive renders by deriving the active locale
//! from `tweaksProvider` and passing it in. This keeps the FRB
//! surface stateless across hot reload.
//!
//! The boundary type is the sealed [`LocaleDto`] enum, not a `String`
//! discriminator, so adding a locale becomes a type-system change on
//! both sides instead of a runtime parse.

use upeg_core::prefs::Locale;

/// Locale discriminator at the FRB boundary.
///
/// Mirrors [`upeg_core::prefs::Locale`] on the Dart side as a `non_opaque`
/// enum so Dart sees `LocaleDto.en` / `LocaleDto.ko` rather than an
/// opaque pointer. Keep the variant set in lock-step with `Locale` —
/// the `From` impl below pins that contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[flutter_rust_bridge::frb(non_opaque)]
pub enum LocaleDto {
    En,
    Ko,
}

impl From<LocaleDto> for Locale {
    fn from(d: LocaleDto) -> Self {
        match d {
            LocaleDto::En => Self::En,
            LocaleDto::Ko => Self::Ko,
        }
    }
}

/// Look up a translation for `key` in the given `locale`.
///
/// Returns the key itself if no translation exists in either the
/// requested locale or English — the same fallback chain that
/// `upeg_pegboard_ui::i18n::t` enforces. The caller treats a key
/// echo as a missing-catalog-entry signal.
#[flutter_rust_bridge::frb(sync)]
pub fn translate(key: String, locale: LocaleDto) -> String {
    upeg_pegboard_ui::i18n::t(&key, locale.into()).to_string()
}

/// Look up a translation and substitute `{name}` placeholders.
///
/// The `(arg_keys, arg_vals)` parallel arrays are zipped into the
/// pair form `t_args` expects. Parallel `Vec<String>`s are used —
/// rather than `Vec<(String, String)>` — because FRB v2 round-trips
/// the flat shape with less codegen churn.
#[flutter_rust_bridge::frb(sync)]
pub fn translate_args(
    key: String,
    locale: LocaleDto,
    arg_keys: Vec<String>,
    arg_vals: Vec<String>,
) -> String {
    let pairs: Vec<(&str, &str)> = arg_keys
        .iter()
        .zip(arg_vals.iter())
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    upeg_pegboard_ui::i18n::t_args(&key, &pairs, locale.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate는_settings_theme_을_영어로_반환한다() {
        let s = translate("settings.section.theme".to_string(), LocaleDto::En);
        assert_eq!(s, "theme");
    }

    #[test]
    fn translate는_settings_theme_을_한국어로_반환한다() {
        let s = translate("settings.section.theme".to_string(), LocaleDto::Ko);
        // A key echo would mean the KO catalog is missing the entry —
        // this asserts the catalog actually carries a Korean translation.
        assert_ne!(s, "settings.section.theme");
        assert_eq!(s, "테마");
    }

    #[test]
    fn translate_args는_placeholders를_치환한다() {
        let s = translate_args(
            "empty.board.title".to_string(),
            LocaleDto::En,
            vec!["board_title".to_string()],
            vec!["dev".to_string()],
        );
        assert!(
            s.contains("\"dev\""),
            "expected substituted board_title=`dev` in {s:?}",
        );
    }

    #[test]
    fn locale_dto는_core_locale로_매핑된다() {
        assert_eq!(Locale::from(LocaleDto::En), Locale::En);
        assert_eq!(Locale::from(LocaleDto::Ko), Locale::Ko);
    }
}
