use upeg_core::{
    Surface,
    interface_inventory::{
        Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
        InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
    },
};

const CLI_SURFACE_VERSION: &str = "v1";
const CLI_OWNER_PATH: &str = "upeg-cli/Cargo.toml";
const CLI_DOCS_PATH: &str = "README.md";
const CLI_TEST_PATH: &str = "upeg-cli/src/inventory/tests.rs";
/// The test inside [`CLI_TEST_PATH`] that actually asserts these
/// entries. Pinned by the inventory honesty check, which fails when
/// no `fn <name>` with this spelling exists in that file.
const CLI_TEST_NAME: &str = "interface_inventory_covers_cli_http_and_mcp";

struct CliCommandDeclaration {
    id: &'static str,
    command_path: &'static str,
}

const CLI_COMMANDS: &[CliCommandDeclaration] = &[
    CliCommandDeclaration {
        id: "cli.tool.list",
        command_path: "tool.list",
    },
    CliCommandDeclaration {
        id: "cli.call",
        command_path: "call",
    },
    CliCommandDeclaration {
        id: "cli.board.list",
        command_path: "board.list",
    },
    CliCommandDeclaration {
        id: "cli.board.call",
        command_path: "board.call",
    },
    CliCommandDeclaration {
        id: "cli.board.pin",
        command_path: "board.pin",
    },
    CliCommandDeclaration {
        id: "cli.board.unpin",
        command_path: "board.unpin",
    },
    CliCommandDeclaration {
        id: "cli.board.move",
        command_path: "board.move",
    },
    CliCommandDeclaration {
        id: "cli.board.context",
        command_path: "board.context",
    },
    CliCommandDeclaration {
        id: "cli.board.connect",
        command_path: "board.connect",
    },
    CliCommandDeclaration {
        id: "cli.board.describe",
        command_path: "board.describe",
    },
    CliCommandDeclaration {
        id: "cli.board.create",
        command_path: "board.create",
    },
    CliCommandDeclaration {
        id: "cli.board.rename",
        command_path: "board.rename",
    },
    CliCommandDeclaration {
        id: "cli.board.delete",
        command_path: "board.delete",
    },
    CliCommandDeclaration {
        id: "cli.board.pin-list",
        command_path: "board.pin-list",
    },
    CliCommandDeclaration {
        id: "cli.board.pin-show",
        command_path: "board.pin-show",
    },
    CliCommandDeclaration {
        id: "cli.board.pin-edit",
        command_path: "board.pin-edit",
    },
    CliCommandDeclaration {
        id: "cli.settings.get",
        command_path: "settings.get",
    },
    CliCommandDeclaration {
        id: "cli.settings.set",
        command_path: "settings.set",
    },
    CliCommandDeclaration {
        id: "cli.host.start",
        command_path: "host.start",
    },
    CliCommandDeclaration {
        id: "cli.host.status",
        command_path: "host.status",
    },
    CliCommandDeclaration {
        id: "cli.host.stop",
        command_path: "host.stop",
    },
];

pub(crate) fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    CLI_COMMANDS
        .iter()
        .map(CliCommandDeclaration::interface_entry)
        .collect()
}

impl CliCommandDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(Surface::Cli),
            kind: InterfaceKind::CliCommand,
            contract: ContractShape::new(
                ContractLocator::command_path(self.command_path),
                ContractIo::declared(
                    ContractIoKind::CommandArgs,
                    None,
                    None,
                    "CLI command arguments",
                ),
                ContractIo::not_declared(
                    "CLI command output is command-specific; tool calls render canonical ToolResult values by output mode",
                ),
            ),
            version: CLI_SURFACE_VERSION.to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(CLI_OWNER_PATH.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(CLI_DOCS_PATH.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some("upeg-cli/src/surfaces/cli/mod.rs".to_string()),
                url: None,
            },
            tests: TestMapping::covered(CLI_TEST_PATH, Some(CLI_TEST_NAME.to_string())),
        }
    }
}
