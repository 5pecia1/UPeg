//! `id` toolkit — UUID v4/v7 + NanoID generators.

use super::fill_os_random;
use upeg_core::tool;

/// `id.uuid_v7` — generate a fresh time-ordered UUID v7.
///
/// Returned as the canonical hyphenated string (e.g.
/// `0193fa2c-8b1d-7e4c-9a6f-0e7a3f8b2c4d`). Uses the standard `uuid` crate
/// so calls produce real UUIDs rather than deterministic placeholders.
// Equalized with `uuid_v4`/`nanoid` (W5 consistency pass, R13/R14): the
// three generators are one equivalence group, so `uuid_v7` drops its lone
// `source = Manual`, its ad-hoc `boards`, its `uuid`-named single output,
// and its odd `U2` sizing to share the trio's simplest common shape
// (default source, no boards, fallback `result` output, `U1`).
#[tool(
    id = "id.uuid_v7",
    display_label = "UUID v7",
    toolkit = "id",
    description = "Generate a fresh time-ordered UUID v7 (RFC 9562).",
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn uuid_v7() -> String {
    uuid::Uuid::now_v7().hyphenated().to_string()
}

/// `id.uuid_v4` — generate a purely-random UUID v4.
///
/// Use this when you don't want time information embedded in the id
/// (v7 carries a millisecond timestamp). Otherwise prefer v7 for its
/// natural sort + database-index-friendly properties.
#[tool(
    id = "id.uuid_v4",
    display_label = "UUID v4",
    toolkit = "id",
    description = "Generate a purely-random UUID v4 (RFC 9562).",
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn uuid_v4() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

/// `id.nanoid` — generate a 21-char URL-safe `NanoID`.
///
/// Hand-rolled to avoid pulling in the `nanoid` crate's `getrandom 0.2`
/// dependency (which conflicts with our `getrandom 0.3` wasm setup).
/// Uses the canonical 64-char URL-safe alphabet (A-Za-z0-9_-).
#[tool(
    id = "id.nanoid",
    display_label = "NanoID",
    toolkit = "id",
    description = "Generate a 21-character URL-safe NanoID (collision-resistant).",
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = [],
)]
pub fn nanoid() -> Result<String, &'static str> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut bytes = [0_u8; 21];
    fill_os_random(&mut bytes)?;
    Ok(bytes
        .iter()
        .map(|b| ALPHABET[(*b & 0x3F) as usize] as char)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── uuid_v7 ────────────────────────────────────────────────

    #[test]
    fn uuid_v7는_정규_하이픈있는_형식을_가진다() {
        let s = uuid_v7();
        assert_eq!(s.len(), 36);
        let groups: Vec<&str> = s.split('-').collect();
        assert_eq!(
            groups.iter().map(|g| g.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(s.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn uuid_v7_버전_니블은_일곱이다() {
        let s = uuid_v7();
        assert_eq!(
            s.chars().nth(14),
            Some('7'),
            "expected version digit '7' at position 14, got: {s}",
        );
    }

    #[test]
    fn uuid_v7_두개_호출들_생성_서로다른_값을_검증한다() {
        let a = uuid_v7();
        let b = uuid_v7();
        assert_ne!(a, b);
    }

    #[test]
    fn uuid_v4는_14번째_자리에_버전_4가_있는_정규_형식이다() {
        let s = uuid_v4();
        assert_eq!(s.len(), 36);
        assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
        assert_eq!(
            s.chars().nth(14),
            Some('4'),
            "expected version digit '4' at position 14, got: {s}"
        );
    }

    #[test]
    fn uuid_v4_두개_호출들_서로다른을_검증한다() {
        assert_ne!(uuid_v4(), uuid_v4());
    }

    #[test]
    fn uuid_v7는_시간_정렬된_안에서_루프이다() {
        let mut prev = uuid_v7();
        for _ in 0..10 {
            let next = uuid_v7();
            assert!(next >= prev, "v7 should be time-ordered: {prev} → {next}");
            prev = next;
        }
    }

    // ─── nanoid ─────────────────────────────────────────────────

    #[test]
    fn 나노id는_21_url_안전한_문자들을_반환한다() {
        let id = nanoid().unwrap();
        assert_eq!(id.len(), 21, "default nanoid alphabet produces 21 chars");
        let url_safe = id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        assert!(url_safe, "expected URL-safe charset, got {id:?}");
    }

    #[test]
    fn nanoid의_연속_두_호출은_서로_다른_값을_생성한다() {
        assert_ne!(nanoid().unwrap(), nanoid().unwrap());
    }
}
