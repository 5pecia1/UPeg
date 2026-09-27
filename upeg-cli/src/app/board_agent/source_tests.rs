use std::ffi::OsString;

use super::*;

struct SourceEnvironment(Vec<(&'static str, Option<OsString>)>);

impl SourceEnvironment {
    #[allow(
        unsafe_code,
        reason = "caller holds the pegboard home environment test lock"
    )]
    fn isolated() -> Self {
        let root = upeg_core::paths::config_root().expect("test config root");
        let variables = [
            ("UPEG_TOOLKITS_DIR", root.join("toolkits").into_os_string()),
            (
                "UPEG_MCP_IMPORTS_DIR",
                root.join("mcp-imports").into_os_string(),
            ),
            ("UPEG_WASM_DIR", root.join("wasm").into_os_string()),
            ("UPEG_PROJECT_MANIFEST_PATH", OsString::from("off")),
        ];
        Self(
            variables
                .into_iter()
                .map(|(key, value)| {
                    let previous = std::env::var_os(key);
                    unsafe {
                        std::env::set_var(key, value);
                    }
                    (key, previous)
                })
                .collect(),
        )
    }
}

impl Drop for SourceEnvironment {
    #[allow(
        unsafe_code,
        reason = "caller holds the pegboard home environment test lock"
    )]
    fn drop(&mut self) {
        for (key, previous) in &self.0 {
            match previous {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}

#[test]
fn a_toolkit_changed_before_the_first_preview_requests_a_restart() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-before-preview",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            let directory = config.toolkits_dir.as_ref().expect("toolkits path");
            std::fs::create_dir_all(directory).expect("toolkits directory");
            let path = directory.join("before-preview.toml");
            std::fs::write(&path, "id = 'before_preview'\n").expect("initial declaration");
            upeg_sources::load_local_runtime_sources(&config);
            std::fs::write(&path, "id = 'after_preview'\n").expect("declaration change");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("board"))
                .expect_err("must not build a connection setup from a stale registry");
            assert!(error.to_string().contains("restart"), "{error}");
        },
    );
}

#[test]
fn an_import_declaration_added_after_start_is_detected_at_first_preview() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-added-import",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            upeg_sources::load_local_runtime_sources(&config);
            let directory = config.mcp_import_dir.as_ref().expect("import path");
            std::fs::create_dir_all(directory).expect("import directory");
            std::fs::write(directory.join("added.toml"), "command = 'never-run'\n")
                .expect("add the import");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("board"))
                .expect_err("an added declaration also requires a restart");
            assert!(error.to_string().contains("restart"), "{error}");
        },
    );
}

#[test]
#[allow(
    unsafe_code,
    reason = "caller holds the pegboard home environment test lock"
)]
fn a_project_manifest_absent_at_start_is_detected_once_created() {
    crate::test_support::with_seeded_pegboard_home(
        "board-source-new-project",
        |_| {},
        || {
            let _environment = SourceEnvironment::isolated();
            let root = upeg_core::paths::config_root()
                .expect("config root")
                .join("new-project");
            std::fs::create_dir_all(&root).expect("project root");
            unsafe {
                std::env::set_var("UPEG_PROJECT_MANIFEST_PATH", &root);
            }
            let config = upeg_sources::RuntimeSourceConfig::from_env();
            assert!(config.project_manifest.is_none());
            upeg_sources::load_local_runtime_sources(&config);
            std::fs::create_dir_all(root.join(".upeg")).expect("create the project marker");

            let error = board_connection_preview(&BoardKey::parse("dev").expect("board"))
                .expect_err("must restart before honouring the new manifest");
            assert!(matches!(
                error,
                BoardAgentError::Sources(upeg_sources::LoadedSourcesError::Changed { .. })
            ));
        },
    );
}
