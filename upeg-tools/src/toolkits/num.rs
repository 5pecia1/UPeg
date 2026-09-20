//! `num` toolkit — numeric base conversions (hex/decimal/binary).
//!
//! `hex_to_decimal` moved in from `convert` (see the historical note that
//! used to sit atop `convert.rs`): a `#[tool]`'s `id` must literally start
//! with its `toolkit` prefix (`upeg-macros/src/validate.rs::validate_tool_identity`),
//! so a `num.*` id needs a real `num` toolkit. Back then that would have
//! forced a single-tool toolkit (the exact R6 violation `qr`/`csv` already
//! carried), so the rename waited for a second numeric tool. With
//! `decimal_to_hex` / `decimal_to_binary` / `binary_to_decimal` joining it,
//! `num` now clears R6 (≥2 tools) on its own.

use upeg_core::tool;

/// `num.hex_to_decimal` — parse a hex string (with or without `0x` prefix)
/// into a decimal u128.
#[tool(
    id = "num.hex_to_decimal",
    display_label = "Hex → Decimal",
    toolkit = "num",
    description = "Parse a hex string (with or without `0x` prefix) into a decimal u128.",
    inputs = [
        required input: String = "Hex string, e.g. \"0xff\" or \"DEADBEEF\"",
    ],
    outputs = [
        result: Number = "Decimal value",
    ],
    pin = Inline,
    pegboard_units = U2,
    invoker = Function,
    boards = ["dev"],
)]
pub fn hex_to_decimal(input: &str) -> Result<u128, &'static str> {
    let trimmed = input.trim();
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if body.is_empty() {
        return Err("empty input");
    }
    u128::from_str_radix(body, 16).map_err(|_| "invalid hex or overflow")
}

/// `num.decimal_to_hex` — format a decimal u128 as lowercase hex, no `0x`
/// prefix. Inverse of `num.hex_to_decimal` modulo case/prefix
/// normalization (hex_to_decimal accepts either).
#[tool(
    id = "num.decimal_to_hex",
    display_label = "Decimal → Hex",
    toolkit = "num",
    description = "Format a decimal number as lowercase hex (no `0x` prefix).",
    inputs = [
        required input: String = "Decimal number, e.g. \"255\"",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = ["dev"],
)]
pub fn decimal_to_hex(input: &str) -> Result<String, &'static str> {
    let n = input
        .trim()
        .parse::<u128>()
        .map_err(|_| "invalid decimal number")?;
    Ok(format!("{n:x}"))
}

/// `num.decimal_to_binary` — format a decimal u128 as a binary digit
/// string (no `0b` prefix).
#[tool(
    id = "num.decimal_to_binary",
    display_label = "Decimal → Binary",
    toolkit = "num",
    description = "Format a decimal number as a binary digit string (no `0b` prefix).",
    inputs = [
        required input: String = "Decimal number, e.g. \"255\"",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = ["dev"],
)]
pub fn decimal_to_binary(input: &str) -> Result<String, &'static str> {
    let n = input
        .trim()
        .parse::<u128>()
        .map_err(|_| "invalid decimal number")?;
    Ok(format!("{n:b}"))
}

/// `num.binary_to_decimal` — parse a binary digit string (with or without
/// `0b` prefix) into a decimal u128. Mirrors `num.hex_to_decimal`'s
/// prefix-tolerant parsing.
#[tool(
    id = "num.binary_to_decimal",
    display_label = "Binary → Decimal",
    toolkit = "num",
    description = "Parse a binary digit string (with or without `0b` prefix) into a decimal u128.",
    inputs = [
        required input: String = "Binary string, e.g. \"11111111\" or \"0b11111111\"",
    ],
    outputs = [
        result: Number = "Decimal value",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = ["dev"],
)]
pub fn binary_to_decimal(input: &str) -> Result<u128, &'static str> {
    let trimmed = input.trim();
    let body = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
        .unwrap_or(trimmed);
    if body.is_empty() {
        return Err("empty input");
    }
    if !body.chars().all(|c| c == '0' || c == '1') {
        return Err("invalid binary digits (only 0/1 allowed)");
    }
    u128::from_str_radix(body, 2).map_err(|_| "invalid binary or overflow")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── hex_to_decimal ─────────────────────────────────────────────

    #[test]
    fn hex_to_decimal_allows_lowercase_prefix() {
        assert_eq!(hex_to_decimal("0xff"), Ok(255));
    }

    #[test]
    fn hex_to_decimal_allows_uppercase_prefix() {
        assert_eq!(hex_to_decimal("0XFF"), Ok(255));
    }

    #[test]
    fn hex_to_decimal_allows_missing_prefix() {
        assert_eq!(hex_to_decimal("ff"), Ok(255));
    }

    #[test]
    fn hex_to_decimal_trims_surrounding_whitespace() {
        assert_eq!(hex_to_decimal("  0xff  "), Ok(255));
    }

    #[test]
    fn hex_to_decimal_handles_zero() {
        assert_eq!(hex_to_decimal("0x0"), Ok(0));
    }

    #[test]
    fn hex_to_decimal_rejects_empty_input() {
        assert_eq!(hex_to_decimal(""), Err("empty input"));
        assert_eq!(hex_to_decimal("0x"), Err("empty input"));
    }

    #[test]
    fn hex_to_decimal_rejects_invalid_chars() {
        assert_eq!(hex_to_decimal("0xZZ"), Err("invalid hex or overflow"));
        assert_eq!(hex_to_decimal("not-hex"), Err("invalid hex or overflow"));
    }

    #[test]
    fn hex_to_decimal_handles_u128_max() {
        let s = "0xffffffffffffffffffffffffffffffff";
        assert_eq!(hex_to_decimal(s), Ok(u128::MAX));
    }

    #[test]
    fn hex_to_decimal_rejects_overflow_above_u128_max() {
        let s = "0xfffffffffffffffffffffffffffffffff";
        assert_eq!(hex_to_decimal(s), Err("invalid hex or overflow"));
    }

    // ─── decimal_to_hex ─────────────────────────────────────────────

    #[test]
    fn decimal_to_hex_formats_lowercase() {
        assert_eq!(decimal_to_hex("255").unwrap(), "ff");
        assert_eq!(decimal_to_hex("0").unwrap(), "0");
        assert_eq!(decimal_to_hex("16").unwrap(), "10");
    }

    #[test]
    fn decimal_to_hex_trims_surrounding_whitespace() {
        assert_eq!(decimal_to_hex("  255  ").unwrap(), "ff");
    }

    #[test]
    fn decimal_to_hex_handles_u128_max() {
        assert_eq!(
            decimal_to_hex(&u128::MAX.to_string()).unwrap(),
            "ffffffffffffffffffffffffffffffff"
        );
    }

    #[test]
    fn decimal_to_hex_rejects_invalid_input() {
        assert!(decimal_to_hex("not-a-number").is_err());
        assert!(decimal_to_hex("-1").is_err());
        assert!(decimal_to_hex("").is_err());
    }

    #[test]
    fn decimal_hex_roundtrip_matches_hex_to_decimal() {
        for n in [0_u128, 1, 16, 255, 4096, u128::MAX] {
            let hex = decimal_to_hex(&n.to_string()).unwrap();
            assert_eq!(hex_to_decimal(&hex), Ok(n), "round-trip failed for {n}");
        }
    }

    // ─── decimal_to_binary ────────────────────────────────────────

    #[test]
    fn decimal_to_binary_formats_binary_string() {
        assert_eq!(decimal_to_binary("255").unwrap(), "11111111");
        assert_eq!(decimal_to_binary("0").unwrap(), "0");
        assert_eq!(decimal_to_binary("5").unwrap(), "101");
    }

    #[test]
    fn decimal_to_binary_trims_surrounding_whitespace() {
        assert_eq!(decimal_to_binary("  5  ").unwrap(), "101");
    }

    #[test]
    fn decimal_to_binary_rejects_invalid_input() {
        assert!(decimal_to_binary("not-a-number").is_err());
        assert!(decimal_to_binary("-1").is_err());
        assert!(decimal_to_binary("").is_err());
    }

    // ─── binary_to_decimal ────────────────────────────────────────

    #[test]
    fn binary_to_decimal_parses_binary() {
        assert_eq!(binary_to_decimal("11111111"), Ok(255));
        assert_eq!(binary_to_decimal("101"), Ok(5));
        assert_eq!(binary_to_decimal("0"), Ok(0));
    }

    #[test]
    fn binary_to_decimal_allows_0b_prefix() {
        assert_eq!(binary_to_decimal("0b101"), Ok(5));
        assert_eq!(binary_to_decimal("0B101"), Ok(5));
    }

    #[test]
    fn binary_to_decimal_trims_surrounding_whitespace() {
        assert_eq!(binary_to_decimal("  101  "), Ok(5));
    }

    #[test]
    fn binary_to_decimal_rejects_empty_input() {
        assert_eq!(binary_to_decimal(""), Err("empty input"));
        assert_eq!(binary_to_decimal("0b"), Err("empty input"));
    }

    #[test]
    fn binary_to_decimal_rejects_chars_beyond_0_and_1() {
        assert!(binary_to_decimal("1012").is_err());
        assert!(binary_to_decimal("not-binary").is_err());
    }

    #[test]
    fn binary_to_decimal_handles_u128_max() {
        let s = "1".repeat(128);
        assert_eq!(binary_to_decimal(&s), Ok(u128::MAX));
    }

    #[test]
    fn binary_decimal_roundtrip_matches_decimal_to_binary() {
        for n in [0_u128, 1, 5, 255, 4096, u128::MAX] {
            let bin = decimal_to_binary(&n.to_string()).unwrap();
            assert_eq!(binary_to_decimal(&bin), Ok(n), "round-trip failed for {n}");
        }
    }
}
