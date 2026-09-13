//! Filesystem / SQLite scanning for `devcontainer`, extracted from the parent
//! to keep every file under the 1000-line workspace budget.
//!
//! This module owns everything that touches the disk: locating each editor's
//! `workspaceStorage` root, walking its per-workspace directories, and reading
//! a Dev Container URI out of either `workspace.json` or `state.vscdb`. It is
//! native-only (mounted behind `#[cfg(not(target_arch = "wasm32"))]` in the
//! parent) — the wasm32 sandbox has neither a real filesystem nor SQLite.

use std::path::{Path, PathBuf};

use super::uri::{extract_first_devcontainer_uri, parse_devcontainer_uri};
use super::{
    APP_NAMES, DEV_AUTH_PREFIX, DEVCONTAINER_URI_MARKER, DevContainerEntry, KEY_DEBUG_SELECTEDROOT,
    KEY_HISTORY_ENTRIES, RESOURCE_AUTHORITY_PREFIX, STATE_VSCDB_BACKUP_FILE, STATE_VSCDB_FILE,
    UNKNOWN_APP, USER_DIR, VSCODE_REMOTE_URI_PREFIX, WORKSPACE_JSON_FILE, WORKSPACE_STORAGE_DIR,
    WORKSPACE_URI_KEYS, uri::ParsedUri,
};

/// SQLite `workspace_kind` label for entries recovered from `state.vscdb`.
pub(super) const WORKSPACE_KIND_SQLITE: &str = "sqlite";

/// `LIKE` wildcard suffix matching any remainder of an authority key.
const SQL_LIKE_WILDCARD: &str = "%";

/// Row cap for the `state.vscdb` URI probe. The `order by` puts the two exact
/// keys first and orders the `resource.authority.os.*` remainder by key, so a
/// bounded read still reaches a usable authority key while a pathological
/// workspace cannot make this scan unbounded.
const STATE_URI_QUERY_LIMIT: i64 = 20;

/// Probe query for a Dev Container URI in a per-workspace `state.vscdb`:
/// `debug.selectedroot` first, then `history.entries`, then any
/// `resource.authority.os.dev-container+*` key (ordered by key so a workspace
/// with several of them resolves deterministically). Every literal is bound
/// from the module's key consts — see [`state_uri_from_db`].
const STATE_URI_QUERY: &str = "select key, value from ItemTable \
     where key in (?1, ?2) or key like ?3 \
     order by case key when ?1 then 0 when ?2 then 1 else 2 end, key \
     limit ?4";

/// Default per-editor `workspaceStorage` roots for the running platform,
/// keeping only directories that exist and deduplicating by canonical path.
pub(super) fn default_storage_roots() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let base: PathBuf = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Roaming"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
    };

    storage_roots_under(&base)
}

/// The existing `<base>/<app>/User/workspaceStorage` roots under `base`,
/// deduplicated by canonical path. Split out of [`default_storage_roots`] so
/// the layout rule can be exercised against a temporary base directory.
pub(super) fn storage_roots_under(base: &Path) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut roots = Vec::<PathBuf>::new();
    for app in APP_NAMES {
        let root = base.join(app).join(USER_DIR).join(WORKSPACE_STORAGE_DIR);
        if !root.is_dir() {
            continue;
        }
        let key = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        if seen.insert(key) {
            roots.push(root);
        }
    }
    roots
}

/// Infer the editor-app label from a `<base>/<app>/User/workspaceStorage`
/// root.
///
/// The `<app>` component is read *positionally* (two levels up) and only then
/// validated against [`APP_NAMES`]. Matching an app name against any component
/// would misattribute every root with an editor-named ancestor — under
/// `/home/Code/.config/Cursor/User/workspaceStorage` the app is Cursor, not the
/// user directory `Code`.
pub(super) fn app_name_from_storage_root(root: &Path) -> String {
    let Some(component) = root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return UNKNOWN_APP.to_string();
    };
    // A known editor directory is reported with its canonical spelling; any
    // other directory name is passed through as the app label unchanged.
    APP_NAMES
        .iter()
        .find(|name| name.eq_ignore_ascii_case(&component))
        .map_or(component, |name| (*name).to_string())
}

/// Newest mtime (seconds since the Unix epoch) among a workspace directory and
/// its state files.
pub(super) fn workspace_mtime(storage_dir: &Path) -> f64 {
    let candidates = [
        storage_dir.to_path_buf(),
        storage_dir.join(WORKSPACE_JSON_FILE),
        storage_dir.join(STATE_VSCDB_FILE),
        storage_dir.join(STATE_VSCDB_BACKUP_FILE),
    ];
    let mut newest = 0.0_f64;
    for candidate in &candidates {
        if let Ok(seconds) = std::fs::metadata(candidate)
            .and_then(|meta| meta.modified())
            .map(system_time_to_secs)
            && seconds > newest
        {
            newest = seconds;
        }
    }
    newest
}

/// Convert a [`std::time::SystemTime`] to fractional seconds since the epoch.
fn system_time_to_secs(time: std::time::SystemTime) -> f64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

/// Open a SQLite database read-only with a short busy timeout. URI filename
/// parsing stays off: these paths come from the filesystem walk, never from a
/// user-supplied connection string, so interpreting one as a URI could only
/// mis-open it.
pub(super) fn open_sqlite_ro(db: &Path) -> Option<rusqlite::Connection> {
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(1));
    Some(conn)
}

/// Build a [`DevContainerEntry`] from a parsed URI plus storage metadata,
/// shared by the `workspace.json` and `state.vscdb` loaders.
fn build_entry(
    storage_root: &Path,
    storage_dir: &Path,
    workspace_kind: &str,
    uri: String,
    parsed: ParsedUri,
) -> DevContainerEntry {
    DevContainerEntry {
        app: app_name_from_storage_root(storage_root),
        storage_root: storage_root.to_string_lossy().into_owned(),
        storage_id: storage_dir
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        storage_path: storage_dir.to_string_lossy().into_owned(),
        workspace_json: storage_dir
            .join(WORKSPACE_JSON_FILE)
            .to_string_lossy()
            .into_owned(),
        workspace_kind: workspace_kind.to_string(),
        uri,
        container_path: parsed.container_path,
        host_workspace_path: parsed.host_workspace_path,
        devcontainer_config_path: parsed.devcontainer_config_path,
        remote_authority: parsed.remote_authority,
        remote_host: parsed.remote_host,
        devcontainer_authority: parsed.devcontainer_authority,
        decoded: Some(parsed.decoded),
        mtime: workspace_mtime(storage_dir),
        open_rank: None,
        recent_rank: None,
    }
}

/// Load a Dev Container entry from `workspace.json`. Returns `None` when the
/// file is absent/unparseable or holds no Dev Container URI.
pub(super) fn load_workspace_entry(
    storage_root: &Path,
    storage_dir: &Path,
) -> Option<DevContainerEntry> {
    let workspace_json = storage_dir.join(WORKSPACE_JSON_FILE);
    let text = std::fs::read_to_string(&workspace_json).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;

    let mut workspace_kind = "";
    let mut uri = String::new();
    for key in WORKSPACE_URI_KEYS {
        if let Some(found) = data.get(*key).and_then(serde_json::Value::as_str) {
            workspace_kind = key;
            uri = found.to_string();
            break;
        }
    }
    if uri.is_empty() {
        return None;
    }

    let parsed = parse_devcontainer_uri(&uri)?;
    Some(build_entry(
        storage_root,
        storage_dir,
        workspace_kind,
        uri,
        parsed,
    ))
}

/// Query `state.vscdb` for a Dev Container URI, preferring
/// [`KEY_DEBUG_SELECTEDROOT`], then [`KEY_HISTORY_ENTRIES`], then any
/// `resource.authority.os.dev-container+*` key. Only a candidate that actually
/// parses is returned, so a malformed early row cannot mask a usable later one.
pub(super) fn state_uri_from_db(db: &Path) -> Option<String> {
    let conn = open_sqlite_ro(db)?;
    let mut statement = conn.prepare(STATE_URI_QUERY).ok()?;
    let authority_pattern =
        format!("{RESOURCE_AUTHORITY_PREFIX}{DEV_AUTH_PREFIX}{SQL_LIKE_WILDCARD}");
    let rows = statement
        .query_map(
            rusqlite::params![
                KEY_DEBUG_SELECTEDROOT,
                KEY_HISTORY_ENTRIES,
                authority_pattern,
                STATE_URI_QUERY_LIMIT,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1).unwrap_or_default(),
                ))
            },
        )
        .ok()?;

    for row in rows {
        let Ok((key, value)) = row else { continue };
        let candidate = if key == KEY_DEBUG_SELECTEDROOT {
            if value.starts_with(DEVCONTAINER_URI_MARKER) {
                value
            } else {
                extract_first_devcontainer_uri(&value)
            }
        } else if key == KEY_HISTORY_ENTRIES {
            extract_first_devcontainer_uri(&value)
        } else if let Some(authority) = key.strip_prefix(RESOURCE_AUTHORITY_PREFIX)
            && authority.starts_with(DEV_AUTH_PREFIX)
        {
            format!("{VSCODE_REMOTE_URI_PREFIX}{authority}")
        } else {
            continue;
        };
        if parse_devcontainer_uri(&candidate).is_some() {
            return Some(candidate);
        }
    }
    None
}

/// Load a Dev Container entry from a workspace's `state.vscdb`.
pub(super) fn load_state_entry(
    storage_root: &Path,
    storage_dir: &Path,
) -> Option<DevContainerEntry> {
    let db = storage_dir.join(STATE_VSCDB_FILE);
    if !db.exists() {
        return None;
    }
    let uri = state_uri_from_db(&db)?;
    let parsed = parse_devcontainer_uri(&uri)?;
    Some(build_entry(
        storage_root,
        storage_dir,
        WORKSPACE_KIND_SQLITE,
        uri,
        parsed,
    ))
}

/// Scan every storage root, collecting one entry per Dev Container workspace
/// (`workspace.json` preferred, `state.vscdb` fallback).
pub(super) fn collect_entries(storage_roots: &[PathBuf]) -> Vec<DevContainerEntry> {
    let mut entries = Vec::new();
    for root in storage_roots {
        let Ok(children) = std::fs::read_dir(root) else {
            continue;
        };
        for child in children.flatten() {
            let path = child.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(entry) =
                load_workspace_entry(root, &path).or_else(|| load_state_entry(root, &path))
            {
                entries.push(entry);
            }
        }
    }
    entries
}
