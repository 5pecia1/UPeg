use super::*;

fn raw_policy(
    max_count: u32,
    extensions: &[&str],
    max_file_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
) -> PluginFileInputPolicy {
    PluginFileInputPolicy {
        max_count,
        extensions: extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
        max_file_bytes,
        max_total_bytes,
    }
}

fn file_field(policy: Option<PluginFileInputPolicy>) -> PluginInputField {
    let mut field = PluginInputField::required("input", PluginInputKind::File);
    field.file_policy = policy;
    field
}

fn file_policy(kind: &InputKind) -> &upeg_core::FileInputPolicy {
    match kind {
        InputKind::File(policy) => policy,
        other => panic!("expected File kind but got {other:?}"),
    }
}

#[test]
fn plugin_file_policy_converts_to_core_policy_losslessly() {
    let spec = PluginInputSpec::new([file_field(Some(raw_policy(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);

    let core = plugin_input_spec_to_core(&spec).expect("lower a valid policy to core");
    let policy = file_policy(&core.fields[0].kind);

    assert_eq!(policy.max_count(), 3);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(5_000_000));
    assert_eq!(policy.max_total_bytes(), Some(10_000_000));
}

#[test]
fn omitted_plugin_file_policy_converts_to_core_defaults() {
    let spec = PluginInputSpec::new([file_field(None)]);

    let core = plugin_input_spec_to_core(&spec).expect("apply defaults for an omitted policy");
    let policy = file_policy(&core.fields[0].kind);

    assert_eq!(policy.max_count(), 1);
    assert_eq!(policy.extensions().count(), 0);
    assert_eq!(policy.max_file_bytes(), None);
    assert_eq!(policy.max_total_bytes(), None);
}

#[test]
fn plugin_file_policy_failing_core_invariant_is_rejected() {
    let spec = PluginInputSpec::new([file_field(Some(raw_policy(0, &[], None, None)))]);

    match plugin_input_spec_to_core(&spec) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("max_count"), "actual error: {detail}");
        }
        other => panic!("expected rejection of max_count=0 but got {other:?}"),
    }
}

#[test]
fn plugin_file_max_count_converges_at_core_cap() {
    for (max_count, is_valid) in [
        (upeg_core::MAX_FILE_INPUT_COUNT, true),
        (upeg_core::MAX_FILE_INPUT_COUNT + 1, false),
    ] {
        let spec = PluginInputSpec::new([file_field(Some(raw_policy(max_count, &[], None, None)))]);

        let result = plugin_input_spec_to_core(&spec);

        assert_eq!(
            result.is_ok(),
            is_valid,
            "core convergence must differ for max_count={max_count}: {result:?}"
        );
    }
}

#[test]
fn file_policy_on_non_file_plugin_input_is_rejected() {
    let mut field = PluginInputField::required("input", PluginInputKind::String);
    field.file_policy = Some(raw_policy(1, &[], None, None));
    let spec = PluginInputSpec::new([field]);

    match plugin_input_spec_to_core(&spec) {
        Err(LoadError::InvalidInputSpec { detail }) => {
            assert!(detail.contains("File"), "actual error: {detail}");
        }
        other => panic!("expected policy rejection for a non-File input but got {other:?}"),
    }
}

#[test]
fn plugin_file_policy_is_exposed_as_json_schema_extension() {
    let spec = PluginInputSpec::new([file_field(Some(raw_policy(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);
    let core = plugin_input_spec_to_core(&spec).expect("lower a valid policy to core");

    let schema = core.to_json_schema_value();

    assert_eq!(
        schema["properties"]["input"]["x-upeg-file-policy"],
        serde_json::json!({
            "maxCount": 3,
            "extensions": ["png", "tar.gz"],
            "maxFileBytes": 5_000_000,
            "maxTotalBytes": 10_000_000
        })
    );
}

#[test]
fn plugin_exposed_file_policy_schema_reimports_into_core() {
    let spec = PluginInputSpec::new([file_field(Some(raw_policy(
        3,
        &["png", "tar.gz"],
        Some(5_000_000),
        Some(10_000_000),
    )))]);
    let core = plugin_input_spec_to_core(&spec).expect("lower a valid policy to core");
    let schema = core.to_json_schema_value();

    let imported = InputSpec::try_from(&schema).expect("reimport the exposed schema");
    let policy = file_policy(&imported.fields[0].kind);

    assert_eq!(policy.max_count(), 3);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(5_000_000));
    assert_eq!(policy.max_total_bytes(), Some(10_000_000));
}
