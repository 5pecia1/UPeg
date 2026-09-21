//! Surface-agnostic translation lookup with English fallback.
//!
//! `i18n` is the *mechanism* — `t` and `t_args` resolve a key against a
//! caller-supplied catalog and walk a deterministic fallback chain. The
//! catalog content (English / Korean text) lives next to each surface
//! (`flutter_app/lib/src/i18n/`, `upeg-cli/src/i18n.rs`), so a desktop-only
//! string never leaks into the TUI catalog and vice versa.
//!
//! Fallback chain for a lookup `(locale, key)`:
//!   1. `(locale, key)` — direct hit.
//!   2. `(Locale::En, key)` — every supported locale must cover at least the
//!      keys English covers, but missing translations gracefully degrade to
//!      the canonical English text rather than blank-screen the user.
//!   3. The raw `key` — last-resort visual marker so a typo'd lookup
//!      surfaces in the UI instead of silently rendering empty.
//!
//! Catalogs are exposed as `fn(Locale, &str) -> Option<&'static str>` so the
//! storage shape (a `phf::Map`, a hand-rolled `match`, a `HashMap` for
//! tests) stays a surface implementation detail.

use crate::prefs::Locale;

/// Catalog lookup function. Returns the registered text for the given
/// locale + key, or `None` to defer to the fallback chain.
pub type CatalogFn = fn(Locale, &str) -> Option<&'static str>;

/// Look up `key` in `catalog` under `locale`, falling back to English
/// and finally to the key itself.
///
/// Returns `&'static str` (not `String`) so callers that don't need
/// argument interpolation pay no allocation cost.
pub fn t(locale: Locale, key: &str, catalog: CatalogFn) -> &'static str {
    if let Some(hit) = catalog(locale, key) {
        return hit;
    }
    if locale != Locale::En
        && let Some(hit) = catalog(Locale::En, key)
    {
        return hit;
    }
    // Last-resort: leak the key into the UI. We can't return the original
    // `&str` (its lifetime isn't 'static), so we hand back a single static
    // placeholder. Production callers should never observe this branch
    // because `tests::catalogs_cover_every_used_key` (in each surface)
    // pins the catalog completeness.
    MISSING_KEY_MARKER
}

const MISSING_KEY_MARKER: &str = "???";

/// Look up `key` and substitute `{name}` placeholders from `args`.
///
/// Unmatched placeholders are emitted verbatim (`{needle}` stays as
/// literal text) so missing arguments surface in the UI rather than
/// silently vanishing. Extra args (names not present in the template)
/// are ignored.
pub fn t_args(locale: Locale, key: &str, args: &[(&str, &str)], catalog: CatalogFn) -> String {
    let template = t(locale, key, catalog);
    substitute_args(template, args)
}

/// Public so each surface's catalog tests can exercise the substitution
/// rules directly without going through a `CatalogFn`.
pub fn substitute_args(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            // Unmatched '{' — emit the rest verbatim and stop scanning.
            out.push('{');
            out.push_str(after);
            return out;
        };
        let name = &after[..close];
        if let Some((_, value)) = args.iter().find(|(n, _)| *n == name) {
            out.push_str(value);
        } else {
            // Leave placeholder intact for debuggability.
            out.push('{');
            out.push_str(name);
            out.push('}');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two-entry catalog: En has `greet` + `english_only`, Ko has only `greet`.
    /// Exercises the full fallback chain without a real surface catalog.
    fn fixture(locale: Locale, key: &str) -> Option<&'static str> {
        match (locale, key) {
            (Locale::En, "greet") => Some("hello"),
            (Locale::Ko, "greet") => Some("안녕"),
            (Locale::En, "english_only") => Some("only here in english"),
            (Locale::En, "with_arg") => Some("hello {name}"),
            (Locale::Ko, "with_arg") => Some("{name}님 안녕"),
            _ => None,
        }
    }

    #[test]
    fn direct_lookup_returns_the_translation() {
        assert_eq!(t(Locale::Ko, "greet", fixture), "안녕");
        assert_eq!(t(Locale::En, "greet", fixture), "hello");
    }

    #[test]
    fn missing_locale_entry_falls_back_to_english() {
        // Ko catalog has no `english_only` — must serve the En text rather
        // than blanking out the UI.
        assert_eq!(
            t(Locale::Ko, "english_only", fixture),
            "only here in english",
        );
    }

    #[test]
    fn missing_key_returns_a_visible_marker() {
        // Both fallback paths fail — render a marker so the typo surfaces
        // in the UI rather than silently rendering empty.
        assert_eq!(t(Locale::En, "no.such.key", fixture), MISSING_KEY_MARKER);
        assert_eq!(t(Locale::Ko, "no.such.key", fixture), MISSING_KEY_MARKER);
    }

    #[test]
    fn arg_substitution_replaces_named_placeholders() {
        assert_eq!(
            t_args(Locale::En, "with_arg", &[("name", "world")], fixture),
            "hello world",
        );
        assert_eq!(
            t_args(Locale::Ko, "with_arg", &[("name", "지구")], fixture),
            "지구님 안녕",
        );
    }

    #[test]
    fn arg_substitution_leaves_unmatched_placeholders_intact() {
        // Missing args must surface in the UI as `{name}` so the bug is
        // visible rather than silently rendering empty.
        assert_eq!(t_args(Locale::En, "with_arg", &[], fixture), "hello {name}",);
    }

    #[test]
    fn arg_substitution_ignores_extra_args() {
        assert_eq!(
            t_args(
                Locale::En,
                "with_arg",
                &[("name", "x"), ("unused", "y")],
                fixture,
            ),
            "hello x",
        );
    }

    #[test]
    fn arg_substitution_handles_multiple_placeholders() {
        let out = substitute_args(
            "{a} and {b}, then {a} again",
            &[("a", "alpha"), ("b", "beta")],
        );
        assert_eq!(out, "alpha and beta, then alpha again");
    }

    #[test]
    fn arg_substitution_preserves_an_unmatched_open_brace() {
        // Pathological input: a literal `{` with no matching `}`. Emit it
        // verbatim rather than panicking or hanging.
        let out = substitute_args("price: {amount with no close", &[("amount", "5")]);
        assert_eq!(out, "price: {amount with no close");
    }

    #[test]
    fn arg_substitution_handles_templates_without_placeholders() {
        let out = substitute_args("static text", &[("ignored", "value")]);
        assert_eq!(out, "static text");
    }
}
