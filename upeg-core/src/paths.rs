//! Canonical per-user config paths shared by every surface.
//!
//! The storage contract is surface-neutral: native TUI and Flutter desktop use
//! the same files under the same root. A surface may ignore settings it cannot
//! apply, but it must not invent a separate durable settings location.

use std::path::{Path, PathBuf};

/// Environment variables that define native shared storage.
pub mod env {
    /// Override the entire native config root.
    pub const UPEG_HOME: &str = "UPEG_HOME";
    /// POSIX home directory fallback for non-Windows platforms.
    pub const HOME: &str = "HOME";
    /// Windows roaming app data fallback.
    pub const APPDATA: &str = "APPDATA";
    /// Override the runtime toolkit directory.
    pub const TOOLKITS_DIR: &str = "UPEG_TOOLKITS_DIR";
    /// Override the runtime WASM plugin directory.
    pub const WASM_DIR: &str = "UPEG_WASM_DIR";
    /// Override the runtime MCP-import descriptor directory.
    pub const MCP_IMPORTS_DIR: &str = "UPEG_MCP_IMPORTS_DIR";
}

pub const APP_DIR_NAME: &str = "upeg";
pub const DOT_DIR_NAME: &str = ".upeg";
pub const TOOLKITS_SUBDIR: &str = "toolkits";
pub const WASM_SUBDIR: &str = "wasm";
pub const MCP_IMPORTS_SUBDIR: &str = "mcp-imports";
pub const TWEAKS_FILENAME: &str = ".upeg-tweaks.json";
/// Shared SQLite store for cross-surface state (pegboard boards /
/// placements / selection today; memos and execution log follow in
/// later migration steps).
pub const STORE_FILE: &str = "upeg.db";
/// Single-instance lock file taken by the desktop shell (PRD §5.9).
/// The path lives here — not in the surface that takes the lock — so
/// the lock owner (`upeg-frb`) and any surface that merely reports on it
/// agree on one location without a surface-to-surface dependency.
pub const DESKTOP_LOCK_FILE: &str = "desktop.lock";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Unix,
    Windows,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// Native shared config root.
///
/// Resolution order:
/// 1. `$UPEG_HOME`, used verbatim.
/// 2. Windows: `%APPDATA%/upeg`.
/// 3. Non-Windows: `$HOME/.upeg`.
pub fn config_root() -> Option<PathBuf> {
    config_root_from_env_lookup(
        |name| std::env::var_os(name).map(PathBuf::from),
        Platform::current(),
    )
}

pub fn config_root_from_env_lookup(
    env_lookup: impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
) -> Option<PathBuf> {
    if let Some(path) = env_lookup(env::UPEG_HOME) {
        return Some(path);
    }
    match platform {
        Platform::Windows => env_lookup(env::APPDATA).map(|path| path.join(APP_DIR_NAME)),
        Platform::Unix => env_lookup(env::HOME).map(|path| path.join(DOT_DIR_NAME)),
    }
}

pub fn env_or_config_subdir(env_name: &str, subdir: &str) -> Option<PathBuf> {
    std::env::var_os(env_name)
        .map(PathBuf::from)
        .or_else(|| config_root().map(|root| root.join(subdir)))
}

pub fn toolkits_dir() -> Option<PathBuf> {
    env_or_config_subdir(env::TOOLKITS_DIR, TOOLKITS_SUBDIR)
}

pub fn wasm_dir() -> Option<PathBuf> {
    env_or_config_subdir(env::WASM_DIR, WASM_SUBDIR)
}

pub fn mcp_import_dir() -> Option<PathBuf> {
    env_or_config_subdir(env::MCP_IMPORTS_DIR, MCP_IMPORTS_SUBDIR)
}

pub fn tweaks_path_in(root: &Path) -> PathBuf {
    root.join(TWEAKS_FILENAME)
}

pub fn tweaks_path() -> Option<PathBuf> {
    config_root().map(|root| tweaks_path_in(&root))
}

pub fn store_path_in(root: &Path) -> PathBuf {
    root.join(STORE_FILE)
}

pub fn store_path() -> Option<PathBuf> {
    config_root().map(|root| store_path_in(&root))
}

pub fn desktop_lock_path_in(root: &Path) -> PathBuf {
    root.join(DESKTOP_LOCK_FILE)
}

pub fn desktop_lock_path() -> Option<PathBuf> {
    config_root().map(|root| desktop_lock_path_in(&root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upeg_home이_있으면_설정_루트로_그_값을_사용한다() {
        let root = config_root_from_env_lookup(
            |name| (name == env::UPEG_HOME).then(|| "/tmp/upeg-home".into()),
            Platform::Unix,
        );
        assert_eq!(
            root.as_deref(),
            Some(std::path::Path::new("/tmp/upeg-home"))
        );
    }

    #[test]
    fn unix는_home_아래_dot_upeg를_설정_루트로_사용한다() {
        let root = config_root_from_env_lookup(
            |name| (name == env::HOME).then(|| "/home/me".into()),
            Platform::Unix,
        );
        assert_eq!(
            root.as_deref(),
            Some(std::path::Path::new("/home/me/.upeg"))
        );
    }

    #[test]
    fn windows는_appdata_아래_upeg를_설정_루트로_사용한다() {
        let root = config_root_from_env_lookup(
            |name| (name == env::APPDATA).then(|| "C:/Users/me/AppData/Roaming".into()),
            Platform::Windows,
        );
        assert_eq!(
            root.as_deref(),
            Some(std::path::Path::new("C:/Users/me/AppData/Roaming/upeg")),
        );
    }

    #[test]
    fn 공유_파일_경로는_같은_설정_루트에서_조립된다() {
        let root = std::path::Path::new("/tmp/upeg");
        assert_eq!(tweaks_path_in(root), root.join(TWEAKS_FILENAME));
        assert_eq!(store_path_in(root), root.join(STORE_FILE));
        assert_eq!(desktop_lock_path_in(root), root.join(DESKTOP_LOCK_FILE));
    }
}
