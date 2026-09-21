//! Tool list / show / validate output formatters.
//!
//! Extracted from `lib.rs` so the primary lib stays under the
//! ~1000 `LoC` complexity budget. These functions render `ToolMeta`
//! data into the human-readable + JSON shapes the CLI's subcommands
//! and the `tool list` / `tool show` / `tool validate` arms emit.
//! `lib.rs` re-exports the public formatter entry points.
//!
//! Dependencies that live in `lib.rs`:
//!   - `CliError` (public error type)
//!   - `match_score`, `warn_unknown_filter_value` (private helpers
//!     promoted to `pub(crate)` for cross-file access)

use std::fmt::Write as _;

use upeg_core::{Surface, ToolMeta};
use upeg_runtime::{
    ToolMetaRuntimeExt, tags_for_surface, toolbox_tool, toolbox_tools, toolkits_for_surface,
    tools_for_toolkit_on_surface, tools_with_tag_on_surface,
};

use crate::{CliError, warn_unknown_filter_value};

pub fn format_toolkit_list(json: bool) -> Result<String, CliError> {
    let ids = toolkits_for_surface(Surface::Cli);
    if json {
        let entries: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| toolkit_json(id, Surface::Cli))
            .collect();
        let mut out = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
        out.push('\n');
        return Ok(out);
    }

    let mut out = String::new();
    for id in ids {
        let tools = tools_for_toolkit_on_surface(id, Surface::Cli);
        let tags = effective_tags_for_tools(&tools);
        let _ = writeln!(
            out,
            "{id}\t{}\t{} tools",
            format_tags_inline(&tags),
            tools.len()
        );
    }
    Ok(out)
}

pub fn format_toolkit_show(id: &str, json: bool) -> Result<String, CliError> {
    let tools = tools_for_toolkit_on_surface(id, Surface::Cli);
    if tools.is_empty() {
        return Err(CliError::tool_failed(format!("unknown toolkit `{id}`")));
    }
    if json {
        let mut out = serde_json::to_string_pretty(&toolkit_json(id, Surface::Cli))
            .unwrap_or_else(|_| "{}".to_string());
        out.push('\n');
        return Ok(out);
    }

    let mut tags: Vec<String> = tools
        .iter()
        .flat_map(|t| t.tag_labels().into_iter())
        .collect();
    tags.sort();
    tags.dedup();
    let mut out = String::new();
    let _ = writeln!(out, "toolkit      {id}");
    let _ = writeln!(out, "tags         {}", format_tags_inline(&tags));
    out.push_str("tools\n");
    for t in tools {
        let tool_tags = t.tag_labels();
        let _ = writeln!(
            out,
            "  {}\t{}\t{}",
            t.id,
            t.display_label,
            format_tags_inline(&tool_tags)
        );
    }
    Ok(out)
}

pub fn format_tag_list(json: bool) -> Result<String, CliError> {
    let tags = tags_for_surface(Surface::Cli);
    if json {
        let entries: Vec<serde_json::Value> =
            tags.iter().map(|tag| tag_json(tag, Surface::Cli)).collect();
        let mut out = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
        out.push('\n');
        return Ok(out);
    }
    let mut out = String::new();
    for tag in tags {
        let count = tools_with_tag_on_surface(&tag, Surface::Cli).len();
        let _ = writeln!(out, "{tag}\t{count} tools");
    }
    Ok(out)
}

pub fn format_tag_show(tag: &str, json: bool) -> Result<String, CliError> {
    let tools = tools_with_tag_on_surface(tag, Surface::Cli);
    if tools.is_empty() {
        return Err(CliError::tool_failed(format!("unknown tag `{tag}`")));
    }
    if json {
        let mut out = serde_json::to_string_pretty(&tag_json(tag, Surface::Cli))
            .unwrap_or_else(|_| "{}".to_string());
        out.push('\n');
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&format!("tag          {tag}\n"));
    out.push_str("tools\n");
    for t in tools {
        out.push_str(&format!("  {}\t{}\n", t.id, t.display_label));
    }
    Ok(out)
}

/// Boards come from the *user's* pegboard state (shared store), not the
/// manifest's static `boards` arrays — same gate the HTTP `/v1/boards`
/// surface and `upeg board <b> call` use, so the surfaces can't
/// disagree about which boards exist or what's on them.
pub fn format_board_list(json: bool) -> Result<String, CliError> {
    let state = upeg_sources::pegboard::load_state();
    let boards = upeg_sources::pegboard::board_keys_in(&state);
    if json {
        let entries: Vec<serde_json::Value> = boards
            .iter()
            .map(|board| board_json(&state, board, Surface::Cli))
            .collect();
        let mut out = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
        out.push('\n');
        return Ok(out);
    }
    let mut out = String::new();
    for board in boards {
        let count = board_tools_in(&state, &board, Surface::Cli).len();
        out.push_str(&format!("{board}\t{count} tools\n"));
    }
    Ok(out)
}

pub fn format_board_show(board: &str, json: bool) -> Result<String, CliError> {
    let state = upeg_sources::pegboard::load_state();
    if !upeg_sources::pegboard::board_exists_in(&state, board) {
        return Err(CliError::tool_failed(format!("unknown board `{board}`")));
    }
    if json {
        let mut out = serde_json::to_string_pretty(&board_json(&state, board, Surface::Cli))
            .unwrap_or_else(|_| "{}".to_string());
        out.push('\n');
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&format!("board        {board}\n"));
    out.push_str("tools\n");
    for t in board_tools_in(&state, board, Surface::Cli) {
        out.push_str(&format!("  {}\t{}\n", t.id, t.description));
    }
    Ok(out)
}

/// A board's pinned tools visible on `surface`, in stored order.
fn board_tools_in(
    state: &upeg_sources::pegboard::PegboardState,
    board: &str,
    surface: Surface,
) -> Vec<&'static ToolMeta> {
    upeg_sources::pegboard::board_entries_on_surface_in(state, board, None, surface)
        .into_iter()
        .map(|(_, tool)| tool)
        .collect()
}

fn toolkit_json(id: &str, surface: Surface) -> serde_json::Value {
    let tools = tools_for_toolkit_on_surface(id, surface);
    let tags = effective_tags_for_tools(&tools);
    serde_json::json!({
        "id": id,
        "tags": tags,
        "toolCount": tools.len(),
        "tools": tools.iter().map(|t| t.to_json_object("id")).collect::<Vec<_>>(),
    })
}

fn tag_json(tag: &str, surface: Surface) -> serde_json::Value {
    let tools = tools_with_tag_on_surface(tag, surface);
    serde_json::json!({
        "tag": tag,
        "toolCount": tools.len(),
        "tools": tools.iter().map(|t| t.to_json_object("id")).collect::<Vec<_>>(),
    })
}

fn board_json(
    state: &upeg_sources::pegboard::PegboardState,
    board: &str,
    surface: Surface,
) -> serde_json::Value {
    let tools = board_tools_in(state, board, surface);
    serde_json::json!({
        "board": board,
        "toolCount": tools.len(),
        "tools": tools.iter().map(|t| t.to_json_object("id")).collect::<Vec<_>>(),
    })
}

fn effective_tags_for_tools(tools: &[&ToolMeta]) -> Vec<String> {
    let mut tags: Vec<String> = tools
        .iter()
        .flat_map(|t| t.tag_labels().into_iter())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn format_tags_inline(tags: &[String]) -> String {
    format!("[{}]", tags.join(", "))
}

/// Kebab-case spelling of a toolkit/tool token for shell-facing docs.
/// The dynamic route normalizes kebab→snake on input, so kebab is the
/// canonical spelling to *show* (`hex_to_decimal` → `hex-to-decimal`).
pub(crate) fn kebab_token(token: &str) -> String {
    token.replace('_', "-")
}

/// `upeg <toolkit> --help` — the dynamic route's toolkit-level help:
/// usage lines plus every CLI-surfaced Tool with a one-line
/// description, so the toolkit is discoverable without `tool list`.
pub fn format_toolkit_help(toolkit: &str) -> Result<String, CliError> {
    let tools = tools_for_toolkit_on_surface(toolkit, Surface::Cli);
    if tools.is_empty() {
        return Err(CliError::tool_failed(format!(
            "unknown toolkit `{toolkit}`"
        )));
    }
    let toolkit_label = kebab_token(toolkit);
    let mut out = String::new();
    let _ = writeln!(out, "usage: upeg {toolkit_label} <tool> [args...]");
    let _ = writeln!(out, "       upeg {toolkit_label} <tool> --help");
    out.push_str("tools\n");
    for t in tools {
        // One-line description; display_label fills in for tools that
        // declare none (same first-time-UX fallback as `tool show`).
        let summary = t
            .description
            .lines()
            .next()
            .filter(|line| !line.is_empty())
            .unwrap_or(t.display_label);
        let _ = writeln!(out, "  {:<24} {summary}", kebab_token(t.tool_id()));
    }
    Ok(out)
}

/// Dynamic-route invocation example: kebab-case tokens plus one
/// placeholder per schema field (`<name>` required, `[name]` optional),
/// bound positionally in declaration order.
fn dynamic_route_example(t: &ToolMeta) -> String {
    let mut out = format!(
        "upeg {} {}",
        kebab_token(t.toolkit),
        kebab_token(t.tool_id())
    );
    for field in &t.input_spec.fields {
        let name = field.name.as_str();
        if field.required {
            let _ = write!(out, " <{name}>");
        } else {
            let _ = write!(out, " [{name}]");
        }
    }
    out
}

/// `upeg call` invocation example: canonical id plus one `-a` pair per
/// required schema field, with the field's type as the value hint.
fn call_example(t: &ToolMeta) -> String {
    let mut out = format!("upeg call {}", t.id);
    for field in t.input_spec.fields.iter().filter(|field| field.required) {
        let _ = write!(out, " -a {}=<{}>", field.name.as_str(), field.kind.label());
    }
    out
}

/// Single filtered list entry point — the legacy `_full`/`_full2`/
/// `_full3` wrapper cascade was collapsed into this one function.
///
/// `surface_filter` defaults to `cli` when `None` (this binary's own
/// surface). `board_filter` narrows by `ToolMeta::boards`.
/// `pin_filter` narrows by pin kind (case-sensitive `PascalCase`:
/// `Inline`/`Launcher`/`Live`/`Action`/`Embed`, matching
/// `PinKind::label()`). When `json` is `true`, emit a JSON array
/// matching the HTTP `/v1/tools` shape rather than tab-separated rows.
/// Returns `Err(ToolFailed)` when `surface_filter` or `pin_filter`
/// names an unknown value.
pub fn format_tool_list_filtered(
    tag_filter: Option<&str>,
    surface_filter: Option<&str>,
    board_filter: Option<&str>,
    pin_filter: Option<&str>,
    json: bool,
) -> Result<String, CliError> {
    let surface = parse_surface_filter(surface_filter)?;
    let pin = validate_pin_filter(pin_filter)?;
    let tag = tag_filter.map(str::trim);
    let board = board_filter.map(str::trim);

    if let Some(tag) = tag {
        warn_if_tag_unknown(surface, tag);
    }
    if let Some(b) = board {
        warn_if_board_unknown(surface, b);
    }

    let tools = filter_tools(surface, tag, board, pin);

    Ok(if json {
        render_tool_list_json(&tools)
    } else {
        render_tool_list_text(&tools)
    })
}

/// Trim before parse, parallel to the TOML/wasm loaders. Common
/// case: shell paste with trailing space (`--surface "cli "`) used
/// to give "unknown surface `cli `" with empty-looking suffix —
/// confusing. Trim forgivingly. The cli displays the trimmed value
/// in the error too, so a typo like `--surface "Cli"` (uppercase)
/// still gets a clean "unknown
/// surface `Cli`" error pointing at the right token.
fn parse_surface_filter(filter: Option<&str>) -> Result<Surface, CliError> {
    match filter {
        None => Ok(Surface::Cli),
        Some(raw) => {
            let s = raw.trim();
            Surface::parse(s).ok_or_else(|| {
                CliError::tool_failed(format!(
                    "unknown surface `{s}` (cli/tui/desktop/pwa/ext/mcp/http)"
                ))
            })
        }
    }
}

/// Trim + validate the pin filter so an unknown value fails up-front
/// (avoids silently filtering to an empty list).
///
/// delegate to `PinKind::parse` so adding a new variant in
/// upeg-core ripples here automatically. The user-facing error hint
/// stays a literal list so a future variant addition is a loud diff
/// that forces the hint to be refreshed alongside the enum.
fn validate_pin_filter(filter: Option<&str>) -> Result<Option<&str>, CliError> {
    let trimmed = filter.map(str::trim);
    if let Some(w) = trimmed
        && upeg_core::PinKind::parse(w).is_none()
    {
        return Err(CliError::tool_failed(format!(
            "unknown pin kind `{w}` (Inline/Launcher/Live/Action/Embed/Chain/Llm)"
        )));
    }
    Ok(trimmed)
}

fn warn_if_tag_unknown(surface: Surface, tag: &str) {
    let known = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .any(|t| t.has_tag(tag));
    if known {
        return;
    }
    let candidates = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .flat_map(|t| t.tag_labels().into_iter());
    warn_unknown_filter_value("tag", surface, tag, candidates);
}

fn warn_if_board_unknown(surface: Surface, board: &str) {
    let known = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .any(|t| t.is_on_board(board));
    if known {
        return;
    }
    let candidates = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .flat_map(|t| t.boards.iter().copied());
    warn_unknown_filter_value("board", surface, board, candidates);
}

fn filter_tools(
    surface: Surface,
    tag: Option<&str>,
    board: Option<&str>,
    pin: Option<&str>,
) -> Vec<&'static ToolMeta> {
    let mut tools: Vec<&'static ToolMeta> = toolbox_tools()
        .filter(|t| t.is_on_surface(surface))
        .filter(|t| tag.is_none_or(|tag| t.has_tag(tag)))
        .filter(|t| board.is_none_or(|b| t.is_on_board(b)))
        .filter(|t| pin.is_none_or(|w| t.pin.label() == w))
        .collect();
    tools.sort_by_key(|t| t.id);
    tools
}

fn render_tool_list_json(tools: &[&'static ToolMeta]) -> String {
    // shared upeg-core helper. CLI uses `id` as the tool-id
    // field name (HTTP uses `name` per MCP convention).
    let entries: Vec<serde_json::Value> = tools.iter().map(|t| t.to_json_object("id")).collect();
    let mut out = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
    out.push('\n');
    out
}

fn render_tool_list_text(tools: &[&'static ToolMeta]) -> String {
    let mut out = String::new();
    for t in tools {
        out.push_str(&format!("{}\t{}\t{}\n", t.id, t.toolkit, t.pin.label()));
    }
    out
}

// `Surface::parse` is the centralised parse in upeg-core; the
// call site invokes it directly rather than wrapping it in a
// one-line passthrough.

/// Dry-run a Declarative TOML file at `path`. On success, return a brief
/// human-readable summary that echoes what the loader understood (id,
/// invoker, chain steps, External command). On failure, surface the
/// `LoadError` via `CliError::ToolFailed`.
///
/// When `resolve_chain` is true, additionally:
///   - Reject any step that equals the tool's own id (self-recursion).
///   - Reject any step that is neither declared in the same manifest nor
///     already present in the registry.
pub fn format_tool_validate(path: &str, resolve_chain: bool) -> Result<String, CliError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CliError::tool_failed(format!("read `{path}`: {e}")))?;

    let (toolkit, tools) = upeg_loader::parse_toolkit_full(&content)
        .map_err(|e| CliError::tool_failed(format!("{e}")))?;

    if resolve_chain {
        let parsed_tool_ids: Vec<&str> = tools.iter().map(|(meta, _)| meta.id).collect();
        for (_meta, toml) in &tools {
            validate_chain_nodes(toml, &parsed_tool_ids)?;
        }
    }

    let mut out = String::new();
    render_toolkit_header(&mut out, &toolkit, tools.len());
    for (meta, toml) in &tools {
        render_tool_detail(&mut out, meta, toml, resolve_chain);
    }
    Ok(out)
}

fn render_toolkit_header(out: &mut String, toolkit: &upeg_core::ToolkitMeta, tool_count: usize) {
    out.push_str(&format!(
        "ok: toolkit {} ({} tool(s))\n",
        toolkit.id, tool_count,
    ));
    if !toolkit.tags.is_empty() {
        out.push_str(&format!("    tags: [{}]\n", toolkit.tags.join(", ")));
    }
    if !toolkit.description.is_empty() {
        out.push_str(&format!("    description: {}\n", toolkit.description));
    }
}

fn render_tool_detail(
    out: &mut String,
    meta: &ToolMeta,
    toml: &upeg_loader::ToolToml,
    resolve_chain: bool,
) {
    out.push_str(&format!(
        "    tool: {} (invoker={}, pin={})\n",
        meta.id,
        meta.invoker.label(),
        meta.pin.label(),
    ));
    if !meta.description.is_empty() {
        out.push_str(&format!("      description: {}\n", meta.description));
    }
    if meta.surfaces.len() < upeg_core::ALL_SURFACES.len() {
        out.push_str(&format!(
            "      surfaces: [{}]\n",
            meta.surface_labels().join(", "),
        ));
    }
    if !meta.boards.is_empty() {
        out.push_str(&format!("      boards: [{}]\n", meta.boards.join(", ")));
    }
    render_chain_detail(out, toml, resolve_chain);
    if let Some(cmd) = &toml.command {
        let template = toml.args_template.as_deref().unwrap_or(&[]);
        out.push_str(&format!("      command: `{cmd}` {template:?}\n"));
    }
    if let Some(url) = &toml.embed_url {
        out.push_str(&format!("      embed_url: {url}\n"));
    }
    render_selector_bindings(out, toml);
    render_toml_inputs(out, toml);
}

fn render_chain_detail(out: &mut String, toml: &upeg_loader::ToolToml, resolve_chain: bool) {
    let Some(steps) = &toml.steps else {
        return;
    };
    let step_tools: Vec<&str> = steps.iter().map(|step| step.tool.trim()).collect();
    let connection_count = toml.connections.as_ref().map_or(0, Vec::len);
    out.push_str(&format!(
        "      chain: {} node(s), {} connection(s) → [{}]\n",
        step_tools.len(),
        connection_count,
        step_tools.join(", ")
    ));
    if resolve_chain {
        out.push_str(&format!(
            "      chain resolved: all {} node(s) available\n",
            step_tools.len()
        ));
    }
}

fn render_selector_bindings(out: &mut String, toml: &upeg_loader::ToolToml) {
    let bindings = toml
        .controlled_embed
        .as_ref()
        .and_then(|ce| ce.bindings.as_ref());
    let Some(bindings) = bindings else {
        return;
    };
    if bindings.is_empty() {
        return;
    }
    let fields: Vec<&str> = bindings.iter().map(|b| b.field.as_str()).collect();
    out.push_str(&format!(
        "      controlled_embed.bindings: {} → [{}]\n",
        bindings.len(),
        fields.join(", "),
    ));
}

fn render_toml_inputs(out: &mut String, toml: &upeg_loader::ToolToml) {
    if toml.inputs.is_empty() {
        return;
    }
    let names: Vec<&str> = toml.inputs.iter().map(|f| f.name.as_str()).collect();
    out.push_str(&format!(
        "      inputs: {} field(s) → [{}]\n",
        toml.inputs.len(),
        names.join(", "),
    ));
}

fn validate_chain_nodes(
    toml: &upeg_loader::ToolToml,
    parsed_tool_ids: &[&str],
) -> Result<(), CliError> {
    if let Some(steps) = &toml.steps {
        let mut missing = Vec::new();
        for step in steps {
            let step = step.tool.trim();
            if step == toml.id {
                return Err(CliError::tool_failed(format!(
                    "chain node `{}` is the tool's own id (would recurse infinitely)",
                    toml.id
                )));
            }
            if !parsed_tool_ids.contains(&step) && toolbox_tool(step).is_none() {
                missing.push(step.to_string());
            }
        }
        if !missing.is_empty() {
            return Err(CliError::tool_failed(format!(
                "chain references {} unknown tool id(s): {}",
                missing.len(),
                missing.join(", "),
            )));
        }
    }
    Ok(())
}

const TOOLKIT_VALIDATE_EXTENSION: &str = "toml";

/// Line suffix reported for a scanned `.toml` file that parses as TOML but
/// lacks the toolkit-manifest shape (`id` string + `tools` array at the
/// top level) — e.g. a `Cargo.toml` sitting next to real toolkit
/// manifests in the same directory. These are reported, not treated as
/// validation failures, and do not affect the command's exit code.
const TOOLKIT_MANIFEST_SKIP_MESSAGE: &str = "skipped (not a toolkit manifest)";

/// Summary clause for the *other* kind of skip: a manifest that is a
/// perfectly good toolkit file, some of whose tools this host cannot
/// run (e.g. `pty = true` on a build without pseudoterminal support).
///
/// Deliberately a separate counter from the file-level
/// [`TOOLKIT_MANIFEST_SKIP_MESSAGE`] tally: one says "I did not look at
/// this file", the other says "I looked, and these tools will not be
/// there". Appended only when non-zero — the zero case would put two
/// different "skipped" numbers on the summary line of every run and
/// invite reading one for the other.
const TOOLKIT_TOOL_SKIP_SUMMARY_SUFFIX: &str = "tool(s) skipped by this host";

/// Classification of a scanned `.toml` file's *shape*, decided before any
/// deeper validation runs.
///
/// This only checks for the two fields every [`upeg_loader::ToolkitToml`]
/// must carry (`id: String`, `tools: Vec<..>`) — it does not run
/// `deny_unknown_fields` or any semantic check. That distinction matters:
/// a file that HAS the shape but is invalid some other way must still be
/// [`ManifestFileClass::Toolkit`] (a real error), while a file that never
/// looked like a toolkit manifest at all (no `id`/`tools`, e.g. a
/// `Cargo.toml`'s `[workspace]` table) is [`ManifestFileClass::NotAToolkit`]
/// and gets skipped instead of failed.
enum ManifestFileClass {
    Toolkit,
    NotAToolkit,
}

/// Decide [`ManifestFileClass`] from an already-parsed generic TOML value.
/// Parsing itself is done by the caller: a file that fails to parse as
/// TOML at all is a real error, never a skip (a broken toolkit manifest
/// must not hide behind this classification).
fn classify_manifest_shape(value: &toml::Value) -> ManifestFileClass {
    let has_toolkit_shape = value.as_table().is_some_and(|table| {
        matches!(table.get("id"), Some(toml::Value::String(_)))
            && matches!(table.get("tools"), Some(toml::Value::Array(_)))
    });
    if has_toolkit_shape {
        ManifestFileClass::Toolkit
    } else {
        ManifestFileClass::NotAToolkit
    }
}

/// Batched sibling of [`format_tool_validate`]: dry-run every `*.toml`
/// file directly under `dir` (no recursion — matches the auto-loader's
/// own top-level scan). Nothing is registered; this stays read-only
/// diagnostics like the single-file form. One `ok`/`error` line per
/// file plus a trailing summary count.
///
/// Returns [`CliError::StdoutFailure`] (prints the full report, exit 1)
/// when any file fails so scripts can gate on it while still seeing
/// every result; [`CliError::ToolFailed`] only for conditions that make
/// scanning impossible at all (no dir resolvable, dir missing, dir
/// unreadable).
pub fn format_toolkit_validate(dir: Option<&str>) -> Result<String, CliError> {
    let dir = resolve_toolkit_validate_dir(dir)?;
    if !dir.is_dir() {
        return Err(CliError::tool_failed(format!(
            "toolkits dir `{}` not found",
            dir.display()
        )));
    }

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| CliError::tool_failed(format!("read `{}`: {e}", dir.display())))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == TOOLKIT_VALIDATE_EXTENSION)
        })
        .collect();
    files.sort();

    let mut out = format!("toolkits dir: {}\n", dir.display());
    if files.is_empty() {
        out.push_str(&format!(
            "  (no *.{TOOLKIT_VALIDATE_EXTENSION} files found)\n"
        ));
        return Ok(out);
    }

    let mut ok_count = 0_usize;
    let mut failed_count = 0_usize;
    let mut skipped_count = 0_usize;
    let mut skipped_tool_count = 0_usize;
    for path in &files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                failed_count += 1;
                out.push_str(&format!("  error: {name}: {e}\n"));
                continue;
            }
        };
        let raw_toml: toml::Value = match toml::from_str(&content) {
            Ok(value) => value,
            Err(e) => {
                // Doesn't even parse as TOML — a broken toolkit manifest
                // must not hide behind the "not a toolkit manifest" skip.
                failed_count += 1;
                out.push_str(&format!("  error: {name}: {e}\n"));
                continue;
            }
        };

        match classify_manifest_shape(&raw_toml) {
            ManifestFileClass::NotAToolkit => {
                skipped_count += 1;
                out.push_str(&format!("  {name}: {TOOLKIT_MANIFEST_SKIP_MESSAGE}\n"));
            }
            ManifestFileClass::Toolkit => match validate_toolkit_content(&content) {
                Ok(parse) => {
                    ok_count += 1;
                    skipped_tool_count += parse.skipped.len();
                    out.push_str(&format!("  ok: {name} ({} tool(s))\n", parse.tools.len()));
                    out.push_str(&render_tool_skip_lines(&parse.skipped));
                }
                Err(msg) => {
                    failed_count += 1;
                    out.push_str(&format!("  error: {name}: {msg}\n"));
                }
            },
        }
    }
    out.push_str(&render_toolkit_validate_summary(
        ok_count,
        failed_count,
        skipped_count,
        skipped_tool_count,
    ));

    if failed_count > 0 {
        Err(CliError::stdout_failure(out))
    } else {
        Ok(out)
    }
}

/// The per-tool skip lines nested under an otherwise-`ok` file.
///
/// Indented one level deeper than the file's own line so the reading is
/// unambiguous: the file validated, and *within* it these tools will not
/// be there. `~` rather than `error:` because nothing is wrong with the
/// manifest — this host simply cannot honour the declaration.
///
/// A separate function because a skip only happens on a host that lacks
/// the capability (`pty` is available on every unix), so the rendering
/// has to be reachable from a test that never gets a real skip.
pub(crate) fn render_tool_skip_lines(skipped: &[upeg_loader::SkippedTool]) -> String {
    let mut out = String::new();
    for tool in skipped {
        let _ = writeln!(out, "    ~ {}: skipped ({})", tool.id, tool.reason);
    }
    out
}

/// The trailing summary line, with the tool-level skip clause appended
/// only when this host actually dropped something — see
/// [`TOOLKIT_TOOL_SKIP_SUMMARY_SUFFIX`].
pub(crate) fn render_toolkit_validate_summary(
    ok_count: usize,
    failed_count: usize,
    skipped_file_count: usize,
    skipped_tool_count: usize,
) -> String {
    let mut summary = format!("{ok_count} ok, {failed_count} failed, {skipped_file_count} skipped");
    if skipped_tool_count > 0 {
        summary.push_str(&format!(
            ", {skipped_tool_count} {TOOLKIT_TOOL_SKIP_SUMMARY_SUFFIX}"
        ));
    }
    summary.push('\n');
    summary
}

/// Resolve the directory `toolkit validate` scans: the explicit `dir`
/// argument, or the runtime toolkits dir the auto-loader itself watches
/// (`$UPEG_TOOLKITS_DIR` / `~/.upeg/toolkits`).
fn resolve_toolkit_validate_dir(dir: Option<&str>) -> Result<std::path::PathBuf, CliError> {
    if let Some(dir) = dir {
        return Ok(std::path::PathBuf::from(dir));
    }
    crate::infrastructure::paths::toolkits_dir().ok_or_else(|| {
        CliError::tool_failed(
            "no toolkits dir configured (`$UPEG_TOOLKITS_DIR` unset and no $HOME); pass a directory explicitly",
        )
    })
}

/// One file's worth of [`format_tool_validate`]'s core check, trimmed to
/// just the signal the directory summary needs. Only called once
/// [`classify_manifest_shape`] has already confirmed the file has the
/// toolkit-manifest shape; `content` was read once by the caller.
///
/// Keeps the host's skip verdict
/// ([`upeg_loader::ToolkitParse::skipped`]) rather than only a tool
/// count: a manifest whose `pty = true` tool drops out on this build
/// still validates `ok`, and the operator has to be told *which* tool
/// they will not find in `upeg tool list` afterwards.
fn validate_toolkit_content(content: &str) -> Result<upeg_loader::ToolkitParse, String> {
    upeg_loader::parse_toolkit_with_skips(content).map_err(|e| e.to_string())
}

/// Render one Tool's manifest as a key-value block. Surfaced fields:
///   - `inputs`: summary so CLI users discover args without `--json`.
///   - `description`: first-time UX — tell the user what this tool
///     does before they call it.
///   - `controlled_embed.bindings`: confirms a TOML's nested Controlled
///     Embed bindings landed in the sidecar registry.
///   - `embed_url`: confirms registration of an Embed Tool's URL.
///
/// All four are conditional on having content (description shows a
/// "(no description)" placeholder when empty; inputs shows "(none)";
/// `embed_url` and `selector_bindings` are suppressed entirely when
/// absent) so the output stays terse for plain function tools.
pub fn format_tool_show(id: &str) -> Result<String, CliError> {
    let t = toolbox_tool(id).ok_or_else(|| CliError::UnknownTool(id.to_string()))?;
    let mut out = String::new();
    out.push_str(&format!("id            {}\n", t.id));
    out.push_str(&format!("display_label {}\n", t.display_label));
    out.push_str(&format!("toolkit       {}\n", t.toolkit));
    out.push_str(&format!("tool          {}\n", t.tool_id()));
    out.push_str(&format!("tags          {}\n", t.tag_labels().join(", ")));
    // text-form `tool show` previously omitted description
    // entirely (the JSON variant always had it). Match the TUI Detail
    // view's "(no description)" placeholder for the empty case so a
    // user running `upeg tool show <id>` actually sees what the tool
    // does — first-time UX gap.
    let description_label = if t.description.is_empty() {
        "(no description)"
    } else {
        t.description
    };
    out.push_str(&format!("description   {description_label}\n"));
    out.push_str(&format!("pin   {}\n", t.pin.label()));
    out.push_str(&format!("pegboard_units {}\n", t.pegboard_units.label()));
    out.push_str(&format!("invoker       {}\n", t.invoker.label()));
    out.push_str(&format!("surfaces      {}\n", t.surfaces_label()));
    if t.boards.is_empty() {
        out.push_str("boards (none)\n");
    } else {
        out.push_str(&format!("boards {}\n", t.boards.join(", ")));
    }
    if let Some(url) = upeg_runtime::embed_url_for(t.id) {
        out.push_str(&format!("embed_url     {url}\n"));
    }
    // Surface the typed input fields so CLI users can discover a
    // tool's args without falling back to `--json`. Empty spec →
    // "(none)" line for consistency with the boards format.
    // Non-empty → header line with field count + one indented row
    // per field showing name (type, required-marker) — description.
    if t.input_spec.fields.is_empty() {
        out.push_str("inputs        (none)\n");
    } else {
        out.push_str(&format!(
            "inputs        {} field(s)\n",
            t.input_spec.fields.len()
        ));
        for field in &t.input_spec.fields {
            let req = if field.required { ", required" } else { "" };
            let desc = if field.description.as_deref().unwrap_or_default().is_empty() {
                String::new()
            } else {
                format!(" — {}", field.description.as_deref().unwrap_or_default())
            };
            // Same two-space-indent + fixed-width pattern as
            // selector_bindings below for vertical alignment.
            out.push_str(&format!(
                "  {:<12} ({}{}){}\n",
                field.name.as_str(),
                field.kind.label(),
                req,
                desc
            ));
        }
    }
    let bindings = upeg_runtime::selector_bindings_for(t.id);
    if !bindings.is_empty() {
        out.push_str(&format!("selector_bindings ({})\n", bindings.len()));
        for b in &bindings {
            // Two-space indent so bindings visually nest under the
            // header. Field is left-padded to a fixed width so the
            // selectors line up vertically — easier to scan.
            out.push_str(&format!("  {:<12} → {}\n", b.field, b.selector));
        }
    }
    // Both CLI invocation forms, so a user landing here from discovery
    // can copy-paste a call directly: the dynamic `{toolkit} {tool}`
    // route and the generic `call` route. This same block backs
    // `upeg <toolkit> <tool> --help`.
    out.push_str("invoke\n");
    out.push_str(&format!("  {}\n", dynamic_route_example(t)));
    out.push_str(&format!("  {}\n", call_example(t)));
    Ok(out)
}

/// JSON variant of `format_tool_show`. Same per-tool shape as the
/// `/v1/tools` HTTP endpoint and `tool list --json`. Includes
/// `embedUrl` (string when registered, null otherwise) for shape
/// consistency with `tool list --json`.
pub fn format_tool_show_json(id: &str) -> Result<String, CliError> {
    let t = toolbox_tool(id).ok_or_else(|| CliError::UnknownTool(id.to_string()))?;
    // shared upeg-core helper. Same shape as `tool list --json`
    // and HTTP `/v1/tools` (HTTP uses `name` instead of `id`).
    let entry = t.to_json_object("id");
    let mut out = serde_json::to_string_pretty(&entry).unwrap_or_else(|_| "{}".to_string());
    out.push('\n');
    Ok(out)
}
