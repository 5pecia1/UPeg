#![cfg(feature = "data")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "table-driven public parser contract assertions"
)]

use serde_json::{Value, json};
use upeg_tools::{data_schema_generate, data_value_diff};

fn same_as_json(source: &str, expected: Value) {
    let result: Value = serde_json::from_str(
        &data_value_diff(source, &expected.to_string(), "yaml", "json")
            .unwrap_or_else(|error| panic!("YAML {source:?}: {error}")),
    )
    .unwrap();
    assert_eq!(result, json!({"status":"equal","changes":[]}), "{source:?}");
}

#[test]
fn yaml_core_null_boolean_and_quoted_scalars_follow_their_types() {
    // Independent fixtures from YAML 1.2.2 §10.3.2 Core tag resolution:
    // https://yaml.org/spec/1.2.2/#1032-tag-resolution
    for (source, expected) in [
        ("null", Value::Null),
        ("Null", Value::Null),
        ("NULL", Value::Null),
        ("~", Value::Null),
        ("True", json!(true)),
        ("TRUE", json!(true)),
        ("False", json!(false)),
        ("FALSE", json!(false)),
        ("'null'", json!("null")),
        ("\"TRUE\"", json!("TRUE")),
        ("''", json!("")),
        ("yes", json!("yes")),
        ("off", json!("off")),
        ("value:\n", json!({"value":null})),
    ] {
        same_as_json(source, expected);
    }
}

#[test]
fn yaml_block_strings_preserve_chomping_and_folding() {
    for (source, expected) in [
        ("text: |\n  first\n  second\n", "first\nsecond\n"),
        ("text: |-\n  first\n  second\n", "first\nsecond"),
        ("text: >\n  first\n  second\n", "first second\n"),
        ("text: >-\n  first\n  second\n", "first second"),
    ] {
        same_as_json(source, json!({"text":expected}));
    }
}

#[test]
fn yaml_comments_crlf_unicode_and_single_document_markers_are_presentation_only() {
    same_as_json(
        "---\r\n# explanation\r\n서버:\r\n  이름: 'API 😀' # ignored\r\n  ports: [80, 443]\r\n...\r\n",
        json!({"서버":{"이름":"API 😀","ports":[80,443]}}),
    );
    same_as_json("---\n# explicit empty document\n...\n", Value::Null);
}

#[test]
fn yaml_quoted_merge_spelling_is_an_ordinary_string_key() {
    same_as_json("'<<': literal\n", json!({"<<":"literal"}));
    same_as_json("\"<<\": {a: 1}\n", json!({"<<":{"a":1}}));
    assert!(data_schema_generate("<<: {a: 1}", "yaml", "document").is_err());
}

#[test]
fn yaml_rejects_ambiguous_keys_features_and_nonfinite_numbers() {
    for source in [
        "a: 1\na: 2",
        "outer: {a: 1, a: 2}",
        "a: &anchor 1\nb: *anchor",
        "value: !!str 1",
        "value: !local data",
        "true: value",
        "null: value",
        "? [one, two]\n: value",
        "a: .nan",
        "a: -.Inf",
        "a: 1e9999",
        "a: +1",
        "a: 0x10",
        "a: 1\n---\na: 2",
        "[unclosed",
        "",
        "# comment only\n",
    ] {
        assert!(
            data_schema_generate(source, "yaml", "document").is_err(),
            "{source:?}"
        );
    }
}

#[test]
fn malformed_json_is_rejected_with_the_correct_input_side() {
    for source in [
        "",
        "{",
        "[1,]",
        "{} {}",
        "// comment\n{}",
        "NaN",
        "01",
        "1e9999",
        "\"\\uD800\"",
    ] {
        let left = data_value_diff(source, "null", "json", "json").unwrap_err();
        let right = data_value_diff("null", source, "json", "json").unwrap_err();
        assert!(left.contains("left JSON"), "{source:?}: {left}");
        assert!(right.contains("right JSON"), "{source:?}: {right}");
    }
}

#[test]
fn formats_and_sampling_modes_are_explicit_and_case_sensitive() {
    for format in ["JSON", "toml", "auto", " yaml "] {
        assert!(
            data_schema_generate("{}", format, "document")
                .unwrap_err()
                .contains("format")
        );
        assert!(
            data_value_diff("{}", "{}", "json", format)
                .unwrap_err()
                .contains("format")
        );
    }
    for mode in ["Samples", "all", " document "] {
        assert!(
            data_schema_generate("{}", "json", mode)
                .unwrap_err()
                .contains("sample_mode")
        );
    }
    assert_eq!(
        data_schema_generate("{}", "", "").unwrap(),
        data_schema_generate("{}", "json", "document").unwrap()
    );
}

#[test]
fn samples_mode_requires_a_nonempty_collection_of_documents() {
    for source in ["[]", "{}", "null", "42", "\"value\""] {
        assert!(
            data_schema_generate(source, "json", "samples")
                .unwrap_err()
                .contains("nonempty array")
        );
    }
    let report: Value =
        serde_json::from_str(&data_schema_generate("[[]]", "json", "samples").unwrap()).unwrap();
    assert_eq!(report["observations"]["sample_count"], 1);
    assert_eq!(report["schema"], json!({"type":"array"}));
}
