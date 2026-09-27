use super::*;
use upeg_runtime::ToolMetaRuntimeExt;

fn root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("upeg-context-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".upeg/toolkits")).expect("project directories");
    root.canonicalize().expect("canonical project root")
}

fn toolkit(path: &Path, id: &str, tool: &str, cwd: Option<&str>) {
    let cwd = cwd.map_or_else(String::new, |cwd| format!("cwd = '{cwd}'\n"));
    std::fs::write(
        path,
        format!(
            "id = '{id}'\n[[tools]]\nid = '{tool}'\npegboard_units = 'U1'\ninvoker = 'External'\ncommand = 'echo'\n{cwd}"
        ),
    )
    .expect("toolkit file");
}

#[test]
fn empty_marker_is_a_project_without_a_config_file() {
    let _guard = project_test_guard();
    let root = root("empty-marker");
    let definition = validate_project_root(&root).expect("empty marker is valid");
    assert_eq!(definition.root, root);
    assert!(definition.toolkits.is_empty());
    assert!(definition.conflicts.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_loads_multiple_toolkits_with_root_relative_cwd_and_replaces_scope() {
    let _guard = project_test_guard();
    let first = root("multi-first");
    let second = root("multi-second");
    toolkit(
        &first.join(".upeg/toolkits/one.toml"),
        "project-first-kit",
        "one",
        Some("nested"),
    );
    toolkit(
        &first.join(".upeg/toolkits/two.toml"),
        "project-second-kit",
        "two",
        None,
    );
    toolkit(
        &second.join(".upeg/toolkits/other.toml"),
        "project-third-kit",
        "three",
        None,
    );
    std::fs::write(
        first.join(".upeg/project.toml"),
        "schema_version = 1\nname = 'First'\n[[boards]]\nid = 'project-work'\n",
    )
    .expect("config");
    let first_result = activate_project(&first).expect("activate first");
    assert!(first_result.failed.is_empty(), "{:?}", first_result.failed);
    assert_eq!(first_result.loaded_tool_ids.len(), 2);
    assert_eq!(current_project_root(), Some(first.clone()));
    assert_eq!(
        upeg_runtime::project_scope::active_project_root(),
        Some(first.clone())
    );
    assert_eq!(
        upeg_runtime::execution_requirements::tool_execution_requirements("project-first-kit.one")
            .expect("requirements")
            .declared_working_directory,
        Some(first.join("nested"))
    );
    assert!(upeg_runtime::pegboard_project::project_board_scope().is_some());

    activate_project(&second).expect("switch second");
    assert!(upeg_runtime::toolbox_tool("project-first-kit.one").is_none());
    assert!(upeg_runtime::toolbox_tool("project-third-kit.three").is_some());
    assert!(
        upeg_runtime::pegboard_project::project_board_scope()
            .expect("second board scope")
            .boards()
            .is_empty()
    );
    close_project().expect("close");
    assert!(upeg_runtime::toolbox_tool("project-third-kit.three").is_none());
    assert_eq!(current_project_root(), None);
    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn duplicate_id_is_blocked_until_an_explicit_choice_restores_global_on_close() {
    let _guard = project_test_guard();
    let global = root("conflict-global");
    let project = root("conflict-project");
    let global_file = global.join(".upeg/toolkits/global.toml");
    toolkit(&global_file, "project-conflict-kit", "run", None);
    let global_outcome = upeg_loader::load_and_register_dir_verbose(&global.join(".upeg/toolkits"));
    assert!(global_outcome.failed.is_empty());
    let project_file = project.join(".upeg/toolkits/local.toml");
    toolkit(&project_file, "project-conflict-kit", "run", None);

    let activation = activate_project(&project).expect("activate conflict");
    assert_eq!(activation.conflicts.len(), 1);
    assert_eq!(activation.conflicts[0].choice, None);
    assert!(upeg_runtime::toolbox_tool("project-conflict-kit.run").is_none());
    let blocked =
        upeg_runtime::try_runtime_dispatch("project-conflict-kit.run", &serde_json::json!({}))
            .expect("typed failure");
    assert!(matches!(blocked, upeg_core::ToolResult::Failure(_)));

    let original_config = std::fs::read(project.join(".upeg/project.toml")).ok();
    let active_call = upeg_runtime::project_scope::begin_call().expect("running call");
    assert!(matches!(
        set_project_tool_choice(
            &project,
            "project-conflict-kit.run",
            ProjectToolChoice::Project
        ),
        Err(ProjectError::Transition(_))
    ));
    assert_eq!(
        std::fs::read(project.join(".upeg/project.toml")).ok(),
        original_config,
        "a busy runtime must not persist a choice it could not activate"
    );
    drop(active_call);

    let selected = set_project_tool_choice(
        &project,
        "project-conflict-kit.run",
        ProjectToolChoice::Project,
    )
    .expect("choose project");
    assert_eq!(selected.loaded_tool_ids, ["project-conflict-kit.run"]);
    assert!(upeg_runtime::tool_provenance("project-conflict-kit.run").is_project_manifest());
    close_project().expect("restore global");
    assert!(upeg_runtime::toolbox_tool("project-conflict-kit.run").is_some());
    assert!(!upeg_runtime::tool_provenance("project-conflict-kit.run").is_project_manifest());
    let _ = std::fs::remove_dir_all(global);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn failed_activation_restores_the_previous_catalog_and_board_scope() {
    let _guard = project_test_guard();
    let first = root("rollback-first");
    let second = root("rollback-second");
    toolkit(
        &first.join(".upeg/toolkits/first.toml"),
        "project-rollback-first",
        "run",
        None,
    );
    toolkit(
        &second.join(".upeg/toolkits/second.toml"),
        "project-rollback-second",
        "run",
        None,
    );
    std::fs::write(
        first.join(".upeg/project.toml"),
        "schema_version = 1\n[[boards]]\nid = 'rollback-work'\n",
    )
    .expect("first board");
    activate_project(&first).expect("activate first");
    let previous_scope =
        upeg_runtime::pegboard_project::project_board_scope().expect("first scope");
    let definition = validate_project_root(&second).expect("valid second project before mutation");
    std::fs::remove_file(second.join(".upeg/toolkits/second.toml"))
        .expect("simulate source deletion after validation");
    let transition = upeg_runtime::project_scope::begin_project_transition().expect("switch guard");
    assert!(matches!(
        activate_project_under_transition(definition),
        Err(ProjectError::Activation(_))
    ));
    drop(transition);
    assert_eq!(current_project_root(), Some(first.clone()));
    assert!(upeg_runtime::toolbox_tool("project-rollback-first.run").is_some());
    assert!(upeg_runtime::has_runtime_dispatcher(
        "project-rollback-first.run"
    ));
    assert!(upeg_runtime::toolbox_tool("project-rollback-second.run").is_none());
    assert_eq!(
        upeg_runtime::pegboard_project::project_board_scope(),
        Some(previous_scope)
    );
    close_project().expect("close restored project");
    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn closing_explicit_project_keeps_global_context_even_when_cwd_has_a_marker() {
    let _guard = project_test_guard();
    let root = root("explicit-close");
    activate_project(&root).expect("activate");
    close_project().expect("close");
    assert_eq!(current_project_root(), None);
    assert_eq!(upeg_runtime::project_scope::active_project_root(), None);
    assert!(
        detect_project_manifest_lookup().is_none(),
        "closed GUI context must not auto-reopen from cwd"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shared_toolkit_id_keeps_each_tools_own_source_tags() {
    let _guard = project_test_guard();
    let global = root("shared-kit-global");
    let project = root("shared-kit-project");
    std::fs::write(global.join(".upeg/toolkits/global.toml"),
        "id = 'project-shared-kit'\ntags = ['global-only']\n[[tools]]\nid = 'keep'\npegboard_units = 'U1'\ninvoker = 'External'\ncommand = 'echo'\n").expect("global declaration");
    std::fs::write(project.join(".upeg/toolkits/project.toml"),
        "id = 'project-shared-kit'\ntags = ['project-only']\n[[tools]]\nid = 'add'\npegboard_units = 'U1'\ninvoker = 'External'\ncommand = 'echo'\n").expect("project declaration");
    let global_outcome = upeg_loader::load_and_register_dir_verbose(&global.join(".upeg/toolkits"));
    assert!(global_outcome.failed.is_empty());
    activate_project(&project).expect("activate local addition");
    let global_tags = upeg_runtime::toolbox_tool("project-shared-kit.keep")
        .expect("global tool")
        .tag_labels();
    let project_tags = upeg_runtime::toolbox_tool("project-shared-kit.add")
        .expect("project tool")
        .tag_labels();
    assert!(global_tags.contains(&"global-only".to_string()));
    assert!(!global_tags.contains(&"project-only".to_string()));
    assert!(project_tags.contains(&"project-only".to_string()));
    assert!(!project_tags.contains(&"global-only".to_string()));
    close_project().expect("close");
    let _ = std::fs::remove_dir_all(global);
    let _ = std::fs::remove_dir_all(project);
}
