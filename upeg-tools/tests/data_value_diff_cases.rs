#![cfg(feature = "data")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "public value-diff contract tests use fixture assertions"
)]

use serde_json::{Value, json};
use upeg_tools::data_value_diff;

fn diff(left: &str, right: &str) -> Value {
    serde_json::from_str(&data_value_diff(left, right, "json", "json").unwrap()).unwrap()
}

fn changes(left: &str, right: &str) -> Value {
    diff(left, right)["changes"].clone()
}

#[test]
fn every_json_type_transition_is_reported_at_the_root() {
    let values = ["null", "true", "1", r#""one""#, "[]", "{}"];
    for (left_index, left) in values.iter().enumerate() {
        for (right_index, right) in values.iter().enumerate() {
            if left_index == right_index {
                continue;
            }
            assert_eq!(
                changes(left, right),
                json!([{
                    "kind": "type_changed",
                    "path": "",
                    "left": serde_json::from_str::<Value>(left).unwrap(),
                    "right": serde_json::from_str::<Value>(right).unwrap(),
                }]),
                "{left} -> {right}"
            );
        }
    }
}

#[test]
fn same_type_scalar_changes_keep_their_json_type() {
    for (left, right) in [("false", "true"), ("1", "2"), (r#""old""#, r#""new""#)] {
        assert_eq!(changes(left, right)[0]["kind"], "value_changed");
        assert_eq!(changes(left, right)[0]["path"], "");
    }
    assert_eq!(diff("null", "null")["status"], "equal");
}

#[test]
fn missing_field_and_explicit_null_are_distinct_in_both_directions() {
    assert_eq!(
        changes("{}", r#"{"item":null}"#),
        json!([{"kind":"added","path":"/item","right":null}])
    );
    assert_eq!(
        changes(r#"{"item":null}"#, "{}"),
        json!([{"kind":"removed","path":"/item","left":null}])
    );
}

#[test]
fn nested_middle_insertion_and_deletion_preserve_following_items() {
    let old = r#"{"items":[{"id":1},{"id":3}]}"#;
    let new = r#"{"items":[{"id":1},{"id":2},{"id":3}]}"#;
    assert_eq!(
        changes(old, new),
        json!([{"kind":"added","path":"/items/1","right":{"id":2}}])
    );
    assert_eq!(
        changes(new, old),
        json!([{"kind":"removed","path":"/items/1","left":{"id":2}}])
    );
}

#[test]
fn nested_array_replacement_reports_the_changed_leaf() {
    assert_eq!(
        changes(r#"{"groups":[[1,2],[3,4]]}"#, r#"{"groups":[[1,2],[3,9]]}"#),
        json!([{"kind":"value_changed","path":"/groups/1/1","left":4,"right":9}])
    );
}

#[test]
fn unequal_replacement_span_reports_each_remaining_element() {
    assert_eq!(
        changes("[1,2,3,4]", "[1,9,8,7,4]"),
        json!([
            {"kind":"value_changed","path":"/1","left":2,"right":9},
            {"kind":"value_changed","path":"/2","left":3,"right":8},
            {"kind":"added","path":"/3","right":7}
        ])
    );
}

#[test]
fn repeated_elements_do_not_turn_a_single_insertion_into_many_changes() {
    assert_eq!(
        changes("[1,2,1,2]", "[1,2,1,1,2]"),
        json!([{"kind":"added","path":"/3","right":1}])
    );
}

#[test]
fn array_reorder_is_reported_without_a_move_operation() {
    let result = diff("[1,2,3]", "[2,1,3]");
    assert_eq!(result["status"], "different");
    assert_eq!(result["changes"].as_array().unwrap().len(), 2);
    let kinds: Vec<_> = result["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"added"));
    assert!(kinds.contains(&"removed"));
}

#[test]
fn pointer_paths_escape_slashes_tildes_and_preserve_unicode() {
    assert_eq!(
        changes(r#"{"a/b":{"~雪":1}}"#, r#"{"a/b":{"~雪":2}}"#),
        json!([{"kind":"value_changed","path":"/a~1b/~0雪","left":1,"right":2}])
    );
}

#[test]
fn object_order_does_not_affect_equality_or_finding_order() {
    assert_eq!(
        diff(
            r#"{"z":1,"a":{"y":2,"x":3}}"#,
            r#"{"a":{"x":3,"y":2},"z":1}"#
        )["status"],
        "equal"
    );
    assert_eq!(
        changes(r#"{"z":1,"a":1,"m":1}"#, r#"{"m":2,"z":2,"a":2}"#),
        json!([
            {"kind":"value_changed","path":"/a","left":1,"right":2},
            {"kind":"value_changed","path":"/m","left":1,"right":2},
            {"kind":"value_changed","path":"/z","left":1,"right":2}
        ])
    );
}

#[test]
fn decimal_spellings_with_the_same_value_compare_equal() {
    for (left, right) in [("1", "1.0"), ("100", "1e2"), ("0", "-0.0"), ("0.5", "5e-1")] {
        assert_eq!(diff(left, right)["status"], "equal", "{left} vs {right}");
    }
}

#[test]
fn adjacent_large_integers_remain_distinct() {
    assert_eq!(
        changes("9007199254740992", "9007199254740993"),
        json!([{
            "kind":"value_changed",
            "path":"",
            "left":9007199254740992_u64,
            "right":9007199254740993_u64
        }])
    );
    assert_eq!(
        diff("18446744073709551615", "18446744073709551615")["status"],
        "equal"
    );
}

#[test]
fn reversing_an_unambiguous_diff_swaps_added_removed_and_values() {
    let old = r#"{"add":null,"change":1,"remove":true}"#;
    let new = r#"{"change":2,"remove":"true"}"#;
    assert_eq!(
        changes(old, new),
        json!([
            {"kind":"removed","path":"/add","left":null},
            {"kind":"value_changed","path":"/change","left":1,"right":2},
            {"kind":"type_changed","path":"/remove","left":true,"right":"true"}
        ])
    );
    assert_eq!(
        changes(new, old),
        json!([
            {"kind":"added","path":"/add","right":null},
            {"kind":"value_changed","path":"/change","left":2,"right":1},
            {"kind":"type_changed","path":"/remove","left":"true","right":true}
        ])
    );
}
