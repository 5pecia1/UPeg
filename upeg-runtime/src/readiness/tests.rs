#![cfg(not(target_arch = "wasm32"))]

use std::ffi::OsString;
use std::path::Path;

use super::*;
use crate::execution_requirements::{ToolSetupInstall, ToolSetupMetadata};

fn inspect(requirements: ToolExecutionRequirements, directory: &Path) -> ToolReadiness {
    inspect_requirements(
        &requirements,
        &ToolReadinessContext {
            working_directory: directory.to_path_buf(),
            search_path: None,
        },
    )
    .expect("External requirements have readiness")
}

#[test]
fn missing_directory_wins_before_an_executable_lookup() {
    let absent = std::env::temp_dir().join("upeg-readiness-absent-directory");
    let readiness = inspect(
        ToolExecutionRequirements {
            command: Some("__upeg_command_that_does_not_exist__".into()),
            ..Default::default()
        },
        &absent,
    );

    assert_eq!(
        readiness.status,
        ToolReadinessStatus::MissingWorkingDirectory
    );
    assert_eq!(
        readiness.working_directory.as_deref(),
        Some(absent.as_path())
    );
    assert!(readiness.executable.is_none());
}

#[test]
fn credential_path_is_not_resolved_or_exposed() {
    let directory = std::env::current_dir().expect("current directory");
    let readiness = inspect(
        ToolExecutionRequirements {
            command: Some("sentinel-not-executed".into()),
            search_path: CommandSearchPath::Credential,
            ..Default::default()
        },
        &directory,
    );

    assert_eq!(
        readiness.status,
        ToolReadinessStatus::UncheckedCredentialPath
    );
    assert_eq!(readiness.command.as_deref(), Some("sentinel-not-executed"));
    assert!(readiness.executable.is_none());
    let wire = serde_json::to_value(&readiness).expect("serializes");
    assert!(wire.get("search_path").is_none());
    assert!(wire.get("credentials").is_none());
}

#[test]
#[cfg(unix)]
fn caller_path_overrides_declared_path_for_a_bare_command_and_selects_setup_for_host() {
    let directory = std::env::current_dir().expect("current directory");
    let requirements = ToolExecutionRequirements {
        command: Some("sh".into()),
        search_path: CommandSearchPath::Declared(OsString::from("/definitely-not-a-path")),
        setup: Some(ToolSetupMetadata {
            guide_url: Some("https://example.test/install".into()),
            instructions: Some("Install the test command.".into()),
            install: ToolSetupInstall {
                linux: vec!["sudo apt install test-command".into()],
                ..Default::default()
            },
        }),
        ..Default::default()
    };
    let readiness = inspect_requirements(
        &requirements,
        &ToolReadinessContext {
            working_directory: directory,
            search_path: Some(OsString::from("/bin")),
        },
    )
    .expect("readiness");

    assert_eq!(readiness.status, ToolReadinessStatus::Ready);
    assert_eq!(
        readiness
            .setup
            .as_ref()
            .and_then(|setup| setup.guide_url.as_deref()),
        Some("https://example.test/install")
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        readiness
            .setup
            .and_then(|setup| setup.install)
            .map(|install| install.commands),
        Some(vec!["sudo apt install test-command".into()])
    );
}

#[test]
#[cfg(unix)]
fn declared_path_overrides_an_inherited_executable_when_the_caller_has_no_path_override() {
    let directory = std::env::current_dir().expect("current directory");
    let readiness = inspect(
        ToolExecutionRequirements {
            command: Some("sh".into()),
            search_path: CommandSearchPath::Declared(OsString::from("/definitely-not-a-path")),
            ..Default::default()
        },
        &directory,
    );

    assert_eq!(readiness.status, ToolReadinessStatus::MissingExecutable);
}

#[test]
#[cfg(unix)]
fn the_same_requirements_recheck_from_missing_to_ready_without_running_a_command() {
    let directory =
        std::env::temp_dir().join(format!("upeg-readiness-recheck-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    let command = "upeg-readiness-probe";
    let requirements = ToolExecutionRequirements {
        command: Some(command.into()),
        search_path: CommandSearchPath::Declared(directory.clone().into_os_string()),
        ..Default::default()
    };
    let context = ToolReadinessContext {
        working_directory: directory.clone(),
        search_path: None,
    };

    assert_eq!(
        inspect_requirements(&requirements, &context)
            .expect("readiness")
            .status,
        ToolReadinessStatus::MissingExecutable
    );
    std::os::unix::fs::symlink(
        std::env::current_exe().expect("current test executable"),
        directory.join(command),
    )
    .expect("make executable available without invoking it");
    assert_eq!(
        inspect_requirements(&requirements, &context)
            .expect("readiness")
            .status,
        ToolReadinessStatus::Ready
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn windows_policy_bare_command_does_not_implicitly_match_a_batch_file() {
    let directory = std::env::temp_dir().join(format!(
        "upeg-readiness-windows-batch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    std::fs::write(directory.join("tool.cmd"), "@echo off\r\n").expect("write batch file");
    #[cfg(unix)]
    std::fs::set_permissions(
        directory.join("tool.cmd"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("mark fixture executable");

    let found = find_windows_executable_in("tool", Some(directory.as_os_str()), None, None, &[]);

    assert!(
        found.is_none(),
        "Command only supplies an implicit .exe suffix"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn windows_policy_checks_app_directory_after_child_path() {
    let directory = std::env::temp_dir().join(format!(
        "upeg-readiness-windows-app-directory-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    let executable = directory.join("tool.exe");
    std::fs::write(&executable, b"fixture").expect("write executable fixture");
    #[cfg(unix)]
    std::fs::set_permissions(
        &executable,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("mark fixture executable");

    let found = find_windows_executable_in(
        "tool",
        Some(std::ffi::OsStr::new("C:\\definitely-not-present")),
        None,
        Some(directory.clone()),
        &[],
    );

    assert_eq!(found.as_deref(), Some(executable.as_path()));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn windows_policy_explicit_extensionless_path_falls_back_after_exe() {
    let directory = std::env::temp_dir().join(format!(
        "upeg-readiness-windows-explicit-fallback-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    let executable = directory.join("tool");
    std::fs::write(&executable, b"fixture").expect("write executable fixture");
    #[cfg(unix)]
    std::fs::set_permissions(
        &executable,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("mark fixture executable");

    let found = find_windows_executable_in(
        &format!("{}/tool", directory.display()),
        None,
        None,
        None,
        &[],
    );

    assert_eq!(found.as_deref(), Some(executable.as_path()));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn windows_policy_explicit_extension_prefers_appended_exe() {
    let directory = std::env::temp_dir().join(format!(
        "upeg-readiness-windows-explicit-suffix-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    let original = directory.join("tool.cmd");
    let suffixed = directory.join("tool.cmd.exe");
    for executable in [&original, &suffixed] {
        std::fs::write(executable, b"fixture").expect("write executable fixture");
        #[cfg(unix)]
        std::fs::set_permissions(
            executable,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .expect("mark fixture executable");
    }

    let found = find_windows_executable_in(
        &format!("{}/tool.cmd", directory.display()),
        None,
        None,
        None,
        &[],
    );

    assert_eq!(found.as_deref(), Some(suffixed.as_path()));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn windows_policy_bare_dot_filename_does_not_append_exe() {
    let directory = std::env::temp_dir().join(format!(
        "upeg-readiness-windows-dot-filename-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create temporary command directory");
    let executable = directory.join(".tool");
    std::fs::write(&executable, b"fixture").expect("write executable fixture");
    #[cfg(unix)]
    std::fs::set_permissions(
        &executable,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("mark fixture executable");

    let found = find_windows_executable_in(".tool", Some(directory.as_os_str()), None, None, &[]);

    assert_eq!(found.as_deref(), Some(executable.as_path()));
    let _ = std::fs::remove_dir_all(directory);
}
