//! Canonical per-user config paths shared by every surface.
//!
//! The storage contract is surface-neutral: native TUI and Flutter desktop use
//! the same files under the same root. A surface may ignore settings it cannot
//! apply, but it must not invent a separate durable settings location.

use std::path::{Path, PathBuf};

#[path = "storage_paths.rs"]
mod storage;
pub use storage::*;

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
    pub const TOOLKIT_CACHE_DIR: &str = "UPEG_TOOLKIT_CACHE_DIR";
}

pub const APP_DIR_NAME: &str = "upeg";
pub const DOT_DIR_NAME: &str = ".upeg";
pub const TOOLKITS_SUBDIR: &str = "toolkits";
pub const WASM_SUBDIR: &str = "wasm";
pub const MCP_IMPORTS_SUBDIR: &str = "mcp-imports";
pub const TOOLKIT_PACKS_SUBDIR: &str = "toolkit-packs";
pub const USER_PATH_LAYOUT: &str = "legacy-v1";
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

pub fn user_paths() -> Option<UserPaths> {
    resolve_storage().ok().map(|location| location.paths)
}

pub fn user_paths_from_env_lookup(
    env_lookup: impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
) -> Option<UserPaths> {
    if let Some(root) = env_lookup(env::UPEG_HOME) {
        let windows_absolute = platform == Platform::Windows
            && root.to_string_lossy().as_bytes().get(1..3) == Some(b":/");
        if !root.is_absolute() && !windows_absolute {
            return Some(StorageLocation::new(root, StorageLayout::LegacyV1).paths);
        }
    }
    resolve_storage_with_lookup(env_lookup, platform, |_| Ok(StorageInventory::default()))
        .ok()
        .map(|location| location.paths)
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
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
    user_paths().map(|paths| paths.config_dir)
}

pub fn config_root_from_env_lookup(
    env_lookup: impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
) -> Option<PathBuf> {
    user_paths_from_env_lookup(env_lookup, platform).map(|paths| paths.config_dir)
}

#[cfg(test)]
fn env_or_user_subdir_from_env_lookup(
    env_lookup: impl Fn(&str) -> Option<PathBuf>,
    platform: Platform,
    env_name: &str,
    subdir: &str,
    role: fn(UserPaths) -> PathBuf,
) -> Option<PathBuf> {
    env_lookup(env_name).or_else(|| {
        user_paths_from_env_lookup(env_lookup, platform).map(|paths| role(paths).join(subdir))
    })
}

fn env_or_user_subdir(
    env_name: &str,
    subdir: &str,
    role: fn(UserPaths) -> PathBuf,
) -> Option<PathBuf> {
    env_path(env_name).or_else(|| user_paths().map(|paths| role(paths).join(subdir)))
}

pub fn env_or_config_subdir(env_name: &str, subdir: &str) -> Option<PathBuf> {
    env_or_user_subdir(env_name, subdir, |paths| paths.config_dir)
}

pub fn toolkits_dir() -> Option<PathBuf> {
    env_or_user_subdir(env::TOOLKITS_DIR, TOOLKITS_SUBDIR, |paths| paths.data_dir)
}

pub fn wasm_dir() -> Option<PathBuf> {
    env_or_user_subdir(env::WASM_DIR, WASM_SUBDIR, |paths| paths.data_dir)
}

pub fn mcp_import_dir() -> Option<PathBuf> {
    env_or_config_subdir(env::MCP_IMPORTS_DIR, MCP_IMPORTS_SUBDIR)
}

pub fn toolkit_packs_dir() -> Option<PathBuf> {
    env_or_user_subdir(env::TOOLKIT_CACHE_DIR, TOOLKIT_PACKS_SUBDIR, |paths| {
        paths.cache_dir
    })
}

pub fn tweaks_path_in(root: &Path) -> PathBuf {
    root.join(TWEAKS_FILENAME)
}

pub fn tweaks_path() -> Option<PathBuf> {
    user_paths().map(|paths| tweaks_path_in(&paths.config_dir))
}

pub fn store_path_in(root: &Path) -> PathBuf {
    root.join(STORE_FILE)
}

pub fn store_path() -> Option<PathBuf> {
    user_paths().map(|paths| store_path_in(&paths.data_dir))
}

pub fn desktop_lock_path_in(root: &Path) -> PathBuf {
    root.join(DESKTOP_LOCK_FILE)
}

pub fn desktop_lock_path() -> Option<PathBuf> {
    user_paths().map(|paths| desktop_lock_path_in(&paths.runtime_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upeg_home_is_used_as_the_config_root_when_set() {
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
    fn unix_uses_dot_upeg_under_home_as_the_config_root() {
        let root = config_root_from_env_lookup(
            |name| (name == env::HOME).then(|| "/home/me".into()),
            Platform::Unix,
        );
        assert_eq!(
            root.as_deref(),
            Some(std::path::Path::new("/home/me/.upeg/config"))
        );
    }

    #[test]
    fn windows_uses_profile_home_not_appdata_for_new_storage() {
        let root = config_root_from_env_lookup(
            |name| match name {
                "USERPROFILE" => Some("C:/Users/me".into()),
                env::APPDATA => Some("C:/Users/me/AppData/Roaming".into()),
                _ => None,
            },
            Platform::Windows,
        );
        assert_eq!(root.as_deref(), Some(Path::new("C:/Users/me/.upeg/config")));
    }

    #[test]
    fn user_roles_preserve_the_legacy_root_on_each_platform() {
        for (platform, name, value, expected) in [
            (
                Platform::Unix,
                env::UPEG_HOME,
                "/custom/upeg",
                "/custom/upeg",
            ),
            (Platform::Windows, env::UPEG_HOME, "D:/upeg", "D:/upeg"),
            (
                Platform::Unix,
                env::UPEG_HOME,
                "relative/root",
                "relative/root",
            ),
            (Platform::Windows, env::UPEG_HOME, "", ""),
        ] {
            let lookup = |key: &str| {
                if key == name {
                    Some(PathBuf::from(value))
                } else if key == env::HOME || key == env::APPDATA {
                    Some(PathBuf::from("ignored-home"))
                } else {
                    None
                }
            };
            let paths = user_paths_from_env_lookup(lookup, platform).expect("legacy root");
            let expected = PathBuf::from(expected);
            assert_eq!(paths.config_dir, expected);
            assert_eq!(paths.data_dir, expected);
            assert_eq!(paths.state_dir, expected);
            assert_eq!(paths.cache_dir, expected);
            assert_eq!(paths.runtime_dir, expected);
            assert_eq!(
                config_root_from_env_lookup(lookup, platform),
                Some(expected)
            );
        }
        assert_eq!(USER_PATH_LAYOUT, "legacy-v1");
    }

    #[test]
    fn unavailable_user_roots_stay_unavailable() {
        for platform in [Platform::Unix, Platform::Windows] {
            assert_eq!(user_paths_from_env_lookup(|_| None, platform), None);
            assert_eq!(config_root_from_env_lookup(|_| None, platform), None);
        }
    }

    #[test]
    fn directory_overrides_remain_verbatim_with_or_without_a_user_root() {
        type Role = fn(UserPaths) -> PathBuf;
        let directories: [(&str, &str, Role); 4] = [
            (env::TOOLKITS_DIR, TOOLKITS_SUBDIR, |paths| paths.data_dir),
            (env::WASM_DIR, WASM_SUBDIR, |paths| paths.data_dir),
            (env::MCP_IMPORTS_DIR, MCP_IMPORTS_SUBDIR, |paths| {
                paths.config_dir
            }),
            (env::TOOLKIT_CACHE_DIR, TOOLKIT_PACKS_SUBDIR, |paths| {
                paths.data_dir
            }),
        ];
        for platform in [Platform::Unix, Platform::Windows] {
            for (name, subdir, role) in directories {
                for root in [None, Some(PathBuf::from("legacy-root"))] {
                    let lookup =
                        |key: &str| (key == env::UPEG_HOME).then(|| root.clone()).flatten();
                    assert_eq!(
                        env_or_user_subdir_from_env_lookup(lookup, platform, name, subdir, role),
                        root.as_ref().map(|path| path.join(subdir)),
                    );
                    for explicit in ["/explicit/path", "legacy/relative", ""] {
                        let overridden = |key: &str| {
                            if key == name {
                                Some(PathBuf::from(explicit))
                            } else {
                                lookup(key)
                            }
                        };
                        assert_eq!(
                            env_or_user_subdir_from_env_lookup(
                                overridden, platform, name, subdir, role
                            ),
                            Some(PathBuf::from(explicit)),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn default_split_roles_and_injected_legacy_selection_are_pure() {
        let lookup = |name: &str| match name {
            env::HOME => Some(PathBuf::from("/literal/home")),
            "XDG_CONFIG_HOME" => Some(PathBuf::from("/ignored")),
            _ => None,
        };
        let split =
            resolve_storage_with_lookup(
                lookup,
                Platform::Unix,
                |_| Ok(StorageInventory::default()),
            )
            .unwrap();
        assert_eq!(split.root, Path::new("/literal/home/.upeg"));
        assert_eq!(split.layout, StorageLayout::SplitV2);
        assert_eq!(
            split.paths.config_dir,
            Path::new("/literal/home/.upeg/config")
        );
        assert_eq!(split.paths.data_dir, Path::new("/literal/home/.upeg/data"));
        assert_eq!(
            split.paths.cache_dir,
            Path::new("/literal/home/.upeg/cache")
        );
        assert_eq!(
            split.paths.runtime_dir,
            Path::new("/literal/home/.upeg/runtime")
        );
        let legacy = resolve_storage_with_lookup(lookup, Platform::Unix, |_| {
            Ok(StorageInventory {
                legacy: true,
                populated: true,
                ..StorageInventory::default()
            })
        })
        .unwrap();
        assert_eq!(legacy.layout, StorageLayout::LegacyV1);
        assert_eq!(legacy.paths.data_dir, legacy.root);
        for invalid in ["", "relative", "/"] {
            assert!(
                resolve_storage_with_lookup(
                    |name| (name == env::HOME).then(|| invalid.into()),
                    Platform::Unix,
                    |_| Ok(StorageInventory::default())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn windows_profile_fallback_legacy_appdata_and_ambiguity_are_explicit() {
        let lookup = |name: &str| match name {
            "HOMEDRIVE" => Some(PathBuf::from("C:")),
            "HOMEPATH" => Some(PathBuf::from("/Users/me")),
            env::APPDATA => Some(PathBuf::from("C:/Users/me/AppData/Roaming")),
            _ => None,
        };
        let inspect = |path: &Path| {
            Ok(StorageInventory {
                legacy: path.ends_with("upeg"),
                populated: path.ends_with("upeg"),
                ..StorageInventory::default()
            })
        };
        let legacy = resolve_storage_with_lookup(lookup, Platform::Windows, inspect).unwrap();
        assert_eq!(legacy.root, Path::new("C:/Users/me/AppData/Roaming/upeg"));
        assert!(matches!(
            resolve_storage_with_lookup(lookup, Platform::Windows, |_| Ok(StorageInventory {
                legacy: true,
                populated: true,
                ..StorageInventory::default()
            })),
            Err(StoragePathError::Ambiguous(_, _))
        ));
        let default = resolve_storage_with_lookup(lookup, Platform::Windows, |_| {
            Ok(StorageInventory::default())
        })
        .unwrap();
        assert_eq!(default.root, Path::new("C:/Users/me/.upeg"));
    }

    #[test]
    fn markers_are_strict_one_hop_and_pending_targets_never_win() {
        let lookup = |name: &str| (name == env::UPEG_HOME).then(|| PathBuf::from("/old"));
        let source = serde_json::json!({"schema_version":1,"app":"upeg","layout":"redirect-v2","root":"/old","target":"/new"});
        let target =
            serde_json::json!({"schema_version":1,"app":"upeg","layout":"split-v2","root":"/new"});
        let resolve = |destination: serde_json::Value, pending| {
            resolve_storage_with_lookup(lookup, Platform::Unix, |path| {
                Ok(StorageInventory {
                    marker: Some(if path == Path::new("/old") {
                        source.clone()
                    } else {
                        destination.clone()
                    }),
                    pending: path == Path::new("/new") && pending,
                    ..StorageInventory::default()
                })
            })
        };
        assert_eq!(
            resolve(target.clone(), false).unwrap().root,
            Path::new("/new")
        );
        assert!(resolve(target, true).is_err());
        assert!(resolve(source.clone(), false).is_err());
        assert!(resolve(serde_json::json!({"layout":"split-v2"}), false).is_err());
    }

    #[test]
    fn shared_file_paths_are_assembled_under_the_same_config_root() {
        let root = std::path::Path::new("/tmp/upeg");
        assert_eq!(tweaks_path_in(root), root.join(TWEAKS_FILENAME));
        assert_eq!(store_path_in(root), root.join(STORE_FILE));
        assert_eq!(desktop_lock_path_in(root), root.join(DESKTOP_LOCK_FILE));
    }
}
