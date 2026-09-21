use super::*;
use serde_json::json;
use upeg_runtime::execution_requirements::{
    ToolExecutionRequirements, set_tool_execution_requirements,
};

#[test]
fn a_declared_default_alone_cannot_omit_a_required_input() {
    let mut meta = upeg_runtime::toolbox_tool("num.hex_to_decimal")
        .unwrap()
        .clone();
    meta.input_spec.fields[0].constraints.string = Some(upeg_core::StringConstraints {
        default: Some("0xff".into()),
        regex: None,
        placeholder: None,
    });
    let schema = effective_tool_schema(&meta, None);
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("input"))
    );
    assert_eq!(schema["properties"]["input"]["default"], "0xff");
    assert!(
        inspect(&meta, None, Path::new("/"), None)
            .defaults
            .get("input")
            .is_none()
    );
}

#[test]
fn an_invalid_preset_keeps_the_required_input_and_guides_a_fix() {
    let meta = upeg_runtime::toolbox_tool("num.hex_to_decimal").unwrap();
    let preset = ArgsPreset::parse(r#"{"input":42}"#).unwrap();
    let schema = effective_tool_schema(meta, Some(&preset));
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("input"))
    );
    assert!(schema["properties"]["input"].get("default").is_none());
    let preview = inspect(meta, Some(&preset), Path::new("/"), None);
    assert_eq!(
        preview.readiness.status,
        BoardToolReadinessStatus::Unavailable
    );
}

#[test]
fn verifies_external_command_existence_and_in_project_call_directory() {
    const ID: &str = "test.board_preflight";
    const MISSING_COMMAND: &str = "__upeg_missing_executable_for_preflight__";
    let root = tempfile::tempdir().unwrap();
    let member = root.path().join("member");
    std::fs::create_dir(&member).unwrap();
    let mut meta = upeg_runtime::toolbox_tool("num.hex_to_decimal")
        .unwrap()
        .clone();
    meta.id = ID;
    meta.invoker = Invoker::External;
    set_tool_execution_requirements(
        ID,
        Some(ToolExecutionRequirements {
            command: Some(MISSING_COMMAND.into()),
            project_root: Some(root.path().into()),
            ..Default::default()
        }),
    );
    let preview = inspect(&meta, None, &member, None);
    assert_eq!(preview.working_directory.as_deref(), Some(member.as_path()));
    assert_eq!(
        preview.readiness.status,
        BoardToolReadinessStatus::Unavailable
    );
    set_tool_execution_requirements(
        ID,
        Some(ToolExecutionRequirements {
            command: Some(
                std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
            project_root: Some(root.path().into()),
            ..Default::default()
        }),
    );
    let preview = inspect(&meta, None, &member, None);
    assert_eq!(preview.readiness.status, BoardToolReadinessStatus::Ready);
    set_tool_execution_requirements(ID, None);
}
