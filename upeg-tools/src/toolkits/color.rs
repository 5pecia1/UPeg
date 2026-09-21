//! `color` toolkit — hex/RGB color conversions and WCAG contrast checking.

use upeg_core::tool;

// ─── WCAG 2.x contrast constants (no magic numbers) ──────────────────
// Formulas per <https://www.w3.org/TR/WCAG21/#dfn-relative-luminance>
// and <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>.

/// sRGB channel value (0..=1) below which the linear conversion is a
/// simple division rather than the gamma-expansion curve.
const SRGB_LINEAR_THRESHOLD: f64 = 0.039_28;
/// Divisor for the simple (sub-threshold) linear conversion branch.
const SRGB_LINEAR_DIVISOR: f64 = 12.92;
/// Additive offset in the gamma-expansion branch.
const SRGB_GAMMA_OFFSET: f64 = 0.055;
/// Divisor in the gamma-expansion branch.
const SRGB_GAMMA_DIVISOR: f64 = 1.055;
/// Exponent in the gamma-expansion branch.
const SRGB_GAMMA_EXPONENT: f64 = 2.4;
/// Relative luminance channel weights (Rec. 709 coefficients WCAG uses).
const LUMINANCE_R_WEIGHT: f64 = 0.2126;
const LUMINANCE_G_WEIGHT: f64 = 0.7152;
const LUMINANCE_B_WEIGHT: f64 = 0.0722;
/// Contrast-ratio formula offset — `(L1 + K) / (L2 + K)`.
const CONTRAST_RATIO_OFFSET: f64 = 0.05;
/// Decimal places the reported ratio is rounded to.
const CONTRAST_RATIO_ROUND_FACTOR: f64 = 100.0;

/// WCAG 2.x minimum contrast ratios (§1.4.3 / §1.4.6).
const WCAG_AA_NORMAL_MIN_RATIO: f64 = 4.5;
const WCAG_AA_LARGE_MIN_RATIO: f64 = 3.0;
const WCAG_AAA_NORMAL_MIN_RATIO: f64 = 7.0;
const WCAG_AAA_LARGE_MIN_RATIO: f64 = 4.5;

/// Parse a hex color (`#ff8800` or `ff8800`) into `(r, g, b)` bytes.
///
/// Strict 6-char form only — 3-char shorthand (`#f80`) is rejected so
/// callers get a clear error rather than a silently-misinterpreted value.
/// Shared by `color.hex_to_rgb` and `color.contrast` so the parsing rules
/// (and their error messages) stay in one place.
fn parse_hex_rgb(input: &str) -> Result<(u8, u8, u8), &'static str> {
    let body = input.trim().strip_prefix('#').unwrap_or(input.trim());
    if body.len() != 6 {
        return Err("hex color must be 6 chars (3-char shorthand not supported)");
    }
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("hex color must contain only 0-9 / a-f / A-F");
    }
    let r = u8::from_str_radix(&body[0..2], 16).map_err(|_| "parse R failed")?;
    let g = u8::from_str_radix(&body[2..4], 16).map_err(|_| "parse G failed")?;
    let b = u8::from_str_radix(&body[4..6], 16).map_err(|_| "parse B failed")?;
    Ok((r, g, b))
}

/// `color.hex_to_rgb` — parse a hex color (`#ff8800` or `ff8800`) into
/// comma-separated decimal RGB (`255,136,0`).
///
/// Strict 6-char form only — 3-char shorthand (`#f80`) is rejected so
/// users get a clear error rather than a silently-misinterpreted value.
#[tool(
    id = "color.hex_to_rgb",
    display_label = "Hex → RGB",
    toolkit = "color",
    description = "Parse a hex color (#ff8800 or ff8800) into comma-separated R,G,B decimals.",
    inputs = [
        required input: String = "Hex color, with or without leading #",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn color_hex_to_rgb(input: &str) -> Result<String, &'static str> {
    let (r, g, b) = parse_hex_rgb(input)?;
    Ok(format!("{r},{g},{b}"))
}

/// `color.rgb_to_hex` — pack comma-separated decimal RGB (`255,136,0`)
/// or whitespace-tolerated variants into a `#`-prefixed lowercase hex
/// color (`#ff8800`).
///
/// Each component must be 0..=255; out-of-range or non-numeric → `Err`.
#[tool(
    id = "color.rgb_to_hex",
    display_label = "RGB → Hex",
    toolkit = "color",
    description = "Pack `r,g,b` decimals (each 0-255) into a #-prefixed lowercase hex color.",
    inputs = [
        required input: String = "`r,g,b` decimals (each 0-255)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn color_rgb_to_hex(input: &str) -> Result<String, &'static str> {
    let parts: Vec<&str> = input.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return Err("expected 3 comma-separated values");
    }
    let mut bytes = [0_u8; 3];
    for (i, p) in parts.iter().enumerate() {
        bytes[i] = p
            .parse::<u8>()
            .map_err(|_| "each component must be 0-255")?;
    }
    Ok(format!("#{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2]))
}

/// `color.contrast` — WCAG 2.x contrast ratio between two hex colors,
/// plus AA/AAA pass/fail for normal and large text.
///
/// Promoted from `gui_meta.rs` (previously `pin = Live`, no dispatcher).
/// A two-input pure computation has no notion of "live" (nothing ticks
/// or streams) — `pin = Live` was a copy-paste leftover from the
/// Live-pin GUI tools nearby in `gui_meta.rs`. Reclassified `Inline` to
/// match `color.hex_to_rgb`'s convention: pure `Function` computations
/// over user-supplied text render inline.
#[tool(
    id = "color.contrast",
    display_label = "Contrast checker",
    toolkit = "color",
    description = "Compute the WCAG 2.x contrast ratio between two hex colors, with AA/AAA pass/fail.",
    inputs = [
        required foreground: String = "Foreground hex color, with or without leading #",
        required background: String = "Background hex color, with or without leading #",
    ],
    outputs = [
        result: Json = "Contrast ratio + AA/AAA pass/fail flags",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn color_contrast(foreground: &str, background: &str) -> Result<String, &'static str> {
    let fg = parse_hex_rgb(foreground)?;
    let bg = parse_hex_rgb(background)?;
    let ratio = contrast_ratio(fg, bg);
    let rounded = (ratio * CONTRAST_RATIO_ROUND_FACTOR).round() / CONTRAST_RATIO_ROUND_FACTOR;
    Ok(serde_json::json!({
        "ratio": rounded,
        "aa": ratio >= WCAG_AA_NORMAL_MIN_RATIO,
        "aa_large": ratio >= WCAG_AA_LARGE_MIN_RATIO,
        "aaa": ratio >= WCAG_AAA_NORMAL_MIN_RATIO,
        "aaa_large": ratio >= WCAG_AAA_LARGE_MIN_RATIO,
    })
    .to_string())
}

/// WCAG relative luminance of one sRGB channel (0..=255).
fn srgb_channel_luminance(channel: u8) -> f64 {
    let normalized = f64::from(channel) / 255.0;
    if normalized <= SRGB_LINEAR_THRESHOLD {
        normalized / SRGB_LINEAR_DIVISOR
    } else {
        ((normalized + SRGB_GAMMA_OFFSET) / SRGB_GAMMA_DIVISOR).powf(SRGB_GAMMA_EXPONENT)
    }
}

/// WCAG relative luminance of an `(r, g, b)` triple.
fn relative_luminance((r, g, b): (u8, u8, u8)) -> f64 {
    LUMINANCE_R_WEIGHT.mul_add(
        srgb_channel_luminance(r),
        LUMINANCE_G_WEIGHT.mul_add(
            srgb_channel_luminance(g),
            LUMINANCE_B_WEIGHT * srgb_channel_luminance(b),
        ),
    )
}

/// WCAG contrast ratio between two colors — always `>= 1.0`, order-independent.
fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + CONTRAST_RATIO_OFFSET) / (darker + CONTRAST_RATIO_OFFSET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_rgb_allows_hash_present_and_absent() {
        assert_eq!(color_hex_to_rgb("#ff8800").unwrap(), "255,136,0");
        assert_eq!(color_hex_to_rgb("ff8800").unwrap(), "255,136,0");
    }

    #[test]
    fn hex_to_rgb_ignores_case() {
        assert_eq!(color_hex_to_rgb("#FF8800").unwrap(), "255,136,0");
        assert_eq!(color_hex_to_rgb("#Ff88AA").unwrap(), "255,136,170");
    }

    #[test]
    fn hex_to_rgb_rejects_bad_lengths_and_chars() {
        assert!(color_hex_to_rgb("#fff").is_err(), "3-char form rejected");
        assert!(
            color_hex_to_rgb("#ff88000").is_err(),
            "7-char form rejected"
        );
        let r = color_hex_to_rgb("#zz0000");
        assert!(matches!(r, Err(m) if m.contains("hex")));
    }

    #[test]
    fn rgb_to_hex_roundtrip() {
        assert_eq!(color_rgb_to_hex("255,136,0").unwrap(), "#ff8800");
        let rt = color_rgb_to_hex(&color_hex_to_rgb("#FF8800").unwrap()).unwrap();
        assert_eq!(rt, "#ff8800");
    }

    #[test]
    fn rgb_to_hex_handles_whitespace_around_components() {
        assert_eq!(color_rgb_to_hex(" 255 , 136 , 0 ").unwrap(), "#ff8800");
    }

    #[test]
    fn rgb_to_hex_rejects_out_of_range_and_bad_counts() {
        assert!(color_rgb_to_hex("256,0,0").is_err(), "out of range");
        assert!(color_rgb_to_hex("-1,0,0").is_err(), "negative");
        assert!(color_rgb_to_hex("255,0").is_err(), "only 2 components");
        assert!(color_rgb_to_hex("a,b,c").is_err(), "non-numeric");
    }

    // ─── color_contrast ──────────────────────────────────────

    fn contrast_json(foreground: &str, background: &str) -> serde_json::Value {
        let raw = color_contrast(foreground, background).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn contrast_check_computes_black_white_max_ratio_21_to_1() {
        let v = contrast_json("#000000", "#ffffff");
        assert!(
            (v["ratio"].as_f64().unwrap() - 21.0).abs() < 0.01,
            "expected ~21.0, got {v}"
        );
        assert_eq!(v["aa"], true);
        assert_eq!(v["aa_large"], true);
        assert_eq!(v["aaa"], true);
        assert_eq!(v["aaa_large"], true);
    }

    #[test]
    fn contrast_check_computes_same_color_min_ratio_1_to_1() {
        let v = contrast_json("#336699", "#336699");
        assert!(
            (v["ratio"].as_f64().unwrap() - 1.0).abs() < 0.01,
            "expected ~1.0, got {v}"
        );
        assert_eq!(v["aa"], false);
        assert_eq!(v["aaa"], false);
    }

    #[test]
    fn contrast_check_is_foreground_background_order_independent() {
        let forward = contrast_json("#123456", "#fedcba");
        let backward = contrast_json("#fedcba", "#123456");
        assert_eq!(forward["ratio"], backward["ratio"]);
    }

    #[test]
    fn contrast_check_scores_gray_near_aa_boundary_accurately() {
        // WebAIM reference pair: #767676 on white is the well-known
        // "just passes AA normal text" boundary (~4.54:1); #777777 is
        // one step darker (lighter gray text) and just fails (~4.48:1).
        let passes = contrast_json("#767676", "#ffffff");
        assert_eq!(
            passes["aa"], true,
            "767676/white should pass AA, got {passes}"
        );
        assert_eq!(
            passes["aaa"], false,
            "767676/white should fail AAA, got {passes}"
        );

        let fails = contrast_json("#777777", "#ffffff");
        assert_eq!(
            fails["aa"], false,
            "777777/white should fail AA, got {fails}"
        );
    }

    #[test]
    fn contrast_check_rejects_invalid_hex_colors() {
        assert!(color_contrast("#zzzzzz", "#ffffff").is_err());
        assert!(
            color_contrast("#fff", "#ffffff").is_err(),
            "3-char shorthand rejected"
        );
        assert!(
            color_contrast("#000000", "#ff").is_err(),
            "background too short"
        );
    }
}
