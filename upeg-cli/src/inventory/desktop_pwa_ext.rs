//! Static Desktop/PWA/Chrome-extension surface contracts.
//!
//! These entries describe interface contracts that live in the
//! `flutter_app/` Dart layer (the Flutter desktop + PWA shells) and
//! the `chrome-ext/` Manifest-V3 extension. They are listed here in
//! `upeg-cli` because the `upeg interface inventory ...` subcommand
//! collects all surfaces and the CLI is the natural workspace-wide
//! aggregator (Rust-only, free of Dart/JS deps).

use upeg_core::Surface;
use upeg_core::interface_inventory::{
    Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
    InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
};

#[derive(Clone, Copy)]
struct ContractDeclaration {
    input_schema_ref: Option<&'static str>,
    command_path: Option<&'static str>,
    http_method: Option<&'static str>,
    http_path: Option<&'static str>,
}

impl ContractDeclaration {
    fn shape(self) -> ContractShape {
        let input = self.input_schema_ref.map_or_else(
            || ContractIo::none("UI interaction declares no structured input"),
            |schema_ref| {
                ContractIo::declared(
                    ContractIoKind::UiInteraction,
                    None,
                    Some(schema_ref.to_string()),
                    "UI interaction contract",
                )
            },
        );
        ContractShape::new(
            ContractLocator {
                command_path: self.command_path.map(ToOwned::to_owned),
                http_method: self.http_method.map(ToOwned::to_owned),
                http_path: self.http_path.map(ToOwned::to_owned),
                jsonrpc_method: None,
            },
            input,
            ContractIo::not_declared(
                "UI surface output presentation renders canonical ToolResult rows from dispatched tools",
            ),
        )
    }
}

/// A declared entry's real test coverage — never the declaring file
/// itself, and never some other existing-but-unrelated file either
/// (both were the self-pin the "inventory declaration honesty" backlog
/// item killed; [`DECLARING_FILE_PATH`] plus the
/// `upeg-cli/src/inventory/tests.rs`-resident honesty check keep them
/// from creeping back in).
///
/// [`Self::Uncovered`] exists so a genuine gap has somewhere honest to
/// go. Without it the only way to satisfy the honesty check is to point
/// at a file that does exist and does not test this entry, which is a
/// worse answer than "nothing tests this, here is why".
#[derive(Clone, Copy)]
enum TestDeclaration {
    /// `test_name` must occur in `path` as a real test — a Rust `fn
    /// <name>` or a Dart `'<name>'` case label.
    Covered {
        path: &'static str,
        test_name: &'static str,
    },
    /// Covered by a whole script rather than one named test case: a
    /// browser-driven contract check has no `fn`/`test(...)` symbol to
    /// point at, and inventing one would be a name nothing declares.
    /// The pointer is the script itself, which the honesty check still
    /// requires to exist and to differ from [`DECLARING_FILE_PATH`].
    CoveredByScript { path: &'static str },
    /// No test covers this entry yet; `reason` says why and is carried
    /// into the generated inventory.
    #[expect(
        dead_code,
        reason = "every entry is currently covered; the honest-gap vocabulary \
                  must outlive the last gap, or the next one gets declared as a \
                  test that does not test it. Constructing it clears this expect."
    )]
    Uncovered { reason: &'static str },
}

impl TestDeclaration {
    fn mapping(self) -> TestMapping {
        match self {
            Self::Covered { path, test_name } => {
                TestMapping::covered(path, Some(test_name.to_string()))
            }
            Self::CoveredByScript { path } => TestMapping::covered(path, None),
            Self::Uncovered { reason } => TestMapping::uncovered(reason),
        }
    }
}

#[derive(Clone, Copy)]
struct SurfaceInventoryDeclaration {
    id: &'static str,
    surface: Surface,
    kind: InterfaceKind,
    contract: ContractDeclaration,
    source_path: &'static str,
    tests: TestDeclaration,
}

const UI_SURFACE_DOC: &str = "docs/ui-ux-surface-contract.md";

/// This file's own repo-relative path. A declared entry's `tests`
/// pointer must never equal this — that is the self-pin the honesty
/// check in `upeg-cli/src/inventory/tests.rs` rejects. Test-only: no
/// production code needs its own path as a string.
#[cfg(test)]
pub(crate) const DECLARING_FILE_PATH: &str = "upeg-cli/src/inventory/desktop_pwa_ext.rs";

/// `desktop.board.view` and `pwa.board.view` both render `BoardPage`;
/// this widget test drives that render path (embed-pin activation).
const BOARD_PAGE_WIDGET_TEST: &str =
    "flutter_app/test/widget_tests/board_page_activation_test.dart";
/// `desktop.tool.expanded` renders `ExpandedModalPage`.
const EXPANDED_MODAL_WIDGET_TEST: &str =
    "flutter_app/test/widget_tests/expanded_modal_page_test.dart";
/// Chrome-extension static-asset contract tests (popup + content script).
const CHROME_EXT_INTEGRATION_TEST: &str = "upeg-cli/tests/chrome_ext.rs";
/// The in-page selector adapter's behaviour lives in a dependency-free
/// `node --test` file (`just test-chrome-ext-file-input` runs it), which has
/// no Rust `fn`/Dart `'case'` symbol for the honesty check to point at.
const CHROME_EXT_SELECTOR_ADAPTER_TEST: &str = "chrome-ext/tests/selector_adapter.test.js";
/// `desktop_deep_link` is unit-tested next to its own definition.
const DESKTOP_DEEP_LINK_SOURCE_TEST: &str = "upeg-pegboard-ui/src/deep_link.rs";
/// `pwa.service-worker.cache` is driven end to end by the headless
/// Chromium contract check behind `just flutter-web-smoke`: it asserts
/// the service worker activates and controls the page, that the app
/// shell cache holds the boot set, and that a reload with the static
/// server stopped still boots from cache.
const PWA_SERVICE_WORKER_CONTRACT_CHECK: &str = "scripts/flutter_web_smoke_check.mjs";

const DESKTOP_PWA_EXT_DECLARATIONS: &[SurfaceInventoryDeclaration] = &[
    SurfaceInventoryDeclaration {
        id: "desktop.board.view",
        surface: Surface::Desktop,
        kind: InterfaceKind::DesktopComponent,
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#display-anatomy"),
            command_path: Some("flutter_app/lib/src/pages/board_page.dart#BoardPage"),
            http_method: None,
            http_path: None,
        },
        source_path: "flutter_app/lib/src/pages/board_page.dart",
        tests: TestDeclaration::Covered {
            path: BOARD_PAGE_WIDGET_TEST,
            test_name: "보드에_핀된_embed는_인라인_핀으로_포커스된다",
        },
    },
    SurfaceInventoryDeclaration {
        id: "desktop.tool.expanded",
        surface: Surface::Desktop,
        kind: InterfaceKind::DesktopComponent,
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg_core::InputSpec"),
            command_path: Some(
                "flutter_app/lib/src/pages/expanded_modal_page.dart#ExpandedModalPage",
            ),
            http_method: None,
            http_path: None,
        },
        source_path: "flutter_app/lib/src/pages/expanded_modal_page.dart",
        tests: TestDeclaration::Covered {
            path: EXPANDED_MODAL_WIDGET_TEST,
            test_name: "ExpandedModalPage_는_tool_헤더와_run_버튼을_표시한다",
        },
    },
    SurfaceInventoryDeclaration {
        id: "pwa.board.view",
        surface: Surface::Pwa,
        kind: InterfaceKind::PwaComponent,
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#display-anatomy"),
            command_path: Some("flutter_app/lib/src/pages/board_page.dart#BoardPage"),
            http_method: None,
            http_path: None,
        },
        source_path: "flutter_app/lib/src/pages/board_page.dart",
        tests: TestDeclaration::Covered {
            path: BOARD_PAGE_WIDGET_TEST,
            test_name: "보드에_핀된_embed는_인라인_핀으로_포커스된다",
        },
    },
    SurfaceInventoryDeclaration {
        id: "pwa.service-worker.cache",
        surface: Surface::Pwa,
        kind: InterfaceKind::PwaComponent,
        contract: ContractDeclaration {
            input_schema_ref: Some("flutter_app/web/manifest.json"),
            command_path: Some("flutter_app/web/flutter_bootstrap.js#upeg_service_worker.js"),
            http_method: Some("GET"),
            http_path: Some("/upeg_service_worker.js"),
        },
        source_path: "flutter_app/web/upeg_service_worker.js",
        tests: TestDeclaration::CoveredByScript {
            path: PWA_SERVICE_WORKER_CONTRACT_CHECK,
        },
    },
    SurfaceInventoryDeclaration {
        id: "ext.popup.board",
        surface: Surface::Ext,
        kind: InterfaceKind::ChromeExtension,
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#chrome-extension-contract"),
            command_path: Some("chrome-ext/popup.html#board-tabs"),
            http_method: None,
            http_path: None,
        },
        source_path: "chrome-ext/popup.html",
        tests: TestDeclaration::Covered {
            path: CHROME_EXT_INTEGRATION_TEST,
            test_name: "popup_html은_desktop_딥_링크_진입점을_제공한다",
        },
    },
    // Was `ext.content.hex-tooltip`: the content script no longer carries
    // one hard-coded rule. `chrome-ext/detectors.js` is a typed table and
    // the entry now names the table, not the single detector it started as.
    SurfaceInventoryDeclaration {
        id: "ext.content.detectors",
        surface: Surface::Ext,
        kind: InterfaceKind::ChromeExtension,
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#chrome-extension-contract"),
            command_path: Some("chrome-ext/detectors.js#DETECTORS"),
            http_method: None,
            http_path: None,
        },
        source_path: "chrome-ext/detectors.js",
        tests: TestDeclaration::Covered {
            path: CHROME_EXT_INTEGRATION_TEST,
            test_name: "감지기_표가_이름한_도구_id와_인자는_toolbox에_실재한다",
        },
    },
    // The extension's own Controlled Embed runner: Desktop drives a webview
    // it owns, this drives the tab the user is already on. Same
    // `SelectorBinding` rows, same DOM semantics.
    SurfaceInventoryDeclaration {
        id: "ext.content.selector-adapter",
        surface: Surface::Ext,
        kind: InterfaceKind::ChromeExtension,
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg_core::SelectorBinding"),
            command_path: Some("chrome-ext/selector_adapter.js#planSelectorApplication"),
            http_method: None,
            http_path: None,
        },
        source_path: "chrome-ext/selector_adapter.js",
        tests: TestDeclaration::CoveredByScript {
            path: CHROME_EXT_SELECTOR_ADAPTER_TEST,
        },
    },
    // Which sites the in-page capabilities are allowed to run on.
    SurfaceInventoryDeclaration {
        id: "ext.site.enablement",
        surface: Surface::Ext,
        kind: InterfaceKind::ChromeExtension,
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#chrome-extension-contract"),
            command_path: Some("chrome-ext/site_access.js#syncContentScripts"),
            http_method: None,
            http_path: None,
        },
        source_path: "chrome-ext/site_access.js",
        tests: TestDeclaration::Covered {
            path: CHROME_EXT_INTEGRATION_TEST,
            test_name: "사이트_접근_모듈은_동적_등록_api를_사용한다",
        },
    },
    SurfaceInventoryDeclaration {
        id: "ext.deep-link.open",
        surface: Surface::Ext,
        kind: InterfaceKind::ChromeExtension,
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg://open?surface=ext&board=<board>&tool=<tool>"),
            command_path: Some("upeg_pegboard_ui::deep_link::desktop_deep_link"),
            http_method: None,
            http_path: None,
        },
        source_path: "upeg-pegboard-ui/src/deep_link.rs",
        tests: TestDeclaration::Covered {
            path: DESKTOP_DEEP_LINK_SOURCE_TEST,
            test_name: "desktop_딥_링크는_보드와_도구와_입력을_인코딩한다",
        },
    },
];

impl SurfaceInventoryDeclaration {
    fn entry(self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(self.surface),
            kind: self.kind,
            contract: self.contract.shape(),
            version: "v1".to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(UI_SURFACE_DOC.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(UI_SURFACE_DOC.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some(self.source_path.to_string()),
                url: None,
            },
            tests: self.tests.mapping(),
        }
    }
}

/// Static Desktop/PWA/Chrome-extension interface contracts.
#[must_use]
pub fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    DESKTOP_PWA_EXT_DECLARATIONS
        .iter()
        .copied()
        .map(SurfaceInventoryDeclaration::entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use upeg_core::interface_inventory::{INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceInventory};

    #[test]
    fn 인터페이스_인벤토리는_desktop_pwa_ext를_포함한다() {
        let inventory = InterfaceInventory {
            schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
            entries: interface_inventory_entries(),
        };

        inventory.validate().expect("valid UI surface inventory");

        let ids: HashSet<_> = inventory
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        for id in [
            "desktop.board.view",
            "desktop.tool.expanded",
            "pwa.board.view",
            "pwa.service-worker.cache",
            "ext.popup.board",
            "ext.content.detectors",
            "ext.content.selector-adapter",
            "ext.site.enablement",
            "ext.deep-link.open",
        ] {
            assert!(ids.contains(id), "missing inventory entry {id}");
        }

        let surfaces: HashSet<_> = inventory
            .entries
            .iter()
            .flat_map(|entry| entry.surfaces.iter().copied())
            .collect();
        assert!(surfaces.contains(&Surface::Desktop));
        assert!(surfaces.contains(&Surface::Pwa));
        assert!(surfaces.contains(&Surface::Ext));

        for entry in &inventory.entries {
            assert_eq!(entry.version, "v1");
            assert_eq!(entry.compatibility, Compatibility::Stable);
            assert!(entry.owner.path.is_some(), "{} missing owner", entry.id);
            assert!(entry.docs.path.is_some(), "{} missing docs", entry.id);
            assert!(entry.source.path.is_some(), "{} missing source", entry.id);
            // Either a real covering test, or an explicit uncovered
            // reason — never a blank pointer and never this file.
            match &entry.tests {
                TestMapping::Covered { path, .. } => {
                    assert!(!path.is_empty(), "{} missing test path", entry.id);
                    assert_ne!(
                        path, DECLARING_FILE_PATH,
                        "{} self-pins its tests field to the declaring file",
                        entry.id
                    );
                }
                TestMapping::Uncovered { reason } => assert!(
                    !reason.trim().is_empty(),
                    "{} declares itself uncovered without a reason",
                    entry.id
                ),
            }
            let [surface] = entry.surfaces.as_ref() else {
                panic!("{} must advertise exactly one surface", entry.id);
            };
            match surface {
                Surface::Desktop => assert_eq!(entry.kind, InterfaceKind::DesktopComponent),
                Surface::Pwa => assert_eq!(entry.kind, InterfaceKind::PwaComponent),
                Surface::Ext => assert_eq!(entry.kind, InterfaceKind::ChromeExtension),
                other => panic!("unexpected surface in desktop inventory: {other:?}"),
            }
        }

        let extension_ids: HashSet<_> = inventory
            .entries
            .iter()
            .filter(|entry| entry.surfaces.contains(&Surface::Ext))
            .map(|entry| entry.id.as_str())
            .collect();
        assert!(extension_ids.contains("ext.popup.board"));
        assert!(extension_ids.contains("ext.content.detectors"));
        assert!(extension_ids.contains("ext.content.selector-adapter"));
        assert!(extension_ids.contains("ext.site.enablement"));
        assert!(extension_ids.contains("ext.deep-link.open"));
    }

    #[test]
    fn 인터페이스_인벤토리_desktop_pwa는_각자_서로_다르다() {
        let entries = interface_inventory_entries();
        let desktop = entries
            .iter()
            .find(|entry| entry.id == "desktop.board.view")
            .expect("desktop board view entry");
        let pwa = entries
            .iter()
            .find(|entry| entry.id == "pwa.board.view")
            .expect("pwa board view entry");

        assert_ne!(desktop.id, pwa.id);
        assert_eq!(desktop.surfaces, SurfaceSet::single(Surface::Desktop));
        assert_eq!(pwa.surfaces, SurfaceSet::single(Surface::Pwa));
        assert_eq!(desktop.kind, InterfaceKind::DesktopComponent);
        assert_eq!(pwa.kind, InterfaceKind::PwaComponent);
        assert_eq!(desktop.source.path, pwa.source.path);
        assert_eq!(
            desktop.contract.locator.command_path,
            pwa.contract.locator.command_path
        );
    }
}
