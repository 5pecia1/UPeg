use super::*;
use serde_json::json;
use upeg_runtime::execution_requirements::{
    ToolExecutionRequirements, set_tool_execution_requirements,
};

#[test]
fn 선언된_기본값만으로_필수_입력을_생략할_수는_없다() {
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
fn 잘못된_프리셋은_필수_입력을_유지하고_수정을_안내한다() {
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
fn 외부_명령의_존재와_프로젝트_내_호출_디렉터리를_확인한다() {
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
