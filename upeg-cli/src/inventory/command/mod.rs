use std::fs;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde_json::{Value, json};
use upeg_core::interface_inventory::{
    ContractIo, INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceInventory,
};

mod capability_summary;
mod diff;

use capability_summary::format_tool_capability_summary;
use diff::{ChangedEntry, DeepChange, InventoryDiff, InventoryKey, SURFACES_CHANGE_PATH};

use crate::adapters::mcp_import;
use crate::error::CliError;
use crate::surfaces;
pub use crate::toolkit_schema_command::ToolkitSchemaCommand;
use crate::toolkit_schema_command::run_toolkit_schema_command;

const COMMENT_MARKER: &str = "<!-- upeg-interface-inventory -->";
/// The committed fixture both sides of a PR comparison are read from.
/// Named once so every note in the rendered comment points at the same
/// file.
const FIXTURE_PATH_LABEL: &str = "`fixtures/interface-inventory.json`";
/// JSON field holding an entry's advertised surface set.
const SURFACES_FIELD: &str = "surfaces";
/// Separator between surface labels in single-line renderings.
const SURFACE_LABEL_SEPARATOR: &str = ", ";
/// Markdown label for the surface-set row in an entry block.
const SURFACES_DETAIL_LABEL: &str = "Surfaces";
/// Placeholder used when a raw JSON entry omits an identifying field.
const UNKNOWN_FIELD_VALUE: &str = "unknown";
const CHANGE_COMMENT_LIMIT: usize = 20;
const ENTRY_COMMENT_LIMIT: usize = 30;
const ENTRY_CELL_LIMIT: usize = 96;

#[derive(Clone, Copy)]
struct CommentLimits {
    change_limit: Option<usize>,
    entry_limit: Option<usize>,
}

const PR_COMMENT_LIMITS: CommentLimits = CommentLimits {
    change_limit: Some(CHANGE_COMMENT_LIMIT),
    entry_limit: Some(ENTRY_COMMENT_LIMIT),
};
const FULL_COMMENT_LIMITS: CommentLimits = CommentLimits {
    change_limit: None,
    entry_limit: None,
};

#[derive(Subcommand, Debug)]
pub enum InterfaceCommand {
    /// Generate or check the cross-surface interface inventory.
    Inventory {
        #[command(subcommand)]
        action: InterfaceInventoryCommand,
    },
    /// Generate or check committed Toolkit manifest schema/docs artifacts.
    ToolkitSchema {
        #[command(subcommand)]
        action: ToolkitSchemaCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum InterfaceInventoryCommand {
    /// Write the current deterministic inventory JSON and Markdown documents.
    Generate {
        /// Destination for deterministic JSON inventory.
        #[arg(long)]
        json: PathBuf,
        /// Destination for deterministic Markdown inventory.
        #[arg(long)]
        markdown: PathBuf,
    },
    /// Compare current inventory against a baseline and write CI artifacts.
    Check {
        /// Baseline deterministic JSON inventory to compare against.
        #[arg(long)]
        baseline: PathBuf,
        /// Destination for current Markdown inventory documentation.
        #[arg(long)]
        docs: PathBuf,
        /// Destination for current deterministic JSON inventory.
        #[arg(long = "current-output")]
        current_output: PathBuf,
        /// Destination for drift diff JSON.
        #[arg(long = "diff-output")]
        diff_output: PathBuf,
        /// Destination for PR comment Markdown.
        #[arg(long = "comment-output")]
        comment_output: PathBuf,
    },
    /// Write a PR comment comparing committed target-branch and PR fixture JSON files.
    FixtureComment {
        /// Target branch fixture JSON path.
        #[arg(long)]
        base: PathBuf,
        /// PR head fixture JSON path.
        #[arg(long)]
        head: PathBuf,
        /// Destination for fixture diff JSON.
        #[arg(long = "diff-output")]
        diff_output: PathBuf,
        /// Destination for PR comment Markdown.
        #[arg(long = "comment-output")]
        comment_output: PathBuf,
        /// Destination for full PR comment Markdown.
        #[arg(long = "full-comment-output")]
        full_comment_output: Option<PathBuf>,
        /// Label for the target branch ref.
        #[arg(long = "base-ref", default_value = "target branch")]
        base_ref: String,
        /// Label for the PR head ref.
        #[arg(long = "head-ref", default_value = "this PR")]
        head_ref: String,
    },
}

pub(crate) fn run_interface_command(action: InterfaceCommand) -> Result<String, CliError> {
    match action {
        InterfaceCommand::Inventory { action } => run_inventory_command(action),
        InterfaceCommand::ToolkitSchema { action } => run_toolkit_schema_command(action),
    }
}

pub(crate) fn run_inventory_command(action: InterfaceInventoryCommand) -> Result<String, CliError> {
    match action {
        InterfaceInventoryCommand::Generate { json, markdown } => {
            let inventory = generate_inventory()?;
            write_inventory_artifacts(&inventory, &json, &markdown)?;
            Ok(format!(
                "wrote interface inventory to {} and {}\n",
                json.display(),
                markdown.display()
            ))
        }
        InterfaceInventoryCommand::Check {
            baseline,
            docs,
            current_output,
            diff_output,
            comment_output,
        } => check_inventory(
            &baseline,
            &docs,
            &current_output,
            &diff_output,
            &comment_output,
        ),
        InterfaceInventoryCommand::FixtureComment {
            base,
            head,
            diff_output,
            comment_output,
            full_comment_output,
            base_ref,
            head_ref,
        } => write_fixture_comment(
            &base,
            &head,
            &diff_output,
            &comment_output,
            full_comment_output.as_deref(),
            &base_ref,
            &head_ref,
        ),
    }
}

pub(crate) fn generate_inventory() -> Result<InterfaceInventory, CliError> {
    let mut entries = upeg_runtime::interface_inventory::collect_tool_entries();
    entries.extend(upeg_runtime::interface_inventory::collect_toolkit_entries());
    entries.extend(surfaces::cli::interface_inventory_entries());
    entries.extend(surfaces::http::interface_inventory_entries());
    entries.extend(surfaces::mcp::interface_inventory_entries());
    entries.extend(mcp_import::interface_inventory_entries());
    entries.extend(surfaces::tui::interface_inventory_entries());
    entries.extend(crate::inventory::desktop_pwa_ext::interface_inventory_entries());

    let mut inventory = InterfaceInventory {
        schema_version: INTERFACE_INVENTORY_SCHEMA_VERSION,
        entries,
    };
    inventory.sort_entries();
    inventory.validate_surface_coverage().map_err(|err| {
        CliError::tool_failed(format!("interface inventory validation failed: {err}"))
    })?;
    Ok(inventory)
}

pub(crate) fn check_inventory(
    baseline_path: &Path,
    docs_path: &Path,
    current_output_path: &Path,
    diff_output_path: &Path,
    comment_output_path: &Path,
) -> Result<String, CliError> {
    let baseline = read_json(baseline_path)?;
    validate_inventory_json(&baseline, baseline_path)?;

    let current_inventory = generate_inventory()?;
    write_inventory_artifacts(&current_inventory, current_output_path, docs_path)?;

    let current = current_inventory.to_deterministic_json();
    let diff = InventoryDiff::between(&baseline, &current)?;
    write_json(diff_output_path, &diff.to_json())?;
    write_text(comment_output_path, &format_pr_comment(&diff))?;

    if diff.has_drift() {
        return Err(CliError::tool_failed(format!(
            "interface inventory drift detected: {} added, {} removed, {} changed",
            diff.added.len(),
            diff.removed.len(),
            diff.changed.len()
        )));
    }

    Ok("no interface inventory drift\n".to_string())
}

pub(crate) fn write_fixture_comment(
    base_path: &Path,
    head_path: &Path,
    diff_output_path: &Path,
    comment_output_path: &Path,
    full_comment_output_path: Option<&Path>,
    base_ref: &str,
    head_ref: &str,
) -> Result<String, CliError> {
    let (base, base_side) = read_optional_inventory_json(base_path)?;
    let (head, head_side) = read_optional_inventory_json(head_path)?;

    let diff = InventoryDiff::between(&base, &head)?;
    write_json(diff_output_path, &diff.to_json())?;
    write_text(
        comment_output_path,
        &format_fixture_pr_comment(&diff, base_ref, head_ref, base_side, head_side),
    )?;
    if let Some(full_comment_output_path) = full_comment_output_path {
        write_text(
            full_comment_output_path,
            &format_fixture_pr_full_comment(&diff, base_ref, head_ref, base_side, head_side),
        )?;
    }

    Ok(format!(
        "wrote interface inventory fixture diff comment: {} added, {} removed, {} changed\n",
        diff.added.len(),
        diff.removed.len(),
        diff.changed.len()
    ))
}

fn write_inventory_artifacts(
    inventory: &InterfaceInventory,
    json_path: &Path,
    markdown_path: &Path,
) -> Result<(), CliError> {
    write_json(json_path, &inventory.to_deterministic_json())?;
    write_text(markdown_path, &format_inventory_markdown(inventory))
}

fn write_json(path: &Path, value: &Value) -> Result<(), CliError> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|err| CliError::tool_failed(format!("failed to serialize JSON: {err}")))?;
    text.push('\n');
    write_text(path, &text)
}

fn write_text(path: &Path, text: &str) -> Result<(), CliError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            CliError::tool_failed(format!("failed to create {}: {err}", parent.display()))
        })?;
    }
    fs::write(path, text)
        .map_err(|err| CliError::tool_failed(format!("failed to write {}: {err}", path.display())))
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    let text = fs::read_to_string(path).map_err(|err| {
        CliError::tool_failed(format!("failed to read {}: {err}", path.display()))
    })?;
    serde_json::from_str(&text)
        .map_err(|err| CliError::tool_failed(format!("failed to parse {}: {err}", path.display())))
}

/// What one side (target branch / PR head) contributed to the fixture
/// comparison rendered into a PR comment.
///
/// The base branch's fixture was written by a DIFFERENT build of upeg
/// than the one rendering the comment, so a PR that bumps
/// [`INTERFACE_INVENTORY_SCHEMA_VERSION`] necessarily faces a base
/// fixture at the old version. That must not abort the render: the
/// comment is a courtesy report, not a gate, and erroring here reds the
/// CI lane for exactly the PR the reader most needs the comment on. An
/// unreadable version is therefore reported as "not comparable" and
/// contributes no entries, exactly like a missing file. The gate —
/// `inventory check --baseline`, which reads its baseline through
/// [`validate_inventory_json`] directly — stays strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureSide {
    /// Present and written against this build's schema version.
    Comparable,
    /// No fixture file at that ref.
    Missing,
    /// A fixture exists but its `schemaVersion` is not this build's.
    Incomparable { schema_version: Option<u64> },
}

impl FixtureSide {
    /// Clause naming why this side contributed nothing, for the
    /// rendered comment. `None` when the side was comparable.
    fn absent_reason(self) -> Option<String> {
        match self {
            Self::Comparable => None,
            Self::Missing => Some(format!("does not contain {FIXTURE_PATH_LABEL}")),
            Self::Incomparable { schema_version } => {
                let found = schema_version.map_or_else(
                    || "no readable schemaVersion".to_string(),
                    |version| format!("schemaVersion {version}"),
                );
                Some(format!(
                    "has a {FIXTURE_PATH_LABEL} that is not comparable ({found}; this build \
                     expects schemaVersion {INTERFACE_INVENTORY_SCHEMA_VERSION})"
                ))
            }
        }
    }
}

fn read_optional_inventory_json(path: &Path) -> Result<(Value, FixtureSide), CliError> {
    if !path.exists() {
        return Ok((empty_inventory_json(), FixtureSide::Missing));
    }

    let value = read_json(path)?;
    let schema_version = inventory_schema_version(&value);
    if schema_version != Some(u64::from(INTERFACE_INVENTORY_SCHEMA_VERSION)) {
        return Ok((
            empty_inventory_json(),
            FixtureSide::Incomparable { schema_version },
        ));
    }
    validate_inventory_json(&value, path)?;
    Ok((value, FixtureSide::Comparable))
}

fn empty_inventory_json() -> Value {
    json!({
        "schemaVersion": INTERFACE_INVENTORY_SCHEMA_VERSION,
        "entries": [],
    })
}

/// The `schemaVersion` a fixture declares, if it declares a readable
/// one. Shared by the strict [`validate_inventory_json`] gate and the
/// lenient [`read_optional_inventory_json`] renderer path so both read
/// the field the same way.
fn inventory_schema_version(value: &Value) -> Option<u64> {
    value.get("schemaVersion").and_then(Value::as_u64)
}

fn validate_inventory_json(value: &Value, path: &Path) -> Result<(), CliError> {
    if inventory_schema_version(value) != Some(u64::from(INTERFACE_INVENTORY_SCHEMA_VERSION)) {
        return Err(CliError::tool_failed(format!(
            "{} is not interface inventory schemaVersion {}",
            path.display(),
            INTERFACE_INVENTORY_SCHEMA_VERSION
        )));
    }
    if !value.get("entries").is_some_and(Value::is_array) {
        return Err(CliError::tool_failed(format!(
            "{} is missing interface inventory entries array",
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn format_inventory_markdown(inventory: &InterfaceInventory) -> String {
    let mut lines = vec![
        "# Interface Inventory".to_string(),
        String::new(),
        format!("Schema version: `{}`", inventory.schema_version),
        format!("Entries: `{}`", inventory.entries.len()),
        String::new(),
    ];
    lines.extend(format_tool_capability_summary(inventory));
    lines.push("| Surfaces | Kind | ID | Version | Input | Output | Source |".to_string());
    lines.push("| --- | --- | --- | --- | --- | --- | --- |".to_string());

    for entry in inventory.sorted_entries() {
        lines.push(format!(
            "| {} | {} | `{}` | {} | {} | {} | {} |",
            escape_markdown(&entry.surfaces.joined_labels()),
            escape_markdown(entry.kind.label()),
            escape_markdown(&entry.id),
            escape_markdown(&entry.version),
            format_contract_io_summary(entry.contract.input.as_ref()),
            format_contract_io_summary(entry.contract.output.as_ref()),
            entry.source.path.as_deref().unwrap_or("")
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn format_contract_io_summary(contract: Option<&ContractIo>) -> String {
    let Some(contract) = contract else {
        return "missing".to_string();
    };

    let mut parts = vec![escape_markdown(contract.kind.label())];
    if let Some(schema_ref) = &contract.schema_ref {
        parts.push(format!("`{}`", escape_markdown(schema_ref)));
    } else if let Some(description) = &contract.description {
        parts.push(escape_markdown(description));
    }
    if !contract.declared {
        parts.push("_not declared_".to_string());
    }

    parts.join("<br>")
}

fn format_pr_comment(diff: &InventoryDiff) -> String {
    format_pr_comment_with_limits(diff, PR_COMMENT_LIMITS)
}

fn format_pr_comment_with_limits(diff: &InventoryDiff, limits: CommentLimits) -> String {
    let mut lines = vec![
        COMMENT_MARKER.to_string(),
        "### Interface Inventory".to_string(),
        String::new(),
    ];

    if !diff.has_drift() {
        lines.push("No interface inventory drift detected.".to_string());
        lines.push(String::new());
        return lines.join("\n");
    }

    append_changed_sections(
        &mut lines,
        &diff.changed,
        "target/interface-inventory/diff.json",
        limits,
    );
    append_entry_section(
        &mut lines,
        "Added",
        &diff.added,
        "target/interface-inventory/diff.json",
        limits,
    );
    append_entry_section(
        &mut lines,
        "Removed",
        &diff.removed,
        "target/interface-inventory/diff.json",
        limits,
    );
    lines.join("\n")
}

fn format_fixture_pr_comment(
    diff: &InventoryDiff,
    base_ref: &str,
    head_ref: &str,
    base: FixtureSide,
    head: FixtureSide,
) -> String {
    format_fixture_pr_comment_with_limits(diff, base_ref, head_ref, base, head, PR_COMMENT_LIMITS)
}

fn format_fixture_pr_full_comment(
    diff: &InventoryDiff,
    base_ref: &str,
    head_ref: &str,
    base: FixtureSide,
    head: FixtureSide,
) -> String {
    format_fixture_pr_comment_with_limits(diff, base_ref, head_ref, base, head, FULL_COMMENT_LIMITS)
}

fn format_fixture_pr_comment_with_limits(
    diff: &InventoryDiff,
    base_ref: &str,
    head_ref: &str,
    base: FixtureSide,
    head: FixtureSide,
    limits: CommentLimits,
) -> String {
    let base_label = escape_markdown(base_ref);
    let head_label = escape_markdown(head_ref);
    let mut lines = vec![
        COMMENT_MARKER.to_string(),
        "### Interface Inventory Fixture Changes".to_string(),
        String::new(),
        format!(
            "Comparing committed `fixtures/interface-inventory.json` from target branch `{base_label}` with this PR `{head_label}`."
        ),
        String::new(),
    ];

    // One note per absent side, phrased by `FixtureSide::absent_reason`
    // so "no file" and "version this build can't compare" read
    // differently — the reader has to be able to tell a missing fixture
    // from a schemaVersion bump.
    match (base.absent_reason(), head.absent_reason()) {
        (Some(base_reason), Some(head_reason)) => {
            lines.push(format!(
                "No fixture comparison is available: the target branch {base_reason}, and this PR {head_reason}."
            ));
            lines.push(String::new());
        }
        (Some(base_reason), None) => {
            lines.push(format!(
                "The target branch {base_reason}; this PR's fixture is shown as newly added."
            ));
            lines.push(String::new());
        }
        (None, Some(head_reason)) => {
            lines.push(format!(
                "This PR {head_reason}; the target branch fixture is shown as removed."
            ));
            lines.push(String::new());
        }
        (None, None) => {}
    }

    if !diff.has_drift() {
        lines.push(
            "No interface inventory fixture changes compared with the target branch.".to_string(),
        );
        lines.push(String::new());
        return lines.join("\n");
    }

    append_changed_sections(
        &mut lines,
        &diff.changed,
        "target/interface-inventory/pr-fixture-diff.json",
        limits,
    );
    append_entry_section(
        &mut lines,
        "Added",
        &diff.added,
        "target/interface-inventory/pr-fixture-diff.json",
        limits,
    );
    append_entry_section(
        &mut lines,
        "Removed",
        &diff.removed,
        "target/interface-inventory/pr-fixture-diff.json",
        limits,
    );
    lines.push(
        "This compares committed fixture files only. CI separately verifies generated interface inventory against this PR's fixture. The full Markdown report and JSON diff are uploaded as the `pr-fixture-diffs` workflow artifact."
            .to_string(),
    );
    lines.push(String::new());
    lines.join("\n")
}

fn append_entry_section(
    lines: &mut Vec<String>,
    title: &str,
    entries: &[Value],
    detail_path: &str,
    limits: CommentLimits,
) {
    if entries.is_empty() {
        return;
    }

    lines.push(format!("#### {title} ({})", entries.len()));
    lines.push(String::new());
    let entry_limit = limits.entry_limit.unwrap_or(entries.len());
    for entry in entries.iter().take(entry_limit) {
        append_entry_block(lines, entry);
    }
    if limits
        .entry_limit
        .is_some_and(|entry_limit| entries.len() > entry_limit)
    {
        lines.push(format!(
            "_Omitted `{}` additional {} entries; see `{detail_path}` for full JSON._",
            entries.len() - entry_limit,
            title.to_ascii_lowercase()
        ));
    }
    lines.push(String::new());
}

fn append_entry_block(lines: &mut Vec<String>, entry: &Value) {
    lines.push(format!("- {}", inventory_entry_label(entry)));
    append_optional_detail(
        lines,
        SURFACES_DETAIL_LABEL,
        entry
            .get(SURFACES_FIELD)
            .map(|surfaces| markdown_code(&format_surface_labels(surfaces))),
    );
    append_optional_detail(
        lines,
        "Version",
        string_field(entry, "version").map(markdown_code),
    );
    append_optional_detail(
        lines,
        "Compatibility",
        string_field(entry, "compatibility").map(markdown_code),
    );
    append_optional_detail(
        lines,
        "Source",
        nested_string_field(entry, &["source", "path"]).map(markdown_code),
    );
    append_optional_detail(
        lines,
        "Owner",
        nested_string_field(entry, &["owner", "path"]).map(markdown_code),
    );
    lines.push(format!("  - Input: {}", contract_summary(entry, "input")));
    lines.push(format!("  - Output: {}", contract_summary(entry, "output")));
}

fn append_optional_detail(lines: &mut Vec<String>, label: &str, value: Option<String>) {
    if let Some(value) = value {
        lines.push(format!("  - {label}: {value}"));
    }
}

fn append_changed_sections(
    lines: &mut Vec<String>,
    entries: &[ChangedEntry],
    detail_path: &str,
    limits: CommentLimits,
) {
    if entries.is_empty() {
        return;
    }

    let mut contract_changes = Vec::new();
    let mut metadata_changes = Vec::new();
    for entry in entries {
        for change in &entry.changes {
            if is_contract_io_path(&change.path) {
                contract_changes.push((entry, change));
            } else {
                metadata_changes.push((entry, change));
            }
        }
    }

    let total_changes = contract_changes.len() + metadata_changes.len();
    lines.push(format!("#### Changed ({})", entries.len()));
    lines.push(String::new());
    let mut remaining = limits.change_limit.unwrap_or(total_changes);
    if !contract_changes.is_empty() {
        remaining -= append_deep_change_list(
            lines,
            "Input/Output contract changes",
            &contract_changes,
            remaining,
            detail_path,
        );
    }
    if !metadata_changes.is_empty() {
        let _ = append_deep_change_list(
            lines,
            "Other metadata changes",
            &metadata_changes,
            remaining,
            detail_path,
        );
    }
    match limits.change_limit {
        Some(change_limit) if total_changes > change_limit => {
            lines.push(format!(
                "Showing first `{change_limit}` of `{total_changes}` path-level changes; see `{detail_path}` for full details."
            ));
        }
        Some(_) => {
            lines.push(format!(
                "See `{detail_path}` for full path-level change details."
            ));
        }
        None => {
            lines.push(format!("Raw JSON diff: `{detail_path}`."));
        }
    }
    lines.push(String::new());
}

fn append_deep_change_list(
    lines: &mut Vec<String>,
    title: &str,
    changes: &[(&ChangedEntry, &DeepChange)],
    limit: usize,
    detail_path: &str,
) -> usize {
    lines.push(format!("##### {title} ({})", changes.len()));
    lines.push(String::new());
    if limit == 0 {
        lines.push(format!(
            "_Omitted by comment row limit; see `{detail_path}`._"
        ));
        lines.push(String::new());
        return 0;
    }

    let mut written = 0;
    let mut current_label = None;
    for (entry, change) in changes.iter().take(limit) {
        let label = inventory_key_label(&entry.key);
        if current_label.as_deref() != Some(label.as_str()) {
            lines.push(format!("- {label}"));
            current_label = Some(label);
        }
        append_deep_change_line(lines, change);
        written += 1;
    }
    lines.push(String::new());
    written
}

fn append_deep_change_line(lines: &mut Vec<String>, change: &DeepChange) {
    if change.path == SURFACES_CHANGE_PATH {
        lines.push(format!(
            "  - {} {SURFACES_FIELD}: {} -> {}",
            markdown_code(&change.kind),
            markdown_code(&format_surface_labels(&change.before)),
            markdown_code(&format_surface_labels(&change.after))
        ));
        return;
    }

    lines.push(format!(
        "  - {} {}: {} -> {}",
        markdown_code(&change.kind),
        markdown_code(&change.path),
        markdown_json_value(&change.before),
        markdown_json_value(&change.after)
    ));
}

fn is_contract_io_path(path: &str) -> bool {
    path.starts_with("/contract/input") || path.starts_with("/contract/output")
}

fn inventory_entry_label(entry: &Value) -> String {
    let kind = string_field(entry, "kind").unwrap_or(UNKNOWN_FIELD_VALUE);
    let id = string_field(entry, "id").unwrap_or(UNKNOWN_FIELD_VALUE);
    format_inventory_label(kind, id)
}

fn inventory_key_label(key: &InventoryKey) -> String {
    format_inventory_label(&key.kind, &key.id)
}

fn format_inventory_label(kind: &str, id: &str) -> String {
    format!("{} / {}", markdown_code(kind), markdown_code(id))
}

/// Render a JSON surface-set array as `cli, http, tui`.
fn format_surface_labels(surfaces: &Value) -> String {
    surfaces.as_array().map_or_else(
        || surfaces.to_string(),
        |labels| {
            labels
                .iter()
                .map(|label| {
                    label
                        .as_str()
                        .map_or_else(|| label.to_string(), ToString::to_string)
                })
                .collect::<Vec<_>>()
                .join(SURFACE_LABEL_SEPARATOR)
        },
    )
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn nested_string_field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        let next = current.get(key)?;
        current = next;
    }
    current.as_str()
}

fn contract_summary(entry: &Value, direction: &str) -> String {
    let Some(contract) = entry.get("contract").and_then(|value| value.get(direction)) else {
        return "missing".to_string();
    };

    let mut parts = Vec::new();
    if let Some(kind) = contract.get("kind").and_then(Value::as_str) {
        parts.push(kind.to_string());
    }
    if let Some(schema_ref) = contract.get("schemaRef").and_then(Value::as_str) {
        parts.push(format!("schemaRef={schema_ref}"));
    } else if let Some(description) = contract.get("description").and_then(Value::as_str) {
        parts.push(description.to_string());
    }
    if contract.get("declared").and_then(Value::as_bool) == Some(false) {
        parts.push("not declared".to_string());
    }

    if parts.is_empty() {
        "missing".to_string()
    } else {
        markdown_cell(&parts.join("; "))
    }
}

fn markdown_cell(value: &str) -> String {
    let text = value.replace('|', "\\|").replace('\n', "<br>");
    if text.chars().count() > ENTRY_CELL_LIMIT {
        let mut truncated = text
            .chars()
            .take(ENTRY_CELL_LIMIT.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        text
    }
}

fn markdown_code(value: &str) -> String {
    format!("`{}`", markdown_cell(value).replace('`', "\\`"))
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|")
}

fn markdown_json_value(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        markdown_code(text)
    } else {
        markdown_code(&value.to_string())
    }
}

#[cfg(test)]
mod tests;
