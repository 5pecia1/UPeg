#![cfg(feature = "data")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "public boundary tests assert documented limits"
)]

use serde_json::{Value, json};
use std::fmt::Write as _;
use upeg_tools::{data_schema_diff, data_schema_generate, data_value_diff};

const MIB: usize = 1024 * 1024;

fn report(left: &str, right: &str) -> Value {
    serde_json::from_str(&data_value_diff(left, right, "json", "json").unwrap()).unwrap()
}

#[test]
fn source_byte_limit_accepts_exact_size_and_rejects_one_byte_more() {
    let exact = format!("\"{}\"", "a".repeat(MIB - 2));
    assert_eq!(exact.len(), MIB);
    assert_eq!(report(&exact, &exact)["status"], "equal");
    let too_long = format!("{exact} ");
    assert!(
        data_value_diff(&too_long, "null", "json", "json")
            .unwrap_err()
            .contains("left input exceeds 1 MiB")
    );
    assert!(
        data_value_diff("null", &too_long, "json", "json")
            .unwrap_err()
            .contains("right input exceeds 1 MiB")
    );
    assert!(
        data_schema_generate(&too_long, "json", "document")
            .unwrap_err()
            .contains("1 MiB")
    );
}

#[test]
fn source_limit_counts_utf8_bytes_not_characters() {
    let exact = format!("\"{}\"", "é".repeat((MIB - 2) / 2));
    assert_eq!(exact.len(), MIB);
    assert!(data_schema_generate(&exact, "json", "document").is_ok());
    let oversized = format!("\"{}é\"", "é".repeat((MIB - 2) / 2));
    assert!(
        data_schema_generate(&oversized, "json", "document")
            .unwrap_err()
            .contains("1 MiB")
    );
}

#[test]
fn array_length_limit_is_inclusive() {
    let exact = json!(vec![0; 2048]).to_string();
    assert_eq!(report(&exact, &exact)["status"], "equal");
    let excessive = json!(vec![0; 2049]).to_string();
    assert!(
        data_value_diff(&excessive, &excessive, "json", "json")
            .unwrap_err()
            .contains("2048")
    );
}

#[test]
fn nesting_limit_agrees_between_json_and_yaml_for_scalar_and_empty_leaves() {
    for leaf in ["null", "[]", "{}"] {
        for levels in [64, 65] {
            let source = format!("{}{leaf}{}", "[".repeat(levels), "]".repeat(levels));
            let json = data_value_diff(&source, &source, "json", "json");
            let yaml = data_value_diff(&source, &source, "yaml", "yaml");
            assert_eq!(
                json.is_ok(),
                levels == 64,
                "json levels {levels} leaf {leaf}"
            );
            assert_eq!(
                yaml.is_ok(),
                levels == 64,
                "yaml levels {levels} leaf {leaf}: {yaml:?}"
            );
        }
    }
}

#[test]
fn value_node_limit_is_inclusive_for_wide_objects() {
    let mut object: serde_json::Map<String, Value> =
        (0..19_999).map(|i| (format!("p{i}"), json!(0))).collect();
    let exact = Value::Object(object.clone()).to_string();
    assert_eq!(report(&exact, &exact)["status"], "equal");
    object.insert("one_more".into(), json!(0));
    let excessive = Value::Object(object).to_string();
    assert!(
        data_value_diff(&excessive, &excessive, "json", "json")
            .unwrap_err()
            .contains("20000")
    );
}

#[test]
fn alignment_work_limit_accepts_exact_budget_and_rejects_above() {
    let left = json!(vec![0; 1000]).to_string();
    let exact = json!(vec![1; 1000]).to_string();
    assert_eq!(
        report(&left, &exact)["changes"].as_array().unwrap().len(),
        1000
    );
    let excessive = json!(vec![1; 1001]).to_string();
    assert!(
        data_value_diff(&left, &excessive, "json", "json")
            .unwrap_err()
            .contains("1000000")
    );
}

#[test]
fn value_finding_limit_does_not_silently_truncate_changes() {
    let mut fields: serde_json::Map<String, Value> =
        (0..5000).map(|i| (format!("p{i}"), Value::Null)).collect();
    let exact = Value::Object(fields.clone()).to_string();
    assert_eq!(
        report("{}", &exact)["changes"].as_array().unwrap().len(),
        5000
    );
    fields.insert("extra".into(), Value::Null);
    let excessive = Value::Object(fields).to_string();
    assert!(
        data_value_diff("{}", &excessive, "json", "json")
            .unwrap_err()
            .contains("5000")
    );
}

#[test]
fn schema_exact_finding_limit_allows_later_equal_fields() {
    let mut properties: serde_json::Map<String, Value> = (0..5000)
        .map(|i| (format!("p{i:04}"), json!({"type":"null"})))
        .collect();
    properties.insert("z_unchanged".into(), json!({"type":"number"}));
    let left = json!({"type":"object","properties":{"z_unchanged":{"type":"number"}}});
    let right = json!({"type":"object","properties":properties});
    let compared: Value = serde_json::from_str(&data_schema_diff(&left, &right).unwrap()).unwrap();
    assert_eq!(compared["changes"].as_array().unwrap().len(), 5000);
}

#[test]
fn direct_schema_inputs_count_key_bytes_before_comparing() {
    let oversized = json!({"type":"object","properties":{"x".repeat(2 * MIB):{"type":"null"}}});
    assert!(
        data_schema_diff(&oversized, &oversized)
            .unwrap_err()
            .contains("2 MiB")
    );
}

#[test]
fn value_output_budget_returns_error_instead_of_partial_success() {
    // Split the path budget across nesting so each input is <1 MiB while
    // repeating the common prefix in path-level findings grows the output.
    let key = "k".repeat(25_000);
    let left =
        json!({key.clone(): {"items": (0..100).map(|_| json!({"v":0})).collect::<Vec<_>>()}});
    let right = json!({key: {"items": (0..100).map(|_| json!({"v":1})).collect::<Vec<_>>()}});
    assert!(left.to_string().len() < MIB && right.to_string().len() < MIB);
    assert!(
        data_value_diff(&left.to_string(), &right.to_string(), "json", "json")
            .unwrap_err()
            .contains("2 MiB")
    );
}

#[test]
fn yaml_event_limit_accepts_exact_budget_and_rejects_one_more_mapping_entry() {
    // Six stream/document/root-map events plus two scalar events per entry.
    let mut source = String::new();
    for index in 0..19_997 {
        writeln!(source, "p{index}: 0").unwrap();
    }
    let same: Value =
        serde_json::from_str(&data_value_diff(&source, &source, "yaml", "yaml").unwrap()).unwrap();
    assert_eq!(same["status"], "equal");
    let excessive = format!("{source}extra: 0\n");
    assert!(
        data_value_diff(&excessive, &excessive, "yaml", "yaml")
            .unwrap_err()
            .contains("40000")
    );
}

#[test]
fn schema_generation_and_diff_refuse_oversized_reports() {
    let key = "k".repeat(25_000);
    let sample = json!({key.clone():{"items":(0..100).map(|i|(format!("p{i}"),json!(0))).collect::<serde_json::Map<_,_>>()}});
    assert!(sample.to_string().len() < MIB);
    assert!(
        data_schema_generate(&sample.to_string(), "json", "document")
            .unwrap_err()
            .contains("2 MiB")
    );

    let schema = |kind| {
        json!({"type":"object","properties":{key.clone():{
            "type":"object","properties":(0..100).map(|i|(format!("p{i}"),json!({"type":kind}))).collect::<serde_json::Map<_,_>>()
        }}})
    };
    assert!(
        data_schema_diff(&schema("number"), &schema("string"))
            .unwrap_err()
            .contains("2 MiB")
    );
}
