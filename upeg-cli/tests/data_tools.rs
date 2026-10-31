#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests assert the public Tool and Chain contracts"
)]

//! Exercise real data pack dispatch and its composition through the Chain loader.

use serde_json::{Value, json};
use upeg_cli::{Outcome, dispatch_tool};

fn result(tool: &str, args: &Value) -> Value {
    let Outcome::Success(success) = dispatch_tool(tool, args) else {
        panic!("{tool} failed for the integration fixture");
    };
    let output = success
        .outputs
        .iter()
        .find(|entry| entry.id == "result")
        .expect("declared result output");
    assert_eq!(output.kind, upeg_core::OutputKind::Json);
    serde_json::from_str(&upeg_runtime::tool_success_primary_text(&success))
        .expect("result is JSON")
}

#[test]
fn data_pack_compares_json_and_yaml_without_textual_noise() {
    let output = result(
        "data.value_diff",
        &json!({
            "left": "{\"name\":\"api\",\"ports\":[80,443]}",
            "right": "ports: [80, 443]\nname: api\n",
            "right_format": "yaml"
        }),
    );
    assert_eq!(output["status"], "equal");
    assert_eq!(output["changes"], json!([]));
}

#[test]
fn two_inference_steps_join_into_the_same_schema_diff_as_direct_calls() {
    // Quotes, backslashes and newlines must survive the Chain argument handoff.
    let before = json!({"input": serde_json::to_string_pretty(&json!({
        "a/b~c": 80,
        "note": "quote \" and slash \\ and newline\n"
    })).unwrap()});
    let after = json!({"input": serde_json::to_string_pretty(&json!({
        "a/b~c": "80",
        "note": "another value"
    })).unwrap()});
    let inferred_before = result("data.schema_generate", &before);
    let inferred_after = result("data.schema_generate", &after);
    let direct = result(
        "data.schema_diff",
        &json!({
            "left": inferred_before,
            "right": inferred_after
        }),
    );

    let manifest = r#"id = "dataflow"
[[tools]]
id = "schema_diff"
invoker = "Chain"
pin = "Launcher"
pegboard_units = "U1"
inputs = [
  { name = "before", type = "json", required = true },
  { name = "after", type = "json", required = true },
]
outputs = [{ name = "result", type = "json" }]
primary_output_id = "result"
connections = [{ from = "before", to = "compare" }, { from = "after", to = "compare" }]
steps = [
  { id = "before", tool = "data.schema_generate", args = '{{input.before}}' },
  { id = "after", tool = "data.schema_generate", args = '{{input.after}}' },
  { id = "compare", tool = "data.schema_diff", args = '{"left":{{steps.before.output}},"right":{{steps.after.output}}}' },
]
"#;
    let directory = tempfile::tempdir().expect("isolated manifest directory");
    std::fs::write(directory.path().join("dataflow.toml"), manifest).expect("write fixture");
    let loaded = upeg_loader::load_and_register_dir_verbose(directory.path());
    assert!(
        loaded.failed.is_empty(),
        "manifest failed: {:?}",
        loaded.failed
    );
    assert!(loaded.skipped.is_empty());
    let chained = result(
        "dataflow.schema_diff",
        &json!({"before": before, "after": after}),
    );
    assert_eq!(chained, direct);
    assert_eq!(chained["status"], "different");
    assert_eq!(chained["changes"].as_array().expect("changes").len(), 1);
    assert_eq!(chained["changes"][0]["path"], "/a~1b~0c");
}
