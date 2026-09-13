//! Tool provenance — where a registered tool came from.
//!
//! First-class `source` field on the tools_list JSON contract so every
//! surface (HTTP `/v1/tools`, MCP `tools/list`, CLI `tool list --json`)
//! can tell a link-time/local tool from an MCP-imported proxy without
//! parsing toolkit ids. Registered by the source loaders (today:
//! `upeg-sources::mcp_import`); everything unregistered is `Local`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Origin of a registered tool.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ToolProvenance {
    /// Built-in inventory, TOML toolkit, WASM plugin — anything running
    /// in-process without proxying to another tool server.
    Local,
    /// Proxied from an upstream MCP server registered via mcp-imports.
    McpImport { server: String },
    /// Declared by the Project Manifest (`upeg.toml`) resolved for this
    /// process — see `upeg_sources::project`. Distinct from [`Self::Local`]
    /// because a project tool is only defined *here*: a host started from
    /// another directory has never heard of it, so the CLI must dispatch
    /// it in-process instead of auto-attaching (D-1).
    ProjectManifest { path: String },
}

/// `source` label prefix for MCP-imported tools (`mcp-import:<server>`).
const MCP_IMPORT_LABEL_PREFIX: &str = "mcp-import:";
/// `source` label prefix for Project Manifest tools (`project-manifest:<path>`).
const PROJECT_MANIFEST_LABEL_PREFIX: &str = "project-manifest:";
/// `source` label for in-process tools.
const LOCAL_LABEL: &str = "local";

impl ToolProvenance {
    /// Canonical `source` label: `"local"`, `"mcp-import:<server>"`, or
    /// `"project-manifest:<path>"`.
    pub fn label(&self) -> String {
        match self {
            Self::Local => LOCAL_LABEL.to_string(),
            Self::McpImport { server } => format!("{MCP_IMPORT_LABEL_PREFIX}{server}"),
            Self::ProjectManifest { path } => format!("{PROJECT_MANIFEST_LABEL_PREFIX}{path}"),
        }
    }

    /// Whether this tool exists only because of a Project Manifest.
    /// The CLI's attach decision reads this: a project tool is dispatched
    /// locally even when a host is reachable, because the host resolved
    /// its own (or no) `upeg.toml`.
    pub const fn is_project_manifest(&self) -> bool {
        matches!(self, Self::ProjectManifest { .. })
    }
}

fn provenance_lock() -> &'static Mutex<HashMap<String, ToolProvenance>> {
    static PROVENANCE: OnceLock<Mutex<HashMap<String, ToolProvenance>>> = OnceLock::new();
    PROVENANCE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record where `tool_id` came from. Later registrations replace earlier
/// ones (mirrors toolbox re-registration semantics).
pub fn register_tool_provenance(tool_id: &str, provenance: ToolProvenance) {
    if let Ok(mut guard) = provenance_lock().lock() {
        guard.insert(tool_id.to_string(), provenance);
    }
}

/// Drop the provenance record for `tool_id` (source unload path).
pub fn clear_tool_provenance(tool_id: &str) {
    if let Ok(mut guard) = provenance_lock().lock() {
        guard.remove(tool_id);
    }
}

/// How many registered tools came from an upstream MCP server.
///
/// Process-local by definition: it counts what *this* process imported.
/// A surface that merely attaches to a separate host sees 0, which is
/// the honest answer for its own Toolbox.
pub fn mcp_import_tool_count() -> usize {
    provenance_lock()
        .lock()
        .map(|guard| {
            guard
                .values()
                .filter(|provenance| matches!(provenance, ToolProvenance::McpImport { .. }))
                .count()
        })
        .unwrap_or(0)
}

/// Provenance for `tool_id`. Unregistered ids are [`ToolProvenance::Local`].
pub fn tool_provenance(tool_id: &str) -> ToolProvenance {
    provenance_lock()
        .lock()
        .ok()
        .and_then(|guard| guard.get(tool_id).cloned())
        .unwrap_or(ToolProvenance::Local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, PoisonError};

    /// Every test below reads/writes the ONE process-global provenance map
    /// behind [`provenance_lock`]. Cargo runs tests on separate threads by
    /// default, so two of these tests interleaving (e.g. one snapshots
    /// `mcp_import_tool_count()` as "before" while another registers or
    /// clears an unrelated `McpImport` entry) makes the count-delta
    /// assertions flaky. Locking this guard at the top of every test
    /// serializes them; poison handling means one test panicking
    /// mid-registration doesn't cascade-fail the rest of the module.
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    fn lock_test_guard() -> MutexGuard<'static, ()> {
        TEST_GUARD.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[test]
    fn mcp_import_도구_수는_import_provenance만_센다() {
        let _guard = lock_test_guard();
        let imported = "test.provenance.counted_import";
        let local = "test.provenance.counted_local";
        let before = mcp_import_tool_count();

        register_tool_provenance(
            imported,
            ToolProvenance::McpImport {
                server: "upstream".to_string(),
            },
        );
        register_tool_provenance(local, ToolProvenance::Local);
        assert_eq!(mcp_import_tool_count(), before + 1);

        clear_tool_provenance(imported);
        clear_tool_provenance(local);
        assert_eq!(mcp_import_tool_count(), before);
    }

    #[test]
    fn project_manifest_provenance는_경로를_라벨에_담고_스스로를_알린다() {
        let _guard = lock_test_guard();
        let id = "test.provenance.project_tool";
        let manifest = "/workspaces/upeg/upeg.toml";
        let imports_before = mcp_import_tool_count();

        register_tool_provenance(
            id,
            ToolProvenance::ProjectManifest {
                path: manifest.to_string(),
            },
        );

        let provenance = tool_provenance(id);
        assert_eq!(provenance.label(), format!("project-manifest:{manifest}"));
        assert!(
            provenance.is_project_manifest(),
            "project manifest 도구는 스스로를 project manifest 출신이라고 답해야 한다"
        );
        assert!(
            !ToolProvenance::Local.is_project_manifest(),
            "local 도구는 project manifest 출신이 아니다"
        );
        assert_eq!(
            mcp_import_tool_count(),
            imports_before,
            "project manifest provenance는 mcp import 집계에 섞이지 않는다"
        );

        clear_tool_provenance(id);
        assert_eq!(tool_provenance(id), ToolProvenance::Local);
    }

    #[test]
    fn 미등록_도구의_provenance는_local이다() {
        let _guard = lock_test_guard();
        assert_eq!(
            tool_provenance("test.provenance.unregistered"),
            ToolProvenance::Local
        );
        assert_eq!(ToolProvenance::Local.label(), "local");
    }

    #[test]
    fn tools_list_json_객체는_source_필드로_provenance를_노출한다() {
        let _guard = lock_test_guard();
        use crate::ToolMetaRuntimeExt as _;

        let id = "test.provenance.json_field";
        let meta = upeg_core::ToolMeta {
            id,
            toolkit: "test",
            local_id: "provenance.json_field",
            tags: &[],
            display_label: "Provenance fixture",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker: upeg_core::Invoker::Function,
            surfaces: upeg_core::ALL_SURFACES,
            boards: &[],
        };

        assert_eq!(meta.to_json_object("name")["source"], "local");

        register_tool_provenance(
            id,
            ToolProvenance::McpImport {
                server: "upstream".into(),
            },
        );
        assert_eq!(meta.to_json_object("name")["source"], "mcp-import:upstream");
        clear_tool_provenance(id);
    }

    #[test]
    fn mcp_import_provenance는_서버_이름을_라벨에_담는다() {
        let _guard = lock_test_guard();
        let id = "test.provenance.imported";
        register_tool_provenance(
            id,
            ToolProvenance::McpImport {
                server: "github".into(),
            },
        );
        assert_eq!(tool_provenance(id).label(), "mcp-import:github");
        clear_tool_provenance(id);
        assert_eq!(tool_provenance(id), ToolProvenance::Local);
    }
}
