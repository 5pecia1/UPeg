//! Interface-inventory declarations for the MCP import source.
//!
//! Declares the `mcp-import.UpstreamConfig` configuration contract so
//! the CLI's `interface inventory` gates track the MCP import surface
//! alongside CLI/HTTP/MCP contracts.

use upeg_core::{
    Surface,
    interface_inventory::{
        Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
        InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
    },
};

const MCP_IMPORT_VERSION: &str = "v1";
const MCP_IMPORT_OWNER_PATH: &str = "upeg-cli/Cargo.toml";
const MCP_IMPORT_DOCS_PATH: &str = "README.md";
const MCP_IMPORT_TEST_PATH: &str = "upeg-cli/src/inventory/tests.rs";
/// The test inside [`MCP_IMPORT_TEST_PATH`] that actually asserts this
/// entry — pinned by the inventory honesty check.
const MCP_IMPORT_TEST_NAME: &str = "인터페이스_인벤토리는_cli_http_mcp를_완전히_포함한다";

struct McpImportContractDeclaration {
    id: &'static str,
    contract_ref: &'static str,
}

const MCP_IMPORT_CONTRACTS: &[McpImportContractDeclaration] = &[McpImportContractDeclaration {
    id: "mcp.import.upstream-config",
    contract_ref: "mcp-import.UpstreamConfig",
}];

pub fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    MCP_IMPORT_CONTRACTS
        .iter()
        .map(McpImportContractDeclaration::interface_entry)
        .collect()
}

impl McpImportContractDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(Surface::Mcp),
            kind: InterfaceKind::McpImport,
            contract: ContractShape::new(
                ContractLocator::default(),
                ContractIo::declared(
                    ContractIoKind::JsonSchema,
                    None,
                    Some(self.contract_ref.to_string()),
                    "MCP import configuration schema",
                ),
                ContractIo::not_declared(
                    "MCP import entries declare configuration schemas; imported tools expose canonical ToolResult outputs",
                ),
            ),
            version: MCP_IMPORT_VERSION.to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(MCP_IMPORT_OWNER_PATH.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(MCP_IMPORT_DOCS_PATH.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some("upeg-sources/src/mcp_import.rs".to_string()),
                url: None,
            },
            tests: TestMapping::covered(
                MCP_IMPORT_TEST_PATH,
                Some(MCP_IMPORT_TEST_NAME.to_string()),
            ),
        }
    }
}
