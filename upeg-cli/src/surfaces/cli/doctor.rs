//! `upeg doctor` — install diagnostics.
//!
//! Surface-agnostic data collection (binary path, build features,
//! runtime-source directory status, toolbox counts, per-surface
//! visibility diagnosis) rendered in two parallel formats:
//!   - `format_doctor` — human-readable text.
//!   - `format_doctor_json` — structured JSON for monitoring scripts.
//!
//! Both walk the same environment and toolbox inputs; only the
//! presentation differs. Future cleanup candidate: build a single
//! `DoctorReport` intermediate and let each formatter render it.
//!
//! The surface-visibility section (per-surface hidden counts + WHY-class
//! tallies) is classified in the sibling `doctor_surfaces` module so this
//! file stays focused on environment/toolbox collection.

use std::path::PathBuf;

use super::doctor_surfaces;

/// Render install diagnostics as plain text. Output is human-readable
/// and stable enough for tests to spot-check key tokens.
pub fn format_doctor() -> String {
    let mut out = String::new();
    out.push_str("=== upeg doctor ===\n");

    // Binary.
    let bin = std::env::current_exe()
        .map_or_else(|_| "<unknown>".to_string(), |p| p.display().to_string());
    out.push_str(&format!("binary:   {bin}\n"));
    out.push_str(&format!("version:  {}\n", env!("CARGO_PKG_VERSION")));

    // Build features.
    out.push_str("features:\n");
    #[cfg(feature = "wasm-plugin")]
    out.push_str("  - wasm-plugin (enabled)\n");
    #[cfg(not(feature = "wasm-plugin"))]
    out.push_str("  - wasm-plugin (disabled — rebuild with --features wasm-plugin)\n");

    fn report_dir(out: &mut String, label: &str, dir: Option<PathBuf>, ext: &str) {
        match dir {
            None => out.push_str(&format!("  ✗ {label}: <no $HOME>\n")),
            Some(p) => {
                if !p.is_dir() {
                    out.push_str(&format!("  ✗ {label}: {} (not present)\n", p.display()));
                    return;
                }
                let count = std::fs::read_dir(&p)
                    .map(|entries| {
                        entries
                            .flatten()
                            .filter(|e| e.path().extension().is_some_and(|x| x == ext))
                            .count()
                    })
                    .unwrap_or(0);
                out.push_str(&format!(
                    "  ✓ {label}: {} ({} *.{} file(s))\n",
                    p.display(),
                    count,
                    ext
                ));
            }
        }
    }

    use crate::infrastructure::paths;
    out.push_str("runtime sources:\n");
    report_dir(&mut out, "toolkits dir", paths::toolkits_dir(), "toml");
    report_dir(&mut out, "wasm dir", paths::wasm_dir(), "wasm");
    report_dir(&mut out, "mcp-imports dir", paths::mcp_import_dir(), "toml");
    out.push_str(&project_manifest_line(
        &upeg_sources::project::project_manifest_status(),
    ));

    let inventory_count = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>().count();
    let total_count = upeg_runtime::toolbox_tools().count();
    let runtime_count = total_count.saturating_sub(inventory_count);
    out.push_str("toolbox:\n");
    out.push_str(&format!("  built-ins: {inventory_count}\n"));
    out.push_str(&format!("  runtime:   {runtime_count}\n"));
    out.push_str(&format!("  total:     {total_count}\n"));

    let tools: Vec<&upeg_core::ToolMeta> = upeg_runtime::toolbox_tools().collect();
    let diagnosis = doctor_surfaces::diagnose(tools.iter().copied());
    out.push_str(&doctor_surfaces::format_text(&diagnosis));
    out
}

/// C-7: `upeg doctor` text line for project-manifest detection state —
/// the resolved path (or `none`) plus the [`upeg_sources::project::ProjectManifestOverride`]
/// that produced it, so `off` / an explicit `UPEG_PROJECT_MANIFEST_PATH`
/// override is visible right next to the other runtime sources instead
/// of silently changing Toolbox contents. Pure over an already-resolved
/// `status` (env/cwd reads happen once, at the `format_doctor*` call
/// site) so it's testable without touching real process state.
fn project_manifest_line(status: &upeg_sources::project::ProjectManifestStatus) -> String {
    let label = upeg_sources::project::project_manifest_override_label(&status.override_state);
    match &status.lookup {
        Some(lookup) => format!(
            "  ✓ project manifest: {} (override: {label})\n",
            lookup.path.display()
        ),
        None => format!("  ✗ project manifest: none (override: {label})\n"),
    }
}

/// JSON form of [`project_manifest_line`], reusing the same typed
/// status/label so the text and JSON renderings can never drift.
fn project_manifest_json(
    status: &upeg_sources::project::ProjectManifestStatus,
) -> serde_json::Value {
    serde_json::json!({
        "path": status.lookup.as_ref().map(|l| l.path.display().to_string()),
        "override": upeg_sources::project::project_manifest_override_label(&status.override_state),
    })
}

/// JSON variant of `format_doctor`. Same data shape, structured for
/// monitoring scripts and CI checks. Fields:
///   `binary` (str), `version` (str), `features` (object of bool flags),
///   `runtimeSources` (array of `{label, path, present, count}`),
///   `projectManifest` (object: `path` (str or null), `override`
///   (`"detect"`/`"off"`/`"explicit"`)),
///   `toolbox` (object: `builtins`, `runtime`, `total`),
///   `surfaceDiagnosis` (object: `bySurface` array of
///   `{surface, visible, hidden}`, `whyClasses` object of
///   `{wasmFeatureStubbed, nativeOnlyExcluded}`) — see `doctor_surfaces`.
pub fn format_doctor_json() -> String {
    let bin = std::env::current_exe()
        .map_or_else(|_| "<unknown>".to_string(), |p| p.display().to_string());

    let mut features = serde_json::Map::new();
    features.insert("wasm-plugin".into(), cfg!(feature = "wasm-plugin").into());

    fn dir_entry(label: &str, dir: Option<PathBuf>, ext: &str) -> serde_json::Value {
        let (path, present, count) = match dir {
            None => ("<no $HOME>".to_string(), false, 0),
            Some(p) => {
                let path_str = p.display().to_string();
                if p.is_dir() {
                    let n = std::fs::read_dir(&p)
                        .map(|entries| {
                            entries
                                .flatten()
                                .filter(|e| e.path().extension().is_some_and(|x| x == ext))
                                .count()
                        })
                        .unwrap_or(0);
                    (path_str, true, n)
                } else {
                    (path_str, false, 0)
                }
            }
        };
        serde_json::json!({
            "label": label,
            "path": path,
            "present": present,
            "count": count,
        })
    }

    use crate::infrastructure::paths;
    let sources = serde_json::json!([
        dir_entry("toolkits", paths::toolkits_dir(), "toml"),
        dir_entry("wasm", paths::wasm_dir(), "wasm"),
        dir_entry("mcp-imports", paths::mcp_import_dir(), "toml"),
    ]);

    let inventory_count = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>().count();
    let total_count = upeg_runtime::toolbox_tools().count();
    let runtime_count = total_count.saturating_sub(inventory_count);

    let tools: Vec<&upeg_core::ToolMeta> = upeg_runtime::toolbox_tools().collect();
    let diagnosis = doctor_surfaces::diagnose(tools.iter().copied());

    let doc = serde_json::json!({
        "binary": bin,
        "version": env!("CARGO_PKG_VERSION"),
        "features": features,
        "runtimeSources": sources,
        "projectManifest": project_manifest_json(&upeg_sources::project::project_manifest_status()),
        "toolbox": {
            "builtins": inventory_count,
            "runtime":  runtime_count,
            "total":    total_count,
        },
        "surfaceDiagnosis": doctor_surfaces::to_json(&diagnosis),
    });
    let mut out = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use upeg_sources::project::{
        ProjectManifestLookup, ProjectManifestOrigin, ProjectManifestOverride,
        ProjectManifestStatus,
    };

    #[test]
    fn 프로젝트_매니페스트_텍스트_줄은_탐지된_경로와_override_상태를_보여준다() {
        let status = ProjectManifestStatus {
            lookup: Some(ProjectManifestLookup {
                path: PathBuf::from("/home/user/project/upeg.toml"),
                origin: ProjectManifestOrigin::Detected,
            }),
            override_state: ProjectManifestOverride::Detect,
        };

        let line = project_manifest_line(&status);

        assert!(line.contains("/home/user/project/upeg.toml"));
        assert!(line.contains("override: detect"));
    }

    #[test]
    fn 프로젝트_매니페스트_텍스트_줄은_없을_때_none과_off를_보여준다() {
        let status = ProjectManifestStatus {
            lookup: None,
            override_state: ProjectManifestOverride::Disabled,
        };

        let line = project_manifest_line(&status);

        assert!(line.contains("none"));
        assert!(line.contains("override: off"));
    }

    #[test]
    fn 프로젝트_매니페스트_json은_경로와_override를_필드로_담는다() {
        let explicit_path = PathBuf::from("/explicit/upeg.toml");
        let status = ProjectManifestStatus {
            lookup: Some(ProjectManifestLookup {
                path: explicit_path.clone(),
                origin: ProjectManifestOrigin::EnvOverride,
            }),
            override_state: ProjectManifestOverride::Explicit(explicit_path.clone()),
        };

        let json = project_manifest_json(&status);

        assert_eq!(json["path"], explicit_path.display().to_string());
        assert_eq!(json["override"], "explicit");
    }

    #[test]
    fn 프로젝트_매니페스트_json은_없을_때_path가_null이다() {
        let status = ProjectManifestStatus {
            lookup: None,
            override_state: ProjectManifestOverride::Detect,
        };

        let json = project_manifest_json(&status);

        assert!(json["path"].is_null());
        assert_eq!(json["override"], "detect");
    }
}
