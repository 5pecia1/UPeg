#![cfg(feature = "data")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use explicit fixture assertions"
)]

use serde_json::{Value, json};
use upeg_tools::{data_schema_diff, data_schema_generate};

fn generate(value: &Value, mode: &str) -> Value {
    serde_json::from_str(&data_schema_generate(&value.to_string(), "json", mode).unwrap()).unwrap()
}

fn diff(left: &Value, right: &Value) -> Value {
    serde_json::from_str(&data_schema_diff(left, right).unwrap()).unwrap()
}

#[test]
fn primitive_documents_have_explicit_types_and_one_observation() {
    let cases = [
        (json!(null), "null"),
        (json!(true), "boolean"),
        (json!(42), "number"),
        (json!(2.5), "number"),
        (json!("name"), "string"),
    ];
    for (source, kind) in cases {
        let report = generate(&source, "document");
        assert_eq!(report["schema"], json!({"type":kind}), "source: {source}");
        assert_eq!(report["observations"]["sample_count"], 1);
        assert_eq!(report["policy"], "observed_types_v1");
    }
}

#[test]
fn mixed_scalars_have_a_deterministic_union() {
    let report = generate(&json!([true, null, "x", 1, 1.5, false]), "samples");
    assert_eq!(
        report["schema"],
        json!({"anyOf":[
            {"type":"boolean"}, {"type":"null"}, {"type":"number"}, {"type":"string"}
        ]})
    );
    assert_eq!(report["observations"]["sample_count"], 6);
}

#[test]
fn absent_and_null_object_fields_are_distinct_observations() {
    let report = generate(&json!([{"a":null}, {}, {"a":1, "b":false}]), "samples");
    assert_eq!(
        report["schema"],
        json!({"type":"object","properties":{
            "a":{"anyOf":[{"type":"null"},{"type":"number"}]},
            "b":{"type":"boolean"}
        }})
    );
    assert!(report["schema"].get("required").is_none());
    assert_eq!(report["observations"]["object_instances"][""], 3);
    assert_eq!(report["observations"]["field_presence"]["/a"], 2);
    assert_eq!(report["observations"]["field_presence"]["/b"], 1);
}

#[test]
fn array_items_merge_nested_shapes_and_empty_evidence() {
    let report = generate(&json!([[{"v":1}, {"v":"s"}], [], [null]]), "samples");
    assert_eq!(
        report["schema"],
        json!({"type":"array","items":{"anyOf":[
            {"type":"null"},
            {"type":"object","properties":{"v":{"anyOf":[{"type":"number"},{"type":"string"}]}}}
        ]}})
    );
    assert_eq!(report["observations"]["object_instances"]["/*"], 2);
    assert_eq!(report["observations"]["field_presence"]["/*/v"], 2);
}

#[test]
fn nested_object_counts_use_the_relevant_parent_denominator() {
    let report = generate(
        &json!([
            {"profile":{"name":"a"},"items":[{"code":1},{}]},
            {"profile":{"age":8},"items":[]},
            {"items":[{"code":2}]}
        ]),
        "samples",
    );
    assert_eq!(
        report["schema"],
        json!({"type":"object","properties":{
            "profile":{"type":"object","properties":{
                "age":{"type":"number"},"name":{"type":"string"}
            }},
            "items":{"type":"array","items":{"type":"object","properties":{"code":{"type":"number"}}}}
        }})
    );
    let seen = &report["observations"];
    assert_eq!(seen["object_instances"][""], 3);
    assert_eq!(seen["object_instances"]["/profile"], 2);
    assert_eq!(seen["field_presence"]["/profile/name"], 1);
    assert_eq!(seen["field_presence"]["/profile/age"], 1);
    assert_eq!(seen["object_instances"]["/items/*"], 3);
    assert_eq!(seen["field_presence"]["/items/*/code"], 2);
}

#[test]
fn sample_reordering_and_duplication_preserve_the_shape() {
    let samples = json!([{"x":1,"nested":[null]}, {"x":"a","nested":[true]}, {}]);
    let reversed = json!([{}, {"nested":[true],"x":"a"}, {"nested":[null],"x":1}]);
    let duplicated =
        json!([{"x":1,"nested":[null]}, {"x":"a","nested":[true]}, {}, {"x":1,"nested":[null]}]);
    let original = generate(&samples, "samples");
    assert_eq!(generate(&reversed, "samples")["schema"], original["schema"]);
    assert_eq!(
        generate(&duplicated, "samples")["schema"],
        original["schema"]
    );
    assert_eq!(
        generate(&duplicated, "samples")["observations"]["sample_count"],
        4
    );
}

#[test]
fn nested_empty_arrays_are_inconclusive_at_the_item_path() {
    let empty = generate(&json!({"groups":[[]]}), "document");
    let known = generate(&json!({"groups":[[1]]}), "document");
    assert_eq!(
        diff(&empty, &known),
        json!({"status":"inconclusive","changes":[],"unknowns":[
            {"kind":"items_unobserved","path":"/groups/*/*","side":"left"}
        ]})
    );
    assert_eq!(diff(&empty, &empty)["unknowns"][0]["side"], "both");
}

#[test]
fn known_difference_and_unknown_evidence_are_reported_together() {
    let left = generate(&json!({"items":[],"state":1}), "document");
    let right = generate(&json!({"items":[true],"state":"one"}), "document");
    let result = diff(&left, &right);
    assert_eq!(result["status"], "different");
    assert_eq!(
        result["changes"],
        json!([{"kind":"type_changed","path":"/state","left":{"type":"number"},"right":{"type":"string"}}])
    );
    assert_eq!(
        result["unknowns"],
        json!([{"kind":"items_unobserved","path":"/items/*","side":"left"}])
    );
}

#[test]
fn nested_property_changes_have_stable_escaped_paths() {
    let before = generate(&json!({"a/b":{"x":1,"~":true}}), "document");
    let after = generate(&json!({"a/b":{"x":"one","new":null}}), "document");
    assert_eq!(
        diff(&before, &after)["changes"],
        json!([
            {"kind":"property_added","path":"/a~1b/new","right":{"type":"null"}},
            {"kind":"type_changed","path":"/a~1b/x","left":{"type":"number"},"right":{"type":"string"}},
            {"kind":"property_removed","path":"/a~1b/~0","left":{"type":"boolean"}}
        ])
    );
}

#[test]
fn generated_envelope_and_bare_schema_roundtrip_equally() {
    let first = generate(&json!([{"a":1}, {"a":null}]), "samples");
    let second = generate(&json!([{"a":9}, {"a":null}, {"a":9}]), "samples");
    assert_eq!(diff(&first, &second)["status"], "equal");
    assert_eq!(diff(&first["schema"], &first)["status"], "equal");
    assert_eq!(diff(&first, &first["schema"])["status"], "equal");
}

#[test]
fn malformed_envelopes_and_unsupported_keywords_fail_clearly() {
    let valid = generate(&json!(1), "document");
    let bad = [
        (
            json!({"schema":{"type":"number"},"version":2,"policy":"observed_types_v1"}),
            "version",
        ),
        (
            json!({"schema":{"type":"number"},"version":1,"policy":"other"}),
            "policy",
        ),
        (json!({"type":"string","format":"date"}), "format"),
        (
            json!({"type":"object","properties":{},"required":["x"]}),
            "required",
        ),
    ];
    for (schema, message) in bad {
        let error = data_schema_diff(&valid, &schema).unwrap_err();
        assert!(error.contains(message), "{schema}: {error}");
    }
}

#[test]
fn malformed_shape_types_and_container_fields_fail() {
    let valid = json!({"type":"number"});
    let bad = [
        json!({"type":["number","string"]}),
        json!({"type":"integer"}),
        json!({"type":"array","items":true}),
        json!({"type":"object","properties":[]}),
        json!({"type":"object"}),
        json!({"anyOf":[]}),
        json!({"anyOf":[{"type":"number"}]}),
    ];
    for schema in bad {
        assert!(data_schema_diff(&valid, &schema).is_err(), "{schema}");
    }
}

#[test]
fn duplicate_type_anyof_alternatives_are_rejected_to_avoid_semantic_broadening() {
    let valid = json!({"type":"object","properties":{}});
    let duplicate = json!({"anyOf":[
        {"type":"object","properties":{"x":{"type":"number"}}},
        {"type":"object","properties":{"y":{"type":"string"}}},
        {"type":"null"}
    ]});
    let error = data_schema_diff(&valid, &duplicate).unwrap_err();
    assert!(error.contains("duplicate type"), "{error}");
}

#[test]
fn independent_json_schema_validator_accepts_source_samples_and_rejects_wrong_types() {
    let cases = [
        (
            json!([{"id":1,"tag":null},{"id":2.5,"tag":"x"}]),
            vec![json!({"id":"bad"})],
        ),
        (json!([[1, "a"], [], [null]]), vec![json!([true])]),
        (json!([true, 1, "x"]), vec![json!({"unexpected":1})]),
    ];
    for (samples, wrong_types) in cases {
        let report = generate(&samples, "samples");
        let schema = &report["schema"];
        let validator =
            jsonschema::validator_for(schema).expect("generated JSON Schema must be valid");
        for sample in samples.as_array().unwrap() {
            assert!(
                validator.is_valid(sample),
                "source {sample} rejected by {schema}"
            );
        }
        for wrong in wrong_types {
            assert!(
                !validator.is_valid(&wrong),
                "mismatch {wrong} accepted by {schema}"
            );
        }
    }
}

fn nested_document(kind: &str, depth: usize) -> Value {
    let mut value = json!(1);
    for _ in 0..depth {
        value = if kind == "object" {
            json!({"next": value})
        } else {
            json!([value])
        };
    }
    value
}

#[test]
fn generated_schema_composes_with_diff_for_deep_documents() {
    for kind in ["object", "array"] {
        let source = nested_document(kind, 40);
        let text = data_schema_generate(&source.to_string(), "json", "document")
            .unwrap_or_else(|error| panic!("{kind} generation failed: {error}"));
        let envelope: Value = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{kind} output JSON parse failed: {error}"));
        let result = data_schema_diff(&envelope, &envelope)
            .unwrap_or_else(|error| panic!("{kind} schema diff failed: {error}"));
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["status"],
            "equal"
        );
    }
}

#[test]
fn generated_schema_composes_at_array_document_depth_limit() {
    let source = nested_document("array", 64);
    let text = data_schema_generate(&source.to_string(), "json", "document").unwrap();
    let envelope: Value = serde_json::from_str(&text).unwrap();
    let result: Value =
        serde_json::from_str(&data_schema_diff(&envelope, &envelope).unwrap()).unwrap();
    assert_eq!(result["status"], "equal");
}

#[test]
fn excessive_generated_object_nesting_fails_before_emitting_schema() {
    let source = nested_document("object", 64);
    let error = data_schema_generate(&source.to_string(), "json", "document").unwrap_err();
    assert!(error.contains("nesting"), "{error}");
}

#[test]
fn every_emitted_deep_schema_can_be_parsed_and_compared() {
    for kind in ["object", "array"] {
        for depth in 40..=64 {
            let source = nested_document(kind, depth);
            match data_schema_generate(&source.to_string(), "json", "document") {
                Ok(text) => {
                    let envelope: Value = serde_json::from_str(&text).unwrap_or_else(|error| {
                        panic!("{kind} depth {depth} output parse failed: {error}")
                    });
                    data_schema_diff(&envelope, &envelope).unwrap_or_else(|error| {
                        panic!("{kind} depth {depth} diff failed: {error}")
                    });
                }
                Err(error) => assert!(error.contains("nesting"), "{kind} depth {depth}: {error}"),
            }
        }
    }
}
