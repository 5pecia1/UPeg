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

const UI_SURFACE_DOC: &str = "docs/ui-ux-surface-contract.md";
/// Section of [`UI_SURFACE_DOC`] that specifies the approval gesture and
/// the live-output pane the two entries below declare.
const APPROVAL_AND_LIVE_OUTPUT_DOC: &str =
    "docs/ui-ux-surface-contract.md#approval-and-live-output";
const TUI_INTERFACE_TEST: &str = "upeg-cli/src/surfaces/tui/interface_tests.rs";
/// The test inside [`TUI_INTERFACE_TEST`] that actually asserts these
/// entries — pinned by the inventory honesty check.
const TUI_INTERFACE_TEST_NAME: &str = "tui_인터페이스_인벤토리는_안정적인_형태로_직렬화된다";

const TUI_INTERFACE_DECLARATIONS: &[TuiInterfaceDeclaration] = &[
    TuiInterfaceDeclaration {
        id: "tui.board.list",
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#display-anatomy"),
            command_path: Some("upeg_cli::surfaces::tui::view::render_board_bar"),
        },
        source_path: "upeg-cli/src/surfaces/tui/view.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tag.filter",
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#canonical-lifecycle-verbs"),
            command_path: Some("upeg_cli::surfaces::tui::update::apply_filter_key"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.detail",
        contract: ContractDeclaration {
            input_schema_ref: Some("docs/ui-ux-surface-contract.md#canonical-lifecycle-verbs"),
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
            input_schema_ref: Some(APPROVAL_AND_LIVE_OUTPUT_DOC),
            command_path: Some("upeg_cli::surfaces::tui::update::dispatch_or_confirm"),
        },
        source_path: "upeg-cli/src/surfaces/tui/update.rs",
    },
    TuiInterfaceDeclaration {
        id: "tui.tool.live_output",
        contract: ContractDeclaration {
            input_schema_ref: Some(APPROVAL_AND_LIVE_OUTPUT_DOC),
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
