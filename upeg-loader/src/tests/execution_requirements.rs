use std::ffi::OsString;
use std::path::PathBuf;

use upeg_runtime::execution_requirements::{CommandSearchPath, tool_execution_requirements};

use crate::load_and_register_dir_verbose;

fn tool_dir(label: &str, command: &str, extra: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("upeg-requirements-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create directory");
    std::fs::write(
        root.join("kit.toml"),
        format!(
            r#"
id = "requirements-{label}"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "External"
command = "{command}"
{extra}
"#
        ),
    )
    .expect("write manifest");
    root
}

#[test]
fn registered_execution_requirements_keep_command_and_manifest_relative_cwd() {
    let root = tool_dir("cwd", " git ", r#"cwd = " workspace ""#);
    let outcome = load_and_register_dir_verbose(&root);
    assert!(
        outcome.failed.is_empty(),
        "registration failed: {:?}",
        outcome.failed
    );
    let requirements =
        tool_execution_requirements("requirements-cwd.run").expect("execution requirements");

    assert_eq!(requirements.command.as_deref(), Some("git"));
    assert_eq!(
        requirements.declared_working_directory,
        Some(root.join("workspace"))
    );
    assert_eq!(requirements.project_root, None);
    assert_eq!(requirements.search_path, CommandSearchPath::Inherit);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_execution_requirements_keep_root_caller_cannot_escape() {
    let root = tool_dir("project", "git", "");
    let outcome = crate::load_and_register_file_verbose(&root.join("kit.toml"));
    assert!(
        outcome.failed.is_empty(),
        "registration failed: {:?}",
        outcome.failed
    );
    let requirements =
        tool_execution_requirements("requirements-project.run").expect("execution requirements");

    assert_eq!(requirements.project_root, Some(root.clone()));
    assert_eq!(requirements.declared_working_directory, None);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn command_search_path_keeps_last_declaration_and_credential_precedence() {
    let root = tool_dir(
        "path",
        "git",
        r#"env = [{ name = " PATH ", value = "/old" }, { name = "PATH", value = "/new" }]"#,
    );
    let outcome = load_and_register_dir_verbose(&root);
    assert!(
        outcome.failed.is_empty(),
        "registration failed: {:?}",
        outcome.failed
    );
    assert_eq!(
        tool_execution_requirements("requirements-path.run")
            .expect("execution requirements")
            .search_path,
        CommandSearchPath::Declared(OsString::from("/new"))
    );

    let secret_root = tool_dir(
        "secret",
        "git",
        r#"
env = [{ name = "PATH", value = "/plain" }]
credentials = [{ name = "command-path", target = " PATH " }]
"#,
    );
    let outcome = load_and_register_dir_verbose(&secret_root);
    assert!(
        outcome.failed.is_empty(),
        "registered without reading the credential: {:?}",
        outcome.failed
    );
    assert_eq!(
        tool_execution_requirements("requirements-secret.run")
            .expect("execution requirements")
            .search_path,
        CommandSearchPath::Credential
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(secret_root);
}

#[test]
fn reregistering_with_different_invoker_drops_prior_requirements() {
    let root = tool_dir("replace", "git", "");
    assert!(load_and_register_dir_verbose(&root).failed.is_empty());
    assert!(tool_execution_requirements("requirements-replace.run").is_some());
    std::fs::write(
        root.join("kit.toml"),
        r#"
id = "requirements-replace"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "Http"
url = "https://example.org"
"#,
    )
    .expect("replace manifest");
    let outcome = load_and_register_dir_verbose(&root);
    assert!(
        outcome.failed.is_empty(),
        "registration failed: {:?}",
        outcome.failed
    );

    assert!(tool_execution_requirements("requirements-replace.run").is_none());
    let _ = std::fs::remove_dir_all(root);
}
