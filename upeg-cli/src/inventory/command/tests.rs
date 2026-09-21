use super::*;

use super::diff::diff_json_values;
use serde_json::json;

#[test]
fn diff_detects_added_removed_and_changed_entries() {
    let baseline = json!({
        "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
        "entries": [
            {"surfaces":["cli"],"kind":"cliCommand","id":"same","version":"v1"},
            {"surfaces":["cli"],"kind":"cliCommand","id":"changed","version":"v1"},
            {"surfaces":["cli"],"kind":"cliCommand","id":"removed","version":"v1"}
        ]
    });
    let current = json!({
        "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
        "entries": [
            {"surfaces":["cli"],"kind":"cliCommand","id":"same","version":"v1"},
            {"surfaces":["cli"],"kind":"cliCommand","id":"changed","version":"v2"},
            {"surfaces":["cli"],"kind":"cliCommand","id":"added","version":"v1"}
        ]
    });

    let diff = InventoryDiff::between(&baseline, &current).expect("diff computes");

    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.changed.len(), 1);
    assert_eq!(diff.changed[0].changes.len(), 1);
    assert_eq!(diff.changed[0].changes[0].path, "/version");

    let diff_json = diff.to_json();
    assert!(diff_json["changed"][0].get("before").is_none());
    assert!(diff_json["changed"][0].get("after").is_none());
    assert!(diff_json["changed"][0]["changes"].is_array());
}

#[test]
fn diff_report_applies_json_pointer_escaping_to_nested_object_changes() {
    let before = json!({
        "contract": {
            "input": {
                "schema": {
                    "properties": {
                        "name/with~escape": {"type": "string"}
                    }
                }
            }
        }
    });
    let after = json!({
        "contract": {
            "input": {
                "schema": {
                    "properties": {
                        "name/with~escape": {"type": ["string", "null"]}
                    }
                }
            }
        }
    });

    let changes = diff_json_values(&before, &after, "");

    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].path,
        "/contract/input/schema/properties/name~1with~0escape/type"
    );
    assert_eq!(changes[0].kind, "changed");
    assert_eq!(changes[0].before, json!("string"));
    assert_eq!(changes[0].after, json!(["string", "null"]));
}

#[test]
fn diff_reports_array_indices_in_order() {
    let before = json!({
        "contract": {
            "errors": [
                {"code": "unchanged"},
                {"code": "old"}
            ]
        }
    });
    let after = json!({
        "contract": {
            "errors": [
                {"code": "unchanged"},
                {"code": "new"},
                {"code": "added"}
            ]
        }
    });

    let changes = diff_json_values(&before, &after, "");
    let paths = changes
        .iter()
        .map(|change| (change.path.as_str(), change.kind.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        vec![
            ("/contract/errors/1/code", "changed"),
            ("/contract/errors/2", "added")
        ]
    );
}

#[test]
fn pr_comment_prioritizes_contract_changes_and_marks_limit_rows() {
    let changes = (0..21)
        .map(|index| DeepChange {
            path: format!("/contract/input/schema/properties/field{index}"),
            kind: "changed".to_string(),
            before: json!("old"),
            after: json!("new"),
        })
        .chain(std::iter::once(DeepChange {
            path: "/version".to_string(),
            kind: "changed".to_string(),
            before: json!("v1"),
            after: json!("v2"),
        }))
        .collect();
    let diff = InventoryDiff {
        added: Vec::new(),
        removed: Vec::new(),
        changed: vec![ChangedEntry {
            key: InventoryKey {
                kind: "tool".to_string(),
                id: "cli.tools.example".to_string(),
            },
            changes,
        }],
    };

    let comment = format_pr_comment(&diff);
    let contract_section = comment
        .find("##### Input/Output contract changes")
        .expect("contract section exists");
    let metadata_section = comment
        .find("##### Other metadata changes")
        .expect("metadata section exists");

    assert!(contract_section < metadata_section);
    assert_eq!(
        comment.matches("  - `changed` `/contract/input").count(),
        20
    );
    assert!(comment.contains("Showing first `20` of `22` path-level changes"));
    assert!(comment.contains("target/interface-inventory/diff.json"));

    let full_comment = format_pr_comment_with_limits(&diff, FULL_COMMENT_LIMITS);
    assert_eq!(
        full_comment
            .matches("  - `changed` `/contract/input")
            .count(),
        21
    );
    assert!(full_comment.contains("  - `changed` `/version`:"));
    assert!(!full_comment.contains("Showing first"));
    assert!(full_comment.contains("Raw JSON diff: `target/interface-inventory/diff.json`."));
}

#[test]
fn fixture_pr_comment_uses_target_branch_language_and_fixture_diff_paths() {
    let diff = InventoryDiff {
        added: vec![json!({
            "surfaces": ["cli", "mcp"],
            "kind": "tool",
            "id": "cli.tools.example",
            "version": "v1",
            "compatibility": "stable",
            "source": {
                "path": "upeg-tools/src/lib.rs"
            },
            "owner": {
                "path": "upeg-tools"
            },
            "contract": {
                "input": {
                    "kind": "schema",
                    "schemaRef": "tool.example.input_schema",
                    "declared": true
                },
                "output": {
                    "kind": "schema",
                    "description": "derived response",
                    "declared": false
                }
            }
        })],
        removed: Vec::new(),
        changed: vec![ChangedEntry {
            key: InventoryKey {
                kind: "route".to_string(),
                id: "GET /v1/tools".to_string(),
            },
            changes: vec![DeepChange {
                path: "/contract/output/schemaRef".to_string(),
                kind: "changed".to_string(),
                before: json!("old"),
                after: json!("new"),
            }],
        }],
    };

    let comment = format_fixture_pr_comment(
        &diff,
        "main",
        "feature/test",
        FixtureSide::Comparable,
        FixtureSide::Comparable,
    );

    assert!(comment.contains("### Interface Inventory Fixture Changes"));
    assert!(comment.contains("target branch `main`"));
    assert!(comment.contains("this PR `feature/test`"));
    assert!(comment.contains("target/interface-inventory/pr-fixture-diff.json"));
    assert!(comment.contains("committed fixture files only"));
    assert!(comment.contains("#### Changed (1)"));
    assert!(comment.contains("#### Added (1)"));
    assert!(!comment.contains("| Surfaces | Kind | ID | Version | Compatibility |"));
    assert!(comment.contains("- `tool` / `cli.tools.example`"));
    assert!(comment.contains("  - Surfaces: `cli, mcp`"));
    assert!(comment.contains("upeg-tools/src/lib.rs"));
    assert!(comment.contains("schemaRef=tool.example.input_schema"));
    assert!(comment.contains("not declared"));

    let missing_base_comment = format_fixture_pr_comment(
        &diff,
        "main",
        "feature/test",
        FixtureSide::Missing,
        FixtureSide::Comparable,
    );
    assert!(missing_base_comment.contains("target branch does not contain"));
    assert_eq!(
        empty_inventory_json()["entries"].as_array().unwrap().len(),
        0
    );
}

/// In a PR that bumps schemaVersion, the base branch fixture is
/// necessarily at the old version. The comment renderer must not gate on
/// that — it should write the side as "not comparable"; this used to
/// error and turn the CI lane red.
#[test]
fn base_fixture_with_different_schema_version_is_marked_not_comparable() {
    let no_drift = InventoryDiff::between(&empty_inventory_json(), &empty_inventory_json())
        .expect("empty fixtures compare");
    let comment = format_fixture_pr_comment(
        &no_drift,
        "main",
        "feature/test",
        FixtureSide::Incomparable {
            schema_version: Some(2),
        },
        FixtureSide::Comparable,
    );

    assert!(
        comment.contains("not comparable"),
        "must state the not-comparable reason: {comment}"
    );
    assert!(
        comment.contains("schemaVersion 2"),
        "must state the found version: {comment}"
    );
    assert!(
        comment.contains(&format!(
            "schemaVersion {INTERFACE_INVENTORY_SCHEMA_VERSION}"
        )),
        "must also state the version this build expects: {comment}"
    );
}

#[test]
fn fixture_with_different_schema_version_reads_like_a_missing_file() {
    let dir =
        std::env::temp_dir().join(format!("upeg-inventory-schema-skew-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("interface-inventory.json");
    let stale = u64::from(INTERFACE_INVENTORY_SCHEMA_VERSION) - 1;
    std::fs::write(
        &path,
        json!({
            "schemaVersion": stale,
            "entries": [
                {"surfaces":["cli"],"kind":"cliCommand","id":"old","version":"v1"}
            ]
        })
        .to_string(),
    )
    .expect("stale fixture");

    let (value, side) = read_optional_inventory_json(&path).expect("must not error");

    assert_eq!(
        side,
        FixtureSide::Incomparable {
            schema_version: Some(stale)
        }
    );
    assert_eq!(
        value["entries"].as_array().map(Vec::len),
        Some(0),
        "an incomparable fixture contributes no entries"
    );

    // The gate (`inventory check --baseline`) stays strict.
    let strict = read_json(&path).expect("json");
    assert!(
        validate_inventory_json(&strict, &path).is_err(),
        "baseline validation must keep rejecting a version mismatch"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn surface_set_change_is_reported_as_one_readable_line() {
    let baseline = json!({
        "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
        "entries": [
            {"surfaces":["cli","http","tui"],"kind":"tool","id":"tool.text.upper","version":"v1"}
        ]
    });
    let current = json!({
        "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
        "entries": [
            {"surfaces":["cli","http"],"kind":"tool","id":"tool.text.upper","version":"v1"}
        ]
    });

    let diff = InventoryDiff::between(&baseline, &current).expect("diff computes");

    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.changed.len(), 1);
    assert_eq!(
        diff.changed[0].changes.len(),
        1,
        "the surface set is compared as a whole"
    );
    assert_eq!(diff.changed[0].changes[0].path, "/surfaces");

    let comment = format_pr_comment(&diff);

    assert!(comment.contains("- `tool` / `tool.text.upper`"));
    assert!(
        comment.contains("  - `changed` surfaces: `cli, http, tui` -> `cli, http`"),
        "the surface set change must render readably: {comment}"
    );
}
