//! Surface-agnostic tool metadata helpers.
//!
//! The canonical `ToolMeta` / `PinKind` records live in `upeg-core` /
//! `upeg-tools` so every surface can resolve the same shared Pegboard
//! layout. This module owns the CSS palette mapping (consumed by the
//! Flutter shell via FRB-passed strings) plus the registration
//! smoke-tests that pin the shared toolbox shape.

pub use upeg_core::{PinKind, ToolMeta};

/// Surface-specific CSS accent color for a pin kind. Lives in
/// upeg-pegboard-ui (not upeg-core) because the value is a CSS
/// variable expression — TUI/CLI surfaces would map to different
/// palettes, and the FRB DTO layer hands the string straight to
/// Flutter for theming parity.
pub const fn pin_color(kind: PinKind) -> &'static str {
    match kind {
        PinKind::Inline => "var(--accent)",
        PinKind::Launcher => "oklch(0.78 0.13 320)",
        PinKind::Live => "var(--accent-2)",
        PinKind::Action => "var(--pin)",
        // Passive Embed — blue-ish, mirrors the "external viewer" feel.
        PinKind::Embed => "oklch(0.78 0.13 250)",
        // Controlled Embed — teal, distinct from passive viewer.
        PinKind::ControlledEmbed => "oklch(0.74 0.15 200)",
        PinKind::Chain => "oklch(0.78 0.12 180)",
        PinKind::Llm => "oklch(0.80 0.14 35)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_종류_색상은_모든_변형에_대해_css를_반환한다() {
        // No `_ =>` catch-all; if a new variant lands without a color, this
        // test fails at compile time once we exhaustively match. For now,
        // smoke-check each known variant.
        for k in [
            PinKind::Inline,
            PinKind::Launcher,
            PinKind::Live,
            PinKind::Action,
            PinKind::Embed,
            PinKind::Chain,
            PinKind::Llm,
        ] {
            let c = pin_color(k);
            assert!(!c.is_empty(), "no color for {k:?}");
            assert!(
                c.contains("var(") || c.contains("oklch("),
                "expected CSS expression, got {c}"
            );
        }
    }

    #[test]
    #[allow(
        clippy::panic,
        reason = "registration failure is a fixture bug, not runtime"
    )]
    fn 데스크톱_도구는_표시_라벨을_가진_메타데이터로_등록된다() {
        upeg_tools::register_all();
        for id in [
            "num.hex_to_decimal",
            "convert.json_format",
            "id.uuid_v7",
            "text.regex_match",
            "memo.scratch",
            "memo.create",
            "embed.transform_tools",
            "eth.gas",
            "time.epoch",
            "net.status",
        ] {
            let meta = upeg_runtime::toolbox_tool(id)
                .unwrap_or_else(|| panic!("desktop tool id must be registered: {id}"));
            assert!(
                !meta.display_label.is_empty(),
                "missing display label for {id}"
            );
        }
    }
}
