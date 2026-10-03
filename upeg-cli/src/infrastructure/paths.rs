//! Cross-platform config-root resolution + canonical paths and env-var
//! names for every per-user runtime artifact. PRD §5.3 / §11.
//!
//! Unix:    `$HOME/.upeg/`
//! Windows: `%APPDATA%\upeg\`
//!
//! All on-disk runtime artifacts (credentials.json, server.json,
//! upeg.db, upeg-http.log, toolkits/, wasm/, mcp/) hang off this
//! root so callers never reinvent path logic.
//!
//! The lowest owner of config-root resolution is
//! [`upeg_core::paths`] — every surface (including the wasm-capable
//! `upeg-pegboard-ui`) reads it from there. This module adds only the
//! host/application-scoped artifacts on top (`server.json`,
//! `credentials.json`, the daemon log), so a UI crate never has to
//! depend on `upeg-cli` just to find `$UPEG_HOME`.
//!
//! Individual files can be overridden by the env vars listed in [`env`]
//! — that override hierarchy is the existing contract from
//! `adapters::credentials` etc.

use std::path::PathBuf;

/// Canonical names of every environment variable upeg reads.
///
/// Centralized here so a rename touches exactly one site and consumers
/// import the constant instead of re-typing the string. Names that are
/// scoped to a single subsystem (e.g. `UPEG_HTTP_DETACHED` for the
/// daemon supervisor) stay with that subsystem.
pub mod env {
    /// Override the entire config root. When set, replaces both the
    /// platform default (`$HOME/.upeg` / `%APPDATA%\upeg`) and any
    /// nested subpath fall-backs.
    pub use upeg_core::paths::env::UPEG_HOME;

    /// Override the toolkits directory (default `<config_root>/toolkits`).
    pub use upeg_core::paths::env::TOOLKITS_DIR;

    /// Override the WASM plugin directory (default `<config_root>/wasm`).
    pub use upeg_core::paths::env::WASM_DIR;

    /// Override the MCP-import descriptor directory (default `<config_root>/mcp-imports`).
    pub use upeg_core::paths::env::MCP_IMPORTS_DIR;

    /// Override the credentials.json path (default `<config_root>/credentials.json`).
    pub const CREDENTIALS_PATH: &str = "UPEG_CREDENTIALS_PATH";

    /// Override the execution-log database path (default: the shared
    /// SQLite store, `<config_root>/upeg.db`). When set, the value is the
    /// full path of a **standalone store database file** holding the
    /// `execution_log` table — not a JSONL file.
    pub const LOG_PATH: &str = "UPEG_LOG_PATH";

    /// Override the daemon HTTP log path (default `<config_root>/upeg-http.log`).
    pub const HTTP_LOG_PATH: &str = "UPEG_HTTP_LOG_PATH";

    /// Allow HTTP server to bind outside loopback. `1` / `true` enables.
    /// Re-exported from `upeg-runtime` so both the runtime status snapshot
    /// and the HTTP surface read the same canonical name.
    pub use upeg_runtime::HTTP_ALLOW_NON_LOOPBACK_ENV as HTTP_ALLOW_NON_LOOPBACK;

    /// Skip writing `server.json` discovery on HTTP server start. `1` / `true` enables.
    pub const PUBLISH_DISCOVERY: &str = "UPEG_PUBLISH_DISCOVERY";

    /// HTTP token used for bearer auth. This is the **operator** token:
    /// whoever presents it has the authority of the person who started
    /// the host.
    pub const HTTP_TOKEN: &str = "UPEG_HTTP_TOKEN";

    /// Comma-separated **agent** bearer tokens. A request authenticated
    /// with one of these may call tools, but it is stamped
    /// `_upeg.principal.role = agent`: its origin-surface header is not
    /// trusted and it cannot approve a gated Chain step. See
    /// `upeg_cli::infrastructure::auth` module docs.
    pub const HTTP_AGENT_TOKENS: &str = "UPEG_HTTP_AGENT_TOKENS";

    /// CLI notification opt-in. `1` / `true` enables OS notifications.
    pub const NOTIFY: &str = "UPEG_NOTIFY";

    /// External-tool subprocess Board key. Re-exported from `upeg-loader`.
    pub use upeg_loader::BOARD_ENV as BOARD;

    /// External-tool subprocess project manifest path. Re-exported from
    /// `upeg-loader`.
    pub use upeg_loader::PROJECT_MANIFEST_ENV as PROJECT_MANIFEST;

    /// Desktop binary tray-build-menu smoke gate. When set, the binary
    /// runs the tray's `build_menu()` check and exits with the verdict.
    pub const DESKTOP_TRAY_SMOKE: &str = "UPEG_TRAY_SMOKE";

    /// Desktop binary popup-mode startup override.
    pub const DESKTOP_POPUP: &str = "UPEG_DESKTOP_POPUP";

    /// Standard POSIX home-directory variable. Exposed for the few sites
    /// that intentionally probe the raw value (e.g. doctor diagnostics).
    pub use upeg_core::paths::env::HOME;

    /// Standard Windows roaming-app-data variable. Same exposure rationale
    /// as [`HOME`].
    pub use upeg_core::paths::env::APPDATA;
}

const SERVER_DISCOVERY_FILE: &str = "server.json";
const HTTP_LOG_FILE: &str = "upeg-http.log";
const CREDENTIALS_FILE: &str = "credentials.json";

/// `~/.upeg/` (Unix) or `%APPDATA%\upeg\` (Windows). Returns `None`
/// when neither the platform default nor any override is available —
/// callers should treat that as "no per-user config available" rather
/// than failing hard.
///
/// Resolution order:
/// 1. `$UPEG_HOME` if set — used verbatim.
/// 2. `dirs::config_dir()` on Windows (`%APPDATA%`) joined with `upeg`.
/// 3. `dirs::home_dir()` on Unix (`$HOME`) joined with `.upeg`.
pub fn config_root() -> Option<PathBuf> {
    upeg_core::paths::config_root()
}

/// `<config_root>/toolkits` or `$UPEG_TOOLKITS_DIR` override.
pub fn toolkits_dir() -> Option<PathBuf> {
    upeg_core::paths::toolkits_dir()
}

/// `<config_root>/wasm` or `$UPEG_WASM_DIR` override.
pub fn wasm_dir() -> Option<PathBuf> {
    upeg_core::paths::wasm_dir()
}

/// `<config_root>/mcp-imports` or `$UPEG_MCP_IMPORTS_DIR` override.
pub fn mcp_import_dir() -> Option<PathBuf> {
    upeg_core::paths::mcp_import_dir()
}

/// `<config_root>/credentials.json` or `$UPEG_CREDENTIALS_PATH` override.
pub fn credentials_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env::CREDENTIALS_PATH) {
        return Some(PathBuf::from(p));
    }
    upeg_core::paths::user_paths().map(|paths| paths.config_dir.join(CREDENTIALS_FILE))
}

/// Database the execution log lives in: the shared SQLite store
/// (`<config_root>/upeg.db`), or a standalone store database at
/// `$UPEG_LOG_PATH` when that override is set (see [`env::LOG_PATH`]).
pub fn execution_log_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env::LOG_PATH) {
        return Some(PathBuf::from(p));
    }
    upeg_core::paths::store_path()
}

/// `<config_root>/server.json` — Discovery file (PRD §5.3).
pub fn server_json_path() -> Option<PathBuf> {
    upeg_core::paths::user_paths().map(|paths| paths.runtime_dir.join(SERVER_DISCOVERY_FILE))
}

/// `<config_root>/upeg-http.log` — Default daemon log file (PRD §5.5).
/// Honors `$UPEG_HTTP_LOG_PATH` for full path override.
pub fn http_log_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env::HTTP_LOG_PATH) {
        return Some(PathBuf::from(p));
    }
    upeg_core::paths::user_paths().map(|paths| paths.state_dir.join(HTTP_LOG_FILE))
}

/// Best-effort `create_dir_all` on the config root. Returns `Ok(())`
/// when the root cannot be located (callers can still operate, just
/// without persistence).
pub fn ensure_config_root() -> std::io::Result<()> {
    if let Some(root) = config_root() {
        std::fs::create_dir_all(&root)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_root_returns_something_under_a_typical_dev_env() {
        if std::env::var_os(env::UPEG_HOME).is_some() || std::env::var_os(env::HOME).is_some() {
            let root = config_root().expect("expected Some when HOME-class env is set");
            assert!(!root.as_os_str().is_empty());
        }
    }

    #[test]
    fn server_json_path_sits_under_runtime_role() {
        if let (Some(roles), Some(path)) = (upeg_core::paths::user_paths(), server_json_path()) {
            assert!(path.starts_with(&roles.runtime_dir));
            assert!(path.ends_with(SERVER_DISCOVERY_FILE));
        }
    }

    #[test]
    fn toolkit_subdirectory_paths_are_assembled_under_storage_roles() {
        if let Some(roles) = upeg_core::paths::user_paths() {
            if std::env::var_os(env::TOOLKITS_DIR).is_none()
                && let Some(p) = toolkits_dir()
            {
                assert_eq!(p, roles.data_dir.join(upeg_core::paths::TOOLKITS_SUBDIR));
            }
            if std::env::var_os(env::WASM_DIR).is_none()
                && let Some(p) = wasm_dir()
            {
                assert_eq!(p, roles.data_dir.join(upeg_core::paths::WASM_SUBDIR));
            }
            if std::env::var_os(env::MCP_IMPORTS_DIR).is_none()
                && let Some(p) = mcp_import_dir()
            {
                assert_eq!(
                    p,
                    roles.config_dir.join(upeg_core::paths::MCP_IMPORTS_SUBDIR)
                );
            }
        }
    }
}
