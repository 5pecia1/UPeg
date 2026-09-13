use super::*;

use super::diff::diff_json_values;
use serde_json::json;

#[test]
fn 차이는_추가_삭제_변경된_항목을_모두_감지한다() {
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
fn 차이_보고는_중첩된_객체_변경에도_json_pointer_escaping을_적용한다() {
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
fn 차이는_배열_인덱스를_순서대로_보고한다() {
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
fn pr_댓글은_계약_변경을_우선하고_제한_행을_명시한다() {
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
fn 픽스처_pr_댓글은_대상_브랜치_언어_와_픽스처_차이_경로를_사용한다() {
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

/// schemaVersion을 올리는 PR에서 base 브랜치 fixture는 필연적으로
/// 옛 버전이다. 코멘트 렌더러는 그걸 게이트로 삼지 말고 "비교 불가"로
/// 적어야 한다 — 예전엔 여기서 에러가 나 CI 레인이 빨개졌다.
#[test]
fn base_fixture의_schema_version이_다르면_비교_불가로_적는다() {
    let no_drift = InventoryDiff::between(&empty_inventory_json(), &empty_inventory_json())
        .expect("빈 fixture끼리는 비교된다");
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
        "비교 불가 사유를 적어야 한다: {comment}"
    );
    assert!(
        comment.contains("schemaVersion 2"),
        "찾은 버전을 적어야 한다: {comment}"
    );
    assert!(
        comment.contains(&format!(
            "schemaVersion {INTERFACE_INVENTORY_SCHEMA_VERSION}"
        )),
        "이 빌드가 기대하는 버전도 적어야 한다: {comment}"
    );
}

#[test]
fn schema_version이_다른_fixture는_없는_파일처럼_읽힌다() {
    let dir =
        std::env::temp_dir().join(format!("upeg-inventory-schema-skew-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("스크래치 디렉터리");
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

    let (value, side) = read_optional_inventory_json(&path).expect("에러가 아니어야 한다");

    assert_eq!(
        side,
        FixtureSide::Incomparable {
            schema_version: Some(stale)
        }
    );
    assert_eq!(
        value["entries"].as_array().map(Vec::len),
        Some(0),
        "비교 불가 fixture는 항목을 하나도 내놓지 않는다"
    );

    // 게이트(`inventory check --baseline`)는 여전히 엄격하다.
    let strict = read_json(&path).expect("json");
    assert!(
        validate_inventory_json(&strict, &path).is_err(),
        "baseline 검증은 버전 불일치를 계속 거절해야 한다"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 표면_집합_변경은_읽을_수_있는_한_줄로_보고된다() {
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
        "표면 집합은 통째로 비교한다"
    );
    assert_eq!(diff.changed[0].changes[0].path, "/surfaces");

    let comment = format_pr_comment(&diff);

    assert!(comment.contains("- `tool` / `tool.text.upper`"));
    assert!(
        comment.contains("  - `changed` surfaces: `cli, http, tui` -> `cli, http`"),
        "표면 집합 변경이 읽을 수 있게 렌더링되어야 한다: {comment}"
    );
}
