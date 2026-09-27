//! Backup export / import FRB surface.
//!
//! The pure schema (struct + version guard + `#[serde(deny_unknown_fields)]`)
//! lives in `upeg_pegboard_ui::features::backup`. This module owns the
//! I/O path: reading persisted state through the same storage adapter
//! the pegboard UI uses, and writing it back during import. Validation
//! failures surface as [`FrbError::Validation`] before any storage
//! write, so a malformed file never corrupts the on-disk state.

use std::collections::BTreeMap;

use upeg_core::{BoardKey, Placement};
use upeg_pegboard_ui::features::backup::{BACKUP_VERSION, EnvironmentBackup};
use upeg_pegboard_ui::features::boards::{BoardData, default_boards, load_boards};
use upeg_pegboard_ui::features::memos::{load_memos, save_memos};
use upeg_pegboard_ui::features::tweaks::load_tweaks;
use upeg_pegboard_ui::platform::storage;

use super::boot::FrbError;
use super::pegboard::is_project_declared_board;

/// Counts surfaced after a successful import. The Flutter side renders
/// this in the post-import toast so the user sees how much was restored.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(non_opaque)]
pub struct BackupImportReportDto {
    pub board_count: u32,
    pub layout_count: u32,
    pub memo_count: u32,
}

/// Snapshot every persisted piece of desktop state as pretty-printed JSON.
///
/// Reads through `upeg_pegboard_ui::platform::storage` so the snapshot
/// matches whatever is currently written to disk — there is no in-memory
/// state machine to race against on the FRB side.
#[flutter_rust_bridge::frb(sync)]
pub fn export_backup() -> Result<String, FrbError> {
    let backup = current_backup();
    Ok(backup.to_json_pretty())
}

/// Validate + apply an imported backup. Returns counts on success;
/// [`FrbError::Validation`] when the JSON shape, version, or any
/// required field disagrees with the current schema; [`FrbError::Io`]
/// when the parse succeeded but persisting to storage failed.
#[flutter_rust_bridge::frb(sync)]
pub fn import_backup(json: String) -> Result<BackupImportReportDto, FrbError> {
    // Step 1: parse (rejects malformed JSON, missing required fields,
    // and any unknown field thanks to `#[serde(deny_unknown_fields)]`).
    let backup = EnvironmentBackup::from_json(&json).map_err(|reason| FrbError::Validation {
        field: "schema".to_string(),
        reason,
    })?;

    // Step 2: exact-version guard — older / newer schemas are a
    // different on-disk format and must be refused.
    backup
        .validate_version()
        .map_err(|reason| FrbError::Validation {
            field: "version".to_string(),
            reason,
        })?;

    // Step 3: board-key validation. `deny_unknown_fields` polices the
    // SHAPE; nothing so far has policed the board keys themselves, and
    // an unparsable key would be written straight into the store where
    // it can neither be selected nor swept.
    let backup = validated_boards(backup)?;

    // Step 4: write through to storage. The pegboard storage adapter
    // owns the on-disk shape (tweaks under the config root, boards /
    // layouts under upeg-sources, memos through the shared memo store).
    apply_backup(&backup)?;

    // Step 5: build the report. `usize → u32` saturates so a wild
    // import doesn't panic on 32-bit casts (every legitimate user
    // backup fits in u32).
    Ok(BackupImportReportDto {
        board_count: u32::try_from(backup.boards.len()).unwrap_or(u32::MAX),
        layout_count: u32::try_from(backup.layouts.len()).unwrap_or(u32::MAX),
        memo_count: u32::try_from(backup.memos.len()).unwrap_or(u32::MAX),
    })
}

// ─── helpers ────────────────────────────────────────────────────────

fn current_backup() -> EnvironmentBackup {
    let tweaks = load_tweaks().unwrap_or_default();
    let boards = current_boards();
    let layouts = current_layouts_owned();
    let memos = load_memos().unwrap_or_default();
    EnvironmentBackup {
        version: BACKUP_VERSION,
        tweaks,
        layouts,
        memos,
        boards,
    }
}

/// Boards a backup carries: the user's own, never the project's.
///
/// A project board exists only while its Project Manifest is detected —
/// the manifest declares it and `upeg-sources` merges it in on every
/// load, keyed under that project's own namespace. Exporting one would
/// copy a row the manifest owns into a file that knows nothing about
/// the manifest, and importing that file elsewhere would re-create the
/// board as a plain global one: same id, no project, pins detached from
/// the project they were pinned for. **Project boards live with the
/// manifest, not in backups.**
fn current_boards() -> Vec<BoardData> {
    load_boards()
        .unwrap_or_else(default_boards)
        .iter()
        .map(BoardData::from)
        .filter(|board| !is_project_declared_board(&board.key))
        .collect()
}

/// Layouts of the boards [`current_boards`] kept — a layout for a board
/// the export dropped is an orphan the import could not attach to
/// anything.
fn current_layouts_owned() -> BTreeMap<String, Vec<Placement>> {
    let Some(raw) = storage::get_item(storage::LAYOUTS_KEY) else {
        return BTreeMap::new();
    };
    let layouts: BTreeMap<String, Vec<Placement>> = serde_json::from_str(&raw).unwrap_or_default();
    layouts
        .into_iter()
        .filter(|(board_key, _)| !is_project_declared_board(board_key))
        .collect()
}

/// Reject an import whose board keys are not real [`BoardKey`]s, and
/// drop the ones the active Project Manifest owns.
///
/// Two different failures, two different answers:
///   - an **unparsable** key is a corrupt file. Refusing the whole
///     import is the only honest response: silently dropping the board
///     would report a `board_count` the user's file does not contain.
///   - a **project-declared** key is a valid board that this backup has
///     no business restoring (see [`current_boards`] — export drops
///     them too). Importing it would overwrite the manifest-owned row
///     with a stale copy, so it is dropped, along with its layout, and
///     the reported counts reflect what was actually written.
fn validated_boards(mut backup: EnvironmentBackup) -> Result<EnvironmentBackup, FrbError> {
    for board in &backup.boards {
        BoardKey::parse(&board.key).map_err(|err| FrbError::Validation {
            field: "boards".to_string(),
            reason: format!("invalid board key `{}`: {err}", board.key),
        })?;
    }
    for board_key in backup.layouts.keys() {
        BoardKey::parse(board_key).map_err(|err| FrbError::Validation {
            field: "layouts".to_string(),
            reason: format!("invalid board key `{board_key}`: {err}"),
        })?;
    }

    backup
        .boards
        .retain(|board| !is_project_declared_board(&board.key));
    backup
        .layouts
        .retain(|board_key, _| !is_project_declared_board(board_key));
    Ok(backup)
}

/// Write the imported backup through the same shared storage modules
/// used by the UI. A successful import replaces every local-first
/// state bucket carried by [`EnvironmentBackup`], including memos.
fn apply_backup(backup: &EnvironmentBackup) -> Result<(), FrbError> {
    let tweaks_json = serde_json::to_string(&backup.tweaks).map_err(|err| FrbError::Io {
        message: format!("tweaks serialize: {err}"),
    })?;
    storage::set_item(storage::TWEAKS_KEY, &tweaks_json).map_err(|()| FrbError::Io {
        message: "tweaks write failed".to_string(),
    })?;

    let boards_json = serde_json::to_string(&backup.boards).map_err(|err| FrbError::Io {
        message: format!("boards serialize: {err}"),
    })?;
    #[cfg(not(target_arch = "wasm32"))]
    upeg_sources::pegboard::save_boards_json(&boards_json).map_err(|error| FrbError::Io {
        message: format!("boards write failed: {error}"),
    })?;
    #[cfg(target_arch = "wasm32")]
    storage::set_item(storage::BOARDS_KEY, &boards_json).map_err(|()| FrbError::Io {
        message: "boards write failed".to_string(),
    })?;

    let layouts_json = serde_json::to_string(&backup.layouts).map_err(|err| FrbError::Io {
        message: format!("layouts serialize: {err}"),
    })?;
    storage::set_item(storage::LAYOUTS_KEY, &layouts_json).map_err(|()| FrbError::Io {
        message: "layouts write failed".to_string(),
    })?;

    save_memos(&backup.memos).map_err(|err| FrbError::Io {
        message: format!("memos write failed: {err}"),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::api::test_support::run_with_storage_backup;
    use upeg_core::Placement;
    use upeg_core::prefs::{Accent, Locale, Theme, Tweaks};
    use upeg_pegboard_ui::features::backup::{BACKUP_VERSION, EnvironmentBackup};
    use upeg_pegboard_ui::features::boards::BoardData;

    use super::super::boot::FrbError;
    use super::{export_backup, import_backup};

    #[test]
    fn export_backup_returns_valid_json() {
        let json = export_backup().expect("export ok");
        let parsed = EnvironmentBackup::from_json(&json).expect("round-trip");
        assert_eq!(parsed.version, BACKUP_VERSION);
    }

    #[test]
    fn import_backup_round_trips_export_result() {
        run_with_storage_backup(|| {
            let json = export_backup().expect("export");
            let report = import_backup(json).expect("import");
            // Default boards are non-empty (`dev/trading/personal`) so the
            // round-trip count is at least 1 regardless of test-suite order.
            assert!(report.board_count >= 1);
        });
    }

    #[test]
    fn import_backup_restores_boards_layouts_memos_tweaks_into_next_export() {
        run_with_storage_backup(|| {
            let mut layouts = BTreeMap::new();
            layouts.insert(
                "dev".to_string(),
                vec![Placement::new("num.hex_to_decimal", 4, 2)],
            );
            layouts.insert(
                "research".to_string(),
                vec![Placement::new("id.uuid_v7", 1, 3)],
            );

            let mut memos = BTreeMap::new();
            memos.insert("scratch".to_string(), "restored note".to_string());

            let imported = EnvironmentBackup {
                version: BACKUP_VERSION,
                tweaks: Tweaks {
                    theme: Theme::Light,
                    accent: Accent::Pink,
                    show_holes: true,
                    local_http_host: false,
                    locale: Locale::Ko,
                },
                layouts,
                memos,
                boards: vec![
                    BoardData {
                        key: "dev".to_string(),
                        title: "Development".to_string(),
                        guidance: upeg_core::BoardGuidance::default(),
                    },
                    BoardData {
                        key: "research".to_string(),
                        title: "Research".to_string(),
                        guidance: upeg_core::BoardGuidance::default(),
                    },
                ],
            };

            import_backup(imported.to_json()).expect("import exact backup");
            let exported_json = export_backup().expect("export restored state");
            let exported =
                EnvironmentBackup::from_json(&exported_json).expect("parse exported backup");

            assert_eq!(exported.boards, imported.boards);
            assert_eq!(exported.layouts, imported.layouts);
            assert_eq!(exported.memos, imported.memos);
            assert_eq!(exported.tweaks, imported.tweaks);
        });
    }

    #[test]
    fn import_backup_accepts_memos_and_reports_count() {
        run_with_storage_backup(|| {
            let json = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{"scratch":"hello"},"boards":[]}"#;
            let report = import_backup(json.to_string()).expect("import with memos");
            assert_eq!(report.memo_count, 1);
        });
    }

    #[test]
    fn import_backup_returns_validation_error_on_version_mismatch() {
        let bad = r#"{"version":999,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[]}"#;
        match import_backup(bad.to_string()) {
            Err(FrbError::Validation { field, .. }) => {
                assert_eq!(field, "version");
            }
            other => panic!("expected version Validation error, got {other:?}"),
        }
    }

    /// A project board belongs to the manifest that declares it, not to
    /// a portable backup file: exporting it would copy a
    /// namespace-owned row into a document that knows nothing about the
    /// namespace.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn export_backup_excludes_project_boards_and_their_layouts() {
        use crate::api::pegboard::project_scope_test_support::ScopedProjectBoard;

        const PROJECT_BOARD_ID: &str = "frb-backup-proj";

        run_with_storage_backup(|| {
            let _project = ScopedProjectBoard::declare(PROJECT_BOARD_ID, "FRB Backup Project");

            // Project boards are merged at load time, so the board is
            // actually visible here.
            let visible =
                upeg_sources::pegboard::board_keys_in(&upeg_sources::pegboard::load_state());
            assert!(
                visible.iter().any(|key| key == PROJECT_BOARD_ID),
                "precondition: the board must be visible inside the project: {visible:?}"
            );

            let exported = EnvironmentBackup::from_json(&export_backup().expect("export"))
                .expect("parse exported backup");

            assert!(
                !exported
                    .boards
                    .iter()
                    .any(|board| board.key == PROJECT_BOARD_ID),
                "backup carried a project board: {:?}",
                exported.boards
            );
            assert!(
                !exported.layouts.contains_key(PROJECT_BOARD_ID),
                "a layout left behind without its board is an orphan: {:?}",
                exported.layouts.keys().collect::<Vec<_>>()
            );
        });
    }

    /// An unparsable board key is a corrupt file, not a droppable row:
    /// silently skipping it would report a count the user's file does
    /// not contain, and writing it would put a key in the store that
    /// can never be selected.
    #[test]
    fn import_backup_rejects_invalid_board_key() {
        let bad = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[{"key":"project:ns:oops","title":"Reserved"}]}"#;
        match import_backup(bad.to_string()) {
            Err(FrbError::Validation { field, reason }) => {
                assert_eq!(field, "boards");
                assert!(reason.contains("project:ns:oops"), "got {reason}");
            }
            other => panic!("expected boards Validation error, got {other:?}"),
        }

        let bad_layout = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{"  ":[]},"memos":{},"boards":[]}"#;
        match import_backup(bad_layout.to_string()) {
            Err(FrbError::Validation { field, .. }) => assert_eq!(field, "layouts"),
            other => panic!("expected layouts Validation error, got {other:?}"),
        }
    }

    /// Importing a backup that names a project-declared board must not
    /// flatten it into a global row — the manifest still owns that id,
    /// and a stale global copy would shadow nothing and confuse
    /// everything.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn import_backup_does_not_flatten_project_boards_to_global() {
        use crate::api::pegboard::project_scope_test_support::ScopedProjectBoard;

        const PROJECT_BOARD_ID: &str = "frb-import-proj";
        const GLOBAL_BOARD_ID: &str = "frb-import-global";

        run_with_storage_backup(|| {
            let imported = EnvironmentBackup {
                version: BACKUP_VERSION,
                tweaks: Tweaks::default(),
                layouts: BTreeMap::new(),
                memos: BTreeMap::new(),
                boards: vec![
                    BoardData {
                        key: GLOBAL_BOARD_ID.to_string(),
                        title: "Global".to_string(),
                        guidance: upeg_core::BoardGuidance::default(),
                    },
                    BoardData {
                        key: PROJECT_BOARD_ID.to_string(),
                        title: "Stale project copy".to_string(),
                        guidance: upeg_core::BoardGuidance::default(),
                    },
                ],
            };

            {
                let _project = ScopedProjectBoard::declare(PROJECT_BOARD_ID, "FRB Import Project");
                let report = import_backup(imported.to_json()).expect("import");
                assert_eq!(
                    report.board_count, 1,
                    "the report must count only what was actually written"
                );
            }

            // Outside the manifest: the project board must not remain as
            // a global row.
            let boards =
                upeg_sources::pegboard::board_keys_in(&upeg_sources::pegboard::load_state());
            assert!(
                boards.iter().any(|key| key == GLOBAL_BOARD_ID),
                "the global board must be restored: {boards:?}"
            );
            assert!(
                !boards.iter().any(|key| key == PROJECT_BOARD_ID),
                "project board was flattened to global: {boards:?}"
            );
        });
    }

    #[test]
    fn import_backup_returns_validation_error_on_unknown_field() {
        let bad = r#"{"version":4,"tweaks":{"theme":"Dark","accent":"Green","show_holes":true,"locale":"En"},"layouts":{},"memos":{},"boards":[],"bogus":1}"#;
        match import_backup(bad.to_string()) {
            Err(FrbError::Validation { field, .. }) => {
                assert_eq!(field, "schema");
            }
            other => panic!("expected schema Validation error, got {other:?}"),
        }
    }
}
