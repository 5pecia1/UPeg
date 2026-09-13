//! `devcontainer` toolkit — locate VS Code / Cursor Dev Container workspaces
//! by scanning each editor's `workspaceStorage`, decoding the `vscode-remote`
//! Dev Container URIs (whose authority carries a hex-encoded JSON payload with
//! the host workspace path and `.devcontainer` config path), and ranking the
//! results by recency.
//!
//! Native-only, for the same physical reason as `eth.rs`/`weather.rs`: the work
//! needs a real filesystem and a SQLite reader (`state.vscdb`), neither of
//! which the `wasm32` (Flutter web / PWA) sandbox provides. Like those
//! toolkits, the `#[tool]`-annotated `pub fn`s (and the `StaticToolMeta` the
//! macro emits alongside them) still compile on every target so the tools stay
//! discoverable for host-attach; only the actual filesystem/SQLite
//! implementation is native-gated, with a `wasm32` stub that returns an error.
//! The runtime *dispatcher* registration
//! (`dispatch.rs::register_devcontainer_dispatchers`) is
//! `#[cfg(not(target_arch = "wasm32"))]`-gated on top of that, exactly like
//! `eth`/`weather`.
//!
//! The implementation is split across submodules to stay within the
//! workspace's 1000-line file budget. This module keeps the shared vocabulary
//! (every path/key/URI const, [`DevContainerEntry`], and the input enums) plus
//! the `#[tool]` entry points; each submodule is native-only and owns one
//! stage of the pipeline:
//!
//! - [`uri`] — `vscode-remote` URI splitting and hex-JSON authority decoding
//! - [`storage`] — `workspaceStorage` scanning and `state.vscdb` reads
//! - [`ranking`] — open/recent rank maps, rank application, sorting
//!
//! The `#[cfg(test)] mod tests;` block lives in `devcontainer/tests.rs`.

// The entire filesystem/SQLite implementation (and every path/key/URI const
// it uses) is `#[cfg(not(target_arch = "wasm32"))]`-gated; on `wasm32` only the
// `#[tool]` `pub fn`s and their error stubs remain, leaving those consts dead.
// Silence dead-code there only — the native build stays strict.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        dead_code,
        reason = "native-only consts/helpers stay declared but unused on wasm32, where only the #[tool] pub fns and their error stubs compile"
    )
)]

use upeg_core::tool;

/// `vscode-remote` authority prefix marking a Dev Container payload; the
/// remainder is the hex-encoded JSON authority.
const DEV_AUTH_PREFIX: &str = "dev-container+";
/// SSH remote authority prefix; its remainder may be a hex-encoded JSON host.
const SSH_REMOTE_PREFIX: &str = "ssh-remote+";
/// URI scheme every remote (Dev Container / SSH) workspace URI uses.
const VSCODE_REMOTE_SCHEME: &str = "vscode-remote";
/// `file://` scheme prefix, decoded into a plain path by `uri::file_uri_to_path`.
const FILE_URI_PREFIX: &str = "file://";
/// `ssh://` scheme prefix stripped from a settings-derived remote host.
const SSH_SCHEME_PREFIX: &str = "ssh://";
/// Authority/URI separator between the Dev Container authority and any outer
/// (SSH) authority: `dev-container+<hex>@<outer>`.
const AUTHORITY_SEPARATOR: char = '@';
/// Scheme/authority separator (`scheme://authority/path`).
const SCHEME_SEPARATOR: &str = "://";
/// Fallback authority prefix key in `state.vscdb` (`resource.authority.os.*`).
const RESOURCE_AUTHORITY_PREFIX: &str = "resource.authority.os.";
/// Prefix a Dev Container URI recovered from `state.vscdb` is rebuilt with.
const VSCODE_REMOTE_URI_PREFIX: &str = "vscode-remote://";
/// Substring marking the start of a Dev Container URI embedded in a larger
/// blob (`debug.selectedroot`, `history.entries`).
const DEVCONTAINER_URI_MARKER: &str = "vscode-remote://dev-container";

// ─── decoded-payload JSON keys ──────────────────────────────────────

/// `settings` sub-object inside a decoded Dev Container authority.
const KEY_SETTINGS: &str = "settings";
/// SSH host name key inside a decoded authority / settings block.
const KEY_HOST_NAME: &str = "hostName";
/// SSH host key (fallback to [`KEY_HOST_NAME`]).
const KEY_HOST: &str = "host";
/// Candidate keys (in priority order) for the host workspace path.
const HOST_WORKSPACE_KEYS: &[&str] = &["workspacePath", "hostPath", "workspaceFolder"];
/// Inline `.devcontainer` config path key.
const KEY_DEVCONTAINER_PATH: &str = "devcontainerPath";
/// URI-object config path key (decoded via `uri::uri_obj_path`).
const KEY_CONFIG_FILE: &str = "configFile";
/// Candidate keys (in priority order) for a path inside a URI object.
const URI_OBJECT_PATH_KEYS: &[&str] = &["fsPath", "path", "external"];

// ─── workspace.json / storage keys ──────────────────────────────────

/// `workspace.json` keys holding the workspace URI, in priority order. The
/// matched key is recorded as the entry's `workspace_kind`.
const WORKSPACE_URI_KEYS: &[&str] = &["folder", "workspace"];
/// `state.vscdb` key: the debugger's selected root (best URI source).
const KEY_DEBUG_SELECTEDROOT: &str = "debug.selectedroot";
/// `state.vscdb` key: opened-history entries (fallback URI source).
const KEY_HISTORY_ENTRIES: &str = "history.entries";
/// `globalStorage/state.vscdb` key: recently-opened paths list (recent ranks).
const KEY_HISTORY_RECENT: &str = "history.recentlyOpenedPathsList";
/// `storage.json` top-level key holding open (backed-up) workspaces.
const KEY_BACKUP_WORKSPACES: &str = "backupWorkspaces";
/// `storage.json` top-level key holding the last menubar data (recent ranks).
const KEY_MENUBAR_DATA: &str = "lastKnownMenubarData";
/// `backupWorkspaces.folders[].folderUri`, and the recent-list folder key.
const KEY_FOLDER_URI: &str = "folderUri";
/// `backupWorkspaces.workspaces[].workspaceUri`, and the recent workspace key.
const KEY_WORKSPACE_URI: &str = "workspaceUri";
/// `backupWorkspaces.folders` array key.
const KEY_BACKUP_FOLDERS: &str = "folders";
/// `backupWorkspaces.workspaces` array key.
const KEY_BACKUP_WORKSPACES_LIST: &str = "workspaces";
/// Recent-list `entries` array key.
const KEY_ENTRIES: &str = "entries";
/// Menubar URI-object key whose `external` field is a workspace URI.
const KEY_URI: &str = "uri";
/// URI-object `external` field (a full URI string).
const KEY_EXTERNAL: &str = "external";

// ─── filesystem layout ──────────────────────────────────────────────

/// Per-editor `User` directory name under the app support/config root.
const USER_DIR: &str = "User";
/// `workspaceStorage` directory name under each `User` directory.
const WORKSPACE_STORAGE_DIR: &str = "workspaceStorage";
/// `globalStorage` directory name under each `User` directory.
const GLOBAL_STORAGE_DIR: &str = "globalStorage";
/// Per-workspace `workspace.json` file name.
const WORKSPACE_JSON_FILE: &str = "workspace.json";
/// Per-workspace SQLite state file name.
const STATE_VSCDB_FILE: &str = "state.vscdb";
/// Per-workspace SQLite state backup file name (mtime candidate only).
const STATE_VSCDB_BACKUP_FILE: &str = "state.vscdb.backup";
/// `globalStorage/storage.json` file name.
const STORAGE_JSON_FILE: &str = "storage.json";

/// Editor application directory names. A storage root's app component is
/// validated against these; the order fixes the scan order of
/// `storage::default_storage_roots`.
const APP_NAMES: &[&str] = &["Code - Insiders", "Code", "Cursor", "VSCodium"];

/// App label for a storage root whose layout has no `<app>` component.
const UNKNOWN_APP: &str = "unknown";

// ─── ranking ────────────────────────────────────────────────────────

/// Sort position of one rank field. `Option`'s own ordering puts `None` first,
/// but an unranked entry must sink *below* every ranked one — the leading
/// "is unranked" flag flips that, and ranked entries then compare by value.
type RankOrder = (bool, Option<i64>);

/// The [`RankOrder`] of a rank field.
const fn rank_order(rank: Option<i64>) -> RankOrder {
    (rank.is_none(), rank)
}

/// Editor-app filter for [`devcontainer_list`] / [`devcontainer_lookup`],
/// modelled as a closed type so an unknown value fails at parse time.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppFilter {
    All,
    Code,
    Cursor,
    Insiders,
    VsCodium,
}

#[cfg(not(target_arch = "wasm32"))]
impl AppFilter {
    /// Parse the `app` input; blank means [`AppFilter::All`].
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Self::All),
            "code" => Ok(Self::Code),
            "cursor" => Ok(Self::Cursor),
            "insiders" => Ok(Self::Insiders),
            "vscodium" => Ok(Self::VsCodium),
            other => Err(format!(
                "unknown app `{other}`; expected one of all, code, cursor, insiders, vscodium"
            )),
        }
    }

    /// Whether an entry's `app` label satisfies this filter (case-insensitive).
    fn matches(self, app: &str) -> bool {
        let app = app.to_ascii_lowercase();
        match self {
            Self::All => true,
            Self::Code => app == "code",
            Self::Cursor => app == "cursor",
            Self::Insiders => app.contains("insiders"),
            Self::VsCodium => app == "vscodium",
        }
    }
}

/// A single selectable field for [`devcontainer_lookup`]'s `field` input,
/// modelled as a closed type so an unknown value fails at parse time.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Container,
    Host,
    Config,
    Uri,
    Remote,
    Storage,
    App,
    Id,
}

#[cfg(not(target_arch = "wasm32"))]
impl Field {
    /// Parse the optional `field` input; blank means "no field" (`None`).
    fn parse(value: &str) -> Result<Option<Self>, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" => Ok(None),
            "container" => Ok(Some(Self::Container)),
            "host" => Ok(Some(Self::Host)),
            "config" => Ok(Some(Self::Config)),
            "uri" => Ok(Some(Self::Uri)),
            "remote" => Ok(Some(Self::Remote)),
            "storage" => Ok(Some(Self::Storage)),
            "app" => Ok(Some(Self::App)),
            "id" => Ok(Some(Self::Id)),
            other => Err(format!(
                "unknown field `{other}`; expected one of container, host, config, uri, remote, storage, app, id"
            )),
        }
    }

    /// Resolve this field to its string value for `entry`.
    fn value(self, entry: &DevContainerEntry) -> String {
        match self {
            Self::Container => entry.container_path.clone(),
            Self::Host => entry.host_workspace_path.clone(),
            Self::Config => entry.devcontainer_config_path.clone(),
            Self::Uri => entry.uri.clone(),
            Self::Remote => {
                if entry.remote_host.is_empty() {
                    entry.remote_authority.clone()
                } else {
                    entry.remote_host.clone()
                }
            }
            Self::Storage => entry.storage_path.clone(),
            Self::App => entry.app.clone(),
            Self::Id => entry.storage_id.clone(),
        }
    }
}

/// A discovered Dev Container workspace: where it was found (`storage_*`), the
/// URI it was found under, the paths that URI decodes to, and its recency
/// ranks. Serialized to JSON via [`Self::to_json`] (kept manual to avoid
/// pulling `serde` derive into `upeg-tools`).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
struct DevContainerEntry {
    app: String,
    storage_root: String,
    storage_id: String,
    storage_path: String,
    workspace_json: String,
    workspace_kind: String,
    uri: String,
    container_path: String,
    host_workspace_path: String,
    devcontainer_config_path: String,
    remote_authority: String,
    remote_host: String,
    devcontainer_authority: String,
    decoded: Option<serde_json::Value>,
    mtime: f64,
    open_rank: Option<i64>,
    recent_rank: Option<i64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DevContainerEntry {
    /// Serialize every field to the JSON object the tools emit. `decoded`
    /// becomes `null` when the authority payload could not be decoded.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "app": self.app,
            "storage_root": self.storage_root,
            "storage_id": self.storage_id,
            "storage_path": self.storage_path,
            "workspace_json": self.workspace_json,
            "workspace_kind": self.workspace_kind,
            "uri": self.uri,
            "container_path": self.container_path,
            "host_workspace_path": self.host_workspace_path,
            "devcontainer_config_path": self.devcontainer_config_path,
            "remote_authority": self.remote_authority,
            "remote_host": self.remote_host,
            "devcontainer_authority": self.devcontainer_authority,
            "decoded": self.decoded.clone().unwrap_or(serde_json::Value::Null),
            "mtime": self.mtime,
            "open_rank": self.open_rank,
            "recent_rank": self.recent_rank,
        })
    }

    /// Ordering key: open rank, then recent rank, then newest mtime first.
    /// Unranked entries sink below ranked ones — see [`RankOrder`].
    fn sort_key(&self) -> (RankOrder, RankOrder, f64) {
        (
            rank_order(self.open_rank),
            rank_order(self.recent_rank),
            -self.mtime,
        )
    }
}

/// Whether `entry` matches every whitespace-separated pattern token
/// (case-insensitive substring, ANDed) across its text fields and decoded
/// payload. An empty pattern matches everything.
#[cfg(not(target_arch = "wasm32"))]
fn entry_matches(entry: &DevContainerEntry, pattern: &str) -> bool {
    let tokens: Vec<String> = pattern
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    if tokens.is_empty() {
        return true;
    }
    let decoded = entry
        .decoded
        .as_ref()
        .map_or_else(|| "{}".to_string(), serde_json::Value::to_string);
    let haystack = [
        entry.app.as_str(),
        entry.storage_id.as_str(),
        entry.storage_path.as_str(),
        entry.uri.as_str(),
        entry.container_path.as_str(),
        entry.host_workspace_path.as_str(),
        entry.devcontainer_config_path.as_str(),
        entry.remote_authority.as_str(),
        entry.remote_host.as_str(),
        decoded.as_str(),
    ]
    .join("\n")
    .to_ascii_lowercase();
    tokens.iter().all(|token| haystack.contains(token.as_str()))
}

// ─── native implementation ──────────────────────────────────────────

/// Open/recent rank maps and entry ordering.
#[cfg(not(target_arch = "wasm32"))]
mod ranking;
/// `workspaceStorage` / `state.vscdb` scanning.
#[cfg(not(target_arch = "wasm32"))]
mod storage;
/// Pure `vscode-remote` URI / hex-JSON authority parsing.
#[cfg(not(target_arch = "wasm32"))]
mod uri;

/// Collect, filter (by app), rank, and sort all Dev Container entries.
#[cfg(not(target_arch = "wasm32"))]
fn gather_entries(app: AppFilter) -> Vec<DevContainerEntry> {
    let roots = storage::default_storage_roots();
    let mut entries: Vec<DevContainerEntry> = storage::collect_entries(&roots)
        .into_iter()
        .filter(|entry| app.matches(&entry.app))
        .collect();
    ranking::apply_ranks(&mut entries);
    ranking::sort_entries(&mut entries);
    entries
}

/// Serialize a JSON value to a string, mapping failures to `String`.
#[cfg(not(target_arch = "wasm32"))]
fn serialize(value: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to serialize result: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn devcontainer_list_impl(app: &str) -> Result<String, String> {
    let app = AppFilter::parse(app)?;
    let entries = gather_entries(app);
    let array: Vec<serde_json::Value> = entries.iter().map(DevContainerEntry::to_json).collect();
    serialize(&serde_json::Value::Array(array))
}

#[cfg(not(target_arch = "wasm32"))]
fn devcontainer_lookup_impl(pattern: &str, field: &str, app: &str) -> Result<String, String> {
    let app = AppFilter::parse(app)?;
    let field = Field::parse(field)?;
    let entries = gather_entries(app);
    let entry = entries
        .iter()
        .find(|entry| entry_matches(entry, pattern))
        .ok_or_else(|| "no matching Dev Container workspace found".to_string())?;

    match field {
        Some(field) => serialize(&serde_json::Value::String(field.value(entry))),
        None => serialize(&entry.to_json()),
    }
}

// ─── wasm32 stubs ───────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn devcontainer_list_impl(_app: &str) -> Result<String, String> {
    Err("devcontainer.list requires native filesystem/sqlite runtime".to_string())
}

#[cfg(target_arch = "wasm32")]
fn devcontainer_lookup_impl(_pattern: &str, _field: &str, _app: &str) -> Result<String, String> {
    Err("devcontainer.lookup requires native filesystem/sqlite runtime".to_string())
}

// ─── tool definitions ───────────────────────────────────────────────

/// `devcontainer.list` — every Dev Container workspace, ranked by recency.
#[tool(
    id = "devcontainer.list",
    display_label = "Dev Container list",
    toolkit = "devcontainer",
    description = "List VS Code/Cursor Dev Container workspaces from workspaceStorage, ranked by recency.",
    inputs = [
        optional app: String = "Editor filter: all|code|cursor|insiders|vscodium (default all)",
    ],
    outputs = [
        result: Json = "JSON array of Dev Container entries (app, paths, uri, ranks, decoded authority)",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http],
)]
pub fn devcontainer_list(app: &str) -> Result<String, String> {
    devcontainer_list_impl(app)
}

/// `devcontainer.lookup` — the top-ranked entry matching a pattern, optionally
/// projected to a single field.
#[tool(
    id = "devcontainer.lookup",
    display_label = "Dev Container lookup",
    toolkit = "devcontainer",
    description = "Find the top-ranked Dev Container workspace matching a pattern; optionally return one field.",
    inputs = [
        required pattern: String = "Case-insensitive filter (space-separated tokens are ANDed)",
        optional field: String = "Single field: container|host|config|uri|remote|storage|app|id",
        optional app: String = "Editor filter: all|code|cursor|insiders|vscodium (default all)",
    ],
    outputs = [
        result: Json = "The matching entry as a JSON object, or the requested field as a JSON string",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http],
)]
pub fn devcontainer_lookup(pattern: &str, field: &str, app: &str) -> Result<String, String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }
    devcontainer_lookup_impl(pattern, field, app)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
