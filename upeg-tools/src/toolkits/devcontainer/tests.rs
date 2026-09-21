//! Tests for `devcontainer`, extracted from `devcontainer.rs` to keep the
//! parent module under the 1000-line workspace budget. Mounted as a
//! `#[cfg(test)] mod tests;`, so `super::*` brings every item of the parent
//! and `super::{uri, storage, ranking}` the submodules' `pub(super)` items.
//!
//! The storage/SQLite half is exercised against real fixtures: [`Fixture`]
//! builds a throwaway `<base>/<app>/User/{workspaceStorage,globalStorage}`
//! tree with `tempfile`, and [`seed_state_vscdb`] writes a real `state.vscdb`
//! with an in-process `rusqlite` writer.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "tests assert on known-good fixtures; a failed assumption should fail the test loudly"
)]

use std::path::{Path, PathBuf};

use super::ranking::{
    RankMap, apply_ranks, load_sqlite_recent_ranks, load_storage_json_ranks, sort_entries,
};
use super::storage::{
    WORKSPACE_KIND_SQLITE, app_name_from_storage_root, collect_entries, load_state_entry,
    load_workspace_entry, state_uri_from_db, storage_roots_under, workspace_mtime,
};
use super::uri::{
    decode_hex, decode_hex_json, decode_remote_authority, file_uri_to_path, norm_uri,
    parse_devcontainer_uri, split_uri,
};
use super::*;

// ─── fixtures ───────────────────────────────────────────────────────

/// Encode a UTF-8 string as lowercase hex, for constructing test URIs.
fn hex_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// A `vscode-remote://dev-container+<hex payload><path>` URI.
fn devcontainer_uri(payload: &serde_json::Value, container_path: &str) -> String {
    format!(
        "{VSCODE_REMOTE_URI_PREFIX}{DEV_AUTH_PREFIX}{}{container_path}",
        hex_encode(&payload.to_string())
    )
}

/// The stock authority payload: one host workspace plus its config path.
fn payload(host: &str) -> serde_json::Value {
    serde_json::json!({
        "workspacePath": host,
        "devcontainerPath": format!("{host}/.devcontainer/devcontainer.json"),
    })
}

/// A throwaway `<base>/<app>/User/…` editor tree.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// The `<base>` every `<app>` directory hangs off.
    fn base(&self) -> &Path {
        self.dir.path()
    }

    /// `<base>/<app>/User`, created.
    fn user_dir(&self, app: &str) -> PathBuf {
        let dir = self.base().join(app).join(USER_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `<base>/<app>/User/workspaceStorage`, created.
    fn storage_root(&self, app: &str) -> PathBuf {
        let root = self.user_dir(app).join(WORKSPACE_STORAGE_DIR);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// `<base>/<app>/User/globalStorage`, created.
    fn global_storage(&self, app: &str) -> PathBuf {
        let dir = self.user_dir(app).join(GLOBAL_STORAGE_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// One per-workspace storage directory under `root`, created.
    fn workspace(root: &Path, id: &str) -> PathBuf {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

/// Write a `workspace.json` holding `uri` under `key` (`folder`/`workspace`).
fn write_workspace_json(storage_dir: &Path, key: &str, uri: &str) {
    let body = serde_json::json!({ key: uri });
    std::fs::write(
        storage_dir.join(WORKSPACE_JSON_FILE),
        body.to_string().as_bytes(),
    )
    .unwrap();
}

/// Create a real `state.vscdb` at `db` holding the given `ItemTable` rows.
fn seed_state_vscdb(db: &Path, rows: &[(&str, &str)]) {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "create table ItemTable (key text unique on conflict replace, value blob)",
        [],
    )
    .unwrap();
    for (key, value) in rows {
        conn.execute(
            "insert into ItemTable (key, value) values (?1, ?2)",
            [*key, *value],
        )
        .unwrap();
    }
}

fn sample_entry() -> DevContainerEntry {
    DevContainerEntry {
        app: "Code".to_string(),
        storage_root: "/home/u/.config/Code/User/workspaceStorage".to_string(),
        storage_id: "abc123".to_string(),
        storage_path: "/home/u/.config/Code/User/workspaceStorage/abc123".to_string(),
        workspace_json: "/home/u/.config/Code/User/workspaceStorage/abc123/workspace.json"
            .to_string(),
        workspace_kind: "folder".to_string(),
        uri: "vscode-remote://dev-container+abc/workspaces/byfactory-api".to_string(),
        container_path: "/workspaces/byfactory-api".to_string(),
        host_workspace_path: "/home/u/projects/byfactory".to_string(),
        devcontainer_config_path: "/home/u/projects/byfactory/.devcontainer/devcontainer.json"
            .to_string(),
        remote_authority: "ssh-remote+deadbeef".to_string(),
        remote_host: "build.example.test".to_string(),
        devcontainer_authority: "dev-container+abc".to_string(),
        decoded: Some(serde_json::json!({"workspacePath": "/home/u/projects/byfactory"})),
        mtime: 1000.0,
        open_rank: None,
        recent_rank: None,
    }
}

// ─── decode_hex / decode_hex_json ───────────────────────────────────

#[test]
fn hex_decodes_to_bytes() {
    assert_eq!(decode_hex("00ff41"), Some(vec![0x00, 0xff, 0x41]));
    assert_eq!(decode_hex(""), Some(Vec::new()));
}

#[test]
fn hex_decode_rejects_odd_length_instead_of_truncating() {
    // Truncating to `[0xab]` would silently accept corrupt input.
    assert_eq!(decode_hex("abc"), None);
    assert_eq!(decode_hex("zz"), None);
}

#[test]
fn hex_json_decodes_object() {
    // hex of `{"a":1}`
    let hex = "7b2261223a317d";
    let decoded = decode_hex_json(hex).expect("decodes object");
    assert_eq!(decoded, serde_json::json!({"a": 1}));
}

#[test]
fn hex_json_rejects_odd_length_and_non_hex() {
    assert!(decode_hex_json("abc").is_none());
    assert!(decode_hex_json("zz").is_none());
    assert!(decode_hex_json("").is_none());
}

#[test]
fn hex_json_rejects_non_objects() {
    // hex of `123` (a bare number, not an object)
    assert!(decode_hex_json("313233").is_none());
}

// ─── file_uri_to_path ───────────────────────────────────────────────

#[test]
fn file_uri_converts_to_path() {
    assert_eq!(
        file_uri_to_path("file:///home/u/my%20project"),
        "/home/u/my project"
    );
}

#[test]
fn non_file_value_is_left_verbatim() {
    assert_eq!(file_uri_to_path("/plain/path"), "/plain/path");
}

// ─── split_uri ──────────────────────────────────────────────────────

#[test]
fn uri_splits_into_scheme_authority_path() {
    let (scheme, authority, path) =
        split_uri("vscode-remote://dev-container+abc@ssh-remote+xyz/workspaces/app");
    assert_eq!(scheme, "vscode-remote");
    assert_eq!(authority, "dev-container+abc@ssh-remote+xyz");
    assert_eq!(path, "/workspaces/app");
}

// ─── parse_devcontainer_uri ─────────────────────────────────────────

#[test]
fn devcontainer_uri_parses() {
    let hex = hex_encode(&payload("/home/u/proj").to_string());
    let uri = format!("vscode-remote://dev-container+{hex}/workspaces/proj");
    let parsed = parse_devcontainer_uri(&uri).expect("parses dev-container uri");
    assert_eq!(parsed.container_path, "/workspaces/proj");
    assert_eq!(parsed.host_workspace_path, "/home/u/proj");
    assert_eq!(
        parsed.devcontainer_config_path,
        "/home/u/proj/.devcontainer/devcontainer.json"
    );
    assert_eq!(
        parsed.devcontainer_authority,
        format!("dev-container+{hex}")
    );
}

#[test]
fn non_devcontainer_uri_is_none() {
    assert!(parse_devcontainer_uri("file:///home/u/proj").is_none());
    assert!(parse_devcontainer_uri("vscode-remote://ssh-remote+abc/x").is_none());
    // The bare marker has no `+<hex>` authority at all.
    assert!(parse_devcontainer_uri("vscode-remote://dev-container").is_none());
}

#[test]
fn config_file_object_path_is_extracted() {
    let body = serde_json::json!({
        "hostPath": "/home/u/proj",
        "configFile": {"fsPath": "/home/u/proj/.devcontainer/devcontainer.json"},
    });
    let hex = hex_encode(&body.to_string());
    let uri = format!("vscode-remote://dev-container+{hex}/workspaces/proj");
    let parsed = parse_devcontainer_uri(&uri).expect("parses dev-container uri");
    assert_eq!(parsed.host_workspace_path, "/home/u/proj");
    assert_eq!(
        parsed.devcontainer_config_path,
        "/home/u/proj/.devcontainer/devcontainer.json"
    );
}

// ─── decode_remote_authority ────────────────────────────────────────

#[test]
fn ssh_remote_authority_extracts_host() {
    let hex = hex_encode(&serde_json::json!({"hostName": "build.example.test"}).to_string());
    let authority = format!("ssh-remote+{hex}");
    let (full, host) = decode_remote_authority(&authority);
    assert_eq!(full, authority);
    assert_eq!(host, "build.example.test");
}

#[test]
fn plain_authority_becomes_host_verbatim() {
    let (full, host) = decode_remote_authority("localhost");
    assert_eq!(full, "localhost");
    assert_eq!(host, "localhost");
}

#[test]
fn empty_authority_is_empty_pair() {
    assert_eq!(decode_remote_authority(""), (String::new(), String::new()));
}

// ─── entry_matches (filter) ─────────────────────────────────────────

#[test]
fn pattern_tokens_match_case_insensitive_and() {
    let entry = sample_entry();
    assert!(entry_matches(&entry, "byfactory"));
    assert!(entry_matches(&entry, "BYFACTORY api"));
    assert!(entry_matches(&entry, "")); // empty matches all
    assert!(!entry_matches(&entry, "nonexistent"));
    assert!(!entry_matches(&entry, "byfactory nonexistent"));
}

// ─── Field::value / parse ───────────────────────────────────────────

#[test]
fn field_value_selects() {
    let entry = sample_entry();
    assert_eq!(Field::Container.value(&entry), "/workspaces/byfactory-api");
    assert_eq!(Field::Host.value(&entry), "/home/u/projects/byfactory");
    assert_eq!(Field::Remote.value(&entry), "build.example.test");
    assert_eq!(Field::App.value(&entry), "Code");
    assert_eq!(Field::Id.value(&entry), "abc123");
}

#[test]
fn remote_field_falls_back_to_authority_without_host() {
    let mut entry = sample_entry();
    entry.remote_host = String::new();
    assert_eq!(Field::Remote.value(&entry), "ssh-remote+deadbeef");
}

#[test]
fn field_parse_treats_empty_as_none_and_unknown_as_error() {
    assert_eq!(Field::parse(""), Ok(None));
    assert_eq!(Field::parse("Container"), Ok(Some(Field::Container)));
    assert!(Field::parse("bogus").is_err());
}

// ─── AppFilter::parse / matches ─────────────────────────────────────

#[test]
fn app_filter_parses_and_matches() {
    assert_eq!(AppFilter::parse(""), Ok(AppFilter::All));
    assert_eq!(AppFilter::parse("code"), Ok(AppFilter::Code));
    assert!(AppFilter::parse("emacs").is_err());

    assert!(AppFilter::All.matches("Cursor"));
    assert!(AppFilter::Code.matches("Code"));
    assert!(!AppFilter::Code.matches("Cursor"));
    assert!(AppFilter::Insiders.matches("Code - Insiders"));
    assert!(AppFilter::VsCodium.matches("VSCodium"));
}

// ─── norm_uri ───────────────────────────────────────────────────────

#[test]
fn norm_uri_decodes_percent_and_strips_trailing_slash() {
    assert_eq!(
        norm_uri("vscode-remote://x/workspaces/app/"),
        "vscode-remote://x/workspaces/app"
    );
    assert_eq!(norm_uri("a%20b"), "a b");
}

// ─── RankMap ────────────────────────────────────────────────────────

#[test]
fn rank_records_first_value_under_raw_and_normalized_keys() {
    let mut ranks = RankMap::default();
    ranks.add("vscode-remote://x/app/", 3);
    ranks.add("vscode-remote://x/app/", 9); // ignored (first wins)
    assert_eq!(ranks.rank_of("vscode-remote://x/app/"), Some(3));
    assert_eq!(ranks.rank_of("vscode-remote://x/app"), Some(3));
    assert_eq!(ranks.rank_of("vscode-remote://x/other"), None);
}

#[test]
fn rank_lookup_prefers_raw_match_over_normalized_collision() {
    let mut ranks = RankMap::default();
    // `%20` normalizes to the space form, which is also a URI in its own right.
    ranks.add("vscode-remote://x/a%20b", 0);
    ranks.add("vscode-remote://x/a b", 7);
    assert_eq!(ranks.rank_of("vscode-remote://x/a%20b"), Some(0));
    // Sharing one map would answer this with the *other* URI's rank (0).
    assert_eq!(ranks.rank_of("vscode-remote://x/a b"), Some(7));
}

#[test]
fn rank_merge_does_not_overwrite_existing() {
    let mut ranks = RankMap::default();
    ranks.add("vscode-remote://x/a", 1);
    let mut other = RankMap::default();
    other.add("vscode-remote://x/a", 5);
    other.add("vscode-remote://x/b", 6);
    ranks.merge_missing(other);
    assert_eq!(ranks.rank_of("vscode-remote://x/a"), Some(1));
    assert_eq!(ranks.rank_of("vscode-remote://x/b"), Some(6));
}

// ─── sort_entries ───────────────────────────────────────────────────

#[test]
fn sort_orders_open_then_recent_rank_and_unranked_last() {
    let mut a = sample_entry();
    a.storage_id = "a".to_string();
    a.open_rank = Some(0);
    let mut b = sample_entry();
    b.storage_id = "b".to_string();
    b.recent_rank = Some(0);
    let mut c = sample_entry();
    c.storage_id = "c".to_string();
    let mut entries = vec![c, b, a];
    sort_entries(&mut entries);
    let order: Vec<&str> = entries.iter().map(|e| e.storage_id.as_str()).collect();
    assert_eq!(order, vec!["a", "b", "c"]);
}

#[test]
fn equal_ranks_order_newest_mtime_first() {
    let mut old = sample_entry();
    old.storage_id = "old".to_string();
    old.open_rank = Some(1);
    old.recent_rank = Some(1);
    old.mtime = 1000.0;
    let mut fresh = sample_entry();
    fresh.storage_id = "fresh".to_string();
    fresh.open_rank = Some(1);
    fresh.recent_rank = Some(1);
    fresh.mtime = 2000.0;
    let mut entries = vec![old, fresh];
    sort_entries(&mut entries);
    let order: Vec<&str> = entries.iter().map(|e| e.storage_id.as_str()).collect();
    assert_eq!(order, vec!["fresh", "old"]);
}

// ─── to_json ────────────────────────────────────────────────────────

#[test]
fn entry_serializes_to_json() {
    // Compared as a whole object: a renamed, dropped, or added field fails
    // here, which per-field assertions would not catch.
    assert_eq!(
        sample_entry().to_json(),
        serde_json::json!({
            "app": "Code",
            "storage_root": "/home/u/.config/Code/User/workspaceStorage",
            "storage_id": "abc123",
            "storage_path": "/home/u/.config/Code/User/workspaceStorage/abc123",
            "workspace_json": "/home/u/.config/Code/User/workspaceStorage/abc123/workspace.json",
            "workspace_kind": "folder",
            "uri": "vscode-remote://dev-container+abc/workspaces/byfactory-api",
            "container_path": "/workspaces/byfactory-api",
            "host_workspace_path": "/home/u/projects/byfactory",
            "devcontainer_config_path": "/home/u/projects/byfactory/.devcontainer/devcontainer.json",
            "remote_authority": "ssh-remote+deadbeef",
            "remote_host": "build.example.test",
            "devcontainer_authority": "dev-container+abc",
            "decoded": {"workspacePath": "/home/u/projects/byfactory"},
            "mtime": 1000.0,
            "open_rank": serde_json::Value::Null,
            "recent_rank": serde_json::Value::Null,
        })
    );
}

#[test]
fn missing_decoded_payload_is_null_in_json() {
    let mut entry = sample_entry();
    entry.decoded = None;
    assert_eq!(entry.to_json()["decoded"], serde_json::Value::Null);
}

// ─── app_name_from_storage_root ─────────────────────────────────────

#[test]
fn app_name_is_positional_not_path_component_search() {
    // The `Code` home directory must not shadow the real app (`Cursor`).
    assert_eq!(
        app_name_from_storage_root(Path::new("/home/Code/.config/Cursor/User/workspaceStorage")),
        "Cursor"
    );
    assert_eq!(
        app_name_from_storage_root(Path::new("/home/u/.config/Code/User/workspaceStorage")),
        "Code"
    );
    assert_eq!(
        app_name_from_storage_root(Path::new(
            "/home/u/.config/Code - Insiders/User/workspaceStorage"
        )),
        "Code - Insiders"
    );
}

#[test]
fn app_name_uses_unknown_dir_verbatim_and_unknown_without_layout() {
    assert_eq!(
        app_name_from_storage_root(Path::new("/opt/MyEditor/User/workspaceStorage")),
        "MyEditor"
    );
    assert_eq!(
        app_name_from_storage_root(Path::new(WORKSPACE_STORAGE_DIR)),
        UNKNOWN_APP
    );
}

// ─── storage_roots_under ────────────────────────────────────────────

#[test]
fn storage_roots_collect_only_existing_app_dirs() {
    let fixture = Fixture::new();
    let cursor = fixture.storage_root("Cursor");
    let code = fixture.storage_root("Code");
    // `VSCodium` is never created, so it must not appear.
    let roots = storage_roots_under(fixture.base());
    // Ordered by `APP_NAMES`: `Code` precedes `Cursor`.
    assert_eq!(roots, vec![code, cursor]);
}

#[test]
fn no_storage_roots_is_empty_list() {
    let fixture = Fixture::new();
    assert!(storage_roots_under(fixture.base()).is_empty());
}

// ─── workspace_mtime ────────────────────────────────────────────────

#[test]
fn workspace_mtime_picks_newest_state_file() {
    const FUTURE_SECS: u64 = 2_000_000_000;
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let db = dir.join(STATE_VSCDB_FILE);
    std::fs::write(&db, b"not really sqlite").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&db)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(FUTURE_SECS))
        .unwrap();

    // The directory's own mtime is "now", so the future-dated state file wins.
    assert_eq!(workspace_mtime(&dir), FUTURE_SECS as f64);
}

#[test]
fn workspace_mtime_is_zero_for_missing_dir() {
    let fixture = Fixture::new();
    assert_eq!(workspace_mtime(&fixture.base().join("missing")), 0.0);
}

// ─── load_workspace_entry ───────────────────────────────────────────

#[test]
fn workspace_json_reads_devcontainer_entry() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Cursor");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&dir, "folder", &uri);

    let entry = load_workspace_entry(&root, &dir).expect("reads workspace.json entry");
    assert_eq!(entry.app, "Cursor");
    assert_eq!(entry.storage_id, "ws1");
    assert_eq!(entry.workspace_kind, "folder");
    assert_eq!(entry.uri, uri);
    assert_eq!(entry.container_path, "/workspaces/proj");
    assert_eq!(entry.host_workspace_path, "/home/u/proj");
    assert_eq!(
        entry.devcontainer_config_path,
        "/home/u/proj/.devcontainer/devcontainer.json"
    );
    assert_eq!(entry.storage_path, dir.to_string_lossy());
    assert!(entry.mtime > 0.0);
}

#[test]
fn workspace_json_workspace_key_also_reads() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&dir, "workspace", &uri);

    let entry = load_workspace_entry(&root, &dir).expect("reads workspace.json entry");
    assert_eq!(entry.workspace_kind, "workspace");
}

#[test]
fn missing_or_non_devcontainer_workspace_json_is_none() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let empty = Fixture::workspace(&root, "empty");
    assert!(load_workspace_entry(&root, &empty).is_none());

    let local = Fixture::workspace(&root, "local");
    write_workspace_json(&local, "folder", "file:///home/u/proj");
    assert!(load_workspace_entry(&root, &local).is_none());

    let broken = Fixture::workspace(&root, "broken");
    std::fs::write(broken.join(WORKSPACE_JSON_FILE), b"{ not json").unwrap();
    assert!(load_workspace_entry(&root, &broken).is_none());
}

// ─── state_uri_from_db / load_state_entry ───────────────────────────

#[test]
fn state_vscdb_reads_uri_from_debug_selectedroot() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[(KEY_DEBUG_SELECTEDROOT, &uri)],
    );

    assert_eq!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)), Some(uri));
}

#[test]
fn state_vscdb_extracts_uri_embedded_in_history_entries_blob() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    let blob = serde_json::json!([{"editor": {"resource": uri}}]).to_string();
    seed_state_vscdb(&dir.join(STATE_VSCDB_FILE), &[(KEY_HISTORY_ENTRIES, &blob)]);

    assert_eq!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)), Some(uri));
}

#[test]
fn state_vscdb_recovers_uri_from_resource_authority_key() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let authority = format!(
        "{DEV_AUTH_PREFIX}{}",
        hex_encode(&payload("/home/u/proj").to_string())
    );
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[(&format!("{RESOURCE_AUTHORITY_PREFIX}{authority}"), "{}")],
    );

    assert_eq!(
        state_uri_from_db(&dir.join(STATE_VSCDB_FILE)),
        Some(format!("{VSCODE_REMOTE_URI_PREFIX}{authority}"))
    );
}

#[test]
fn state_vscdb_drops_unparseable_candidate_and_uses_next_key() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    let blob = serde_json::json!({"resource": uri}).to_string();
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[
            // Preferred key, but its value has no `+<hex>` authority.
            (KEY_DEBUG_SELECTEDROOT, DEVCONTAINER_URI_MARKER),
            (KEY_HISTORY_ENTRIES, &blob),
        ],
    );

    assert_eq!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)), Some(uri));
}

#[test]
fn state_vscdb_does_not_return_unparseable_candidate() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    // The sole candidate starts with the marker but has no `+<hex>` authority,
    // so it never parses. Returning it unvalidated would hand the caller a URI
    // that is guaranteed to fail — the value must be dropped instead.
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[(KEY_DEBUG_SELECTEDROOT, DEVCONTAINER_URI_MARKER)],
    );

    assert!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)).is_none());
}

#[test]
fn state_vscdb_without_usable_uri_is_none() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[
            // Matches the marker but never parses — must not be returned.
            (KEY_DEBUG_SELECTEDROOT, DEVCONTAINER_URI_MARKER),
            (KEY_HISTORY_ENTRIES, "file:///home/u/proj"),
        ],
    );

    assert!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)).is_none());
}

#[test]
fn non_sqlite_state_file_is_none() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    std::fs::write(dir.join(STATE_VSCDB_FILE), b"definitely not sqlite").unwrap();

    assert!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)).is_none());
    assert!(load_state_entry(&root, &dir).is_none());
}

#[test]
fn state_vscdb_builds_entry() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[(KEY_DEBUG_SELECTEDROOT, &uri)],
    );

    let entry = load_state_entry(&root, &dir).expect("reads state.vscdb entry");
    assert_eq!(entry.workspace_kind, WORKSPACE_KIND_SQLITE);
    assert_eq!(entry.uri, uri);
    assert_eq!(entry.app, "Code");
    assert_eq!(entry.host_workspace_path, "/home/u/proj");
}

#[test]
fn missing_state_vscdb_yields_no_entry() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    assert!(load_state_entry(&root, &dir).is_none());
}

// ─── collect_entries ────────────────────────────────────────────────

#[test]
fn storage_scan_prefers_workspace_json_over_state() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let json_uri = devcontainer_uri(&payload("/home/u/from-json"), "/workspaces/proj");
    let db_uri = devcontainer_uri(&payload("/home/u/from-db"), "/workspaces/proj");
    write_workspace_json(&dir, "folder", &json_uri);
    seed_state_vscdb(
        &dir.join(STATE_VSCDB_FILE),
        &[(KEY_DEBUG_SELECTEDROOT, &db_uri)],
    );

    let entries = collect_entries(&[root]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].uri, json_uri);
    assert_eq!(entries[0].workspace_kind, "folder");
}

#[test]
fn storage_scan_skips_non_devcontainer_workspaces() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&Fixture::workspace(&root, "remote"), "folder", &uri);
    write_workspace_json(
        &Fixture::workspace(&root, "local"),
        "folder",
        "file:///home/u/local",
    );
    Fixture::workspace(&root, "empty");
    std::fs::write(root.join("stray-file"), b"ignored").unwrap();

    let entries = collect_entries(&[root]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].storage_id, "remote");
}

#[test]
fn storage_scan_merges_multiple_app_roots() {
    let fixture = Fixture::new();
    let code = fixture.storage_root("Code");
    let cursor = fixture.storage_root("Cursor");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&Fixture::workspace(&code, "ws1"), "folder", &uri);
    write_workspace_json(&Fixture::workspace(&cursor, "ws2"), "folder", &uri);

    let entries = collect_entries(&[code, cursor, fixture.base().join("missing")]);
    let mut apps: Vec<&str> = entries.iter().map(|e| e.app.as_str()).collect();
    apps.sort_unstable();
    assert_eq!(apps, vec!["Code", "Cursor"]);
}

// ─── rank loading ───────────────────────────────────────────────────

#[test]
fn storage_json_reads_open_and_recent_ranks() {
    let fixture = Fixture::new();
    let global = fixture.global_storage("Code");
    let open_uri = devcontainer_uri(&payload("/home/u/open"), "/workspaces/open");
    let recent_uri = devcontainer_uri(&payload("/home/u/recent"), "/workspaces/recent");
    let body = serde_json::json!({
        KEY_BACKUP_WORKSPACES: {
            KEY_BACKUP_FOLDERS: [{KEY_FOLDER_URI: open_uri}],
            KEY_BACKUP_WORKSPACES_LIST: [{KEY_WORKSPACE_URI: recent_uri}],
        },
        KEY_MENUBAR_DATA: {
            "menus": [{KEY_URI: {KEY_EXTERNAL: recent_uri}}],
        },
    });
    std::fs::write(global.join(STORAGE_JSON_FILE), body.to_string().as_bytes()).unwrap();

    let (open, recent) = load_storage_json_ranks(&fixture.user_dir("Code"));
    assert_eq!(open.rank_of(&open_uri), Some(0));
    assert_eq!(open.rank_of(&recent_uri), Some(1));
    assert_eq!(recent.rank_of(&recent_uri), Some(0));
    assert_eq!(recent.rank_of(&open_uri), None);
}

#[test]
fn missing_or_broken_storage_json_yields_empty_ranks() {
    let fixture = Fixture::new();
    let user_dir = fixture.user_dir("Code");
    let (open, recent) = load_storage_json_ranks(&user_dir);
    assert_eq!(open.rank_of("vscode-remote://x/a"), None);
    assert_eq!(recent.rank_of("vscode-remote://x/a"), None);

    let global = fixture.global_storage("Code");
    std::fs::write(global.join(STORAGE_JSON_FILE), b"{ not json").unwrap();
    let (open, _) = load_storage_json_ranks(&user_dir);
    assert_eq!(open.rank_of("vscode-remote://x/a"), None);
}

#[test]
fn global_state_vscdb_reads_recent_ranks_in_list_order() {
    let fixture = Fixture::new();
    let global = fixture.global_storage("Code");
    let first = devcontainer_uri(&payload("/home/u/first"), "/workspaces/first");
    let second = devcontainer_uri(&payload("/home/u/second"), "/workspaces/second");
    let body = serde_json::json!({
        KEY_ENTRIES: [
            {KEY_FOLDER_URI: first},
            {KEY_WORKSPACE_URI: second},
        ],
    });
    seed_state_vscdb(
        &global.join(STATE_VSCDB_FILE),
        &[(KEY_HISTORY_RECENT, &body.to_string())],
    );

    let ranks = load_sqlite_recent_ranks(&fixture.user_dir("Code"));
    assert_eq!(ranks.rank_of(&first), Some(0));
    assert_eq!(ranks.rank_of(&second), Some(1));
}

#[test]
fn missing_global_state_vscdb_yields_empty_recent_ranks() {
    let fixture = Fixture::new();
    fixture.global_storage("Code");
    let ranks = load_sqlite_recent_ranks(&fixture.user_dir("Code"));
    assert_eq!(ranks.rank_of("vscode-remote://x/a"), None);
}

// ─── apply_ranks ────────────────────────────────────────────────────

#[test]
fn apply_ranks_attaches_ranks_matching_entry_storage_root() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let global = fixture.global_storage("Code");
    let open_uri = devcontainer_uri(&payload("/home/u/open"), "/workspaces/open");
    let sqlite_uri = devcontainer_uri(&payload("/home/u/sqlite"), "/workspaces/sqlite");

    write_workspace_json(&Fixture::workspace(&root, "open"), "folder", &open_uri);
    write_workspace_json(&Fixture::workspace(&root, "sqlite"), "folder", &sqlite_uri);

    let storage_body = serde_json::json!({
        KEY_BACKUP_WORKSPACES: { KEY_BACKUP_FOLDERS: [{KEY_FOLDER_URI: open_uri}] },
    });
    std::fs::write(
        global.join(STORAGE_JSON_FILE),
        storage_body.to_string().as_bytes(),
    )
    .unwrap();
    // Only the SQLite recent list knows about `sqlite_uri`, so this also proves
    // the two recent sources are merged.
    let recent_body = serde_json::json!({ KEY_ENTRIES: [{KEY_FOLDER_URI: sqlite_uri}] });
    seed_state_vscdb(
        &global.join(STATE_VSCDB_FILE),
        &[(KEY_HISTORY_RECENT, &recent_body.to_string())],
    );

    let mut entries = collect_entries(&[root]);
    apply_ranks(&mut entries);
    entries.sort_by(|a, b| a.storage_id.cmp(&b.storage_id));

    assert_eq!(entries[0].storage_id, "open");
    assert_eq!(entries[0].open_rank, Some(0));
    assert_eq!(entries[0].recent_rank, None);
    assert_eq!(entries[1].storage_id, "sqlite");
    assert_eq!(entries[1].open_rank, None);
    assert_eq!(entries[1].recent_rank, Some(0));
}

#[test]
fn apply_ranks_without_rank_sources_leaves_all_none() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&Fixture::workspace(&root, "ws1"), "folder", &uri);

    let mut entries = collect_entries(&[root]);
    apply_ranks(&mut entries);
    assert_eq!(entries[0].open_rank, None);
    assert_eq!(entries[0].recent_rank, None);
}

// ─── input validation (no filesystem) ───────────────────────────────

#[test]
fn lookup_rejects_empty_pattern() {
    assert_eq!(
        devcontainer_lookup("  ", "", ""),
        Err("pattern must not be empty".to_string())
    );
}
