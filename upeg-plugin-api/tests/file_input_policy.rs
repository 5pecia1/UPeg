#![allow(
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect idiomatically and live at crate root"
)]

use upeg_plugin_api::{PluginInputKind, PluginManifest};

#[test]
fn plugin_file_정책은_manifest_json을_손실없이_왕복한다() {
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
        serde_json::from_str(json).expect("유효한 plugin manifest를 역직렬화한다");
    let field = &manifest.tools[0]
        .input_spec
        .as_ref()
        .expect("입력 명세가 있다")
        .fields[0];
    let policy = field
        .file_policy
        .as_ref()
        .expect("명시한 file 정책이 보존된다");

    assert_eq!(field.kind, PluginInputKind::File);
    assert_eq!(policy.max_count, 3);
    assert_eq!(policy.extensions, ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes, Some(5_000_000));
    assert_eq!(policy.max_total_bytes, Some(10_000_000));

    let round_trip = serde_json::to_value(&manifest).expect("manifest를 다시 직렬화한다");
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
fn plugin_file_정책은_생략되면_wire에서_빠진다() {
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
        serde_json::from_str(json).expect("정책을 생략한 manifest를 역직렬화한다");
    let field = &manifest.tools[0]
        .input_spec
        .as_ref()
        .expect("입력 명세가 있다")
        .fields[0];
    assert!(field.file_policy.is_none());

    let value = serde_json::to_value(&manifest).expect("manifest를 다시 직렬화한다");
    assert!(
        value["tools"][0]["input_spec"]["fields"][0]
            .get("file_policy")
            .is_none()
    );
}
