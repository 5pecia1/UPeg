//! `upeg doctor` surface-visibility diagnosis.
//!
//! For every declared [`Surface`], classify why a registered Tool would be
//! invisible or non-dispatchable on this build. Three closed WHY-classes:
//!   - **surface-filtered** — [`ToolMeta::is_on_surface`] excludes it
//!     because the manifest's `surfaces` list is narrower than
//!     [`ALL_SURFACES`]. Reported per-surface as `hidden` counts.
//!   - **wasm-plugin feature stubbed** — `invoker = Wasm` (a declarative
//!     TOML `wasm_path` tool), but this binary wasn't built with
//!     `--features wasm-plugin`. The tool is still registered and listed
//!     — only dispatch fails, with the stub error in
//!     `upeg-loader/src/dispatcher.rs`.
//!   - **native-only excluded** — reserved for a wasm32-target build,
//!     where `upeg-tools` compiles native-only builtins (e.g. `net.*`)
//!     out at the source level, so they never make it into the registry
//!     at all. This binary is always compiled native (upeg-cli never
//!     targets wasm32), so this class is structurally `0` here; the
//!     shape exists so a future wasm32-target report can reuse it.
//!
//! `format_text` / `to_json` render the same [`SurfaceDiagnosis`] as text
//! and JSON respectively — mirrors the `format_doctor` / `format_doctor_json`
//! split in `doctor.rs`.

use upeg_core::{ALL_SURFACES, Invoker, Surface, ToolMeta};

/// Per-surface visibility counts for the current toolbox snapshot.
pub struct SurfaceVisibility {
    pub surface: Surface,
    pub visible: usize,
    pub hidden: usize,
}

/// Full diagnosis: per-surface visibility plus the cross-cutting
/// WHY-classes that apply regardless of which surface is asked about.
pub struct SurfaceDiagnosis {
    pub by_surface: Vec<SurfaceVisibility>,
    pub wasm_feature_stubbed: usize,
    pub native_only_excluded: usize,
}

/// Classify `tools` (typically `upeg_runtime::toolbox_tools()`) into a
/// [`SurfaceDiagnosis`]. Takes a `Clone` iterator so it can walk the same
/// tool set once per surface plus once for the WHY-class tally without
/// requiring the caller to materialize more than one collection.
pub fn diagnose<'a>(tools: impl Iterator<Item = &'a ToolMeta> + Clone) -> SurfaceDiagnosis {
    let by_surface = ALL_SURFACES
        .iter()
        .map(|&surface| {
            let (mut visible, mut hidden) = (0_usize, 0_usize);
            for tool in tools.clone() {
                if tool.is_on_surface(surface) {
                    visible += 1;
                } else {
                    hidden += 1;
                }
            }
            SurfaceVisibility {
                surface,
                visible,
                hidden,
            }
        })
        .collect();

    let wasm_feature_stubbed = if cfg!(feature = "wasm-plugin") {
        0
    } else {
        tools.filter(|t| t.invoker == Invoker::Wasm).count()
    };

    SurfaceDiagnosis {
        by_surface,
        wasm_feature_stubbed,
        // See module doc: always 0 on this native binary.
        native_only_excluded: 0,
    }
}

/// Human-readable rendering, appended to `upeg doctor`'s plain-text output.
pub fn format_text(diagnosis: &SurfaceDiagnosis) -> String {
    let mut out = String::new();
    out.push_str("surface diagnosis:\n");
    for v in &diagnosis.by_surface {
        out.push_str(&format!(
            "  {:<8} {} visible, {} hidden (surface-filtered)\n",
            v.surface.label(),
            v.visible,
            v.hidden
        ));
    }
    out.push_str(&format!(
        "  wasm-plugin feature: {} tool(s) stubbed (requires --features wasm-plugin)\n",
        diagnosis.wasm_feature_stubbed
    ));
    out.push_str(&format!(
        "  native-only (wasm32 exclusion): {} tool(s)\n",
        diagnosis.native_only_excluded
    ));
    out
}

/// JSON rendering with the same field shape as [`format_text`], merged
/// into `upeg doctor --json`'s top-level object under
/// `surfaceDiagnosis`.
pub fn to_json(diagnosis: &SurfaceDiagnosis) -> serde_json::Value {
    let by_surface: Vec<serde_json::Value> = diagnosis
        .by_surface
        .iter()
        .map(|v| {
            serde_json::json!({
                "surface": v.surface.label(),
                "visible": v.visible,
                "hidden": v.hidden,
            })
        })
        .collect();
    serde_json::json!({
        "bySurface": by_surface,
        "whyClasses": {
            "wasmFeatureStubbed": diagnosis.wasm_feature_stubbed,
            "nativeOnlyExcluded": diagnosis.native_only_excluded,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(id: &'static str, invoker: Invoker, surfaces: &'static [Surface]) -> ToolMeta {
        ToolMeta {
            id,
            toolkit: "test",
            local_id: id,
            tags: &[],
            display_label: "Test",
            description: "",
            input_spec: upeg_core::InputSpec::empty(),
            output_spec: upeg_core::OutputSpec::empty(),
            primary_output_id: None,
            source: upeg_core::Source::UserInput,
            pin: upeg_core::PinKind::Inline,
            pegboard_units: upeg_core::PegboardUnits::U1,
            invoker,
            surfaces,
            boards: &[],
        }
    }

    #[test]
    fn 표면_필터는_해당_표면에_없는_도구를_숨김으로_센다() {
        let tools = [
            make_tool("a.one", Invoker::Function, upeg_core::ALL_SURFACES),
            make_tool("a.two", Invoker::Function, &[Surface::Mcp, Surface::Http]),
        ];
        let diagnosis = diagnose(tools.iter());
        let cli = diagnosis
            .by_surface
            .iter()
            .find(|v| v.surface == Surface::Cli)
            .expect("cli row present");
        assert_eq!(cli.visible, 1, "only `a.one` is on cli");
        assert_eq!(cli.hidden, 1, "`a.two` is surface-filtered off cli");

        let mcp = diagnosis
            .by_surface
            .iter()
            .find(|v| v.surface == Surface::Mcp)
            .expect("mcp row present");
        assert_eq!(mcp.visible, 2, "both tools are on mcp");
        assert_eq!(mcp.hidden, 0);
    }

    #[test]
    fn 모든_표면이_보고서에_한_번씩_등장한다() {
        let tools: Vec<ToolMeta> = Vec::new();
        let diagnosis = diagnose(tools.iter());
        assert_eq!(diagnosis.by_surface.len(), ALL_SURFACES.len());
    }

    #[test]
    fn wasm_기능이_없으면_wasm_invoker_도구가_스텁으로_집계된다() {
        let tools = [
            make_tool("a.wasm", Invoker::Wasm, upeg_core::ALL_SURFACES),
            make_tool("a.fn", Invoker::Function, upeg_core::ALL_SURFACES),
        ];
        let diagnosis = diagnose(tools.iter());
        if cfg!(feature = "wasm-plugin") {
            assert_eq!(diagnosis.wasm_feature_stubbed, 0);
        } else {
            assert_eq!(diagnosis.wasm_feature_stubbed, 1);
        }
    }

    #[test]
    fn 네이티브_전용_제외는_이_네이티브_바이너리에서_항상_0이다() {
        let tools: Vec<ToolMeta> = Vec::new();
        let diagnosis = diagnose(tools.iter());
        assert_eq!(diagnosis.native_only_excluded, 0);
    }

    #[test]
    fn json_렌더링은_표면_개수와_why_클래스_필드를_포함한다() {
        let tools: Vec<ToolMeta> = Vec::new();
        let diagnosis = diagnose(tools.iter());
        let json = to_json(&diagnosis);
        assert_eq!(
            json["bySurface"].as_array().unwrap().len(),
            ALL_SURFACES.len()
        );
        assert!(json["whyClasses"]["wasmFeatureStubbed"].is_u64());
        assert!(json["whyClasses"]["nativeOnlyExcluded"].is_u64());
    }
}
