use std::path::{Path, PathBuf};

use crate::{McpImportRegistrations, mcp_import};

/// Declaration-file extensions each runtime source directory holds.
/// Named constants so [`survey_runtime_source_dirs`] can never drift
/// from the loaders that actually read those files.
pub(crate) const TOOLKIT_FILE_EXTENSION: &str = "toml";
pub(crate) const MCP_IMPORT_FILE_EXTENSION: &str = mcp_import::DECLARATION_EXTENSION;
pub(crate) const WASM_FILE_EXTENSION: &str = "wasm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryStatus {
    Unconfigured,
    Missing(PathBuf),
    Loaded { path: PathBuf, count: usize },
}

impl DirectoryStatus {
    pub const fn count(&self) -> usize {
        match self {
            Self::Loaded { count, .. } => *count,
            Self::Unconfigured | Self::Missing(_) => 0,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Missing(path) | Self::Loaded { path, .. } => Some(path.as_path()),
            Self::Unconfigured => None,
        }
    }
}

#[derive(Debug)]
pub struct ProjectManifestLoad {
    pub path: PathBuf,
    /// Detected vs `UPEG_PROJECT_MANIFEST_PATH`-overridden — lets callers
    /// (e.g. the CLI's B-4 consent notice, `upeg-cli/src/main.rs`) fire a
    /// notice only for a manifest detection actually found on its own.
    pub origin: crate::project::ProjectManifestOrigin,
    pub outcome: upeg_loader::LoadOutcome,
}

#[derive(Debug)]
pub struct RuntimeSourceReport {
    pub toolkits: DirectoryStatus,
    pub toolkits_outcome: upeg_loader::LoadOutcome,
    pub project_manifest: Option<ProjectManifestLoad>,
    pub wasm: DirectoryStatus,
    pub wasm_failures: Vec<(PathBuf, String)>,
    /// Directory survey only — [`load_local_runtime_sources`] never
    /// spawns an upstream MCP server. Eager import loading is the job of
    /// [`load_mcp_imports_for_host`], called only by long-lived server
    /// processes.
    pub mcp_imports: DirectoryStatus,
}

/// Outcome of the eager MCP-import registration performed by long-lived
/// server processes (`upeg host start`, the desktop-embedded host, and
/// the in-process `upeg mcp` stdio server).
///
/// Partial success is normal: `servers` keeps one entry per upstream
/// declaration so a caller can report exactly which server failed while
/// the rest stay registered.
#[derive(Debug)]
pub struct McpImportLoad {
    pub directory: DirectoryStatus,
    pub servers: McpImportRegistrations,
}

impl McpImportLoad {
    /// Upstream servers whose import succeeded.
    pub fn loaded_server_count(&self) -> usize {
        self.servers
            .iter()
            .filter(|(_, result)| result.is_ok())
            .count()
    }

    /// Upstream servers whose import failed outright.
    pub fn failed_server_count(&self) -> usize {
        self.servers.len() - self.loaded_server_count()
    }

    /// Tools registered into the Toolbox across every successful server.
    pub fn tool_count(&self) -> usize {
        self.outcomes()
            .map(|outcome| outcome.registered_ids().len())
            .sum()
    }

    /// Tools dropped tool-by-tool (unconvertible schema, empty name).
    pub fn skipped_tool_count(&self) -> usize {
        self.outcomes().map(|outcome| outcome.skipped.len()).sum()
    }

    fn outcomes(&self) -> impl Iterator<Item = &mcp_import::ImportOutcome> {
        self.servers
            .iter()
            .filter_map(|(_, result)| result.as_ref().ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceConfig {
    pub toolkits_dir: Option<PathBuf>,
    /// Path *and* provenance (detected vs `UPEG_PROJECT_MANIFEST_PATH`
    /// override) — same field, richer type, so [`load_local_runtime_sources`]
    /// can stamp [`ProjectManifestLoad::origin`] without re-reading env.
    pub project_manifest: Option<crate::project::ProjectManifestLookup>,
    pub wasm_dir: Option<PathBuf>,
    pub mcp_import_dir: Option<PathBuf>,
}

impl RuntimeSourceConfig {
    pub fn from_env() -> Self {
        Self {
            toolkits_dir: upeg_core::paths::toolkits_dir(),
            project_manifest: crate::project::detect_project_manifest_lookup(),
            wasm_dir: upeg_core::paths::wasm_dir(),
            mcp_import_dir: upeg_core::paths::mcp_import_dir(),
        }
    }
}

impl Default for RuntimeSourceConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Register every *local* runtime source: declarative Toolkits, the
/// project manifest, and local WASM Toolkits. Never spawns a
/// subprocess, so one-shot CLI commands, the TUI, and the desktop boot
/// path can all call it unconditionally.
pub fn load_local_runtime_sources(config: &RuntimeSourceConfig) -> RuntimeSourceReport {
    crate::source_freshness::record_loaded_sources(config);
    upeg_tools::register_all();

    let (toolkits, toolkits_outcome) = load_toolkits_dir(config.toolkits_dir.as_deref());
    let project_manifest = config.project_manifest.as_ref().map(load_project_manifest);
    let (wasm, wasm_failures) = load_wasm_dir(config.wasm_dir.as_deref());

    RuntimeSourceReport {
        toolkits: match toolkits {
            DirectoryStatus::Loaded { path, .. } => DirectoryStatus::Loaded {
                path,
                count: toolkits_outcome.loaded.len(),
            },
            other => other,
        },
        toolkits_outcome,
        project_manifest,
        wasm,
        wasm_failures,
        mcp_imports: survey_dir(config.mcp_import_dir.as_deref(), MCP_IMPORT_FILE_EXTENSION),
    }
}

/// Load one resolved project manifest and stamp every tool it
/// registered with [`upeg_runtime::ToolProvenance::ProjectManifest`].
///
/// The provenance stamp is what lets the CLI keep a project tool
/// in-process instead of auto-attaching it to a host that resolved a
/// different `upeg.toml` (D-1) — see
/// `upeg-cli/src/domain/execution/context.rs::requires_local_dispatch`.
/// Doing it here, at the single load site, is why no surface can forget.
fn load_project_manifest(lookup: &crate::project::ProjectManifestLookup) -> ProjectManifestLoad {
    let outcome = upeg_loader::load_and_register_file_verbose(&lookup.path);
    let path = lookup.path.display().to_string();
    for id in &outcome.loaded {
        upeg_runtime::register_tool_provenance(
            id,
            upeg_runtime::ToolProvenance::ProjectManifest { path: path.clone() },
        );
    }
    ProjectManifestLoad {
        path: lookup.path.clone(),
        origin: lookup.origin,
        outcome,
    }
}

fn load_toolkits_dir(dir: Option<&Path>) -> (DirectoryStatus, upeg_loader::LoadOutcome) {
    let Some(dir) = dir else {
        return (
            DirectoryStatus::Unconfigured,
            upeg_loader::LoadOutcome::default(),
        );
    };
    if !dir.is_dir() {
        return (
            DirectoryStatus::Missing(dir.to_path_buf()),
            upeg_loader::LoadOutcome::default(),
        );
    }
    let outcome = upeg_loader::load_and_register_dir_verbose(dir);
    (
        DirectoryStatus::Loaded {
            path: dir.to_path_buf(),
            count: outcome.loaded.len(),
        },
        outcome,
    )
}

#[cfg(feature = "wasm-plugin")]
fn load_wasm_dir(dir: Option<&Path>) -> (DirectoryStatus, Vec<(PathBuf, String)>) {
    let Some(dir) = dir else {
        return (DirectoryStatus::Unconfigured, Vec::new());
    };
    if !dir.is_dir() {
        return (DirectoryStatus::Missing(dir.to_path_buf()), Vec::new());
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (
            DirectoryStatus::Loaded {
                path: dir.to_path_buf(),
                count: 0,
            },
            Vec::new(),
        );
    };
    let mut loaded_tools = 0_usize;
    let mut failures = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "wasm") {
            continue;
        }
        match upeg_wasm::load_and_register(&path) {
            Ok(ids) => loaded_tools += ids.len(),
            Err(err) => failures.push((path, err.to_string())),
        }
    }
    (
        DirectoryStatus::Loaded {
            path: dir.to_path_buf(),
            count: loaded_tools,
        },
        failures,
    )
}

#[cfg(not(feature = "wasm-plugin"))]
fn load_wasm_dir(dir: Option<&Path>) -> (DirectoryStatus, Vec<(PathBuf, String)>) {
    let status = match dir {
        None => DirectoryStatus::Unconfigured,
        Some(dir) if dir.is_dir() => DirectoryStatus::Loaded {
            path: dir.to_path_buf(),
            count: 0,
        },
        Some(dir) => DirectoryStatus::Missing(dir.to_path_buf()),
    };
    (status, Vec::new())
}

fn load_mcp_import_dir(dir: Option<&Path>) -> (DirectoryStatus, McpImportRegistrations) {
    let Some(dir) = dir else {
        return (DirectoryStatus::Unconfigured, Vec::new());
    };
    if !dir.is_dir() {
        return (DirectoryStatus::Missing(dir.to_path_buf()), Vec::new());
    }
    let servers = mcp_import::register_dir(dir);
    let count = servers
        .iter()
        .filter_map(|(_, result)| result.as_ref().ok())
        .map(|outcome| outcome.registered_ids().len())
        .sum();
    (
        DirectoryStatus::Loaded {
            path: dir.to_path_buf(),
            count,
        },
        servers,
    )
}

/// Eagerly import every upstream MCP server declared under the
/// configured `mcp-imports` directory and register their tools.
///
/// **Only long-lived server processes call this**: `upeg host start`
/// (foreground and `--daemon`), the desktop-embedded host, and the
/// in-process `upeg mcp` stdio server. One-shot CLI commands and the
/// TUI deliberately skip it so they never spawn upstream subprocesses;
/// they reach imported tools through an attached host instead. There is
/// no reload — restarting the host process re-imports
/// (docs/architecture/mcp.md).
pub fn load_mcp_imports_for_host(config: &RuntimeSourceConfig) -> McpImportLoad {
    let (directory, servers) = load_mcp_import_dir(config.mcp_import_dir.as_deref());
    McpImportLoad { directory, servers }
}

/// Does any declared upstream opt into re-exposing its tools on upeg's
/// own `mcp` Surface (`reexport = true`)?
///
/// Reads the declaration files only — never spawns an upstream, so it
/// is safe on a latency-sensitive startup path. The in-process `upeg
/// mcp` lane calls this before deciding whether an eager
/// [`load_mcp_imports_for_host`] can pay for itself: with every
/// declaration on the default `Blocked` policy, the imported tools
/// register on `ALL_SURFACES_EXCEPT_MCP`, so that surface would spawn
/// (and wait on) every upstream to list none of their tools.
pub fn mcp_import_reexport_policy(config: &RuntimeSourceConfig) -> mcp_import::McpReexport {
    match config.mcp_import_dir.as_deref() {
        Some(dir) if dir.is_dir() => mcp_import::scan_dir_reexport_policy(dir),
        _ => mcp_import::McpReexport::Blocked,
    }
}

/// Read-only survey of every runtime source directory: which paths are
/// configured, whether they exist, and how many declaration files each
/// holds. Costs one `read_dir` per directory — no registration, no
/// subprocess. Backs `upeg host status`.
pub fn survey_runtime_source_dirs(config: &RuntimeSourceConfig) -> RuntimeSourceDirs {
    RuntimeSourceDirs {
        toolkits: survey_dir(config.toolkits_dir.as_deref(), TOOLKIT_FILE_EXTENSION),
        wasm: survey_dir(config.wasm_dir.as_deref(), WASM_FILE_EXTENSION),
        mcp_imports: survey_dir(config.mcp_import_dir.as_deref(), MCP_IMPORT_FILE_EXTENSION),
    }
}

/// Where declaration files live for each runtime source directory, with
/// the file count [`survey_runtime_source_dirs`] measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceDirs {
    pub toolkits: DirectoryStatus,
    pub wasm: DirectoryStatus,
    pub mcp_imports: DirectoryStatus,
}

/// Count files with `extension` in `dir` without loading anything.
fn survey_dir(dir: Option<&Path>, extension: &str) -> DirectoryStatus {
    let Some(dir) = dir else {
        return DirectoryStatus::Unconfigured;
    };
    if !dir.is_dir() {
        return DirectoryStatus::Missing(dir.to_path_buf());
    }
    let count = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
                })
                .count()
        })
        .unwrap_or(0);
    DirectoryStatus::Loaded {
        path: dir.to_path_buf(),
        count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_source_load_treats_missing_directory_as_empty() {
        let root =
            std::env::temp_dir().join(format!("upeg-sources-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config = RuntimeSourceConfig {
            toolkits_dir: Some(root.join("toolkits")),
            project_manifest: None,
            wasm_dir: Some(root.join("wasm")),
            mcp_import_dir: Some(root.join("mcp_imports")),
        };

        let report = load_local_runtime_sources(&config);

        assert!(matches!(report.toolkits, DirectoryStatus::Missing(_)));
        assert!(matches!(report.wasm, DirectoryStatus::Missing(_)));
        assert!(matches!(report.mcp_imports, DirectoryStatus::Missing(_)));
    }

    #[test]
    fn local_runtime_source_load_does_not_import_even_when_mcp_imports_dir_exists() {
        let root = std::env::temp_dir().join(format!("upeg-sources-local-{}", std::process::id()));
        let mcp_imports = root.join("mcp_imports");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&mcp_imports).unwrap();
        std::fs::write(
            mcp_imports.join("silent.toml"),
            r#"command = "sleep"
args = ["60"]"#,
        )
        .unwrap();
        let config = RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: Some(mcp_imports),
        };

        let report = load_local_runtime_sources(&config);

        // Directory survey only: counts the one declaration file but
        // spawns no server.
        assert!(matches!(
            report.mcp_imports,
            DirectoryStatus::Loaded { count: 1, .. }
        ));
        assert!(
            upeg_runtime::toolbox_tool("silent.anything").is_none(),
            "local startup must not spawn or register MCP servers"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn directory_survey_counts_only_declaration_files_and_registers_nothing() {
        let root = std::env::temp_dir().join(format!("upeg-sources-survey-{}", std::process::id()));
        let toolkits = root.join("toolkits");
        let mcp_imports = root.join("mcp_imports");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&toolkits).unwrap();
        std::fs::create_dir_all(&mcp_imports).unwrap();
        std::fs::write(toolkits.join("a.toml"), "").unwrap();
        std::fs::write(toolkits.join("b.toml"), "").unwrap();
        std::fs::write(toolkits.join("notes.md"), "").unwrap();
        std::fs::write(mcp_imports.join("upstream.toml"), "").unwrap();
        let config = RuntimeSourceConfig {
            toolkits_dir: Some(toolkits),
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: Some(mcp_imports),
        };

        let dirs = survey_runtime_source_dirs(&config);

        assert_eq!(dirs.toolkits.count(), 2, "counts only toml");
        assert_eq!(dirs.mcp_imports.count(), 1);
        assert!(matches!(dirs.wasm, DirectoryStatus::Unconfigured));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn host_mcp_import_load_returns_empty_for_unconfigured_directory() {
        let config = RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: None,
        };

        let load = load_mcp_imports_for_host(&config);

        assert!(matches!(load.directory, DirectoryStatus::Unconfigured));
        assert_eq!(load.loaded_server_count(), 0);
        assert_eq!(load.failed_server_count(), 0);
        assert_eq!(load.tool_count(), 0);
    }

    /// Verifies that the single import entry point used by long-lived
    /// server processes (`upeg host start`, desktop embed, in-process
    /// `upeg mcp`) actually spawns an upstream server and registers its
    /// tools in the Toolbox. With the supervisor gone, this path is the
    /// only production loader.
    #[cfg(unix)]
    #[test]
    fn host_mcp_import_load_registers_upstream_tools_in_toolbox() {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!("upeg_sources_host_import_{}", std::process::id());
        let root = std::env::temp_dir().join(&unique);
        let mcp = root.join("mcp-imports");
        let script = root.join("server.sh");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&mcp).unwrap();
        std::fs::write(
            &script,
            r#"#!/bin/sh
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"fixture","inputSchema":{"type":"object","properties":{}}}]}}'
while IFS= read -r line; do
  sleep 60
done
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        std::fs::write(
            mcp.join(format!("{unique}.toml")),
            format!("command = {:?}\n", script.to_str().unwrap()),
        )
        .unwrap();
        let config = RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: Some(mcp),
        };

        let load = load_mcp_imports_for_host(&config);

        assert_eq!(
            load.loaded_server_count(),
            1,
            "exactly one server must be imported"
        );
        assert_eq!(load.failed_server_count(), 0);
        assert_eq!(load.tool_count(), 1);
        let tool_id = format!("{unique}.echo");
        assert!(
            upeg_runtime::toolbox_tool(&tool_id).is_some(),
            "host import must register {tool_id}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The declaration-directory prescan does not spawn upstream; it
    /// only checks the `reexport` opt-in. The in-process `upeg mcp`
    /// reads this to skip eager import loading entirely.
    #[test]
    fn reexport_prescan_reports_opt_in_only_when_opted_in_declaration_exists() {
        let root = std::env::temp_dir().join(format!("upeg-reexport-scan-{}", std::process::id()));
        let blocked_dir = root.join("blocked");
        let opted_dir = root.join("opted");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&blocked_dir).unwrap();
        std::fs::create_dir_all(&opted_dir).unwrap();
        // A command that must never be spawned — proof the scan only
        // reads files.
        std::fs::write(
            blocked_dir.join("plain.toml"),
            "command = \"definitely_not_a_real_program\"\n",
        )
        .unwrap();
        std::fs::write(
            opted_dir.join("plain.toml"),
            "command = \"definitely_not_a_real_program\"\n",
        )
        .unwrap();
        std::fs::write(
            opted_dir.join("shared.toml"),
            "command = \"definitely_not_a_real_program\"\nreexport = true\n",
        )
        .unwrap();

        let blocked = mcp_import_reexport_policy(&RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: Some(blocked_dir),
        });
        let opted = mcp_import_reexport_policy(&RuntimeSourceConfig {
            toolkits_dir: None,
            project_manifest: None,
            wasm_dir: None,
            mcp_import_dir: Some(opted_dir),
        });

        assert_eq!(blocked, mcp_import::McpReexport::Blocked);
        assert_eq!(opted, mcp_import::McpReexport::OptedIn);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reexport_prescan_treats_unconfigured_and_missing_directories_as_blocked() {
        let missing =
            std::env::temp_dir().join(format!("upeg-reexport-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);

        for dir in [None, Some(missing)] {
            let policy = mcp_import_reexport_policy(&RuntimeSourceConfig {
                toolkits_dir: None,
                project_manifest: None,
                wasm_dir: None,
                mcp_import_dir: dir,
            });
            assert_eq!(policy, mcp_import::McpReexport::Blocked);
        }
    }

    #[test]
    fn path_computation_delegates_to_core_paths() {
        // This used to test a config_root delegation helper, but once
        // memos moved to the SQLite store this crate's config-root
        // helper disappeared. Remaining invariant: all directory path
        // policy delegates to upeg_core::paths.
        let source = include_str!("sources.rs");
        for delegated_fn in ["toolkits_dir()", "wasm_dir()", "mcp_import_dir()"] {
            let delegated = ["upeg_core", "paths", delegated_fn].join("::");
            assert!(
                source.contains(&delegated),
                "Runtime source config must not keep a separate path policy ({delegated_fn})"
            );
        }
    }

    #[test]
    fn runtime_source_load_loads_toolkits_and_project_manifest() {
        let root = std::env::temp_dir().join(format!("upeg-sources-load-{}", std::process::id()));
        let toolkits = root.join("toolkits");
        let project_manifest = root.join("upeg.toml");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&toolkits).unwrap();
        std::fs::write(
            toolkits.join("runtime.toml"),
            r#"id = "sources_runtime"

[[tools]]
id = "echo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["runtime"]"#,
        )
        .unwrap();
        std::fs::write(
            &project_manifest,
            r#"id = "sources_project"

[[tools]]
id = "echo"
pegboard_units = "U1"
invoker = "External"
command = "printf"
args_template = ["project"]"#,
        )
        .unwrap();
        let config = RuntimeSourceConfig {
            toolkits_dir: Some(toolkits),
            project_manifest: Some(crate::project::ProjectManifestLookup {
                path: project_manifest,
                origin: crate::project::ProjectManifestOrigin::Detected,
            }),
            wasm_dir: None,
            mcp_import_dir: None,
        };

        let report = load_local_runtime_sources(&config);

        assert_eq!(report.toolkits.count(), 1);
        assert!(report.toolkits_outcome.failed.is_empty());
        let project = report.project_manifest.expect("project manifest");
        assert_eq!(
            project.origin,
            crate::project::ProjectManifestOrigin::Detected
        );
        assert_eq!(project.outcome.loaded.len(), 1);
        assert!(project.outcome.failed.is_empty());
        // D-1: every tool the project manifest registered is stamped
        // with its origin, so the CLI can keep it in-process instead of
        // auto-attaching it to a host that resolved a different
        // `upeg.toml`.
        for id in &project.outcome.loaded {
            assert_eq!(
                upeg_runtime::tool_provenance(id),
                upeg_runtime::ToolProvenance::ProjectManifest {
                    path: project.path.display().to_string(),
                },
                "project manifest tool `{id}` must carry provenance"
            );
        }
        // Toolkit-dir tools stay `Local` — the stamp is manifest-only.
        for id in &report.toolkits_outcome.loaded {
            assert_eq!(
                upeg_runtime::tool_provenance(id),
                upeg_runtime::ToolProvenance::Local,
                "toolkits-directory tool `{id}` is not from the project manifest"
            );
        }
        assert!(matches!(report.wasm, DirectoryStatus::Unconfigured));
        assert!(matches!(report.mcp_imports, DirectoryStatus::Unconfigured));
        let _ = std::fs::remove_dir_all(&root);
    }
}
