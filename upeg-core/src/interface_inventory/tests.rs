//! Unit tests for the interface inventory schema (`super`).
//!
//! Split out of `interface_inventory.rs` per the workspace test-layout
//! convention (docs/rules/engineering-rules.md §10): the colocated
//! `#[cfg(test)]` block had grown past the ~200-LoC budget and pushed
//! the module itself over the 1,000-line file budget.

use super::*;
use crate::Surface;

fn entry(id: &str, surfaces: impl Into<SurfaceSet>, kind: InterfaceKind) -> InterfaceEntry {
    InterfaceEntry {
        id: id.to_string(),
        surfaces: surfaces.into(),
        kind,
        contract: ContractShape::new(
            ContractLocator::default(),
            ContractIo::declared(
                ContractIoKind::JsonSchema,
                None,
                Some("docs/LEXICON.md#tool".to_string()),
                "tool input schema",
            ),
            ContractIo::not_declared("test output contract placeholder"),
        ),
        version: "1.0.0".to_string(),
        compatibility: Compatibility::Stable,
        owner: OwnerRef {
            path: Some("docs/LEXICON.md".to_string()),
            url: None,
        },
        docs: DocRef {
            path: Some("docs/LEXICON.md".to_string()),
            url: None,
        },
        source: SourceRef {
            path: Some("upeg-core/src/interface_inventory.rs".to_string()),
            url: None,
        },
        tests: TestMapping::covered(
            "upeg-core/src/interface_inventory.rs",
            Some("인터페이스_인벤토리의_직렬화는_결정적이다".to_string()),
        ),
    }
}

fn surface_kind(surface: Surface) -> InterfaceKind {
    match surface {
        Surface::Cli => InterfaceKind::CliCommand,
        Surface::Tui => InterfaceKind::TuiInteraction,
        Surface::Desktop => InterfaceKind::DesktopComponent,
        Surface::Pwa => InterfaceKind::PwaComponent,
        Surface::Ext => InterfaceKind::ChromeExtension,
        Surface::Mcp => InterfaceKind::McpMethod,
        Surface::Http => InterfaceKind::HttpRoute,
    }
}

fn assert_missing_local_path(entry: InterfaceEntry, expected_field: &str, expected_path: &str) {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![entry],
    };

    assert!(matches!(
        inventory.validate_surface_coverage(),
        Err(InterfaceInventoryError::MissingLocalPath { field, path })
            if field == expected_field && path == expected_path
    ));
}

#[test]
fn 인터페이스_인벤토리의_직렬화는_결정적이다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![
            entry("zeta", Surface::Http, InterfaceKind::HttpRoute),
            entry("alpha", Surface::Cli, InterfaceKind::CliCommand),
            entry("tool", Surface::Cli, InterfaceKind::Tool),
        ],
    };

    inventory.validate().expect("valid inventory");
    let serialized = serde_json::to_string(&inventory.to_deterministic_json())
        .expect("interface inventory JSON serializes");
    let reversed = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: inventory.entries.iter().cloned().rev().collect(),
    };
    let reversed_serialized = serde_json::to_string(&reversed.to_deterministic_json())
        .expect("interface inventory JSON serializes deterministically");

    assert_eq!(serialized, reversed_serialized);
    assert!(
        serialized.find("\"id\":\"tool\"").expect("tool entry")
            < serialized.find("\"id\":\"alpha\"").expect("alpha entry")
    );
    assert!(
        serialized.find("\"id\":\"alpha\"").expect("alpha entry")
            < serialized.find("\"id\":\"zeta\"").expect("zeta entry")
    );
    assert!(serialized.contains("\"schemaVersion\":3"));
    assert!(serialized.contains("\"surfaces\":[\"cli\"]"));
}

#[test]
fn 인터페이스_인벤토리는_중복된_id를_거부한다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![
            entry("duplicate", Surface::Cli, InterfaceKind::CliCommand),
            entry("duplicate", Surface::Cli, InterfaceKind::CliCommand),
        ],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::DuplicateEntry { kind, id })
            if kind == InterfaceKind::CliCommand && id == "duplicate"
    ));
}

#[test]
fn 인터페이스_인벤토리는_표면_집합이_다른_같은_계약도_중복으로_본다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![
            entry("shared", Surface::Cli, InterfaceKind::Tool),
            entry("shared", Surface::Http, InterfaceKind::Tool),
        ],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::DuplicateEntry { kind, id })
            if kind == InterfaceKind::Tool && id == "shared"
    ));
}

#[test]
fn 인터페이스_인벤토리는_표면이_없는_항목을_거부한다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![entry("surfaceless", Vec::new(), InterfaceKind::CliCommand)],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::EmptySurfaces { kind, id })
            if kind == InterfaceKind::CliCommand && id == "surfaceless"
    ));
}

#[test]
fn 인터페이스_항목은_가장_낮은_표면_rank로_정렬된다() {
    let mut entries = [
        entry("mcp-only", Surface::Mcp, InterfaceKind::Tool),
        entry(
            "cli-and-http",
            vec![Surface::Http, Surface::Cli],
            InterfaceKind::Tool,
        ),
        entry("tui-only", Surface::Tui, InterfaceKind::Tool),
    ];
    entries.sort();

    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cli-and-http", "tui-only", "mcp-only"]
    );
    assert_eq!(entries[0].surfaces.joined_labels(), "cli, http");
}

#[test]
fn 인터페이스_인벤토리는_schema_버전_일_문서를_거부한다() {
    let inventory = InterfaceInventory {
        schema_version: 1,
        entries: vec![entry("v1", Surface::Cli, InterfaceKind::CliCommand)],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::UnsupportedSchemaVersion { expected, actual })
            if expected == INTERFACE_INVENTORY_SCHEMA_VERSION && actual == 1
    ));
}

#[test]
fn 인터페이스_인벤토리는_표면_커버리지를_보장한다() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: ALL_SURFACES
            .iter()
            .map(|surface| entry(surface.label(), *surface, surface_kind(*surface)))
            .collect(),
    };

    inventory
        .validate_surface_coverage()
        .expect("all seven surfaces have inventory entries");

    let mut missing_pwa = inventory;
    missing_pwa
        .entries
        .retain(|entry| !entry.surfaces.contains(&Surface::Pwa));

    assert!(matches!(
        missing_pwa.validate_surface_coverage(),
        Err(InterfaceInventoryError::MissingSurfaceCoverage { surface })
            if surface == Surface::Pwa.label()
    ));
}

#[test]
fn 인터페이스_인벤토리는_누락된_로컬_참조를_거부한다() {
    let mut missing_docs = entry("missing-docs", Surface::Cli, InterfaceKind::CliCommand);
    missing_docs.docs.path = Some("docs/does-not-exist.md".to_string());
    assert_missing_local_path(missing_docs, "entry.docs", "docs/does-not-exist.md");

    let mut missing_source = entry("missing-source", Surface::Http, InterfaceKind::HttpRoute);
    missing_source.source.path = Some("upeg-core/src/does-not-exist.rs".to_string());
    assert_missing_local_path(
        missing_source,
        "entry.source",
        "upeg-core/src/does-not-exist.rs",
    );

    let mut missing_test = entry("missing-test", Surface::Mcp, InterfaceKind::McpMethod);
    missing_test.tests = TestMapping::covered("upeg-core/tests/does-not-exist.rs", None);
    assert_missing_local_path(
        missing_test,
        "entry.tests.path",
        "upeg-core/tests/does-not-exist.rs",
    );
}

#[test]
fn 인터페이스_인벤토리는_누락된_필수_필드를_거부한다() {
    let mut invalid = entry("", Surface::Cli, InterfaceKind::CliCommand);
    invalid.tests = TestMapping::covered(String::new(), None);
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![invalid],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::MissingRequiredField { field }) if field == "entry.id"
    ));

    let missing_contract = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![InterfaceEntry {
            contract: ContractShape::default(),
            ..entry("missing-contract", Surface::Cli, InterfaceKind::CliCommand)
        }],
    };

    assert!(matches!(
        missing_contract.validate(),
        Err(InterfaceInventoryError::MissingRequiredField { field }) if field == "entry.contract.input"
    ));

    let mut missing_output_entry = entry("missing-output", Surface::Cli, InterfaceKind::CliCommand);
    missing_output_entry.contract.output = None;
    let missing_output = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![missing_output_entry],
    };

    assert!(matches!(
        missing_output.validate(),
        Err(InterfaceInventoryError::MissingRequiredField { field }) if field == "entry.contract.output"
    ));
}
