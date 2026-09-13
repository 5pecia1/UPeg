//! First-run bootstrap policy for the persisted `Tweaks` record (native).
//!
//! The caller (a surface's startup code) supplies the file path and a raw
//! locale hint string sourced from the platform — typically `$LANG` /
//! `$LC_ALL` on Unix, `navigator.language` on web (web doesn't compile
//! this module, but the policy is identical there). The bootstrap rule:
//!
//! 1. **Prefs file parses cleanly →** return the saved record verbatim.
//!    Even if its `locale` field is the default (English), respect that;
//!    flipping a long-time English user to Korean because `$LANG=ko_KR`
//!    is a surprise we explicitly avoid.
//! 2. **No file, or unparseable file →** treat as first run. Materialise
//!    a default `Tweaks` with `locale = detect_from_str(hint)` and save
//!    it best-effort to `path`. Save failure (read-only mount, unwritable
//!    parent, etc.) is logged-and-ignored; the in-memory result is still
//!    returned so the UI starts in a sane state.

use std::path::Path;

use upeg_core::prefs::{Locale, Tweaks, parse_tweaks};

use super::io;

/// Resolve `Tweaks` for the current process. See module docs for the rule.
///
/// `detect_hint` is the raw platform locale string (`$LANG`, browser
/// `navigator.language`, etc.) — pass `""` to force the English default
/// when no detection source is available.
pub fn bootstrap_tweaks(path: &Path, detect_hint: &str) -> Tweaks {
    if let Some(json) = io::load_from_path(path)
        && let Some(tweaks) = parse_tweaks(&json)
    {
        return tweaks;
    }
    // Either the file doesn't exist or it's corrupt — both branches treat
    // this as the user's first launch on this machine and seed the prefs.
    let tweaks = Tweaks {
        locale: Locale::detect_from_str(detect_hint),
        ..Tweaks::default_const()
    };
    if let Ok(json) = serde_json::to_string(&tweaks) {
        let _ = io::save_to_path(path, &json);
    }
    tweaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::prefs::{Accent, Theme};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "upeg-prefs-bootstrap-{label}-{}-{nanos}",
            std::process::id(),
        ))
    }

    #[test]
    fn prefs는_파일로_저장되고_다시_읽힌다() {
        // Prefs-level round trip: a full Tweaks record persisted through the
        // io layer must parse back byte-for-byte identical.
        let dir = temp_path("prefs-round-trip");
        let path = dir.join("tweaks.json");
        let original = Tweaks {
            theme: Theme::Dark,
            accent: Accent::Cyan,
            show_holes: false,
            local_http_host: false,
            locale: Locale::Ko,
        };

        io::save_to_path(&path, &serde_json::to_string(&original).unwrap()).expect("write prefs");
        let reloaded = parse_tweaks(&io::load_from_path(&path).expect("read prefs"))
            .expect("parse round-tripped prefs");
        assert_eq!(reloaded, original);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 파일이_유효하면_저장된_레코드를_반환한다() {
        let dir = temp_path("saved");
        let path = dir.join("tweaks.json");
        let probe = Tweaks {
            theme: Theme::Dark,
            accent: Accent::Pink,
            show_holes: false,
            local_http_host: false,
            locale: Locale::Ko,
        };
        io::save_to_path(&path, &serde_json::to_string(&probe).unwrap()).unwrap();

        // Even if the hint asks for English, the saved Ko preference wins.
        let loaded = bootstrap_tweaks(&path, "en_US.UTF-8");
        assert_eq!(loaded, probe);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 파일이_없으면_감지된_한국어로_기본값을_준비하고_저장한다() {
        let dir = temp_path("first-ko");
        let path = dir.join("tweaks.json");
        assert!(!path.exists());

        let result = bootstrap_tweaks(&path, "ko_KR.UTF-8");
        assert_eq!(result.locale, Locale::Ko);
        assert_eq!(result.theme, Theme::Light); // other fields are default
        assert!(path.exists(), "first run must persist the seeded record");

        // Round-trip through the saved file to confirm persistence.
        let again = bootstrap_tweaks(&path, "en_US.UTF-8");
        assert_eq!(
            again.locale,
            Locale::Ko,
            "second call must read the persisted Ko preference, not re-detect"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 파일이_없고_힌트가_지원되지_않으면_영어_기본값을_준비한다() {
        let dir = temp_path("first-en");
        let path = dir.join("tweaks.json");

        let result = bootstrap_tweaks(&path, "ja_JP.UTF-8");
        assert_eq!(
            result.locale,
            Locale::En,
            "unsupported hint falls back to English"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 손상된_파일은_첫_실행으로_취급된다() {
        let dir = temp_path("corrupt");
        let path = dir.join("tweaks.json");
        io::save_to_path(&path, "{not valid json").unwrap();

        let result = bootstrap_tweaks(&path, "ko");
        assert_eq!(result.locale, Locale::Ko);

        // First-run path also rewrites the file with a valid record so
        // future loads find a parseable shape.
        let next = bootstrap_tweaks(&path, "en");
        assert_eq!(next.locale, Locale::Ko, "rewritten file must parse");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn 저장_실패에도_메모리_결과를_반환한다() {
        // Stage a regular file where bootstrap would want to create a
        // directory — std::fs::create_dir_all then errors with NotADirectory,
        // so the best-effort save aborts. Bootstrap must still return a
        // sane Tweaks value rather than panic.
        let dir = temp_path("blocked");
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("tweaks.json");
        std::fs::write(&blocker, "blocker").unwrap();
        // Treat the regular file as if it were a parent directory.
        let unwritable = blocker.join("inner").join("tweaks.json");

        let result = bootstrap_tweaks(&unwritable, "ko");
        assert_eq!(result.locale, Locale::Ko);
        assert!(
            !unwritable.exists(),
            "save must have failed silently — file should not exist"
        );

        let _ = std::fs::remove_file(&blocker);
        let _ = std::fs::remove_dir(&dir);
    }
}
