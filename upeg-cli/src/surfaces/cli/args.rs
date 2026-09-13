//! Clap value parsers specific to the CLI surface.
//!
//! `parse_kv_arg` powers the `-a key=value` flag on `upeg call` and
//! `upeg trigger fire`. The coercion contract is documented on the
//! function itself.

/// Clap value parser for `-a key=value`. Splits on the first `=`.
///
/// Coercion rule: bare numbers, `true`/`false`/`null`, and
/// JSON-quoted strings keep their JSON type; **objects and arrays
/// stay strings** so tools that expect JSON-content-as-string (like
/// `convert.json_minify` / `convert.json_format`) work without
/// double-escaping. Anything else that doesn't parse as JSON also stays
/// a string, which is what makes `-a input=0xff` (not valid JSON) just
/// work.
pub fn parse_kv_arg(s: &str) -> Result<(String, serde_json::Value), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected `key=value`, got `{s}`"))?;
    // Trim the key so a shell paste like `-a " input = 0xff"` does
    // not produce a key `" input "` that no tool dispatcher will
    // look up. Value stays un-trimmed: trailing whitespace in
    // string values can be meaningful (e.g. `text.repeat input=" "`
    // for padding), and numeric values go through JSON parse which
    // already accepts surrounding whitespace.
    let k = k.trim();
    if k.is_empty() {
        return Err(format!("empty key in `{s}`"));
    }
    let value = match serde_json::from_str::<serde_json::Value>(v) {
        // Primitives → keep their JSON type.
        Ok(jv) if jv.is_number() || jv.is_boolean() || jv.is_null() || jv.is_string() => jv,
        // Objects/arrays — the user typed literal JSON, almost
        // certainly wants the surrounding string form (e.g. json_minify).
        Ok(_) => serde_json::Value::String(v.to_string()),
        Err(_) => serde_json::Value::String(v.to_string()),
    };
    Ok((k.to_string(), value))
}

#[cfg(test)]
mod parse_kv_trim_tests_iter222 {
    use super::parse_kv_arg;

    #[test]
    fn 파싱_kv_인자는_키를_잘라낸다() {
        // shell paste with whitespace around `=` no longer
        // silently produces a key tools can't look up.
        let (k, v) = parse_kv_arg(" input = 0xff").expect("parse");
        assert_eq!(
            k, "input",
            "leading/trailing whitespace must be trimmed from key"
        );
        assert_eq!(v, serde_json::Value::String(" 0xff".into()));
    }

    #[test]
    fn 파싱_kv_인자는_공백_만_키를_거부한다() {
        match parse_kv_arg("   =value") {
            Err(msg) => assert!(
                msg.contains("empty key"),
                "whitespace-only key must produce empty-key error; got `{msg}`"
            ),
            Ok(other) => panic!("whitespace-only key should be rejected, got {other:?}"),
        }
    }

    #[test]
    fn kv_인자_파싱의_잘라냄은_문자열_값에_영향을_주지_않는다() {
        let (k, v) = parse_kv_arg("input= ").expect("parse");
        assert_eq!(k, "input");
        assert_eq!(
            v,
            serde_json::Value::String(" ".into()),
            "value must preserve a literal space (text.repeat padding case)"
        );
    }
}
