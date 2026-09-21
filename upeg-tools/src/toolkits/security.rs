//! `security` toolkit — local password generation and zxcvbn-backed strength estimate.

use super::fill_os_random;
use upeg_core::tool;

pub const PASSWORD_DEFAULT_LENGTH: usize = 20;
pub(crate) const PASSWORD_DEFAULT_INCLUDE_SYMBOLS: bool = false;
pub(crate) const PASSWORD_MIN_LENGTH: usize = 8;
pub(crate) const PASSWORD_MAX_LENGTH: usize = 128;
const PASSWORD_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const PASSWORD_SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?";

/// Max byte count accepted by `security.bytes_generate`. Keeps the hex
/// response bounded — generating huge random strings isn't the use case.
const BYTES_GENERATE_MAX_COUNT: usize = 1024;
/// Default byte count for `security.bytes_generate` — a 32-char hex token
/// (~128 bits of entropy).
pub const BYTES_GENERATE_DEFAULT_COUNT: usize = 16;

/// `security.bytes_generate` — generate `count` cryptographically random
/// bytes formatted as `2*count` lowercase hex chars. Default `count=16`
/// (32-char token, ~128 bits of entropy).
///
/// Consolidated (W5 consistency pass, R6) from the former single-tool
/// `random` toolkit (`random.hex_bytes`). It joins `security` next to
/// `password_generate` — both are OS-entropy generators, so a shared
/// `generator`-tagged home removes the lone-tool `random` toolkit. The id
/// follows the neighbour's `<noun>_generate` verb shape (cf.
/// `password_generate`); the output is always hex.
///
/// Capped at [`BYTES_GENERATE_MAX_COUNT`] bytes to keep the response bounded.
#[tool(
    id = "security.bytes_generate",
    display_label = "Random bytes",
    toolkit = "security",
    description = "Generate `count` cryptographically random bytes as lowercase hex (default 16, max 1024).",
    inputs = [
        optional count: Integer = "Byte count (default 16, max 1024)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn bytes_generate(count: usize) -> Result<String, &'static str> {
    if count > BYTES_GENERATE_MAX_COUNT {
        return Err("count too large (max 1024 bytes)");
    }
    let mut buf = vec![0_u8; count];
    fill_os_random(&mut buf)?;
    let mut hex = String::with_capacity(count * 2);
    for b in buf {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}

/// `security.password_generate` — generate a random local password.
///
/// Uses the OS RNG and rejection sampling so character selection is not
/// modulo-biased. This is intentionally a pure local Tool: no `SaaS` call,
/// no persistence, and no password leaves the process.
#[tool(
    id = "security.password_generate",
    display_label = "Password generator",
    toolkit = "security",
    description = "Generate a random local password with OS entropy (count 8-128, default 20).",
    inputs = [
        // Range/default are machine-readable constraints, not prose: they
        // must stay equal to the PASSWORD_* consts below, which
        // `password_length_constraints_match_declared_constants` pins (the macro
        // grammar only accepts literals here).
        optional count: Integer(min=8, max=128, default=20) = "Password length",
        optional include_symbols: Boolean = "Include punctuation symbols (default false)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn password_generate(count: usize, include_symbols: bool) -> Result<String, String> {
    // Defense in depth: the declared `Integer(min, max)` constraints are
    // enforced by the runtime before dispatch, but a direct Rust caller
    // bypasses that path, so the bounds are re-checked here from the same
    // consts the declaration mirrors.
    if count < PASSWORD_MIN_LENGTH {
        return Err(password_too_short_message());
    }
    if count > PASSWORD_MAX_LENGTH {
        return Err(password_too_long_message());
    }

    let mut alphabet = PASSWORD_ALPHA.to_vec();
    if include_symbols {
        alphabet.extend_from_slice(PASSWORD_SYMBOLS);
    }

    let mut out = String::with_capacity(count);
    for _ in 0..count {
        let idx = random_index(alphabet.len()).map_err(str::to_string)?;
        out.push(alphabet[idx] as char);
    }
    Ok(out)
}

fn password_too_short_message() -> String {
    format!("count too short (min {PASSWORD_MIN_LENGTH})")
}

fn password_too_long_message() -> String {
    format!("count too large (max {PASSWORD_MAX_LENGTH})")
}

fn random_index(alphabet_len: usize) -> Result<usize, &'static str> {
    let zone = 256_usize - (256_usize % alphabet_len);
    loop {
        let mut b = [0_u8; 1];
        fill_os_random(&mut b)?;
        let n = b[0] as usize;
        if n < zone {
            return Ok(n % alphabet_len);
        }
    }
}

/// `security.password_estimate` — zxcvbn-backed password strength estimate.
///
/// Delegates the hard part to the maintained `zxcvbn` estimator instead of
/// a local composition-rule heuristic. Output remains JSON so CLI/HTTP/MCP/
/// Desktop callers can render the same fields without parsing prose.
#[tool(
    id = "security.password_estimate",
    display_label = "Password strength estimate",
    toolkit = "security",
    description = "Estimate password strength locally with zxcvbn and return score/warnings as JSON.",
    inputs = [
        required input: String = "Password to evaluate",
    ],
    outputs = [
        result: Json = "Score, label, warnings, and crack-time estimates",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn password_estimate(input: &str) -> String {
    let length = input.chars().count();
    let has_lower = input.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = input.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = input.chars().any(|c| c.is_ascii_digit());
    let has_symbol = input.chars().any(|c| !c.is_ascii_alphanumeric());

    let entropy = zxcvbn::zxcvbn(input, &[]);
    let score = u8::from(entropy.score());
    let label = match score {
        0 => "weak",
        1 => "fair",
        2 => "good",
        3 => "strong",
        _ => "excellent",
    };

    let mut warnings = password_estimate_warnings(input, length, entropy.feedback());
    warnings.sort_unstable();
    warnings.dedup();

    let feedback_warning = entropy
        .feedback()
        .and_then(zxcvbn::feedback::Feedback::warning)
        .map(|warning| warning.to_string());
    let feedback_suggestions: Vec<String> = entropy
        .feedback()
        .map(|feedback| {
            feedback
                .suggestions()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let crack_times = entropy.crack_times();

    serde_json::json!({
        "score": score,
        "label": label,
        "length": length,
        "classes": {
            "lower": has_lower,
            "upper": has_upper,
            "digit": has_digit,
            "symbol": has_symbol,
        },
        "warnings": warnings,
        "feedback": {
            "warning": feedback_warning,
            "suggestions": feedback_suggestions,
        },
        "guesses": crack_times.guesses(),
        "guesses_log10": entropy.guesses_log10(),
        "crack_times": {
            "online_throttling_100_per_hour": crack_times.online_throttling_100_per_hour().to_string(),
            "online_no_throttling_10_per_second": crack_times.online_no_throttling_10_per_second().to_string(),
            "offline_slow_hashing_1e4_per_second": crack_times.offline_slow_hashing_1e4_per_second().to_string(),
            "offline_fast_hashing_1e10_per_second": crack_times.offline_fast_hashing_1e10_per_second().to_string(),
        },
    })
    .to_string()
}

fn password_estimate_warnings(
    input: &str,
    length: usize,
    feedback: Option<&zxcvbn::feedback::Feedback>,
) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if length < PASSWORD_MIN_LENGTH {
        warnings.push("too_short");
    }
    if input
        .chars()
        .next()
        .is_some_and(|first| input.chars().all(|c| c == first))
    {
        warnings.push("repeated_character");
    }
    if let Some(feedback) = feedback
        && let Some(warning) = feedback.warning()
    {
        use zxcvbn::feedback::Warning;
        warnings.push(match warning {
            Warning::StraightRowsOfKeysAreEasyToGuess
            | Warning::ShortKeyboardPatternsAreEasyToGuess
            | Warning::SequencesLikeAbcAreEasyToGuess => "sequence_pattern",
            Warning::RepeatsLikeAaaAreEasyToGuess
            | Warning::RepeatsLikeAbcAbcAreOnlySlightlyHarderToGuess => "repeated_character",
            Warning::ThisIsATop10Password
            | Warning::ThisIsATop100Password
            | Warning::ThisIsACommonPassword
            | Warning::ThisIsSimilarToACommonlyUsedPassword
            | Warning::AWordByItselfIsEasyToGuess
            | Warning::NamesAndSurnamesByThemselvesAreEasyToGuess
            | Warning::CommonNamesAndSurnamesAreEasyToGuess => "common_pattern",
            Warning::RecentYearsAreEasyToGuess | Warning::DatesAreOftenEasyToGuess => {
                "date_pattern"
            }
        });
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── bytes_generate (consolidated from random.hex_bytes) ─────

    #[test]
    fn random_bytes_use_default_length() {
        let s = bytes_generate(BYTES_GENERATE_DEFAULT_COUNT).unwrap();
        assert_eq!(s.len(), 32);
        assert!(
            s.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn random_bytes_return_empty_string_for_zero_length() {
        assert_eq!(bytes_generate(0).unwrap(), "");
    }

    #[test]
    fn random_bytes_cap_length_at_1024() {
        assert!(bytes_generate(BYTES_GENERATE_MAX_COUNT).is_ok());
        match bytes_generate(BYTES_GENERATE_MAX_COUNT + 1) {
            Err(msg) => assert!(msg.contains("count too large")),
            Ok(_) => panic!("expected error for count over the cap"),
        }
    }

    #[test]
    fn random_bytes_two_calls_almost_always_differ() {
        let a = bytes_generate(32).unwrap();
        let b = bytes_generate(32).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn password_generate_default_contract_is_local_alphanumeric() {
        const { assert!(!PASSWORD_DEFAULT_INCLUDE_SYMBOLS) };
        let s =
            password_generate(PASSWORD_DEFAULT_LENGTH, PASSWORD_DEFAULT_INCLUDE_SYMBOLS).unwrap();
        assert_eq!(s.len(), PASSWORD_DEFAULT_LENGTH);
        assert!(
            s.chars().all(|c| c.is_ascii_alphanumeric()),
            "default password must match untouched Boolean form dispatch, got {s:?}"
        );
    }

    #[test]
    fn password_generate_rejects_extreme_lengths() {
        assert_eq!(
            password_generate(PASSWORD_MIN_LENGTH - 1, true),
            Err(password_too_short_message())
        );
        assert_eq!(
            password_generate(PASSWORD_MAX_LENGTH + 1, true),
            Err(password_too_long_message())
        );
    }

    /// The `#[tool]` grammar only accepts numeric literals inside
    /// `Integer(min=…, max=…, default=…)`, so the declared range cannot
    /// reference the PASSWORD_* consts directly. This pins the two
    /// together so they can never drift apart silently.
    #[test]
    fn password_length_constraints_match_declared_constants() {
        let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
            .find(|meta| meta.id == crate::PASSWORD_GENERATE_TOOL_ID)
            .expect("security.password_generate must exist in the static inventory");
        let count = meta
            .input_spec
            .fields
            .iter()
            .find(|field| field.name == "count")
            .expect("security.password_generate must declare a `count` input");

        assert_eq!(
            count.constraints.number,
            Some(upeg_core::StaticNumberConstraints {
                min: Some(PASSWORD_MIN_LENGTH as f64),
                max: Some(PASSWORD_MAX_LENGTH as f64),
                default: Some(PASSWORD_DEFAULT_LENGTH as f64),
            }),
            "declared Integer(min, max, default) must mirror the PASSWORD_* consts",
        );
    }

    #[test]
    fn password_generate_without_symbols_uses_alphanumeric_only() {
        let s = password_generate(32, false).unwrap();
        assert!(
            s.chars().all(|c| c.is_ascii_alphanumeric()),
            "expected alphanumeric-only password, got {s:?}"
        );
    }

    #[test]
    fn password_estimate_flags_common_weak_inputs() {
        let raw = password_estimate("password123");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["label"], "weak");
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str() == Some("common_pattern"))
        );
    }

    #[test]
    fn password_estimate_uses_zxcvbn_for_l33t_common_passwords() {
        let raw = password_estimate("P@ssword1");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            v["score"].as_u64().unwrap() <= 1,
            "zxcvbn should classify l33t common-password variants as weak/fair, got {raw}"
        );
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str() == Some("common_pattern")),
            "expected common-pattern warning from zxcvbn feedback, got {raw}"
        );
        assert!(
            v["feedback"]["suggestions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str().unwrap_or_default().contains("substitutions")),
            "expected zxcvbn-specific substitution feedback, got {raw}"
        );
    }

    #[test]
    fn password_estimate_scores_long_mixed_passwords_highly() {
        let raw = password_estimate("Tr0ub4dor&Correct-Horse-2026");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            v["score"].as_u64().unwrap() >= 4,
            "expected strong score, got {raw}"
        );
        assert_eq!(v["classes"]["lower"], true);
        assert_eq!(v["classes"]["upper"], true);
        assert_eq!(v["classes"]["digit"], true);
        assert_eq!(v["classes"]["symbol"], true);
    }
}
