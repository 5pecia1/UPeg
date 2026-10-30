#![cfg(feature = "data")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "public data-tool contract tests use fixture assertions"
)]

use serde_json::{Value, json};
use upeg_tools::{data_schema_diff, data_schema_generate, data_value_diff};

fn generated(source: &str, format: &str, mode: &str) -> Value {
    serde_json::from_str(&data_schema_generate(source, format, mode).unwrap()).unwrap()
}

fn schema_diff(left: &Value, right: &Value) -> Value {
    serde_json::from_str(&data_schema_diff(left, right).unwrap()).unwrap()
}

#[test]
fn changed_values_and_sample_counts_do_not_change_observed_types() {
    let left = generated(r#"[{"id":1,"active":true}]"#, "json", "samples");
    let right = generated(
        r#"[{"active":false,"id":9},{"id":10,"active":true}]"#,
        "json",
        "samples",
    );
    assert_eq!(
        left["schema"],
        json!({
            "type":"object",
            "properties":{"active":{"type":"boolean"},"id":{"type":"number"}}
        })
    );
    assert_eq!(schema_diff(&left, &right)["status"], "equal");
}

#[test]
fn missing_null_and_unobserved_items_have_different_meanings() {
    let empty = generated("[]", "json", "document");
    let number_items = generated("[1]", "json", "document");
    assert_eq!(schema_diff(&empty, &empty)["status"], "inconclusive");
    assert_eq!(schema_diff(&empty, &number_items)["status"], "inconclusive");
    let missing = generated("{}", "json", "document");
    let explicit_null = generated(r#"{"item":null}"#, "json", "document");
    let diff = schema_diff(&missing, &explicit_null);
    assert_eq!(
        diff["changes"][0],
        json!({
            "kind":"property_added","path":"/item","right":{"type":"null"}
        })
    );
}

#[test]
fn yaml_plain_dates_and_hyphenated_strings_remain_strings() {
    let yaml = "date: 2026-10-08\nlabel: 1-2\nname: foo.inf\n";
    let json = r#"{"date":"2026-10-08","label":"1-2","name":"foo.inf"}"#;
    let result: Value =
        serde_json::from_str(&data_value_diff(json, yaml, "json", "yaml").unwrap()).unwrap();
    assert_eq!(result["status"], "equal");
    assert_eq!(
        generated(json, "json", "document")["schema"],
        generated(yaml, "yaml", "document")["schema"]
    );
}

#[test]
fn inferred_union_order_does_not_change_the_type() {
    let left = generated(r#"[null,1,"name"]"#, "json", "samples");
    let mut right = left.clone();
    right["schema"]["anyOf"].as_array_mut().unwrap().reverse();
    assert_eq!(schema_diff(&left, &right)["status"], "equal");
}

#[test]
fn schema_observations_distinguish_array_items_from_literal_star_properties() {
    let report = generated(r#"[{"x":[{}]},{"x":{"*":{}}}]"#, "json", "samples");
    assert_eq!(report["observations"]["object_instances"]["/x/*"], 1);
    assert_eq!(report["observations"]["object_instances"]["/x/~2"], 1);
    let before = generated(r#"{"*":1,"~2":true}"#, "json", "document");
    let after = generated(r#"{"*":"1","~2":false}"#, "json", "document");
    let diff = schema_diff(&before, &after);
    assert_eq!(diff["changes"][0]["path"], "/~2");
    assert_eq!(before["observations"]["field_presence"]["/~02"], 1);
}

#[test]
fn unsupported_schema_versions_and_contract_constraints_are_rejected() {
    let left = generated("{}", "json", "document");
    let mut future = left.clone();
    future["version"] = json!(2);
    assert!(data_schema_diff(&left, &future).is_err());
    let mut constrained = left.clone();
    constrained["schema"]["required"] = json!(["id"]);
    assert!(
        data_schema_diff(&left, &constrained)
            .unwrap_err()
            .contains("required")
    );
}

#[test]
fn schema_finding_limit_applies_to_unmatched_properties() {
    let properties: serde_json::Map<String, Value> = (0..5001)
        .map(|index| (format!("field_{index}"), json!({"type":"number"})))
        .collect();
    let left = json!({"type":"object","properties":properties});
    let right = json!({"type":"object","properties":{}});
    assert!(
        data_schema_diff(&left, &right)
            .unwrap_err()
            .contains("5000")
    );
}

#[test]
fn adjacent_large_integers_are_not_rounded_into_equality() {
    let result: Value = serde_json::from_str(
        &data_value_diff("9007199254740992", "9007199254740993", "json", "json").unwrap(),
    )
    .unwrap();
    assert_eq!(result["status"], "different");
    assert_eq!(result["changes"][0]["kind"], "value_changed");
    assert!(data_value_diff("9007199254740993.0", "9007199254740992.0", "json", "json").is_err());
}

#[test]
fn array_alignment_matches_objects_independent_of_key_order() {
    let result: Value = serde_json::from_str(
        &data_value_diff(
            r#"[{"id":1,"name":"a"},{"id":2,"name":"b"}]"#,
            r#"[{"id":0},{"name":"a","id":1},{"name":"b","id":2}]"#,
            "json",
            "json",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        result["changes"],
        json!([{"kind":"added","path":"/0","right":{"id":0}}])
    );
}
