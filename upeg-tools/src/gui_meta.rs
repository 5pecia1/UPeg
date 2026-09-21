//! GUI-only Tool metadata shared by Desktop, PWA, Ext, and TUI inspection.
//!
//! These Tools do not have headless dispatchers. Keeping their `ToolMeta`
//! records in `upeg-tools` lets every surface resolve the same shared
//! Pegboard layout while each surface still enforces its own run capability.

use upeg_core::{Invoker, PegboardUnits, PinKind, StaticInputSpec, StaticToolMeta, ToolEffect};

/// Public ids for the Live+Timer GUI tools that ship with dispatchers.
/// Mirror the `*_TOOL_ID` constants emitted by `#[upeg::tool]` so the
/// dispatch table (and its `built-in dispatcher ids must come from
/// *_TOOL_ID constants` lint) references each tool by name instead of
/// by raw string literal. Other gui_meta tools get their own constants
/// when their dispatchers land.
pub const TIME_EPOCH_TOOL_ID: &str = "time.epoch";
// `net.status` is native-only (no `std::net` on wasm32) — gate the
// constant so the wasm clippy lane doesn't flag it as dead code.
#[cfg(not(target_arch = "wasm32"))]
pub const NET_STATUS_TOOL_ID: &str = "net.status";

pub(crate) const MEMO_SCRATCH_OUTPUT_ID: &str = "memo";
pub(crate) const EMBED_TRANSFORM_TOOLS_OUTPUT_ID: &str = "view";
pub(crate) const TIME_EPOCH_EPOCH_OUTPUT_ID: &str = "epoch";
pub(crate) const TIME_EPOCH_ISO_OUTPUT_ID: &str = "iso";
pub(crate) const NET_STATUS_STATUS_OUTPUT_ID: &str = "status";
pub(crate) const NET_STATUS_PING_MS_OUTPUT_ID: &str = "ping_ms";
pub(crate) const NET_STATUS_IPV6_OUTPUT_ID: &str = "ipv6";

upeg_core::inventory::submit! {
    StaticToolMeta {
        id: "memo.scratch", toolkit: "memo", local_id: "scratch",
        tags: &["memo", "note"],
        display_label: "Scratch memo",
        description: "Quick scratch memo on the pegboard (local-first).",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec {
            fields: &[upeg_core::StaticOutputFieldSpec {
                name: MEMO_SCRATCH_OUTPUT_ID,
                label: None,
                description: Some("Persisted scratch memo content"),
                kind: upeg_core::StaticOutputKind::Markdown,
                constraints: upeg_core::StaticFieldConstraints::empty(),
            }],
        },
        primary_output_id: Some(MEMO_SCRATCH_OUTPUT_ID),
        effect: ToolEffect::Unknown,
        source: upeg_core::StaticSource::UserInput,
        pin: PinKind::Live, pegboard_units: PegboardUnits::U1, invoker: Invoker::Function,
        surfaces: upeg_core::GUI_SURFACES,
        boards: &["dev", "personal"],
    }
}

upeg_core::inventory::submit! {
    StaticToolMeta {
        id: "memo.create", toolkit: "memo", local_id: "create",
        tags: &["memo", "action"],
        display_label: "New memo",
        description: "Action button: create a new memo.",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec::empty(),
        primary_output_id: None,
        effect: ToolEffect::Unknown,
        source: upeg_core::StaticSource::Shortcut { keys: "Cmd+Shift+N" },
        pin: PinKind::Action, pegboard_units: PegboardUnits::U1, invoker: Invoker::Function,
        surfaces: upeg_core::GUI_SURFACES,
        boards: &["personal"],
    }
}

upeg_core::inventory::submit! {
    StaticToolMeta {
        id: "embed.transform_tools", toolkit: "embed", local_id: "transform_tools",
        tags: &["embed", "web"],
        display_label: "Transform.tools embed",
        description: "WebView embed of transform.tools (selector-mapped).",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec {
            fields: &[upeg_core::StaticOutputFieldSpec {
                name: EMBED_TRANSFORM_TOOLS_OUTPUT_ID,
                label: None,
                description: Some("Embedded transform.tools workspace"),
                kind: upeg_core::StaticOutputKind::EmbeddedView {
                    url: "https://transform.tools/json-to-typescript",
                },
                constraints: upeg_core::StaticFieldConstraints::empty(),
            }],
        },
        primary_output_id: Some(EMBED_TRANSFORM_TOOLS_OUTPUT_ID),
        effect: ToolEffect::Unknown,
        source: upeg_core::StaticSource::Static,
        // Passive Embed: pin = Embed pairs with invoker = Static
        // (no app-driven invocation — user interacts with the webview
        // directly). Controlled Embed tools use pin = ControlledEmbed +
        // invoker = Embed instead.
        pin: PinKind::Embed, pegboard_units: PegboardUnits::U2, invoker: Invoker::Static,
        surfaces: upeg_core::EMBED_SURFACES,
        boards: &["dev"],
    }
}

upeg_core::inventory::submit! {
    StaticToolMeta {
        id: "time.epoch", toolkit: "time", local_id: "epoch",
        tags: &["live", "ticker"],
        display_label: "Unix epoch (live)",
        description: "Unix epoch + ISO timestamp (live ticker).",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec {
            fields: &[
                upeg_core::StaticOutputFieldSpec {
                    name: TIME_EPOCH_EPOCH_OUTPUT_ID,
                    label: None,
                    description: Some("Current Unix epoch seconds"),
                    kind: upeg_core::StaticOutputKind::Number,
                    constraints: upeg_core::StaticFieldConstraints::empty(),
                },
                upeg_core::StaticOutputFieldSpec {
                    name: TIME_EPOCH_ISO_OUTPUT_ID,
                    label: None,
                    description: Some("ISO 8601 timestamp"),
                    kind: upeg_core::StaticOutputKind::DateTime,
                    constraints: upeg_core::StaticFieldConstraints::empty(),
                },
            ],
        },
        primary_output_id: Some(TIME_EPOCH_EPOCH_OUTPUT_ID),
        effect: ToolEffect::Unknown,
        source: upeg_core::StaticSource::Timer { interval_ms: 1000 },
        pin: PinKind::Live, pegboard_units: PegboardUnits::U1, invoker: Invoker::Function,
        surfaces: upeg_core::GUI_SURFACES,
        boards: &["trading", "personal"],
    }
}

upeg_core::inventory::submit! {
    StaticToolMeta {
        id: "net.status", toolkit: "net", local_id: "status",
        tags: &["live", "network"],
        display_label: "Net status",
        description: "Network status + latency.",
        input_spec: StaticInputSpec::empty(),
        output_spec: upeg_core::StaticOutputSpec {
            fields: &[
                upeg_core::StaticOutputFieldSpec {
                    name: NET_STATUS_STATUS_OUTPUT_ID,
                    label: None,
                    description: Some("OK / FAIL"),
                    kind: upeg_core::StaticOutputKind::String,
                    constraints: upeg_core::StaticFieldConstraints::empty(),
                },
                upeg_core::StaticOutputFieldSpec {
                    name: NET_STATUS_PING_MS_OUTPUT_ID,
                    label: None,
                    description: Some("Round-trip latency in milliseconds"),
                    kind: upeg_core::StaticOutputKind::Number,
                    constraints: upeg_core::StaticFieldConstraints::empty(),
                },
                upeg_core::StaticOutputFieldSpec {
                    name: NET_STATUS_IPV6_OUTPUT_ID,
                    label: None,
                    description: Some("IPv6 reachable"),
                    kind: upeg_core::StaticOutputKind::Boolean,
                    constraints: upeg_core::StaticFieldConstraints::empty(),
                },
            ],
        },
        primary_output_id: Some(NET_STATUS_STATUS_OUTPUT_ID),
        effect: ToolEffect::Unknown,
        source: upeg_core::StaticSource::Timer { interval_ms: 5000 },
        pin: PinKind::Live, pegboard_units: PegboardUnits::U1, invoker: Invoker::Function,
        surfaces: upeg_core::GUI_SURFACES,
        boards: &["trading"],
    }
}

// csv.diff, qr.encode, color.contrast, and eth.gas/eth.address_lookup were
// promoted out of this GUI-only registry to real `#[tool]` definitions with
// dispatchers — see `toolkits/csv.rs`, `toolkits/qr.rs`, `toolkits/color.rs`
// (`color_contrast`), and `toolkits/eth.rs`. They no longer belong here
// because they now run headlessly (CLI/TUI/MCP/HTTP), which is exactly what
// this module's doc comment says GUI-only metas cannot do. `eth.*`'s runtime
// dispatcher is still native-only (see `toolkits/eth.rs`'s module doc), but
// unlike this module's remaining rows, its `StaticToolMeta` now comes from a
// real `#[tool]`-annotated function rather than a hand-written `submit!`.
