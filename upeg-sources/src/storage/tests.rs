use super::*;
use rusqlite::Connection;

/// These tests must not depend on whatever `UPEG_*` overrides happen to be
/// set in the ambient process environment (e.g. a wrapper script that
/// exports `UPEG_TOOLKITS_DIR`/`UPEG_WASM_DIR`/`UPEG_MCP_IMPORTS_DIR` before
/// running `cargo test`): fixtures build their own source/target trees and
/// must see a plan's real `toolkits`/`wasm`/`mcp-imports` contents
/// regardless. These local definitions shadow the `plan`/`apply`/
/// `apply_inner` names this module's `use super::*` brings in, routing every
/// call below through an explicit, empty override set instead of
/// `storage::bindings_from_lookup(&env_lookup)`'s real-env default.
fn no_overrides(_: &str) -> Option<PathBuf> {
    None
}

fn plan(source: &Path, target: &Path, ecosystem_prefix: Option<&Path>) -> Result<Plan> {
    super::plan_with_lookup(source, target, ecosystem_prefix, &no_overrides)
}

fn apply_inner(plan: &Plan, yes: bool, quiesced: bool, stop: Option<&str>) -> Result<Value> {
    super::apply_inner(plan, yes, quiesced, stop, &no_overrides)
}

fn apply(plan: &Plan, yes: bool, quiesced: bool) -> Result<Value> {
    apply_inner(plan, yes, quiesced, None)
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("upeg-storage-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("old/toolkits")).unwrap();
        fs::write(root.join("old/.upeg-tweaks.json"), b"{\"theme\":\"dark\"}").unwrap();
        fs::write(
            root.join("old/credentials.json"),
            b"{\"sentinel\":\"private\"}",
        )
        .unwrap();
        fs::write(
            root.join("old/toolkits/local.toml"),
            b"literal-toolkit-bytes",
        )
        .unwrap();
        fs::write(root.join("old/unknown.txt"), b"never move me").unwrap();
        Self(root)
    }
    fn source(&self) -> PathBuf {
        self.0.join("old")
    }
    fn target(&self) -> PathBuf {
        self.0.join("new")
    }
    fn plan(&self) -> Plan {
        plan(&self.source(), &self.target(), None).unwrap()
    }
    fn database(&self) -> Connection {
        let connection = Connection::open(self.source().join("upeg.db")).unwrap();
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
            PRAGMA user_version=37;
            CREATE TABLE meta(key TEXT PRIMARY KEY,value TEXT);
            INSERT INTO meta VALUES('device_id','literal-device-117'),('rev','92');
            CREATE TABLE boards(namespace TEXT PRIMARY KEY,payload BLOB);
            INSERT INTO boards VALUES('project:A',X'414243'),('project:B',X'00FF');
            CREATE TABLE sentinel(value TEXT);
            INSERT INTO sentinel VALUES('committed-in-wal');",
            )
            .unwrap();
        connection
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn storage_plan_is_readonly_and_leaves_unknown_files_outside_entries() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    assert!(!fixture.target().exists());
    assert!(!fixture.source().join(STORAGE_LOCK).exists());
    assert!(plan.unclassified.contains(&PathBuf::from("unknown.txt")));
    assert!(
        plan.entries
            .iter()
            .all(|entry| entry.source != Path::new("unknown.txt"))
    );
    assert!(
        !String::from_utf8(canonical(&plan).unwrap())
            .unwrap()
            .contains("private")
    );
}

#[test]
fn storage_sqlite_backup_preserves_literal_wal_schema_device_and_project_namespaces() {
    let fixture = Fixture::new();
    let writer = fixture.database();
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    let target = Connection::open(fixture.target().join("data/upeg.db")).unwrap();
    assert_eq!(
        target
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        37
    );
    assert_eq!(
        target
            .query_row("SELECT value FROM sentinel", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "committed-in-wal"
    );
    assert_eq!(
        target
            .query_row("SELECT value FROM meta WHERE key='device_id'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
        "literal-device-117"
    );
    assert_eq!(
        target
            .query_row(
                "SELECT hex(payload) FROM boards WHERE namespace='project:B'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "00FF"
    );
    assert_eq!(
        target
            .query_row(
                "SELECT hex(payload) FROM boards WHERE namespace='project:A'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "414243"
    );
    assert_eq!(
        fs::read(fixture.source().join("unknown.txt")).unwrap(),
        b"never move me"
    );
    assert!(!fixture.target().join("unknown.txt").exists());
    verify(&fixture.target()).unwrap();
    drop(target);
    drop(writer);
    rollback(&fixture.target(), true, true).unwrap();
    assert!(fixture.target().join("data/upeg.db").exists());
    assert!(fixture.target().join(MIGRATION_PENDING).exists());
    assert!(!fixture.source().join(STORAGE_MARKER).exists());
}

#[test]
fn storage_same_root_retains_flat_files_and_supports_immediate_rollback() {
    let fixture = Fixture::new();
    let plan = plan(&fixture.source(), &fixture.source(), None).unwrap();
    apply(&plan, true, true).unwrap();
    assert_eq!(
        fs::read(fixture.source().join("config/credentials.json")).unwrap(),
        b"{\"sentinel\":\"private\"}"
    );
    rollback(&fixture.source(), true, true).unwrap();
    assert!(!fixture.source().join(STORAGE_MARKER).exists());
    assert!(!fixture.source().join("config").exists());
    assert_eq!(
        fs::read(fixture.source().join("credentials.json")).unwrap(),
        b"{\"sentinel\":\"private\"}"
    );
    assert!(
        journal_dir(&plan)
            .join("inactive/config/credentials.json")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn storage_migrates_read_only_toolkit_directories_and_cleans_staging() {
    for same_root in [false, true] {
        let fixture = Fixture::new();
        let source_dir = fixture.source().join("toolkits/read-only");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("tool.toml"), b"read-only toolkit").unwrap();
        set_mode(&source_dir, 0o555).unwrap();
        let target = if same_root {
            fixture.source()
        } else {
            fixture.target()
        };
        let plan = plan(&fixture.source(), &target, None).unwrap();
        assert!(apply_inner(&plan, true, true, Some("copy")).is_err());
        assert_eq!(mode(&source_dir).unwrap(), 0o555);
        assert_eq!(
            fs::read(source_dir.join("tool.toml")).unwrap(),
            b"read-only toolkit"
        );
        assert!(!target.join(STORAGE_MARKER).exists());
        if let Err(error) = apply(&plan, true, true) {
            // Restore permissions before the fixture removes a failed run.
            let _ = set_mode(&source_dir, 0o700);
            let _ = set_mode(
                &journal_dir(&plan).join("staging/data/toolkits/read-only"),
                0o700,
            );
            let _ = set_mode(&target.join("data/toolkits/read-only"), 0o700);
            panic!("read-only directory migration failed: {error}");
        }
        let copied = target.join("data/toolkits/read-only");
        assert_eq!(
            fs::read(copied.join("tool.toml")).unwrap(),
            b"read-only toolkit"
        );
        assert_eq!(mode(&source_dir).unwrap(), 0o555);
        assert_eq!(mode(&copied).unwrap(), 0o555);
        assert!(!journal_dir(&plan).join("staging").exists());
        verify(&target).unwrap();
        rollback(&target, true, true).unwrap();
        assert_eq!(mode(&source_dir).unwrap(), 0o555);
        assert_eq!(
            fs::read(source_dir.join("tool.toml")).unwrap(),
            b"read-only toolkit"
        );
        set_mode(&source_dir, 0o700).unwrap();
        let preserved_copy = if same_root {
            journal_dir(&plan).join("inactive/data/toolkits/read-only")
        } else {
            copied
        };
        set_mode(&preserved_copy, 0o700).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn storage_resume_refuses_changed_destination_directory_mode() {
    let fixture = Fixture::new();
    let source_dir = fixture.source().join("toolkits/read-only");
    fs::create_dir(&source_dir).unwrap();
    fs::write(source_dir.join("tool.toml"), b"read-only toolkit").unwrap();
    set_mode(&source_dir, 0o555).unwrap();
    let plan = fixture.plan();
    assert!(apply_inner(&plan, true, true, Some("copy")).is_err());
    let destination = fixture.target().join("data/toolkits/read-only");
    set_mode(&destination, 0o755).unwrap();
    assert!(apply(&plan, true, true).is_err());
    assert_eq!(mode(&destination).unwrap(), 0o755);
    assert_eq!(mode(&source_dir).unwrap(), 0o555);
    assert_eq!(
        fs::read(source_dir.join("tool.toml")).unwrap(),
        b"read-only toolkit"
    );
    set_mode(&destination, 0o700).unwrap();
    set_mode(&source_dir, 0o700).unwrap();
}

#[test]
fn storage_changed_input_tampered_plan_collision_and_runtime_are_refused() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let mut tampered = plan.clone();
    tampered.entries[0].destination = "../../outside".into();
    tampered.plan_hash = plan_digest(&tampered).unwrap();
    assert!(apply(&tampered, true, true).is_err());
    fs::write(fixture.source().join("credentials.json"), b"changed").unwrap();
    assert!(apply(&plan, true, true).is_err());
    let fresh = fixture.plan();
    fs::create_dir_all(fixture.target()).unwrap();
    fs::write(
        fixture.target().join("unknown"),
        b"same or different is irrelevant",
    )
    .unwrap();
    assert!(apply(&fresh, true, true).is_err());
    fs::remove_file(fixture.target().join("unknown")).unwrap();
    fs::write(fixture.source().join("server.json"), b"{}").unwrap();
    assert!(apply(&fresh, true, true).is_err());
}

#[test]
fn storage_live_database_writer_and_shared_lease_refuse_cutover() {
    let fixture = Fixture::new();
    let connection = fixture.database();
    let plan = fixture.plan();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(apply(&plan, true, true).is_err());
    assert!(!fixture.target().join(STORAGE_MARKER).exists());
    connection.execute_batch("ROLLBACK").unwrap();
    let lease = paths::StorageLease::for_location(&paths::StorageLocation::new(
        fixture.source(),
        paths::StorageLayout::LegacyV1,
    ))
    .unwrap();
    assert!(apply(&plan, true, true).is_err());
    drop(lease);
    apply(&plan, true, true).unwrap();
    assert!(crate::store::Store::open_at(&fixture.source().join("upeg.db")).is_err());
}

#[test]
fn storage_resumes_after_copy_and_before_marker_without_selecting_partial_target() {
    for phase in ["copy", "marker"] {
        let fixture = Fixture::new();
        let plan = fixture.plan();
        assert!(apply_inner(&plan, true, true, Some(phase)).is_err());
        let result = paths::resolve_storage_with_lookup(
            |key| (key == "UPEG_HOME").then(|| fixture.target()),
            paths::Platform::Unix,
            paths::inspect_storage,
        );
        assert!(matches!(result, Err(StoragePathError::Pending(_))));
        apply(&plan, true, true).unwrap();
        apply(&plan, true, true).unwrap();
        verify(&fixture.target()).unwrap();
    }
}

#[test]
fn storage_rollback_never_overwrites_new_destination_or_source_writes() {
    for change_source in [false, true] {
        let fixture = Fixture::new();
        let plan = fixture.plan();
        apply(&plan, true, true).unwrap();
        let changed = if change_source {
            fixture.source().join("credentials.json")
        } else {
            fixture.target().join("config/credentials.json")
        };
        fs::write(&changed, b"new user write").unwrap();
        assert!(rollback(&fixture.target(), true, true).is_err());
        assert_eq!(fs::read(changed).unwrap(), b"new user write");
        assert!(fixture.target().join(STORAGE_MARKER).exists());
    }
}

#[test]
fn storage_rollback_refuses_new_database_rows_and_marker_corruption() {
    let fixture = Fixture::new();
    let _connection = fixture.database();
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    let target = Connection::open(fixture.target().join("data/upeg.db")).unwrap();
    target
        .execute("INSERT INTO sentinel VALUES ('new-after-cutover')", [])
        .unwrap();
    assert!(rollback(&fixture.target(), true, true).is_err());
    fs::write(fixture.target().join(STORAGE_MARKER), b"{broken").unwrap();
    assert!(verify(&fixture.target()).is_err());
}

#[test]
fn storage_closed_wal_planning_does_not_create_sidecars_and_unknown_edits_block_rollback() {
    let fixture = Fixture::new();
    drop(fixture.database());
    let before = fs::read(fixture.source().join("upeg.db")).unwrap();
    let plan = fixture.plan();
    assert!(!fixture.source().join("upeg.db-wal").exists());
    assert!(!fixture.source().join("upeg.db-shm").exists());
    assert_eq!(fs::read(fixture.source().join("upeg.db")).unwrap(), before);
    apply(&plan, true, true).unwrap();
    fs::write(
        fixture.source().join("unknown.txt"),
        b"new unknown user bytes",
    )
    .unwrap();
    assert!(rollback(&fixture.target(), true, true).is_err());
}

#[test]
fn storage_activation_and_same_root_rollback_interruption_are_recoverable() {
    let fixture = Fixture::new();
    let plan = plan(&fixture.source(), &fixture.source(), None).unwrap();
    assert!(apply_inner(&plan, true, true, Some("marker")).is_err());
    atomic_write(
        &plan.target_root.join(STORAGE_MARKER),
        &serde_json::to_vec(&split_marker(&plan)).unwrap(),
        0o600,
    )
    .unwrap();
    apply(&plan, true, true).unwrap();
    let mut journal = verify::active_journal(&plan.target_root).unwrap();
    journal.phase = "rollback_pending".into();
    save_journal(&journal).unwrap();
    pending(&plan.target_root, &plan).unwrap();
    fs::remove_file(plan.target_root.join(STORAGE_MARKER)).unwrap();
    let inactive = journal_dir(&plan).join("inactive");
    private_dir(&inactive).unwrap();
    fs::rename(plan.target_root.join("config"), inactive.join("config")).unwrap();
    rollback(&plan.target_root, true, true).unwrap();
    assert_eq!(
        fs::read(plan.source_root.join("credentials.json")).unwrap(),
        b"{\"sentinel\":\"private\"}"
    );
    assert!(inactive.join("config/credentials.json").exists());
}

#[test]
fn storage_missing_marker_or_migrated_data_never_reopens_retained_flat_database() {
    let fixture = Fixture::new();
    drop(fixture.database());
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    fs::remove_file(plan.target_root.join("data/upeg.db")).unwrap();
    assert!(crate::store::Store::open_at(&plan.target_root.join("data/upeg.db")).is_err());
    assert!(!plan.target_root.join("data/upeg.db").exists());
    fs::remove_file(plan.target_root.join(STORAGE_MARKER)).unwrap();
    assert!(
        paths::resolve_storage_with_lookup(
            |name| (name == "UPEG_HOME").then(|| plan.target_root.clone()),
            paths::Platform::Unix,
            paths::inspect_storage
        )
        .is_err()
    );
    assert!(plan.source_root.join("upeg.db").exists());
}

#[cfg(unix)]
#[test]
fn storage_symlink_ancestors_and_nested_toolkit_symlinks_fail_closed() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.source(), fixture.0.join("link")).unwrap();
    assert!(plan(&fixture.0.join("link"), &fixture.target(), None).is_err());
    std::os::unix::fs::symlink(
        fixture.source().join("credentials.json"),
        fixture.source().join("toolkits/link"),
    )
    .unwrap();
    assert!(plan(&fixture.source(), &fixture.target(), None).is_err());
}

#[cfg(unix)]
#[test]
fn storage_ecosystem_receipt_recovers_and_changes_only_owned_path_keys() {
    let fixture = Fixture::new();
    let toolkit = fixture.source().join("toolkits/ecosystem.toml");
    fs::write(&toolkit, b"literal ecosystem toolkit").unwrap();
    assert!(plan(&fixture.source(), &fixture.target(), None).is_err());
    let prefix = fixture.0.join("prefix");
    fs::create_dir_all(prefix.join("share/ecosystem")).unwrap();
    let path = prefix.join("share/ecosystem/install.json");
    let before = json!({"managed_by":"ecosystem-installer-v3","prefix":prefix,"connections":["upeg","claude"],
        "upeg_toolkit":toolkit,"files":{toolkit.to_str().unwrap():{"sha256":digest(b"literal ecosystem toolkit"),"mode":mode(&toolkit).unwrap()},"unrelated":{"sha256":"unchanged","mode":493}},"unknown":{"retain":true}});
    let bytes = serde_json::to_vec(&before).unwrap();
    fs::write(&path, &bytes).unwrap();
    let plan = plan(&fixture.source(), &fixture.target(), Some(&prefix)).unwrap();
    assert!(apply_inner(&plan, true, true, Some("receipt")).is_err());
    apply(&plan, true, true).unwrap();
    let mut expected = before;
    let fingerprint = expected["files"]
        .as_object_mut()
        .unwrap()
        .remove(toolkit.to_str().unwrap())
        .unwrap();
    let destination = fixture.target().join("data/toolkits/ecosystem.toml");
    expected["upeg_toolkit"] = json!(destination);
    expected["files"][destination.to_str().unwrap()] = fingerprint;
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(fs::read(destination).unwrap(), b"literal ecosystem toolkit");
    rollback(&fixture.target(), true, true).unwrap();
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn storage_ecosystem_receipt_with_unrecognized_managed_by_is_refused_without_changes() {
    let fixture = Fixture::new();
    let toolkit = fixture.source().join("toolkits/ecosystem.toml");
    fs::write(&toolkit, b"literal ecosystem toolkit").unwrap();
    let prefix = fixture.0.join("prefix");
    fs::create_dir_all(prefix.join("share/ecosystem")).unwrap();
    let path = prefix.join("share/ecosystem/install.json");
    // Retired and unrelated identities must all be refused; only the
    // current "ecosystem-installer-v3" is accepted.
    for managed_by in [
        "ecosystem-installer-v1",
        "ecosystem-installer-v2",
        "ecosystem-installer-v4",
        "other",
    ] {
        let before = json!({"managed_by":managed_by,"prefix":prefix,"connections":["upeg","claude"],
            "upeg_toolkit":toolkit,"files":{toolkit.to_str().unwrap():{"sha256":digest(b"literal ecosystem toolkit"),"mode":mode(&toolkit).unwrap()}}});
        let bytes = serde_json::to_vec(&before).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(plan(&fixture.source(), &fixture.target(), Some(&prefix)).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    assert_eq!(fs::read(&toolkit).unwrap(), b"literal ecosystem toolkit");
}

#[test]
fn storage_rollback_aborts_pre_activation_same_root_migration_without_touching_source() {
    let fixture = Fixture::new();
    let interrupted = plan(&fixture.source(), &fixture.source(), None).unwrap();
    assert!(apply_inner(&interrupted, true, true, Some("copy")).is_err());
    let copied = fs::read(interrupted.target_root.join("config/credentials.json")).unwrap();
    let result = rollback(&interrupted.target_root, true, true).unwrap();
    assert_eq!(result["status"], "aborted");
    // The published same-root copies were never activated: they are stashed
    // for inspection, not left in place and not deleted.
    assert!(!interrupted.target_root.join("config").exists());
    assert!(!interrupted.target_root.join(MIGRATION_PENDING).exists());
    let aborted = journal_dir(&interrupted).join("aborted");
    assert_eq!(
        fs::read(aborted.join("config/credentials.json")).unwrap(),
        copied
    );
    // The original, never-touched source content is untouched (copy_entries
    // only ever reads from the source).
    assert_eq!(
        fs::read(interrupted.source_root.join("credentials.json")).unwrap(),
        b"{\"sentinel\":\"private\"}"
    );
    let journal: Journal =
        serde_json::from_slice(&fs::read(journal_dir(&interrupted).join("journal.json")).unwrap())
            .unwrap();
    assert_eq!(journal.phase, "aborted");
    // A5: a fresh plan for the same root is not blocked by the retired
    // state/ directory the abort left behind.
    let fresh = plan(&interrupted.source_root, &interrupted.target_root, None).unwrap();
    assert!(
        !fresh
            .conflicts
            .iter()
            .any(|conflict| conflict.contains("state")),
        "unexpected conflicts: {:?}",
        fresh.conflicts
    );
}

#[cfg(unix)]
#[test]
fn storage_rollback_restores_ecosystem_receipt_when_aborting_before_activation() {
    let fixture = Fixture::new();
    let toolkit = fixture.source().join("toolkits/ecosystem.toml");
    fs::write(&toolkit, b"literal ecosystem toolkit").unwrap();
    let prefix = fixture.0.join("prefix");
    fs::create_dir_all(prefix.join("share/ecosystem")).unwrap();
    let path = prefix.join("share/ecosystem/install.json");
    let before = json!({"managed_by":"ecosystem-installer-v3","prefix":prefix,"connections":["upeg","claude"],
        "upeg_toolkit":toolkit,"files":{toolkit.to_str().unwrap():{"sha256":digest(b"literal ecosystem toolkit"),"mode":mode(&toolkit).unwrap()},"unrelated":{"sha256":"unchanged","mode":493}},"unknown":{"retain":true}});
    let bytes = serde_json::to_vec(&before).unwrap();
    fs::write(&path, &bytes).unwrap();
    let plan = plan(&fixture.source(), &fixture.target(), Some(&prefix)).unwrap();
    // Interrupt right after the ecosystem receipt was rewritten to
    // receipt_after, but before verification/marker/activation.
    assert!(apply_inner(&plan, true, true, Some("receipt")).is_err());
    assert_ne!(fs::read(&path).unwrap(), bytes);
    let result = rollback(&fixture.target(), true, true).unwrap();
    assert_eq!(result["status"], "aborted");
    assert_eq!(fs::read(&path).unwrap(), bytes);
    // Cross-root: source was never touched, and the target keeps only an
    // informational inactive pending pointer (no active/split marker).
    assert_eq!(fs::read(&toolkit).unwrap(), b"literal ecosystem toolkit");
    assert!(!fixture.target().join(STORAGE_MARKER).exists());
    let pending: Value =
        serde_json::from_slice(&fs::read(fixture.target().join(MIGRATION_PENDING)).unwrap())
            .unwrap();
    assert_eq!(pending["status"], "inactive");
    assert!(!fixture.source().join(MIGRATION_PENDING).exists());
}

#[test]
fn storage_plan_does_not_conflict_after_same_root_rollback_of_active_migration() {
    let fixture = Fixture::new();
    let activated = plan(&fixture.source(), &fixture.source(), None).unwrap();
    apply(&activated, true, true).unwrap();
    rollback(&activated.target_root, true, true).unwrap();
    let fresh = plan(&activated.source_root, &activated.target_root, None).unwrap();
    assert!(
        !fresh
            .conflicts
            .iter()
            .any(|conflict| conflict.contains("state")),
        "unexpected conflicts: {:?}",
        fresh.conflicts
    );
}

#[test]
fn storage_control_temp_files_at_storage_roots_are_excluded_from_inventories() {
    let fixture = Fixture::new();
    fs::write(
        fixture
            .source()
            .join(format!("{STORAGE_TMP_PREFIX}leftover")),
        b"stale in-flight temp file",
    )
    .unwrap();
    let plan = fixture.plan();
    assert!(
        !plan
            .unclassified
            .iter()
            .any(|path| path.to_string_lossy().starts_with(STORAGE_TMP_PREFIX))
    );
    apply(&plan, true, true).unwrap();
    fs::write(
        fixture
            .target()
            .join(format!("{STORAGE_TMP_PREFIX}leftover")),
        b"stale in-flight temp file",
    )
    .unwrap();
    // A stray control-temp file at the target must not be treated as foreign
    // content blocking rollback's "source changed since cutover" check.
    rollback(&fixture.target(), true, true).unwrap();
}

#[test]
fn storage_pending_marker_write_is_idempotent_and_survives_a_leftover_temp_file() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    // `pending()` is normally called after `locks()` has already created the
    // target root as a side effect; recreate that precondition directly
    // since this test drives `pending()` on its own.
    fs::create_dir_all(&plan.target_root).unwrap();
    pending(&plan.target_root, &plan).unwrap();
    let first = fs::read(plan.target_root.join(MIGRATION_PENDING)).unwrap();
    // A crashed writer could leave its hard-link source behind; a retry must
    // still converge on the same content rather than failing outright.
    fs::write(
        plan.target_root
            .join(format!("{STORAGE_TMP_PREFIX}pin-leftover")),
        &first,
    )
    .unwrap();
    pending(&plan.target_root, &plan).unwrap();
    assert_eq!(
        fs::read(plan.target_root.join(MIGRATION_PENDING)).unwrap(),
        first
    );
}

#[test]
fn storage_sqlite_backup_removes_stale_wal_journal_shm_siblings_before_copying() {
    let fixture = Fixture::new();
    let _connection = fixture.database();
    let destination = fixture.0.join("staged.db");
    fs::create_dir_all(&fixture.0).ok();
    fs::write(
        destination.with_file_name("staged.db-wal"),
        b"stale wal bytes",
    )
    .unwrap();
    fs::write(
        destination.with_file_name("staged.db-shm"),
        b"stale shm bytes",
    )
    .unwrap();
    fs::write(
        destination.with_file_name("staged.db-journal"),
        b"stale journal bytes",
    )
    .unwrap();
    super::sqlite::backup(&fixture.source().join("upeg.db"), &destination).unwrap();
    assert!(!destination.with_file_name("staged.db-wal").exists());
    assert!(!destination.with_file_name("staged.db-shm").exists());
    assert!(!destination.with_file_name("staged.db-journal").exists());
    let copied = Connection::open(&destination).unwrap();
    assert_eq!(
        copied
            .query_row("SELECT value FROM sentinel", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "committed-in-wal"
    );
}

#[test]
fn storage_staging_copies_are_removed_after_activation() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    assert!(!journal_dir(&plan).join("staging").exists());
}

#[test]
fn storage_lease_reports_missing_journal_distinctly_from_generic_marker_errors() {
    let fixture = Fixture::new();
    let plan = plan(&fixture.source(), &fixture.source(), None).unwrap();
    apply(&plan, true, true).unwrap();
    fs::remove_file(journal_dir(&plan).join("journal.json")).unwrap();
    let location = paths::StorageLocation::new(plan.target_root, paths::StorageLayout::SplitV2);
    let result = paths::StorageLease::for_location(&location);
    assert!(
        matches!(result, Err(StoragePathError::MissingJournal(_))),
        "{result:?}"
    );
}

#[test]
fn storage_redirect_marker_carries_migration_id_matching_target_split_marker() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    let redirect: Value =
        serde_json::from_slice(&fs::read(fixture.source().join(STORAGE_MARKER)).unwrap()).unwrap();
    let target_marker: Value =
        serde_json::from_slice(&fs::read(fixture.target().join(STORAGE_MARKER)).unwrap()).unwrap();
    assert_eq!(redirect["migration_id"], json!(plan.plan_hash));
    assert_eq!(redirect["migration_id"], target_marker["migration_id"]);
}

#[test]
fn storage_redirect_marker_is_rejected_when_migration_id_does_not_match_target() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    apply(&plan, true, true).unwrap();
    let marker_path = fixture.source().join(STORAGE_MARKER);
    let mut marker: Value = serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["migration_id"] = json!("0".repeat(64));
    fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
    let result = paths::resolve_storage_with_lookup(
        |key| (key == "UPEG_HOME").then(|| fixture.source()),
        paths::Platform::Unix,
        paths::inspect_storage,
    );
    assert!(
        matches!(result, Err(StoragePathError::Marker(_))),
        "{result:?}"
    );
}

#[test]
fn storage_inactive_pending_marker_falls_back_to_normal_resolution_on_unix() {
    let root = std::env::temp_dir().join(format!("upeg-inactive-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(MIGRATION_PENDING),
        serde_json::to_vec(&json!({
            "schema_version":1,"app":"upeg","status":"inactive","target":root,"plan_hash":"0".repeat(64),
        }))
        .unwrap(),
    )
    .unwrap();
    let result = paths::resolve_storage_with_lookup(
        |key| (key == "UPEG_HOME").then(|| root.clone()),
        paths::Platform::Unix,
        paths::inspect_storage,
    );
    let location =
        result.unwrap_or_else(|error| panic!("expected fallback resolution, got {error:?}"));
    assert_eq!(location.root, root);
    let _ = fs::remove_dir_all(&root);
}
