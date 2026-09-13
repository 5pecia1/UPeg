//! `convert` toolkit — base64/base32, URL percent-encoding, URL query
//! parse/format, JSON pretty/minify, HTML entity escape/unescape.
//!
//! Numeric base conversions (`hex_to_decimal` and friends) live in the
//! `num` toolkit instead — see `toolkits::num`.

use upeg_core::tool;

/// `convert.base64_encode` — Standard Base64 (RFC 4648, with padding).
#[tool(
    id = "convert.base64_encode",
    display_label = "Base64 encode",
    toolkit = "convert",
    description = "Encode a UTF-8 string as Standard Base64 (RFC 4648, padded).",
    inputs = [
        required input: String = "UTF-8 string to encode",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = ["dev"],
)]
pub fn base64_encode(input: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}

/// `convert.base64_decode` — decode a Standard Base64 string back to UTF-8.
/// Returns `Err` if the body is not valid Base64 or the result isn't UTF-8.
#[tool(
    id = "convert.base64_decode",
    display_label = "Base64 decode",
    toolkit = "convert",
    description = "Decode a Standard Base64 string back to UTF-8.",
    inputs = [
        required input: String = "Base64 string to decode",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = ["dev"],
)]
pub fn base64_decode(input: &str) -> Result<String, &'static str> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let trimmed = input.trim();
    let bytes = STANDARD.decode(trimmed).map_err(|_| "invalid base64")?;
    String::from_utf8(bytes).map_err(|_| "decoded bytes are not valid UTF-8")
}

/// `convert.base32_encode` — RFC 4648 Base32 (uppercase, padded) of a UTF-8 string.
#[tool(
    id = "convert.base32_encode",
    display_label = "Base32 encode",
    toolkit = "convert",
    description = "Encode a UTF-8 string as RFC 4648 Base32 (uppercase, padded).",
    inputs = [
        required input: String = "UTF-8 string to encode",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn base32_encode(input: &str) -> String {
    base32::encode(
        base32::Alphabet::Rfc4648 { padding: true },
        input.as_bytes(),
    )
}

/// `convert.base32_decode` — decode RFC 4648 Base32 (uppercase or lowercase
/// accepted; padding optional in the decoder) back to UTF-8.
#[tool(
    id = "convert.base32_decode",
    display_label = "Base32 decode",
    toolkit = "convert",
    description = "Decode a Base32 string (RFC 4648) back to UTF-8.",
    inputs = [
        required input: String = "Base32 string to decode",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn base32_decode(input: &str) -> Result<String, &'static str> {
    let trimmed = input.trim().to_ascii_uppercase();
    let bytes = base32::decode(base32::Alphabet::Rfc4648 { padding: true }, &trimmed)
        .ok_or("invalid base32")?;
    String::from_utf8(bytes).map_err(|_| "decoded bytes are not valid UTF-8")
}

/// `convert.nfc` — Unicode-normalize a string to NFC (composed form),
/// e.g. macOS's decomposed Hangul (ㅎ+ㅏ+ㄴ) → Windows's composed
/// Hangul (한).
#[tool(
    id = "convert.nfc",
    display_label = "To NFC (Windows)",
    toolkit = "convert",
    description = "Unicode-normalize a string to NFC (composed form), e.g. for Windows filenames.",
    inputs = [
        required input: String = "String to normalize",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn convert_nfc(input: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    input.nfc().collect::<String>()
}

/// `convert.nfd` — Unicode-normalize a string to NFD (decomposed form),
/// e.g. Windows's composed Hangul (한) → macOS's decomposed Hangul
/// (ㅎ+ㅏ+ㄴ).
#[tool(
    id = "convert.nfd",
    display_label = "To NFD (macOS)",
    toolkit = "convert",
    description = "Unicode-normalize a string to NFD (decomposed form), e.g. for macOS filenames.",
    inputs = [
        required input: String = "String to normalize",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn convert_nfd(input: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    input.nfd().collect::<String>()
}

/// `convert.url_encode` — Percent-encode a string per RFC 3986.
#[tool(
    id = "convert.url_encode",
    display_label = "URL encode",
    toolkit = "convert",
    description = "Percent-encode a string per RFC 3986 (every reserved char escaped).",
    inputs = [
        required input: String = "String to encode",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = [],
)]
pub fn url_encode(input: &str) -> String {
    urlencoding::encode(input).into_owned()
}

/// `convert.url_decode` — Reverse `url_encode`. Returns `Err` on malformed
/// percent sequences or non-UTF-8 bytes.
#[tool(
    id = "convert.url_decode",
    display_label = "URL decode",
    toolkit = "convert",
    description = "Decode a percent-encoded string back to UTF-8.",
    inputs = [
        required input: String = "Percent-encoded string",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
    boards = [],
)]
pub fn url_decode(input: &str) -> Result<String, &'static str> {
    urlencoding::decode(input)
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| "invalid percent encoding")
}

/// `convert.url_query_parse` — parse `?a=1&b=hello%20world` into a
/// JSON object `{"a":"1","b":"hello world"}`.
///
/// Leading `?` is tolerated; empty pairs are skipped; missing `=` (bare
/// key) becomes `key=""`. Repeated keys: **last wins** (documented). All
/// values are strings — query-string semantics don't carry types.
#[tool(
    id = "convert.url_query_parse",
    display_label = "URL query parse",
    toolkit = "convert",
    description = "Parse a URL query string (`a=1&b=2`) into a JSON object of strings.",
    inputs = [
        required input: String = "Query string with or without leading `?`",
    ],
    outputs = [
        result: Json = "Query pairs as a JSON object of strings",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn url_query_parse(input: &str) -> Result<String, &'static str> {
    let s = input.strip_prefix('?').unwrap_or(input);
    let mut obj = serde_json::Map::new();
    for pair in s.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let dk = urlencoding::decode(k)
            .map_err(|_| "invalid percent encoding in key")?
            .into_owned();
        let dv = urlencoding::decode(v)
            .map_err(|_| "invalid percent encoding in value")?
            .into_owned();
        obj.insert(dk, serde_json::Value::String(dv));
    }
    serde_json::to_string(&serde_json::Value::Object(obj)).map_err(|_| "serialize failed")
}

/// `convert.url_query_format` — serialize a JSON object back into a
/// URL query string. Inverse of `url_query_parse` for string-valued
/// objects. Non-string values stringify via `to_string` (e.g.
/// numbers/bools); object/array values would produce JSON-shaped
/// strings, which is rarely what's wanted — caller's responsibility.
#[tool(
    id = "convert.url_query_format",
    display_label = "URL query format",
    toolkit = "convert",
    description = "Serialize a JSON object back to a URL query string (no leading `?`).",
    inputs = [
        required input: String = "JSON object whose keys/values become query pairs",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn url_query_format(input: &str) -> Result<String, &'static str> {
    let v: serde_json::Value =
        serde_json::from_str(input).map_err(|_| "input must be a JSON object (string)")?;
    let obj = v.as_object().ok_or("input JSON must be an object")?;
    let mut parts = Vec::with_capacity(obj.len());
    for (k, val) in obj {
        let s = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        parts.push(format!(
            "{}={}",
            urlencoding::encode(k),
            urlencoding::encode(&s),
        ));
    }
    Ok(parts.join("&"))
}

/// `convert.json_format` — pretty-print a JSON string with 2-space
/// indentation. Returns `Err` if the input isn't valid JSON.
#[tool(
    id = "convert.json_format",
    display_label = "JSON format",
    toolkit = "convert",
    description = "Pretty-print a JSON string with 2-space indentation.",
    inputs = [
        required input: String = "JSON to format",
    ],
    outputs = [
        result: Json = "Pretty JSON output",
    ],
    pin = Inline,
    pegboard_units = U2T,
    invoker = Function,
    boards = ["dev"],
)]
pub fn json_format(input: &str) -> Result<String, &'static str> {
    let value: serde_json::Value = serde_json::from_str(input).map_err(|_| "invalid JSON")?;
    serde_json::to_string_pretty(&value).map_err(|_| "format failed")
}

/// `convert.json_minify` — re-serialize a JSON document with no
/// whitespace. Symmetric with `convert.json_format`. Returns Err on
/// invalid input JSON.
#[tool(
    id = "convert.json_minify",
    display_label = "JSON minify",
    toolkit = "convert",
    description = "Re-serialize a JSON document with no whitespace.",
    inputs = [
        required input: String = "JSON to minify",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn json_minify(input: &str) -> Result<String, &'static str> {
    let v: serde_json::Value = serde_json::from_str(input).map_err(|_| "invalid JSON input")?;
    serde_json::to_string(&v).map_err(|_| "failed to re-serialize JSON")
}

/// `convert.html_encode` — escape the 5 standard XML/HTML entities:
/// `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`, `"` → `&quot;`, `'` → `&#39;`.
///
/// `&` MUST be replaced first, otherwise we'd escape our own escapes.
#[tool(
    id = "convert.html_encode",
    display_label = "HTML encode",
    toolkit = "convert",
    description = "Escape the 5 standard HTML/XML entities (& < > \" ').",
    inputs = [
        required input: String = "Text to escape",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn html_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// `convert.html_decode` — invert `html_encode` for the 5 standard
/// entities plus numeric forms `&#42;` (decimal) and `&#x2A;` (hex).
///
/// Named entities beyond the standard 5 (`&nbsp;`, `&copy;`, …) are
/// passed through unchanged — supporting them would require shipping
/// the full ~250-entity table, which is intentionally outside this compact utility.
/// Returns `Err` on malformed numeric escapes.
#[tool(
    id = "convert.html_decode",
    display_label = "HTML decode",
    toolkit = "convert",
    description = "Decode the 5 standard HTML entities + numeric forms (&#42;, &#x2A;).",
    inputs = [
        required input: String = "HTML-escaped text",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn html_decode(input: &str) -> Result<String, &'static str> {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            // Loop invariant: `i` is always on a UTF-8 char boundary
            // (either incremented by ch.len_utf8() or moved past ASCII
            // ';'). The remaining slice is non-empty because we're
            // inside `while i < bytes.len()`.
            #[allow(
                clippy::expect_used,
                reason = "char boundary invariant proven by loop arithmetic"
            )]
            let ch = input[i..]
                .chars()
                .next()
                .expect("html_decode loop preserves char boundary alignment");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        // Find the terminating ';'. If none within reasonable distance,
        // pass through the '&' literally (matches lenient decoders).
        let Some(end_off) = bytes[i..].iter().position(|&b| b == b';') else {
            out.push('&');
            i += 1;
            continue;
        };
        let entity = &input[i + 1..i + end_off];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            // Numeric: &#42; or &#x2A;
            n if n.starts_with('#') => {
                let body = &n[1..];
                let code =
                    if let Some(hex) = body.strip_prefix('x').or_else(|| body.strip_prefix('X')) {
                        u32::from_str_radix(hex, 16).map_err(|_| "invalid hex numeric entity")?
                    } else {
                        body.parse::<u32>()
                            .map_err(|_| "invalid decimal numeric entity")?
                    };
                let ch = char::from_u32(code).ok_or("numeric entity out of Unicode range")?;
                out.push(ch);
            }
            // Unknown named entity — pass through verbatim.
            other => {
                out.push('&');
                out.push_str(other);
                out.push(';');
            }
        }
        i += end_off + 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── base64 ─────────────────────────────────────────────────

    #[test]
    fn base64_인코딩은_알려진_벡터와_일치한다() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg==");
        assert_eq!(base64_encode("fo"), "Zm8=");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
    }

    #[test]
    fn base64_디코딩은_알려진_벡터와_일치한다() {
        assert_eq!(base64_decode("").as_deref(), Ok(""));
        assert_eq!(base64_decode("Zg==").as_deref(), Ok("f"));
        assert_eq!(base64_decode("Zm8=").as_deref(), Ok("fo"));
        assert_eq!(base64_decode("Zm9v").as_deref(), Ok("foo"));
        assert_eq!(base64_decode("aGVsbG8=").as_deref(), Ok("hello"));
    }

    #[test]
    fn base64_디코딩은_주변_공백을_제거한다() {
        assert_eq!(base64_decode("  Zg==  \n").as_deref(), Ok("f"));
    }

    #[test]
    fn base64_왕복은_임의_텍스트를_보존한다() {
        let inputs = [
            "",
            "x",
            "hello world",
            "한글 텍스트",
            "  spaces  ",
            "\nlines\n",
        ];
        for s in inputs {
            assert_eq!(
                base64_decode(&base64_encode(s)).as_deref(),
                Ok(s),
                "roundtrip failed for {s:?}",
            );
        }
    }

    #[test]
    fn base64_디코딩은_쓰레기값을_거부한다() {
        assert_eq!(base64_decode("!!!"), Err("invalid base64"));
        assert_eq!(base64_decode("not_b64"), Err("invalid base64"));
    }

    #[test]
    fn base64_디코딩은_utf8이_아닌_바이트를_거부한다() {
        assert_eq!(
            base64_decode("//79"),
            Err("decoded bytes are not valid UTF-8"),
        );
    }

    // ─── nfc / nfd ──────────────────────────────────────────────

    #[test]
    fn nfc_변환은_분해형_한글을_조합형으로_합친다() {
        let decomposed = "\u{1112}\u{1161}\u{11AB}"; // ㅎ + ㅏ + ㄴ
        assert_eq!(convert_nfc(decomposed), "한");
        assert_eq!(convert_nfc(decomposed), "\u{D55C}");
    }

    #[test]
    fn nfd_변환은_조합형_한글을_분해형으로_쪼갠다() {
        let composed = "한"; // U+D55C
        let expected = "\u{1112}\u{1161}\u{11AB}";
        assert_eq!(convert_nfd(composed), expected);
    }

    #[test]
    fn nfc와_nfd는_한글에_대해_서로를_왕복한다() {
        let composed = "한글 파일명.txt";
        let decomposed = convert_nfd(composed);
        assert_ne!(decomposed, composed, "NFD form must actually decompose");
        assert_eq!(convert_nfc(&decomposed), composed);
        assert_eq!(convert_nfd(&convert_nfc(&decomposed)), decomposed);
    }

    #[test]
    fn nfc_변환은_아스키_문자열을_그대로_둔다() {
        assert_eq!(convert_nfc("hello world"), "hello world");
        assert_eq!(convert_nfd("hello world"), "hello world");
    }

    #[test]
    fn nfc_변환은_빈_문자열을_처리한다() {
        assert_eq!(convert_nfc(""), "");
        assert_eq!(convert_nfd(""), "");
    }

    // ─── url_encode / url_decode ────────────────────────────────

    #[test]
    fn url_인코딩은_알려진_벡터와_일치한다() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_encode(""), "");
    }

    #[test]
    fn url_인코딩의_유니코드는_utf8_바이트를_사용한다() {
        let encoded = url_encode("한글");
        assert!(encoded.starts_with('%'));
        assert!(encoded.chars().all(|c| c == '%' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn url_디코딩은_알려진_벡터와_일치한다() {
        assert_eq!(url_decode("hello%20world").as_deref(), Ok("hello world"));
        assert_eq!(url_decode("a%26b%3Dc").as_deref(), Ok("a&b=c"));
        assert_eq!(url_decode("").as_deref(), Ok(""));
    }

    #[test]
    fn url_왕복은_텍스트를_보존한다() {
        for s in ["", "x", "hello world", "한글 텍스트", "a&b=c&d=e", "100%"] {
            assert_eq!(
                url_decode(&url_encode(s)).as_deref(),
                Ok(s),
                "roundtrip failed for {s:?}",
            );
        }
    }

    #[test]
    fn url_디코딩은_잘못된_형식의_퍼센트를_그대로_통과시킨다() {
        assert_eq!(url_decode("%2").as_deref(), Ok("%2"));
        assert_eq!(url_decode("%ZZ").as_deref(), Ok("%ZZ"));
    }

    #[test]
    fn url_디코딩은_utf8이_아닌_퍼센트_시퀀스를_거부한다() {
        assert_eq!(url_decode("%FF"), Err("invalid percent encoding"));
    }

    // ─── base32 ─────────────────────────────────────────────────

    #[test]
    fn base32_왕복은_아스키를_보존한다() {
        let enc = base32_encode("foo");
        let dec = base32_decode(&enc).expect("round trip");
        assert_eq!(dec, "foo");
    }

    #[test]
    fn base32_인코딩은_대문자와_패딩이_있는_알려진_벡터와_일치한다() {
        assert_eq!(base32_encode("foo"), "MZXW6===");
    }

    #[test]
    fn base32_디코더는_소문자를_허용한다() {
        let dec = base32_decode("mzxw6===").expect("lowercase ok");
        assert_eq!(dec, "foo");
    }

    #[test]
    fn base32_디코딩은_유효하지_않은_입력에_오류를_반환한다() {
        match base32_decode("not-valid-base32!") {
            Err(msg) => assert!(msg.contains("invalid base32")),
            Ok(_) => panic!("expected invalid"),
        }
    }

    // ─── json_format ────────────────────────────────────────────

    #[test]
    fn json_형식은_두_칸_공백으로_예쁘게_출력한다() {
        let out = json_format(r#"{"x":1,"y":[2,3]}"#).expect("format");
        assert!(out.contains("\"x\""));
        assert!(
            out.contains("  \"x\": 1"),
            "expected 2-space indent, got: {out}"
        );
        assert!(out.ends_with('}'));
    }

    #[test]
    fn json_형식은_중첩된_구조를_처리한다() {
        let out = json_format(r#"{"a":{"b":{"c":42}}}"#).expect("format");
        assert!(out.contains("    \"b\""));
        assert!(out.contains("      \"c\""));
    }

    #[test]
    fn json_형식은_유니코드를_보존한다() {
        let out = json_format(r#"{"k":"한글"}"#).expect("format");
        assert!(out.contains("한글"));
    }

    #[test]
    fn json_형식은_유효하지_않은_json을_거부한다() {
        assert_eq!(json_format("not json"), Err("invalid JSON"));
        assert_eq!(json_format("{trailing,}"), Err("invalid JSON"));
        assert_eq!(json_format(""), Err("invalid JSON"));
    }

    #[test]
    fn json_형식은_이미_예쁘게_출력된_입력에_대해_멱등이다() {
        let pretty = "{\n  \"x\": 1\n}";
        assert_eq!(json_format(pretty).as_deref(), Ok(pretty));
    }

    // ─── json_minify ────────────────────────────────────────────

    #[test]
    fn json_압축은_공백을_제거한다() {
        let pretty = "{\n  \"a\": 1,\n  \"b\": [2, 3]\n}";
        let mini = json_minify(pretty).unwrap();
        assert_eq!(mini, r#"{"a":1,"b":[2,3]}"#);
    }

    #[test]
    fn json_압축은_문자열_내부_공백을_보존한다() {
        let v = r#"{"msg": "hello world"}"#;
        let mini = json_minify(v).unwrap();
        assert_eq!(mini, r#"{"msg":"hello world"}"#);
    }

    #[test]
    fn json_압축은_유효하지_않은_입력을_거부한다() {
        match json_minify("totally not json") {
            Err(msg) => assert!(msg.contains("invalid JSON")),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn json_압축과_형식_사이의_왕복은_안정적이다() {
        let original = r#"{"b":2,"a":1}"#;
        let pretty = json_format(original).unwrap();
        let mini = json_minify(&pretty).unwrap();
        assert_eq!(mini, r#"{"b":2,"a":1}"#);
    }

    // ─── url_query_parse / format ─────────────────

    #[test]
    fn url_쿼리는_기본을_파싱한다() {
        let s = url_query_parse("a=1&b=2").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["a"], "1");
        assert_eq!(v["b"], "2");
    }

    #[test]
    fn url_쿼리_파싱은_앞의_물음표_표시를_제거한다() {
        let s = url_query_parse("?x=hello").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["x"], "hello");
    }

    #[test]
    fn url_쿼리_파싱은_퍼센트_인코딩을_디코딩한다() {
        let s = url_query_parse("greeting=hello%20world&%E1%84%82=k").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["greeting"], "hello world");
        let obj = v.as_object().unwrap();
        assert!(
            obj.keys().any(|k| k.chars().any(|c| (c as u32) == 0x1102)),
            "expected a key containing U+1102, got {v}"
        );
    }

    #[test]
    fn url_쿼리_파싱은_값이_없는_키와_빈_쌍을_처리한다() {
        let s = url_query_parse("a&b=&&c=1").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["a"], "");
        assert_eq!(v["b"], "");
        assert_eq!(v["c"], "1");
    }

    #[test]
    fn url_쿼리_파싱은_반복된_키에_대해_마지막_값을_우선한다() {
        let s = url_query_parse("k=first&k=second&k=third").unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["k"], "third");
    }

    #[test]
    fn url_쿼리_형식은_문자열_값을_왕복한다() {
        let qs = "name=alice&age=30&note=hello+world";
        let _parsed = url_query_parse(qs).unwrap();
        let original_obj = url_query_parse(qs).unwrap();
        let reformatted = url_query_format(&original_obj).unwrap();
        let final_obj = url_query_parse(&reformatted).unwrap();
        let v1: serde_json::Value = serde_json::from_str(&original_obj).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&final_obj).unwrap();
        assert_eq!(v1, v2, "round-trip must be lossless");
    }

    #[test]
    fn url_쿼리_형식은_객체가_아닌_것을_거부한다() {
        assert!(url_query_format(r#"["a","b"]"#).is_err());
        assert!(url_query_format("not json").is_err());
    }

    #[test]
    fn url_쿼리_형식은_특수_문자를_인코딩한다() {
        let json = r#"{"key":"a b/c?d"}"#;
        let s = url_query_format(json).unwrap();
        let back = url_query_parse(&s).unwrap();
        let v: serde_json::Value = serde_json::from_str(&back).unwrap();
        assert_eq!(v["key"], "a b/c?d");
    }

    // ─── html_encode / html_decode ─────────────────────────────

    #[test]
    fn html_인코딩은_다섯개_표준_엔티티들을_이스케이프한다() {
        assert_eq!(html_encode("a & b"), "a &amp; b");
        assert_eq!(html_encode("<p>hi</p>"), "&lt;p&gt;hi&lt;/p&gt;");
        assert_eq!(html_encode(r#"say "hi""#), "say &quot;hi&quot;");
        assert_eq!(html_encode("don't"), "don&#39;t");
    }

    #[test]
    fn html_인코딩은_앰퍼샌드_안전한이다() {
        assert_eq!(html_encode("&amp;"), "&amp;amp;");
    }

    #[test]
    fn html_인코딩과_디코딩은_표준_집합에_대해_왕복된다() {
        for s in ["plain", "a & b", "<p>", r#""hi""#, "don't"] {
            let round_trip = html_decode(&html_encode(s)).unwrap();
            assert_eq!(round_trip, s, "round-trip failed for {s:?}");
        }
    }

    #[test]
    fn html_디코딩은_숫자_와_hex_엔티티들을_처리한다() {
        assert_eq!(html_decode("&#42;").unwrap(), "*");
        assert_eq!(html_decode("&#x2A;").unwrap(), "*");
        assert_eq!(html_decode("&#x1F600;").unwrap(), "😀");
        assert_eq!(html_decode("alpha &#945; beta").unwrap(), "alpha α beta");
    }

    #[test]
    fn html_디코딩은_알수없는_엔티티를_그대로_통과시킨다() {
        assert_eq!(html_decode("&nbsp;").unwrap(), "&nbsp;");
        assert_eq!(html_decode("&copy; 2026").unwrap(), "&copy; 2026");
    }

    #[test]
    fn html_디코딩은_잘못된형식_숫자를_거부한다() {
        assert!(html_decode("&#xZZ;").is_err());
        assert!(
            html_decode("&#9999999999;").is_err(),
            "out-of-Unicode value"
        );
    }

    #[test]
    fn html_디코딩은_그대로의_앰퍼샌드를_처리한다() {
        assert_eq!(html_decode("M&Ms").unwrap(), "M&Ms");
    }
}
