use std::path::{Path, PathBuf};

use upeg_core::{
    StaticToolMeta, Surface,
    interface_inventory::{
        Compatibility, ContractIoKind, ContractShape, DocRef, INTERFACE_INVENTORY_SCHEMA_VERSION,
        InterfaceEntry, InterfaceInventory, InterfaceInventoryError, InterfaceKind, OwnerRef,
        SourceRef, SurfaceSet, TestMapping,
    },
};

use clap::Parser;

use crate::adapters::mcp_import;
use crate::inventory::command as interface_inventory_command;
use crate::inventory::desktop_pwa_ext;
use crate::{Cli, run, surfaces};

fn all_surface_entries() -> Vec<InterfaceEntry> {
    let mut entries = surfaces::cli::interface_inventory_entries();
    entries.extend(surfaces::http::interface_inventory_entries());
    entries.extend(surfaces::mcp::interface_inventory_entries());
    entries.extend(mcp_import::interface_inventory_entries());
    entries
}

/// Every hand-written surface declaration in the workspace — the four
/// in [`all_surface_entries`] plus TUI and desktop/PWA/ext. Tool and
/// toolkit entries are derived from manifests and are deliberately out:
/// they cannot carry a stale hand-typed pointer.
fn all_declared_surface_entries() -> Vec<InterfaceEntry> {
    let mut entries = all_surface_entries();
    entries.extend(surfaces::tui::interface_inventory_entries());
    entries.extend(desktop_pwa_ext::interface_inventory_entries());
    entries
}

fn find_entry<'a>(
    entries: &'a [InterfaceEntry],
    id: &str,
    kind: InterfaceKind,
) -> &'a InterfaceEntry {
    entries
        .iter()
        .find(|entry| entry.id == id && entry.kind == kind)
        .unwrap_or_else(|| panic!("missing interface inventory entry {kind:?}/{id}"))
}

fn repo_root() -> PathBuf {
    // This crate's manifest dir is `upeg-cli/`; the repo root is its parent.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("upeg-cli crate dir has a parent directory (the repo root)")
        .to_path_buf()
}

/// desktop/pwa/ext declarations must point at real source/docs/tests
/// coordinates, not self-pins — the cheap but real check behind the
/// "inventory declaration honesty" backlog item. For each hand-declared
/// entry in `desktop_pwa_ext.rs`, verifies (1) the `source`/`command_path`
/// locators point at a real file (+symbol), (2) doc anchors point at a
/// real heading, and (3) `tests` pointers point at a real file other than
/// the declaring file itself.
#[test]
fn desktop_pwa_ext_declarations_point_to_real_source_docs_tests_coordinates() {
    let repo_root = repo_root();

    for entry in desktop_pwa_ext::interface_inventory_entries() {
        let source_path = entry
            .source
            .path
            .as_deref()
            .unwrap_or_else(|| panic!("{}: has no source.path", entry.id));
        assert!(
            repo_root.join(source_path).exists(),
            "{}: source.path {source_path} does not exist",
            entry.id
        );

        if let Some(command_path) = entry.contract.locator.command_path.as_deref() {
            assert_locator_resolves(&repo_root, command_path, source_path, &entry.id);
        }

        if let Some(schema_ref) = entry
            .contract
            .input
            .as_ref()
            .and_then(|input| input.schema_ref.as_deref())
        {
            assert_schema_ref_resolves(&repo_root, schema_ref, &entry.id);
        }

        // An uncovered entry has no path to check — the gap itself is
        // the declaration, and `all_surface_declaration_tests_pointers_...`
        // asserts its reason is present.
        if let Some(tests_path) = entry.tests.path() {
            assert_ne!(
                tests_path,
                desktop_pwa_ext::DECLARING_FILE_PATH,
                "{}: tests.path is still a self-pin pointing at the declaring file itself",
                entry.id
            );
            assert!(
                repo_root.join(tests_path).exists(),
                "{}: tests.path {tests_path} does not exist",
                entry.id
            );
        }
    }
}

/// `command_path` locator: either `file#Symbol` (symbol must occur in the
/// file) or a bare Rust path like `a::b::desktop_deep_link` (cheap check:
/// `fn desktop_deep_link` must occur in the entry's own declared source).
fn assert_locator_resolves(repo_root: &Path, locator: &str, source_path: &str, entry_id: &str) {
    if let Some((path, anchor)) = locator.split_once('#') {
        assert_file_contains(repo_root, path, anchor, entry_id);
        return;
    }

    if locator.contains("::") {
        let symbol = locator
            .rsplit("::")
            .next()
            .unwrap_or_else(|| panic!("{entry_id}: empty Rust path locator `{locator}`"));
        assert_file_contains(repo_root, source_path, &format!("fn {symbol}"), entry_id);
        return;
    }

    assert!(
        repo_root.join(locator).exists(),
        "{entry_id}: command_path {locator} does not exist"
    );
}

/// `schemaRef`: docs paths need a real heading behind their anchor; other
/// repo-relative file paths just need to exist. Opaque Rust type names
/// (`upeg_core::InputSpec`) and URI templates (`upeg://open?...`) are not
/// locators and are skipped.
fn assert_schema_ref_resolves(repo_root: &Path, schema_ref: &str, entry_id: &str) {
    if let Some((path, anchor)) = schema_ref.split_once('#') {
        if path.starts_with("docs/") {
            assert_doc_heading_exists(repo_root, path, anchor, entry_id);
        } else {
            assert_file_contains(repo_root, path, anchor, entry_id);
        }
        return;
    }

    if schema_ref.contains("://") || schema_ref.contains("::") || !schema_ref.contains('/') {
        return;
    }

    assert!(
        repo_root.join(schema_ref).exists(),
        "{entry_id}: schemaRef {schema_ref} does not exist"
    );
}

fn assert_file_contains(repo_root: &Path, rel_path: &str, needle: &str, entry_id: &str) {
    let file = repo_root.join(rel_path);
    let content = std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("{entry_id}: failed to read {rel_path}: {e}"));
    assert!(
        content.contains(needle),
        "{entry_id}: symbol `{needle}` not found in {rel_path} (stale locator)"
    );
}

fn assert_doc_heading_exists(repo_root: &Path, doc_path: &str, anchor: &str, entry_id: &str) {
    let file = repo_root.join(doc_path);
    let content = std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("{entry_id}: failed to read {doc_path}: {e}"));
    let has_heading = content
        .lines()
        .filter_map(heading_slug)
        .any(|slug| slug == anchor);
    assert!(
        has_heading,
        "{entry_id}: no heading in {doc_path} matching anchor `#{anchor}`"
    );
}

/// Cheap `GitHub`-style heading slug (`## Chrome extension contract` ->
/// `chrome-extension-contract`). Good enough for this one doc's headings —
/// not a full GFM slugger.
fn heading_slug(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let heading = trimmed.trim_start_matches('#').trim();
    if heading.is_empty() {
        return None;
    }

    let mut slug = String::with_capacity(heading.len());
    let mut last_was_hyphen = false;
    for ch in heading.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }
    Some(slug.trim_matches('-').to_string())
}

/// How a `tests.test_name` occurs in its file, per test language. A
/// Rust test is a `fn <name>`; a Dart test is a quoted case label.
/// Anything else is not a language this repo writes tests in.
enum TestFileLanguage {
    Rust,
    Dart,
}

impl TestFileLanguage {
    fn of(path: &str) -> Option<Self> {
        match Path::new(path).extension()?.to_str()? {
            "rs" => Some(Self::Rust),
            "dart" => Some(Self::Dart),
            _ => None,
        }
    }

    /// Spellings that count as "this file declares that test". Dart gets
    /// both quote styles because `dart format` leaves either in place.
    fn declaration_forms(&self, test_name: &str) -> Vec<String> {
        match self {
            Self::Rust => vec![format!("fn {test_name}")],
            Self::Dart => vec![format!("'{test_name}'"), format!("\"{test_name}\"")],
        }
    }
}

/// Every hand-declared surface entry's `tests` pointer must be real —
/// not only desktop/pwa/ext but cli/http/mcp/mcp-import/tui too. It is
/// not enough for the file to exist: **a test of that name must actually
/// exist inside it**. Writing down a nonexistent test name makes the
/// inventory look covered while nothing is verified — that is the failure
/// this check prevents. Entries with no test must be honestly declared as
/// `TestMapping::Uncovered { reason }`, in which case only the reason's
/// non-emptiness is checked.
#[test]
fn all_surface_declaration_tests_pointers_point_to_real_tests() {
    let repo_root = repo_root();

    for entry in all_declared_surface_entries() {
        let (path, test_name) = match &entry.tests {
            upeg_core::interface_inventory::TestMapping::Uncovered { reason } => {
                assert!(
                    !reason.trim().is_empty(),
                    "{}: uncovered declaration has no reason",
                    entry.id
                );
                continue;
            }
            upeg_core::interface_inventory::TestMapping::Covered { path, test_name } => {
                (path, test_name)
            }
        };

        let file = repo_root.join(path);
        assert!(
            file.exists(),
            "{}: tests.path {path} does not exist",
            entry.id
        );

        let Some(test_name) = test_name else {
            continue;
        };
        let language = TestFileLanguage::of(path)
            .unwrap_or_else(|| panic!("{}: tests.path {path} is neither .rs nor .dart", entry.id));
        let content = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: failed to read {path}: {e}", entry.id));
        let forms = language.declaration_forms(test_name);
        assert!(
            forms.iter().any(|form| content.contains(form)),
            "{}: no `{test_name}` test in {path} (forms tried: {forms:?}) — \
             a declaration pointing at a nonexistent test fabricates coverage",
            entry.id
        );
    }
}

#[test]
fn interface_inventory_covers_cli_http_and_mcp() {
    let entries = all_surface_entries();
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: entries.clone(),
    };

    inventory.validate().expect("surface inventory validates");

    // The count is derived — the expected id list is the single source of
    // truth and `len()` follows from it. Adding a new surface entry means
    // editing this list, not a number.
    const EXPECTED_SURFACE_ENTRY_IDS: &[&str] = &[
        "cli.tool.list",
        "cli.call",
        "cli.board.list",
        "cli.board.call",
        "cli.board.pin",
        "cli.board.unpin",
        "cli.board.move",
        "cli.board.context",
        "cli.board.connect",
        "cli.board.describe",
        "cli.host.start",
        "cli.host.status",
        "cli.host.stop",
        "http.v1.tools.list",
        "http.v1.tools.call",
        "http.v1.tools.call.stream",
        "mcp.board.context",
        "mcp.tools.list",
        "mcp.tools.call",
        "mcp.import.upstream-config",
    ];

    let mut actual_ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    actual_ids.sort_unstable();
    let mut expected_ids = EXPECTED_SURFACE_ENTRY_IDS.to_vec();
    expected_ids.sort_unstable();
    assert_eq!(
        actual_ids, expected_ids,
        "the surface inventory id set changed"
    );
    assert_eq!(entries.len(), EXPECTED_SURFACE_ENTRY_IDS.len());
    assert!(entries.iter().all(|entry| entry.version == "v1"));
    assert!(
        entries
            .iter()
            .all(|entry| entry.compatibility == Compatibility::Stable)
    );

    let cli_tool_list = find_entry(&entries, "cli.tool.list", InterfaceKind::CliCommand);
    assert_eq!(cli_tool_list.surfaces, SurfaceSet::single(Surface::Cli));
    assert_eq!(
        cli_tool_list.contract.locator.command_path.as_deref(),
        Some("tool.list")
    );
    assert_eq!(
        find_entry(&entries, "cli.call", InterfaceKind::CliCommand)
            .contract
            .locator
            .command_path
            .as_deref(),
        Some("call")
    );
    assert_eq!(
        find_entry(&entries, "cli.host.start", InterfaceKind::CliCommand)
            .contract
            .locator
            .command_path
            .as_deref(),
        Some("host.start")
    );
    // The service/source control plane was removed — it must not remain
    // in the inventory either.
    assert!(
        !entries
            .iter()
            .any(|entry| entry.id.starts_with("cli.service.")
                || entry.id.starts_with("cli.source.")
                || entry.id.starts_with("http.control.")),
        "removed control-plane entries remain in the interface inventory"
    );

    let http_tools_list = find_entry(&entries, "http.v1.tools.list", InterfaceKind::HttpRoute);
    assert_eq!(http_tools_list.surfaces, SurfaceSet::single(Surface::Http));
    assert_eq!(
        http_tools_list.contract.locator.http_method.as_deref(),
        Some("GET")
    );
    assert_eq!(
        http_tools_list.contract.locator.http_path.as_deref(),
        Some("/v1/tools")
    );

    let http_tools_call = find_entry(&entries, "http.v1.tools.call", InterfaceKind::HttpRoute);
    assert_eq!(
        http_tools_call.contract.locator.http_method.as_deref(),
        Some("POST")
    );
    assert_eq!(
        http_tools_call.contract.locator.http_path.as_deref(),
        Some("/v1/tools/{id}")
    );
    let mcp_tools_list = find_entry(&entries, "mcp.tools.list", InterfaceKind::McpMethod);
    assert_eq!(mcp_tools_list.surfaces, SurfaceSet::single(Surface::Mcp));
    assert_eq!(
        mcp_tools_list.contract.locator.jsonrpc_method.as_deref(),
        Some("tools/list")
    );

    let mcp_tools_call = find_entry(&entries, "mcp.tools.call", InterfaceKind::McpTool);
    assert_eq!(
        mcp_tools_call.contract.locator.jsonrpc_method.as_deref(),
        Some("tools/call")
    );

    let mcp_import = find_entry(
        &entries,
        "mcp.import.upstream-config",
        InterfaceKind::McpImport,
    );
    assert_eq!(mcp_import.surfaces, SurfaceSet::single(Surface::Mcp));
    assert_eq!(
        mcp_import
            .contract
            .input
            .as_ref()
            .expect("mcp import has input contract")
            .schema_ref
            .as_deref(),
        Some("mcp-import.UpstreamConfig")
    );
    assert_eq!(mcp_import.contract.locator.jsonrpc_method, None);
    assert!(entries.iter().all(|entry| {
        entry.contract.input.is_some()
            && entry.contract.output.as_ref().is_some_and(|output| {
                output.kind == ContractIoKind::NotDeclared && !output.declared
            })
    }));
    assert!(
        entries
            .iter()
            .all(|entry| !(entry.id.starts_with("mcp.import.")
                && entry.kind == InterfaceKind::McpMethod))
    );
}

#[test]
fn interface_inventory_rejects_invalid_http_route_contract() {
    let mut invalid = surfaces::http::interface_inventory_entries()
        .into_iter()
        .find(|entry| entry.id == "http.v1.tools.list")
        .expect("http route inventory entry exists");
    invalid.contract.locator.http_method = Some(" POST ".to_string());

    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![invalid],
    };

    assert!(matches!(
        inventory.validate(),
        Err(InterfaceInventoryError::MissingRequiredField { field })
            if field == "entry.contract.locator.httpMethod"
    ));
}

#[test]
fn interface_inventory_generate_succeeds() {
    let dir = temp_inventory_dir("generate");
    let json_path = dir.join("inventory.json");
    let markdown_path = dir.join("inventory.md");

    let cli = Cli::parse_from([
        "upeg",
        "interface",
        "inventory",
        "generate",
        "--json",
        json_path.to_str().expect("json path is utf-8"),
        "--markdown",
        markdown_path.to_str().expect("markdown path is utf-8"),
    ]);

    let output = run(cli).expect("generate succeeds");

    assert!(output.contains("wrote interface inventory"));
    assert!(json_path.exists());
    assert!(markdown_path.exists());

    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&json_path).expect("json inventory is readable"),
    )
    .expect("json inventory parses");
    assert_eq!(
        json["schemaVersion"],
        serde_json::json!(INTERFACE_INVENTORY_SCHEMA_VERSION)
    );
    assert!(
        json["entries"]
            .as_array()
            .is_some_and(|entries| !entries.is_empty())
    );
    let entries = json["entries"].as_array().expect("entries is an array");
    let cli_call = entries
        .iter()
        .find(|entry| entry["id"] == "cli.call")
        .expect("cli.call entry exists");
    assert_eq!(
        cli_call["contract"]["locator"]["commandPath"],
        serde_json::json!("call")
    );
    assert_eq!(
        cli_call["contract"]["input"]["kind"],
        serde_json::json!("commandArgs")
    );
    assert_eq!(
        cli_call["contract"]["output"]["kind"],
        serde_json::json!("notDeclared")
    );
    assert!(cli_call["contract"]["errors"].is_array());

    let tool_entry = entries
        .iter()
        .find(|entry| entry["kind"] == "tool")
        .expect("generated inventory includes tool entries");
    assert_eq!(
        tool_entry["contract"]["input"]["kind"],
        serde_json::json!("jsonSchema")
    );
    assert!(tool_entry["contract"]["input"]["schema"].is_object());
    assert!(tool_entry["contract"]["input"]["schemaRef"].is_string());
    assert_eq!(
        tool_entry["contract"]["output"]["declared"],
        serde_json::json!(true)
    );
    assert_eq!(
        tool_entry["contract"]["output"]["kind"],
        serde_json::json!("jsonSchema")
    );

    let markdown = std::fs::read_to_string(&markdown_path).expect("markdown inventory is readable");
    assert!(markdown.starts_with("# Interface Inventory"));
    assert!(markdown.contains("| Surfaces | Kind | ID | Version | Input | Output | Source |"));
    assert!(markdown.contains("| cli | cliCommand | `cli.call` | v1 | commandArgs"));
    assert!(markdown.contains("jsonSchema<br>`tool."));
    assert!(markdown.contains(".output_schema`"));
}

#[test]
fn interface_inventory_markdown_shows_headless_coherence_summary() {
    let markdown = interface_inventory_command::format_inventory_markdown(&static_tool_inventory());

    assert!(markdown.contains(
        "## Tool Capability Summary\n\
| Category | Unique tool count | Rule |\n\
| --- | ---: | --- |\n\
| Headless-dispatchable tools | 68 | Advertises at least one of the `cli`, `tui`, `mcp`, or `http` surfaces |\n\
| GUI-only tools | 5 | Advertises only `desktop`, `pwa`, and/or `ext` surfaces |"
    ));
}

#[test]
fn mixed_surface_tools_count_as_headless_in_markdown() {
    let inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: vec![
            test_inventory_entry(
                "mixed.tool",
                vec![Surface::Cli, Surface::Desktop],
                InterfaceKind::Tool,
            ),
            test_inventory_entry(
                "gui.tool",
                vec![Surface::Desktop, Surface::Pwa],
                InterfaceKind::Tool,
            ),
            test_inventory_entry("cli.synthetic", Surface::Cli, InterfaceKind::CliCommand),
        ],
    };

    let markdown = interface_inventory_command::format_inventory_markdown(&inventory);

    assert!(markdown.contains(
        "| Headless-dispatchable tools | 1 | Advertises at least one of the `cli`, `tui`, `mcp`, or `http` surfaces |"
    ));
    assert!(markdown.contains(
        "| GUI-only tools | 1 | Advertises only `desktop`, `pwa`, and/or `ext` surfaces |"
    ));
}

#[test]
fn interface_inventory_check_detects_drift() {
    let dir = temp_inventory_dir("check");
    let baseline_path = dir.join("baseline.json");
    let docs_path = dir.join("docs.md");
    let current_path = dir.join("current.json");
    let diff_path = dir.join("diff.json");
    let comment_path = dir.join("comment.md");
    std::fs::write(
        &baseline_path,
        serde_json::json!({
            "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
            "entries": [],
        })
        .to_string(),
    )
    .expect("baseline fixture writes");

    let err = interface_inventory_command::check_inventory(
        &baseline_path,
        &docs_path,
        &current_path,
        &diff_path,
        &comment_path,
    )
    .expect_err("empty baseline drifts from generated inventory");

    assert!(err.message().contains("interface inventory drift detected"));
    assert!(docs_path.exists());
    assert!(current_path.exists());

    let diff: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&diff_path).expect("diff artifact is readable"),
    )
    .expect("diff artifact parses");
    assert_eq!(
        diff["schemaVersion"],
        serde_json::json!(INTERFACE_INVENTORY_SCHEMA_VERSION)
    );
    assert!(
        diff["added"]
            .as_array()
            .is_some_and(|entries| !entries.is_empty())
    );
    assert!(diff["removed"].as_array().is_some_and(Vec::is_empty));
    assert!(diff["changed"].as_array().is_some_and(Vec::is_empty));

    let comment = std::fs::read_to_string(&comment_path).expect("comment artifact is readable");
    assert!(comment.starts_with("<!-- upeg-interface-inventory -->"));
    assert!(comment.contains("### Interface Inventory"));
    assert!(comment.contains("#### Added"));
}

#[test]
fn interface_inventory_check_reports_path_level_contract_drift_first() {
    let dir = temp_inventory_dir("check-path-drift");
    let baseline_path = dir.join("baseline.json");
    let docs_path = dir.join("docs.md");
    let current_path = dir.join("current.json");
    let diff_path = dir.join("diff.json");
    let comment_path = dir.join("comment.md");
    let mut baseline = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries: all_surface_entries(),
    }
    .to_deterministic_json();
    let tool_entry = baseline["entries"]
        .as_array_mut()
        .expect("entries is mutable array")
        .iter_mut()
        .find(|entry| entry["id"] == "cli.call")
        .expect("inventory includes cli.call");
    tool_entry["version"] = serde_json::json!("v0");
    tool_entry["contract"]["input"]["description"] = serde_json::json!("legacy input");
    tool_entry["contract"]["output"]["description"] = serde_json::json!("legacy output");
    std::fs::write(&baseline_path, baseline.to_string()).expect("baseline fixture writes");

    let err = interface_inventory_command::check_inventory(
        &baseline_path,
        &docs_path,
        &current_path,
        &diff_path,
        &comment_path,
    )
    .expect_err("mutated baseline drifts from generated inventory");

    assert!(err.message().contains("interface inventory drift detected"));
    let diff: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&diff_path).expect("diff artifact is readable"),
    )
    .expect("diff artifact parses");
    let changes = diff["changed"][0]["changes"]
        .as_array()
        .expect("changed entry has path-level changes");
    assert!(changes.iter().any(|change| {
        change["path"]
            .as_str()
            .is_some_and(|path| path.starts_with("/contract/input"))
    }));
    assert!(changes.iter().any(|change| {
        change["path"]
            .as_str()
            .is_some_and(|path| path.starts_with("/contract/output"))
    }));
    assert!(changes.iter().any(|change| change["path"] == "/version"));

    let comment = std::fs::read_to_string(&comment_path).expect("comment artifact is readable");
    let contract_section = comment
        .find("##### Input/Output contract changes")
        .expect("contract change section exists");
    let metadata_section = comment
        .find("##### Other metadata changes")
        .expect("metadata change section exists");
    assert!(contract_section < metadata_section);
    assert!(comment.contains("target/interface-inventory/diff.json"));
}

fn temp_inventory_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "upeg-interface-inventory-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp inventory dir is created");
    dir
}

fn test_inventory_entry(
    id: &str,
    surfaces: impl Into<SurfaceSet>,
    kind: InterfaceKind,
) -> InterfaceEntry {
    InterfaceEntry {
        id: id.to_string(),
        surfaces: surfaces.into(),
        kind,
        contract: ContractShape::default(),
        version: "v1".to_string(),
        compatibility: Compatibility::Stable,
        owner: OwnerRef::default(),
        docs: DocRef::default(),
        source: SourceRef::default(),
        tests: TestMapping::covered("upeg-cli/src/inventory/tests.rs", None),
    }
}

fn static_tool_inventory() -> InterfaceInventory {
    let entries = upeg_core::inventory::iter::<StaticToolMeta>()
        .map(|tool| {
            test_inventory_entry(
                tool.id,
                SurfaceSet::new(tool.surfaces.iter().copied()),
                InterfaceKind::Tool,
            )
        })
        .collect();

    InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries,
    }
}
