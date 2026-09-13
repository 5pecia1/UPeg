use upeg_core::Surface;
use upeg_core::interface_inventory::{InterfaceInventory, InterfaceKind};

const TOOL_CAPABILITY_SUMMARY_HEADING: &str = "## Tool Capability Summary";
const TOOL_CAPABILITY_SUMMARY_HEADER: &str = "| Category | Unique tool count | Rule |";
const TOOL_CAPABILITY_SUMMARY_SEPARATOR: &str = "| --- | ---: | --- |";
const HEADLESS_DISPATCHABLE_TOOLS_LABEL: &str = "Headless-dispatchable tools";
const GUI_ONLY_TOOLS_LABEL: &str = "GUI-only tools";
const HEADLESS_DISPATCHABLE_TOOLS_RULE: &str =
    "Advertises at least one of the `cli`, `tui`, `mcp`, or `http` surfaces";
const GUI_ONLY_TOOLS_RULE: &str = "Advertises only `desktop`, `pwa`, and/or `ext` surfaces";

pub(super) fn format_tool_capability_summary(inventory: &InterfaceInventory) -> Vec<String> {
    let tool_capabilities = summarize_tool_capabilities(inventory);
    vec![
        TOOL_CAPABILITY_SUMMARY_HEADING.to_string(),
        TOOL_CAPABILITY_SUMMARY_HEADER.to_string(),
        TOOL_CAPABILITY_SUMMARY_SEPARATOR.to_string(),
        format!(
            "| {HEADLESS_DISPATCHABLE_TOOLS_LABEL} | {} | {HEADLESS_DISPATCHABLE_TOOLS_RULE} |",
            tool_capabilities.headless_dispatchable_count
        ),
        format!(
            "| {GUI_ONLY_TOOLS_LABEL} | {} | {GUI_ONLY_TOOLS_RULE} |",
            tool_capabilities.gui_only_count
        ),
        String::new(),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ToolCapabilitySummary {
    headless_dispatchable_count: usize,
    gui_only_count: usize,
}

/// Count Tool contracts by capability.
///
/// Schema v3 records one entry per Tool carrying its whole surface set, so
/// the entry count *is* the unique tool count — no per-surface folding.
fn summarize_tool_capabilities(inventory: &InterfaceInventory) -> ToolCapabilitySummary {
    let mut summary = ToolCapabilitySummary {
        headless_dispatchable_count: 0,
        gui_only_count: 0,
    };

    for entry in inventory
        .entries
        .iter()
        .filter(|entry| entry.kind == InterfaceKind::Tool)
    {
        if entry
            .surfaces
            .iter()
            .copied()
            .any(Surface::supports_headless_dispatch)
        {
            summary.headless_dispatchable_count += 1;
        } else {
            summary.gui_only_count += 1;
        }
    }

    summary
}
