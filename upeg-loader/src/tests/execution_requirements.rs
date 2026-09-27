use std::ffi::OsString;
use std::path::PathBuf;

use upeg_runtime::execution_requirements::{CommandSearchPath, tool_execution_requirements};
use upeg_runtime::readiness::{ToolReadinessContext, inspect_tool_readiness};

use crate::load_and_register_dir_verbose;
use crate::parse_toolkit_full;

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
    let ids = std::collections::HashSet::from(["requirements-project.run".to_string()]);
    let outcome = crate::load_project_toolkit_file_verbose(&root.join("kit.toml"), &root, &ids);
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

#[test]
fn external_setup_is_registered_as_non_secret_os_selected_guidance() {
    let root = tool_dir(
        "setup",
        "__upeg_missing_setup_command__",
        r#"
[tools.setup]
guide_url = "https://example.test/install"
instructions = "Install the command and reconnect."
[tools.setup.install]
linux = ["sudo apt install example"]
macos = ["brew install example"]
windows = ["winget install Example.Command"]
"#,
    );
    let outcome = load_and_register_dir_verbose(&root);
    assert!(
        outcome.failed.is_empty(),
        "registration failed: {:?}",
        outcome.failed
    );

    let readiness = inspect_tool_readiness(
        "requirements-setup.run",
        &ToolReadinessContext {
            working_directory: root.clone(),
            search_path: Some(OsString::from("/definitely-not-a-path")),
        },
    )
    .expect("External readiness");

    assert_eq!(
        readiness.status,
        upeg_runtime::readiness::ToolReadinessStatus::MissingExecutable
    );
    let expected_command = match readiness.platform {
        upeg_runtime::readiness::ToolPlatform::Linux => "sudo apt install example",
        upeg_runtime::readiness::ToolPlatform::Macos => "brew install example",
        upeg_runtime::readiness::ToolPlatform::Windows => "winget install Example.Command",
    };
    assert_eq!(
        readiness
            .setup
            .clone()
            .and_then(|setup| setup.install)
            .map(|install| install.commands),
        Some(vec![expected_command.into()])
    );
    let wire = serde_json::to_value(readiness).expect("serializes");
    assert!(wire.get("search_path").is_none());
    assert!(wire.get("credentials").is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn setup_rejects_non_external_and_unsafe_or_blank_guidance() {
    let non_external = r#"
id = "setup-invalid"
[[tools]]
id = "request"
pegboard_units = "U1"
invoker = "Http"
url = "https://example.test"
[tools.setup]
instructions = "Install it"
"#;
    assert!(matches!(
        parse_toolkit_full(non_external),
        Err(crate::LoadError::InvokerFieldConflict { field: "setup", .. })
    ));

    for (field, value) in [
        ("guide_url", "file:///tmp/install"),
        ("guide_url", "https://?install"),
        ("guide_url", "https://user:password@example.test/install"),
        ("guide_url", "https://example.test/has space"),
        ("instructions", "   "),
    ] {
        let manifest = format!(
            r#"
id = "setup-invalid"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "External"
command = "git"
[tools.setup]
{field} = "{value}"
"#
        );
        assert!(
            parse_toolkit_full(&manifest).is_err(),
            "must reject {field}={value:?}"
        );
    }
}

#[test]
fn setup_accepts_http_urls_with_ports_queries_fragments_and_ipv6_hosts() {
    for url in [
        "https://example.test:8443/install?channel=stable#linux",
        "http://[::1]:8080/install?source=local#guide",
    ] {
        let manifest = format!(
            r#"
id = "setup-valid"
[[tools]]
id = "run"
pegboard_units = "U1"
invoker = "External"
command = "git"
[tools.setup]
guide_url = "{url}"
"#
        );
        assert!(parse_toolkit_full(&manifest).is_ok(), "must accept {url}");
    }
}
