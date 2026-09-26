use upeg_core::Surface;
use upeg_core::interface_inventory::{
    Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
    InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
};

#[derive(Clone, Copy)]
struct ContractDeclaration {
    input_schema_ref: Option<&'static str>,
    command_path: Option<&'static str>,
}

impl ContractDeclaration {
    fn shape(self) -> ContractShape {
        let locator = self
            .command_path
            .map_or_else(ContractLocator::default, ContractLocator::command_path);
        let input = self.input_schema_ref.map_or_else(
            || ContractIo::none("TUI interaction declares no structured input"),
            |schema_ref| {
                ContractIo::declared(
                    ContractIoKind::UiInteraction,
                    None,
                    Some(schema_ref.to_string()),
                    "TUI interaction contract",
                )
            },
        );
        ContractShape::new(
            locator,
            input,
            ContractIo::not_declared(
                "TUI output presentation renders canonical ToolResult rows from dispatched tools",
            ),
        )
    }
}

#[derive(Clone, Copy)]
struct TuiInterfaceDeclaration {
    id: &'static str,
    contract: ContractDeclaration,
    source_path: &'static str,
}

/// The one-way doc pointer carried on `owner`/`docs` — the public
/// big-picture map these contracts summarize. Per-contract detail lives
/// in the code the `schema_ref` locators point at.
const SURFACE_OVERVIEW_DOC: &str = "docs/architecture.md";
/// The approval policy + live-output contract the two entries below
/// declare — owned by `upeg_runtime::approval`/`upeg_runtime::progress`
/// module docs.
const APPROVAL_POLICY_REF: &str = "upeg-runtime/src/approval.rs#ToolApprovalPolicy";
const TUI_INTERFACE_TEST: &str = "upeg-cli/src/surfaces/tui/interface_tests.rs";
/// The test inside [`TUI_INTERFACE_TEST`] that actually asserts these
/// entries — pinned by the inventory honesty check.
const TUI_INTERFACE_TEST_NAME: &str = "tui_interface_inventory_serializes_to_stable_shape";

const TUI_INTERFACE_DECLARATIONS: &[TuiInterfaceDeclaration] = &[
    TuiInterfaceDeclaration {
        id: "tui.board.list",
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg-core/src/ux.rs#display_label"),
            command_path: Some("upeg_cli::surfaces::tui::view::render_board_bar"),
        },
        source_path: "upeg-cli/src/surfaces/tui/view.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tag.filter",
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg-core/src/keyboard_catalog.rs#binding_catalog"),
            command_path: Some("upeg_cli::surfaces::tui::update::apply_filter_key"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.detail",
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg-core/src/keyboard_catalog.rs#binding_catalog"),
            command_path: Some("upeg_cli::surfaces::tui::update::handle_key"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.form",
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg_core::InputSpec"),
            command_path: Some("upeg_cli::surfaces::tui::model::TuiFormState"),
        },
        source_path: "upeg-cli/src/surfaces/tui/model.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.dispatch",
        contract: ContractDeclaration {
            input_schema_ref: Some("upeg_core::InputSpec"),
            command_path: Some("upeg_cli::surfaces::tui::effects::Effect::Dispatch"),
        },
        source_path: "upeg-cli/src/surfaces/tui/effects.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.approval",
        contract: ContractDeclaration {
            input_schema_ref: Some(APPROVAL_POLICY_REF),
            command_path: Some("upeg_cli::surfaces::tui::update::dispatch_or_confirm"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.live_output",
        contract: ContractDeclaration {
            input_schema_ref: Some(APPROVAL_POLICY_REF),
            command_path: Some("upeg_cli::surfaces::tui::update::apply_progress"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
];

impl TuiInterfaceDeclaration {
    fn entry(self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(Surface::Tui),
            kind: InterfaceKind::TuiInteraction,
            contract: self.contract.shape(),
            version: "v1".to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(SURFACE_OVERVIEW_DOC.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(SURFACE_OVERVIEW_DOC.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some(self.source_path.to_string()),
                url: None,
            },
            tests: TestMapping::covered(
                TUI_INTERFACE_TEST,
                Some(TUI_INTERFACE_TEST_NAME.to_string()),
            ),
        }
    }
}

pub(crate) fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    TUI_INTERFACE_DECLARATIONS
        .iter()
        .copied()
        .map(TuiInterfaceDeclaration::entry)
        .collect()
}
