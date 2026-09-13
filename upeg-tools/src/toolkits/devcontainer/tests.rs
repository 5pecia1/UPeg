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
fn hex를_바이트로_디코드한다() {
    assert_eq!(decode_hex("00ff41"), Some(vec![0x00, 0xff, 0x41]));
    assert_eq!(decode_hex(""), Some(Vec::new()));
}

#[test]
fn hex_디코드는_홀수_길이를_잘라내지_않고_거부한다() {
    // Truncating to `[0xab]` would silently accept corrupt input.
    assert_eq!(decode_hex("abc"), None);
    assert_eq!(decode_hex("zz"), None);
}

#[test]
fn hex_json_객체를_디코드한다() {
    // hex of `{"a":1}`
    let hex = "7b2261223a317d";
    let decoded = decode_hex_json(hex).expect("decodes object");
    assert_eq!(decoded, serde_json::json!({"a": 1}));
}

#[test]
fn hex_json은_홀수_길이나_비hex를_거부한다() {
    assert!(decode_hex_json("abc").is_none());
    assert!(decode_hex_json("zz").is_none());
    assert!(decode_hex_json("").is_none());
}

#[test]
fn hex_json은_객체가_아니면_거부한다() {
    // hex of `123` (a bare number, not an object)
    assert!(decode_hex_json("313233").is_none());
}

// ─── file_uri_to_path ───────────────────────────────────────────────

#[test]
fn file_uri를_경로로_변환한다() {
    assert_eq!(
        file_uri_to_path("file:///home/u/my%20project"),
        "/home/u/my project"
    );
}

#[test]
fn file가_아닌_값은_그대로_둔다() {
    assert_eq!(file_uri_to_path("/plain/path"), "/plain/path");
}

// ─── split_uri ──────────────────────────────────────────────────────

#[test]
fn uri를_스킴_권한_경로로_분리한다() {
    let (scheme, authority, path) =
        split_uri("vscode-remote://dev-container+abc@ssh-remote+xyz/workspaces/app");
    assert_eq!(scheme, "vscode-remote");
    assert_eq!(authority, "dev-container+abc@ssh-remote+xyz");
    assert_eq!(path, "/workspaces/app");
}

// ─── parse_devcontainer_uri ─────────────────────────────────────────

#[test]
fn devcontainer_uri를_파싱한다() {
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
fn devcontainer가_아닌_uri는_none이다() {
    assert!(parse_devcontainer_uri("file:///home/u/proj").is_none());
    assert!(parse_devcontainer_uri("vscode-remote://ssh-remote+abc/x").is_none());
    // The bare marker has no `+<hex>` authority at all.
    assert!(parse_devcontainer_uri("vscode-remote://dev-container").is_none());
}

#[test]
fn config_file_객체에서_경로를_추출한다() {
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
fn ssh_remote_권한에서_호스트를_추출한다() {
    let hex = hex_encode(&serde_json::json!({"hostName": "build.example.test"}).to_string());
    let authority = format!("ssh-remote+{hex}");
    let (full, host) = decode_remote_authority(&authority);
    assert_eq!(full, authority);
    assert_eq!(host, "build.example.test");
}

#[test]
fn 평범한_권한은_그대로_호스트가_된다() {
    let (full, host) = decode_remote_authority("localhost");
    assert_eq!(full, "localhost");
    assert_eq!(host, "localhost");
}

#[test]
fn 빈_권한은_빈_쌍이다() {
    assert_eq!(decode_remote_authority(""), (String::new(), String::new()));
}

// ─── entry_matches (filter) ─────────────────────────────────────────

#[test]
fn 패턴_토큰은_대소문자_무시하고_and로_매칭한다() {
    let entry = sample_entry();
    assert!(entry_matches(&entry, "byfactory"));
    assert!(entry_matches(&entry, "BYFACTORY api"));
    assert!(entry_matches(&entry, "")); // empty matches all
    assert!(!entry_matches(&entry, "nonexistent"));
    assert!(!entry_matches(&entry, "byfactory nonexistent"));
}

// ─── Field::value / parse ───────────────────────────────────────────

#[test]
fn 필드_값을_선택한다() {
    let entry = sample_entry();
    assert_eq!(Field::Container.value(&entry), "/workspaces/byfactory-api");
    assert_eq!(Field::Host.value(&entry), "/home/u/projects/byfactory");
    assert_eq!(Field::Remote.value(&entry), "build.example.test");
    assert_eq!(Field::App.value(&entry), "Code");
    assert_eq!(Field::Id.value(&entry), "abc123");
}

#[test]
fn remote_필드는_호스트가_없으면_권한으로_대체한다() {
    let mut entry = sample_entry();
    entry.remote_host = String::new();
    assert_eq!(Field::Remote.value(&entry), "ssh-remote+deadbeef");
}

#[test]
fn 필드_파싱은_빈값을_none으로_알수없는값을_에러로_처리한다() {
    assert_eq!(Field::parse(""), Ok(None));
    assert_eq!(Field::parse("Container"), Ok(Some(Field::Container)));
    assert!(Field::parse("bogus").is_err());
}

// ─── AppFilter::parse / matches ─────────────────────────────────────

#[test]
fn 앱_필터를_파싱하고_매칭한다() {
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
fn uri_정규화는_퍼센트를_디코드하고_후행_슬래시를_제거한다() {
    assert_eq!(
        norm_uri("vscode-remote://x/workspaces/app/"),
        "vscode-remote://x/workspaces/app"
    );
    assert_eq!(norm_uri("a%20b"), "a b");
}

// ─── RankMap ────────────────────────────────────────────────────────

#[test]
fn 랭크는_원본과_정규화_키_모두에_최초값으로_기록된다() {
    let mut ranks = RankMap::default();
    ranks.add("vscode-remote://x/app/", 3);
    ranks.add("vscode-remote://x/app/", 9); // ignored (first wins)
    assert_eq!(ranks.rank_of("vscode-remote://x/app/"), Some(3));
    assert_eq!(ranks.rank_of("vscode-remote://x/app"), Some(3));
    assert_eq!(ranks.rank_of("vscode-remote://x/other"), None);
}

#[test]
fn 랭크_조회는_정규화_충돌보다_원본_일치를_우선한다() {
    let mut ranks = RankMap::default();
    // `%20` normalizes to the space form, which is also a URI in its own right.
    ranks.add("vscode-remote://x/a%20b", 0);
    ranks.add("vscode-remote://x/a b", 7);
    assert_eq!(ranks.rank_of("vscode-remote://x/a%20b"), Some(0));
    // Sharing one map would answer this with the *other* URI's rank (0).
    assert_eq!(ranks.rank_of("vscode-remote://x/a b"), Some(7));
}

#[test]
fn 랭크_병합은_기존_값을_덮어쓰지_않는다() {
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
fn 정렬은_오픈_랭크_다음_최근_랭크_순이며_랭크가_없으면_뒤로_밀린다() {
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
fn 랭크가_같으면_mtime이_최신인_엔트리가_앞선다() {
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
fn 엔트리를_json으로_직렬화한다() {
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
fn 디코드된_페이로드가_없으면_json에서_null이다() {
    let mut entry = sample_entry();
    entry.decoded = None;
    assert_eq!(entry.to_json()["decoded"], serde_json::Value::Null);
}

// ─── app_name_from_storage_root ─────────────────────────────────────

#[test]
fn 앱_이름은_경로_성분_검색이_아니라_위치로_정해진다() {
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
fn 앱_이름은_알려지지_않은_디렉터리를_그대로_쓰고_레이아웃이_없으면_unknown이다() {
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
fn 스토리지_루트는_실제로_존재하는_앱_디렉터리만_모은다() {
    let fixture = Fixture::new();
    let cursor = fixture.storage_root("Cursor");
    let code = fixture.storage_root("Code");
    // `VSCodium` is never created, so it must not appear.
    let roots = storage_roots_under(fixture.base());
    // Ordered by `APP_NAMES`: `Code` precedes `Cursor`.
    assert_eq!(roots, vec![code, cursor]);
}

#[test]
fn 스토리지_루트가_하나도_없으면_빈_목록이다() {
    let fixture = Fixture::new();
    assert!(storage_roots_under(fixture.base()).is_empty());
}

// ─── workspace_mtime ────────────────────────────────────────────────

#[test]
fn workspace_mtime은_상태_파일_중_가장_최신을_고른다() {
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
fn workspace_mtime은_존재하지_않는_디렉터리에_대해_0이다() {
    let fixture = Fixture::new();
    assert_eq!(workspace_mtime(&fixture.base().join("missing")), 0.0);
}

// ─── load_workspace_entry ───────────────────────────────────────────

#[test]
fn workspace_json에서_devcontainer_엔트리를_읽는다() {
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
fn workspace_json의_workspace_키도_읽는다() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    write_workspace_json(&dir, "workspace", &uri);

    let entry = load_workspace_entry(&root, &dir).expect("reads workspace.json entry");
    assert_eq!(entry.workspace_kind, "workspace");
}

#[test]
fn workspace_json이_없거나_devcontainer가_아니면_none이다() {
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
fn state_vscdb의_debug_selectedroot에서_uri를_읽는다() {
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
fn state_vscdb는_history_entries_블롭에_박힌_uri를_뽑아낸다() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    let uri = devcontainer_uri(&payload("/home/u/proj"), "/workspaces/proj");
    let blob = serde_json::json!([{"editor": {"resource": uri}}]).to_string();
    seed_state_vscdb(&dir.join(STATE_VSCDB_FILE), &[(KEY_HISTORY_ENTRIES, &blob)]);

    assert_eq!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)), Some(uri));
}

#[test]
fn state_vscdb는_resource_authority_키에서_uri를_복원한다() {
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
fn state_vscdb는_파싱되지_않는_후보를_버리고_다음_키를_쓴다() {
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
fn state_vscdb는_파싱되지_않는_후보를_돌려주지_않는다() {
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
fn state_vscdb에_쓸만한_uri가_없으면_none이다() {
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
fn sqlite가_아닌_state_파일은_none이다() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    std::fs::write(dir.join(STATE_VSCDB_FILE), b"definitely not sqlite").unwrap();

    assert!(state_uri_from_db(&dir.join(STATE_VSCDB_FILE)).is_none());
    assert!(load_state_entry(&root, &dir).is_none());
}

#[test]
fn state_vscdb에서_엔트리를_만든다() {
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
fn state_vscdb가_없으면_엔트리가_없다() {
    let fixture = Fixture::new();
    let root = fixture.storage_root("Code");
    let dir = Fixture::workspace(&root, "ws1");
    assert!(load_state_entry(&root, &dir).is_none());
}

// ─── collect_entries ────────────────────────────────────────────────

#[test]
fn 스토리지_스캔은_workspace_json을_state보다_우선한다() {
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
fn 스토리지_스캔은_devcontainer가_아닌_워크스페이스를_건너뛴다() {
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
fn 스토리지_스캔은_여러_앱_루트를_합친다() {
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
fn storage_json에서_오픈_랭크와_최근_랭크를_읽는다() {
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
fn storage_json이_없거나_깨졌으면_랭크가_비어있다() {
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
fn global_state_vscdb에서_최근_랭크를_리스트_순서대로_읽는다() {
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
fn global_state_vscdb가_없으면_최근_랭크가_비어있다() {
    let fixture = Fixture::new();
    fixture.global_storage("Code");
    let ranks = load_sqlite_recent_ranks(&fixture.user_dir("Code"));
    assert_eq!(ranks.rank_of("vscode-remote://x/a"), None);
}

// ─── apply_ranks ────────────────────────────────────────────────────

#[test]
fn apply_ranks는_엔트리의_스토리지_루트에_맞는_랭크를_붙인다() {
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
fn apply_ranks는_랭크_소스가_없으면_모두_none으로_둔다() {
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
fn 룩업은_빈_패턴을_거부한다() {
    assert_eq!(
        devcontainer_lookup("  ", "", ""),
        Err("pattern must not be empty".to_string())
    );
}
