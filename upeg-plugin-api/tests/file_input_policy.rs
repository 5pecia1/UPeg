#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically and live at crate root"
)]

use upeg_plugin_api::{PluginInputKind, PluginManifest};

#[test]
fn plugin_file_policy_round_trips_manifest_json_losslessly() {
    let json = r#"{
        "id": "files",
        "tools": [{
            "id": "files.inspect",
            "toolkit": "files",
            "pegboard_units": "U1",
            "input_spec": {
                "fields": [{
                    "name": "input",
                    "required": true,
                    "kind": {"type": "file"},
                    "file_policy": {
                        "max_count": 3,
                        "extensions": ["png", "tar.gz"],
                        "max_file_bytes": 5000000,
                        "max_total_bytes": 10000000
                    }
                }]
            }
        }]
    }"#;

    let manifest: PluginManifest =
        serde_json::from_str(json).expect("deserialize a valid plugin manifest");
    let field = &manifest.tools[0]
        .input_spec
        .as_ref()
        .expect("input spec is present")
        .fields[0];
    let policy = field
        .file_policy
        .as_ref()
        .expect("the declared file policy is preserved");

    assert_eq!(field.kind, PluginInputKind::File);
    assert_eq!(policy.max_count, 3);
    assert_eq!(policy.extensions, ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes, Some(5_000_000));
    assert_eq!(policy.max_total_bytes, Some(10_000_000));

    let round_trip = serde_json::to_value(&manifest).expect("serialize the manifest again");
    assert_eq!(
        round_trip["tools"][0]["input_spec"]["fields"][0]["file_policy"],
        serde_json::json!({
            "max_count": 3,
            "extensions": ["png", "tar.gz"],
            "max_file_bytes": 5_000_000,
            "max_total_bytes": 10_000_000
        })
    );
}

#[test]
fn plugin_file_policy_is_omitted_from_wire_when_absent() {
    let json = r#"{
        "id": "files",
        "tools": [{
            "id": "files.inspect",
            "toolkit": "files",
            "pegboard_units": "U1",
            "input_spec": {
                "fields": [{
                    "name": "input",
                    "required": true,
                    "kind": {"type": "file"}
                }]
            }
        }]
    }"#;

    let manifest: PluginManifest =
        serde_json::from_str(json).expect("deserialize a manifest without a policy");
    let field = &manifest.tools[0]
        .input_spec
        .as_ref()
        .expect("input spec is present")
        .fields[0];
    assert!(field.file_policy.is_none());

    let value = serde_json::to_value(&manifest).expect("serialize the manifest again");
    assert!(
        value["tools"][0]["input_spec"]["fields"][0]
            .get("file_policy")
            .is_none()
    );
}
