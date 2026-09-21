use crate::parse_kv_arg;

#[test]
fn kv_arg_parsing_keeps_a_non_json_value_as_a_string() {
    let (k, v) = parse_kv_arg("input=0xff").unwrap();
    assert_eq!(k, "input");
    // 0xff is not valid JSON, falls through to string.
    assert_eq!(v, serde_json::Value::String("0xff".into()));
}

#[test]
fn kv_arg_parsing_coerces_json_primitives() {
    // Numbers, booleans, null keep their JSON type — matters for
    // tools whose schema says `"type": "integer"` etc.
    assert_eq!(parse_kv_arg("n=42").unwrap().1, serde_json::json!(42));
    assert_eq!(parse_kv_arg("b=true").unwrap().1, serde_json::json!(true));
    assert_eq!(parse_kv_arg("z=null").unwrap().1, serde_json::Value::Null);
    // pin the documented "quoted string" case. The doc-comment
    // on `parse_kv_arg` says "numbers/bools/null/quoted strings keep
    // their JSON type" — meaning `key="hello"` parses to the bare
    // string `hello` (quotes stripped by serde_json). If a future
    // edit changes the is_string() branch (e.g., to fall through to
    // the raw string form like arrays/objects do), the user's
    // `-a key="hello"` would suddenly contain literal quotes —
    // silent regression for shell users who quote.
    assert_eq!(
        parse_kv_arg(r#"key="hello""#).unwrap().1,
        serde_json::Value::String("hello".into()),
        "JSON-quoted string must round-trip without surrounding quotes",
    );
    // Negative numbers and floats — same primitive path.
    assert_eq!(parse_kv_arg("n=-7").unwrap().1, serde_json::json!(-7));
    assert_eq!(parse_kv_arg("n=0.5").unwrap().1, serde_json::json!(0.5));
}

#[test]
fn kv_arg_parsing_keeps_objects_and_arrays_as_strings() {
    // Tools that take JSON-content-as-string (json_minify,
    // json_format) need the literal `{"x":1}` to arrive as a string,
    // not a parsed JSON object. Primitive coercion still applies.
    assert_eq!(
        parse_kv_arg("a=[1,2]").unwrap().1,
        serde_json::Value::String("[1,2]".into()),
    );
    assert_eq!(
        parse_kv_arg(r#"input={"x":1}"#).unwrap().1,
        serde_json::Value::String(r#"{"x":1}"#.into()),
    );
}

#[test]
fn kv_arg_parsing_rejects_a_missing_equals_sign() {
    assert!(parse_kv_arg("noequals").is_err());
}
