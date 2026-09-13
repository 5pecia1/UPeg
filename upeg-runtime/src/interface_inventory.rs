//! Runtime collectors for generated interface inventory entries.

use upeg_core::interface_inventory::{
    Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
    InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
};
use upeg_core::{ALL_SURFACES, Invoker, Surface, ToolMeta, ToolkitMeta};

use crate::{toolbox_toolkits, toolbox_tools};

const VERSION: &str = "v1";
const OWNER_PATH: &str = "upeg-runtime/Cargo.toml";
const DOCS_PATH: &str = "docs/LEXICON.md";
const RUNTIME_MANIFEST_SOURCE_PATH: &str = "upeg-runtime/src/manifest.rs";
const RUNTIME_REGISTRY_SOURCE_PATH: &str = "upeg-runtime/src/toolbox.rs";
const TOOL_MACRO_SOURCE_PATH: &str = "upeg-tools/src/lib.rs";
const TOML_MANIFEST_SOURCE_PATH: &str = "upeg-loader/src/model.rs";
const WASM_MANIFEST_SOURCE_PATH: &str = "upeg-plugin-api/src/lib.rs";
const TEST_PATH: &str = "upeg-runtime/src/interface_inventory.rs";
const BUILTIN_TEST_NAME: &str = "인터페이스_인벤토리는_내장_도구를_포함한다";
const TOOL_INPUT_SCHEMA_DESCRIPTION: &str = "Tool input schema generated from ToolMeta.input_spec";
const TOOL_OUTPUT_SCHEMA_DESCRIPTION: &str = "Tool output contract is canonical ToolResult JSON generated from ToolMeta.output_spec and ToolMeta.primary_output_id";
const NO_RUNTIME_OUTPUT_DESCRIPTION: &str =
    "Manifest schema entries describe static schema artifacts and do not emit ToolResult outputs";

/// Collect one inventory entry per registered Tool, carrying every surface
/// the Tool is advertised on.
///
/// Tools that advertise no surface are not part of any interface contract
/// and stay out of the inventory.
#[must_use]
pub fn collect_tool_entries() -> Vec<InterfaceEntry> {
    let mut entries = toolbox_tools()
        .filter(|tool| !tool.surfaces.is_empty())
        .map(|tool| {
            entry(
                tool_entry_id(tool),
                SurfaceSet::new(tool.surfaces.iter().copied()),
                InterfaceKind::Tool,
                tool_contract(tool),
                source_for_tool(tool),
                BUILTIN_TEST_NAME,
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

/// Collect one inventory entry per registered Toolkit, carrying every
/// surface used by its Tools.
#[must_use]
pub fn collect_toolkit_entries() -> Vec<InterfaceEntry> {
    let tools = toolbox_tools().collect::<Vec<_>>();
    let mut entries = toolbox_toolkits()
        .map(|toolkit| {
            entry(
                toolkit_entry_id(toolkit),
                toolkit_surfaces(toolkit, &tools),
                InterfaceKind::Toolkit,
                schema_ref_contract(
                    format!("toolkit.{}.manifest", toolkit.id),
                    "Toolkit manifest contract",
                ),
                RUNTIME_REGISTRY_SOURCE_PATH,
                BUILTIN_TEST_NAME,
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

/// Collect inventory entries for declarative TOML and WASM plugin manifest schemas.
#[must_use]
pub fn collect_manifest_schema_entries() -> Vec<InterfaceEntry> {
    let mut entries = vec![
        entry(
            "manifest_schema.toml.toolkit".to_string(),
            SurfaceSet::single(Surface::Cli),
            InterfaceKind::ManifestSchema,
            schema_ref_contract(
                "upeg-loader::model::{ToolkitToml,ToolEntryToml}".to_string(),
                "TOML manifest schema contract",
            ),
            TOML_MANIFEST_SOURCE_PATH,
            BUILTIN_TEST_NAME,
        ),
        entry(
            "manifest_schema.wasm.plugin".to_string(),
            SurfaceSet::single(Surface::Cli),
            InterfaceKind::ManifestSchema,
            schema_ref_contract(
                "upeg-plugin-api::{PluginManifest,PluginToolDecl}".to_string(),
                "WASM plugin manifest schema contract",
            ),
            WASM_MANIFEST_SOURCE_PATH,
            BUILTIN_TEST_NAME,
        ),
        entry(
            "manifest_schema.runtime.tool".to_string(),
            SurfaceSet::single(Surface::Cli),
            InterfaceKind::ManifestSchema,
            schema_ref_contract(
                "upeg-runtime::manifest::{ExternalToolManifest,RuntimeToolManifest}".to_string(),
                "Runtime tool manifest schema contract",
            ),
            RUNTIME_MANIFEST_SOURCE_PATH,
            BUILTIN_TEST_NAME,
        ),
    ];
    entries.sort();
    entries
}

fn entry(
    id: String,
    surfaces: SurfaceSet,
    kind: InterfaceKind,
    contract: ContractShape,
    source_path: &'static str,
    test_name: &'static str,
) -> InterfaceEntry {
    InterfaceEntry {
        id,
        surfaces,
        kind,
        contract,
        version: VERSION.to_string(),
        compatibility: Compatibility::Stable,
        owner: OwnerRef {
            path: Some(OWNER_PATH.to_string()),
            url: None,
        },
        docs: DocRef {
            path: Some(DOCS_PATH.to_string()),
            url: None,
        },
        source: SourceRef {
            path: Some(source_path.to_string()),
            url: None,
        },
        tests: TestMapping::covered(TEST_PATH, Some(test_name.to_string())),
    }
}

fn tool_entry_id(tool: &ToolMeta) -> String {
    format!("tool.{}.{}", tool.toolkit, tool.local_id)
}

fn toolkit_entry_id(toolkit: &ToolkitMeta) -> String {
    format!("toolkit.{}", toolkit.id)
}

fn tool_input_schema_ref(tool: &ToolMeta) -> String {
    format!("tool.{}.{}.input_schema", tool.toolkit, tool.local_id)
}

fn tool_contract(tool: &ToolMeta) -> ContractShape {
    ContractShape::new(
        ContractLocator::default(),
        ContractIo::declared(
            ContractIoKind::JsonSchema,
            Some(tool.input_spec.to_json_schema_value()),
            Some(tool_input_schema_ref(tool)),
            TOOL_INPUT_SCHEMA_DESCRIPTION,
        ),
        tool_output_contract(tool),
    )
}

fn schema_ref_contract(schema_ref: String, description: &'static str) -> ContractShape {
    ContractShape::new(
        ContractLocator::default(),
        ContractIo::declared(
            ContractIoKind::JsonSchema,
            None,
            Some(schema_ref),
            description,
        ),
        no_runtime_output_contract(),
    )
}

fn tool_output_schema_ref(tool: &ToolMeta) -> String {
    format!("tool.{}.{}.output_schema", tool.toolkit, tool.local_id)
}

fn tool_output_contract(tool: &ToolMeta) -> ContractIo {
    ContractIo::declared(
        ContractIoKind::JsonSchema,
        Some(tool_result_schema_value(tool)),
        Some(tool_output_schema_ref(tool)),
        TOOL_OUTPUT_SCHEMA_DESCRIPTION,
    )
}

fn no_runtime_output_contract() -> ContractIo {
    ContractIo::none(NO_RUNTIME_OUTPUT_DESCRIPTION)
}

fn tool_result_schema_value(tool: &ToolMeta) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["ok", "primary_output_id", "outputs"],
        "properties": {
            "ok": { "type": "boolean", "enum": [true] },
            "primary_output_id": primary_output_id_schema(tool.primary_output_id),
            "outputs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "label", "kind", "value"],
                    "properties": {
                        "id": { "type": "string" },
                        "label": { "oneOf": [{ "type": "string" }, { "type": "null" }] },
                        "kind": { "type": "string" },
                        "value": {}
                    }
                }
            }
        },
        "additionalProperties": false,
        "x-upeg-output-fields": tool.output_spec.to_json_schema_value(),
        "x-upeg-primary-output-id": tool.primary_output_id,
    })
}

fn primary_output_id_schema(primary_output_id: Option<&str>) -> serde_json::Value {
    match primary_output_id {
        Some(id) => serde_json::json!({ "type": "string", "const": id }),
        None => serde_json::json!({ "type": "null" }),
    }
}

fn toolkit_surfaces(toolkit: &ToolkitMeta, tools: &[&ToolMeta]) -> SurfaceSet {
    let surfaces = SurfaceSet::new(ALL_SURFACES.iter().copied().filter(|surface| {
        tools
            .iter()
            .any(|tool| tool.toolkit == toolkit.id && tool.surfaces.contains(surface))
    }));
    if surfaces.is_empty() {
        return SurfaceSet::new(ALL_SURFACES.iter().copied());
    }
    surfaces
}

fn source_for_tool(tool: &ToolMeta) -> &'static str {
    match tool.invoker {
        Invoker::Function => TOOL_MACRO_SOURCE_PATH,
        Invoker::Wasm => WASM_MANIFEST_SOURCE_PATH,
        // Static (Passive Embed) and Embed (Controlled Embed) are both
        // declared in the runtime manifest path — neither has a
        // headless dispatcher today.
        Invoker::External
        | Invoker::Http
        | Invoker::Static
        | Invoker::Embed
        | Invoker::Chain
        | Invoker::Llm => RUNTIME_MANIFEST_SOURCE_PATH,
    }
}

#[cfg(test)]
#[path = "interface_inventory_file_wire_test.rs"]
mod interface_inventory_file_wire_test;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{toolbox_add_tool, toolbox_add_toolkit};
    use upeg_core::interface_inventory::{INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceInventory};
    use upeg_core::{GUI_SURFACES, InputSpec, PinKind, ToolId};

    fn local_id_for(id: &'static str, toolkit: &'static str) -> &'static str {
        ToolId::parse_canonical_in_toolkit(id, toolkit)
            .expect("테스트 ToolMeta id는 정규 형식이어야 한다")
            .local()
    }

    fn test_tool(id: &'static str, toolkit: &'static str) -> ToolMeta {
        ToolMeta {
            id,
            toolkit,
            local_id: local_id_for(id, toolkit),
            tags: &["inventory"],
            display_label: "Inventory test tool",
            description: "runtime inventory fixture",
            input_spec: InputSpec::try_from(&serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "additionalProperties": false,
            }))
            .expect("테스트 입력 명세를 가져와야 한다"),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: Invoker::External,
            surfaces: &[Surface::Cli, Surface::Mcp],
            boards: &[],
        }
    }

    fn test_gui_tool(id: &'static str, toolkit: &'static str) -> ToolMeta {
        ToolMeta {
            id,
            toolkit,
            local_id: local_id_for(id, toolkit),
            tags: &["inventory", "gui-only"],
            display_label: "Inventory GUI test tool",
            description: "runtime inventory fixture for GUI-only tool",
            input_spec: InputSpec::try_from(&serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "additionalProperties": false,
            }))
            .expect("테스트 입력 명세를 가져와야 한다"),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: Invoker::External,
            surfaces: GUI_SURFACES,
            boards: &[],
        }
    }

    fn register_inventory_fixture() {
        toolbox_add_toolkit(ToolkitMeta {
            id: "inventory_test",
            tags: &["inventory"],
            description: "inventory test toolkit",
        });
        toolbox_add_tool(test_tool("inventory_test.echo", "inventory_test"));
        toolbox_add_tool(test_gui_tool("inventory_test.gui_panel", "inventory_test"));
    }

    #[test]
    fn 인터페이스_인벤토리는_내장_도구를_포함한다() {
        register_inventory_fixture();

        let tool_entries = collect_tool_entries();
        let cli_tool = tool_entries
            .iter()
            .find(|entry| {
                entry.id == "tool.inventory_test.echo"
                    && entry.surfaces.contains(&Surface::Cli)
                    && entry.kind == InterfaceKind::Tool
            })
            .expect("등록된 도구에는 CLI 인벤토리 항목이 있어야 한다");

        assert_eq!(cli_tool.version, "v1");
        assert_eq!(cli_tool.compatibility, Compatibility::Stable);
        let input = cli_tool
            .contract
            .input
            .as_ref()
            .expect("도구 항목은 입력 계약을 선언해야 한다");
        assert_eq!(input.kind, ContractIoKind::JsonSchema);
        assert_eq!(
            input.schema_ref.as_deref(),
            Some("tool.inventory_test.echo.input_schema")
        );
        assert_eq!(
            input.schema.as_ref(),
            Some(&serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "required": [],
                "additionalProperties": false,
            }))
        );
        let output = cli_tool
            .contract
            .output
            .as_ref()
            .expect("도구 항목은 출력 계약 센티널을 선언해야 한다");
        assert_eq!(output.kind, ContractIoKind::JsonSchema);
        assert!(output.declared);
        assert_eq!(
            output.schema_ref.as_deref(),
            Some("tool.inventory_test.echo.output_schema")
        );
        assert_eq!(
            output.description.as_deref(),
            Some(TOOL_OUTPUT_SCHEMA_DESCRIPTION)
        );
        let schema = output
            .schema
            .as_ref()
            .expect("도구 항목은 정식 출력 스키마를 선언해야 한다");
        assert_eq!(schema["x-upeg-primary-output-id"], serde_json::Value::Null);
        assert_eq!(schema["properties"]["primary_output_id"]["type"], "null");
        assert_eq!(
            cli_tool.source.path.as_deref(),
            Some(RUNTIME_MANIFEST_SOURCE_PATH)
        );

        assert_eq!(
            tool_entries
                .iter()
                .filter(|entry| entry.id == "tool.inventory_test.echo")
                .count(),
            1,
            "하나의 계약은 하나의 항목이어야 한다"
        );
        assert!(cli_tool.surfaces.contains(&Surface::Mcp));
        assert_eq!(cli_tool.surfaces.joined_labels(), "cli, mcp");

        let toolkit_entries = collect_toolkit_entries();
        assert!(toolkit_entries.iter().any(|entry| {
            entry.id == "toolkit.inventory_test"
                && entry.surfaces.contains(&Surface::Cli)
                && entry.kind == InterfaceKind::Toolkit
        }));

        let schema_entries = collect_manifest_schema_entries();
        assert!(schema_entries.iter().any(|entry| {
            entry.id == "manifest_schema.toml.toolkit"
                && entry.kind == InterfaceKind::ManifestSchema
        }));
        assert!(schema_entries.iter().any(|entry| {
            entry.id == "manifest_schema.wasm.plugin" && entry.kind == InterfaceKind::ManifestSchema
        }));
    }

    #[test]
    fn 인터페이스_인벤토리_도구는_테스트_매핑을_요구한다() {
        register_inventory_fixture();

        let entries = collect_tool_entries()
            .into_iter()
            .chain(collect_toolkit_entries())
            .chain(collect_manifest_schema_entries())
            .collect::<Vec<_>>();

        for entry in &entries {
            assert_eq!(entry.tests.path(), Some(TEST_PATH));
            assert_eq!(entry.tests.test_name(), Some(BUILTIN_TEST_NAME));
        }

        InterfaceInventory {
            schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
            entries,
        }
        .validate()
        .expect("생성된 런타임 인터페이스 인벤토리 항목은 검증되어야 한다");
    }

    #[test]
    fn gui_전용_도구는_headless_인벤토리에_나타나지_않는다() {
        register_inventory_fixture();

        let tool_entries = collect_tool_entries();

        let gui_tool = tool_entries
            .iter()
            .find(|entry| entry.id == "tool.inventory_test.gui_panel")
            .expect("GUI 전용 도구에도 인벤토리 항목이 하나 있어야 한다");

        assert!(gui_tool.surfaces.contains(&Surface::Desktop));
        assert!(gui_tool.surfaces.contains(&Surface::Pwa));
        assert!(gui_tool.surfaces.contains(&Surface::Ext));

        assert!(!gui_tool.surfaces.contains(&Surface::Cli));
        assert!(!gui_tool.surfaces.contains(&Surface::Tui));
        assert!(!gui_tool.surfaces.contains(&Surface::Mcp));
        assert!(!gui_tool.surfaces.contains(&Surface::Http));
    }

    #[test]
    fn 하나의_도구_계약은_하나의_인벤토리_항목으로_모인다() {
        register_inventory_fixture();

        let tool_entries = collect_tool_entries();
        let mut keys = tool_entries
            .iter()
            .map(|entry| (entry.kind, entry.id.as_str()))
            .collect::<Vec<_>>();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();

        assert_eq!(total, keys.len(), "(kind, id)는 인벤토리에서 유일해야 한다");
        assert!(
            tool_entries.iter().all(|entry| !entry.surfaces.is_empty()),
            "모든 인벤토리 항목은 최소 하나의 표면을 광고해야 한다"
        );
    }

    #[test]
    fn 출력_계약은_정식_tool_result_schema로_기록된다() {
        register_inventory_fixture();

        let tool_entries = collect_tool_entries();
        let cli_tool = tool_entries
            .iter()
            .find(|entry| {
                entry.id == "tool.inventory_test.echo" && entry.surfaces.contains(&Surface::Cli)
            })
            .expect("테스트 도구 항목이 존재해야 한다");

        let output = cli_tool
            .contract
            .output
            .as_ref()
            .expect("출력 계약을 가져와야 한다");

        assert_eq!(
            output.description.as_deref(),
            Some(TOOL_OUTPUT_SCHEMA_DESCRIPTION),
            "출력 계약 설명은 정식 ToolResult 모델과 일치해야 한다"
        );
        assert_eq!(output.kind, ContractIoKind::JsonSchema);
        assert!(output.declared);
    }
}
