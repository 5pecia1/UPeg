//! Thin wrapper that turns `upeg_cli::run` results into stdout + exit code.
//!
//! Before dispatch, we best-effort load every runtime tool source through
//! `upeg-sources`:
//!   - **TOML** Declarative Toolkits (PRD §5.1) from `$UPEG_TOOLKITS_DIR` or
//!     `~/.upeg/toolkits/`.
//!   - **Project Manifest** (`upeg.toml`, PRD §5.8) from cwd → `$HOME`-bounded
//!     parents → `$HOME`, unless `$UPEG_PROJECT_MANIFEST_PATH` overrides or
//!     disables detection (`upeg_sources::project` module docs).
//!   - **WASM Toolkits** from `$UPEG_WASM_DIR` or `~/.upeg/wasm/`
//!     (only with `--features wasm-plugin`).
//!
//! MCP upstream servers are NOT part of ordinary CLI startup — this keeps
//! one-shot commands from spawning subprocesses. Only long-lived server
//! processes (`upeg host start`, the desktop-embedded host, the
//! in-process `upeg mcp` server) import them, at their own startup
//! (`upeg_sources::mcp_import` module docs).

// Tests routinely use unwrap/expect/panic — restriction lints configured
// for prod code at workspace level are noise in tests. Same header as
// `lib.rs`, for the same reason.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

use clap::Parser;
use std::process::ExitCode;
use upeg_cli::{Cli, run};
use upeg_sources::{DirectoryStatus, RuntimeSourceConfig, RuntimeSourceReport};

const WORKING_DIRECTORY_FLAG: &str = "--working-directory";

fn main() -> ExitCode {
    // Parse FIRST so --quiet (and any future global flags) can suppress
    // the auto-loader's stderr summaries before they fire. clap exits
    // with the usual code for --help / --version / parse errors.
    let cli = Cli::parse();
    let quiet = cli.quiet;
    if let Err(error) = apply_working_directory(cli.working_directory.as_deref()) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    // Install the Controlled Embed headless backend when compiled in.
    // The default `NoopControlledEmbedBackend` returns a friendly
    // "feature disabled" error otherwise.
    install_controlled_embed_backend();

    let mut config = RuntimeSourceConfig::from_env();
    if let Some(root) = cli.project.as_deref() {
        let Some(project) = upeg_core::ProjectRoot::new(root) else {
            eprintln!("--project: {} has no .upeg directory", root.display());
            return ExitCode::FAILURE;
        };
        config.project_manifest = Some(upeg_sources::project::ProjectManifestLookup {
            path: project.marker_dir(),
            origin: upeg_sources::project::ProjectManifestOrigin::EnvOverride,
        });
    }
    let report = upeg_sources::load_local_runtime_sources(&config);
    upeg_sources::diagnostics::record_runtime_source_failures(&report);
    emit_runtime_source_report(&report, quiet);

    match run(cli) {
        Ok(stdout) => {
            // CLI binary; stdout IS the payload that pipes / shell
            // redirection consume.
            #[allow(clippy::print_stdout, reason = "CLI program's primary output channel")]
            {
                print!("{stdout}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            if let Some(stdout) = err.stdout() {
                #[allow(clippy::print_stdout, reason = "CLI program's primary output channel")]
                {
                    print!("{stdout}");
                }
            } else {
                eprintln!("{}", err.message());
            }
            ExitCode::from(err.exit_code())
        }
    }
}

fn apply_working_directory(path: Option<&std::path::Path>) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    std::env::set_current_dir(path).map_err(|error| {
        format!(
            "{WORKING_DIRECTORY_FLAG}: cannot use `{}`: {error}",
            path.display()
        )
    })
}

/// Install the chromiumoxide-backed Controlled Embed implementation
/// when the `controlled-embed` cargo feature is on. The default
/// `NoopControlledEmbedBackend` lives in `upeg-runtime` so this
/// function is a no-op when the feature is off; that keeps WASM
/// builds and `--no-default-features` callers from pulling in
/// chromiumoxide.
#[cfg(all(feature = "controlled-embed", not(target_arch = "wasm32")))]
fn install_controlled_embed_backend() {
    use std::sync::Arc;
    upeg_runtime::controlled_embed::set_controlled_embed_backend(Arc::new(
        upeg_runtime::controlled_embed_headless::HeadlessControlledEmbedBackend::new(),
    ));
}

#[cfg(not(all(feature = "controlled-embed", not(target_arch = "wasm32"))))]
fn install_controlled_embed_backend() {
    // NoopControlledEmbedBackend stays installed; ControlledEmbed
    // tools dispatch with a clear "feature is disabled" error.
}

fn emit_runtime_source_report(report: &RuntimeSourceReport, quiet: bool) {
    if quiet {
        return;
    }

    if let DirectoryStatus::Loaded { path, .. } = &report.toolkits {
        // Skips count toward "is there anything to report": a directory
        // whose only manifest declares one `pty = true` tool loads zero
        // tools and fails zero files, and staying silent about it is
        // exactly the hole `LoadOutcome::skipped` exists to close.
        let total = report.toolkits_outcome.loaded.len()
            + report.toolkits_outcome.failed.len()
            + report.toolkits_outcome.skipped.len();
        if total > 0 {
            eprintln!(
                "upeg: loaded {} tool(s) from {} ({} failed)",
                report.toolkits_outcome.loaded.len(),
                path.display(),
                report.toolkits_outcome.failed.len(),
            );
            for (path, err) in &report.toolkits_outcome.failed {
                eprintln!("upeg:   ✗ {}: {err}", path.display());
            }
            emit_skipped_tools(&report.toolkits_outcome.skipped);
        }
    }

    if let Some(project) = &report.project_manifest {
        if let Some(line) = project_manifest_summary_line(
            project.origin,
            &project.path,
            project.outcome.loaded.len(),
            project.outcome.failed.len(),
            project.outcome.skipped.len(),
        ) {
            eprintln!("{line}");
        }
        for (path, err) in &project.outcome.failed {
            eprintln!("upeg:   ✗ {}: {err}", path.display());
        }
        emit_skipped_tools(&project.outcome.skipped);
    }

    if let DirectoryStatus::Loaded { path, count } = &report.wasm {
        let total = *count + report.wasm_failures.len();
        if total > 0 {
            for (path, err) in &report.wasm_failures {
                eprintln!("upeg:   ✗ wasm {}: {err}", path.display());
            }
            eprintln!(
                "upeg: loaded {count} wasm tool(s) from {} ({} plugin(s) failed)",
                path.display(),
                report.wasm_failures.len(),
            );
        }
    }
}

/// One stderr line per tool this host dropped, printed under the source
/// that declared it — the skip sibling of the `✗` failure list.
///
/// `~` rather than `✗` because a skip is not an error: the manifest is
/// legal and every other tool in it loaded, this machine merely cannot
/// honour one declaration (see [`upeg_loader::SkippedTool`]). The same
/// marker already means the same thing in the MCP-import report
/// (`upeg-cli/src/infrastructure/mcp_imports.rs`).
fn emit_skipped_tools(skipped: &[upeg_loader::SkippedTool]) {
    for line in skipped_tool_lines(skipped) {
        eprintln!("{line}");
    }
}

/// The `~` lines themselves, built as values so they can be asserted on.
///
/// Kept separate from [`emit_skipped_tools`] because a skip only occurs
/// on a host that lacks the capability: no `pty = true` tool is ever
/// skipped on Linux, so a test that loaded a real manifest and looked
/// at stderr would pass while asserting nothing.
fn skipped_tool_lines(skipped: &[upeg_loader::SkippedTool]) -> Vec<String> {
    skipped
        .iter()
        .map(|tool| format!("upeg:   ~ {}: skipped ({})", tool.id, tool.reason))
        .collect()
}

/// The ONE stderr summary line for a loaded project manifest — or
/// `None` when there is nothing to say.
///
/// The B-4 consent notice ("which file is upeg trusting?") and the load
/// summary ("how did that file go?") are about the SAME file, so they
/// are folded into a single line instead of printing two near-identical
/// ones per invocation:
///
/// | origin | tools declared | line |
/// |---|---|---|
/// | `Detected` | some | `upeg: loaded project manifest <path> (N tool(s), M failed)` |
/// | `Detected` | none | `upeg: loaded project manifest <path>` |
/// | `EnvOverride` | some | `upeg: loaded N project tool(s) from <path> (M failed)` |
/// | `EnvOverride` | none | *(nothing)* |
///
/// "tools declared" counts host-skipped tools too, even though no
/// column reports them: a manifest whose single tool this build cannot
/// run has still declared one, and dropping to the "none" row would
/// swallow the header that [`emit_skipped_tools`]'s `~` lines hang
/// under. The skip itself is named there, not here — this line stays
/// one line.
///
/// The consent half is `Detected`-only on purpose: a path the user
/// named through `UPEG_PROJECT_MANIFEST_PATH` needs no consent notice,
/// and with zero tools there is nothing else to report either. `-q`
/// suppresses the whole report, this line included
/// (`upeg_sources::project` module docs, "consent notice").
fn project_manifest_summary_line(
    origin: upeg_sources::project::ProjectManifestOrigin,
    path: &std::path::Path,
    loaded: usize,
    failed: usize,
    skipped: usize,
) -> Option<String> {
    let path = path.display();
    let detected = origin == upeg_sources::project::ProjectManifestOrigin::Detected;
    match (detected, loaded + failed + skipped) {
        (true, 0) => Some(format!("upeg: loaded project manifest {path}")),
        (true, _) => Some(format!(
            "upeg: loaded project manifest {path} ({loaded} tool(s), {failed} failed)"
        )),
        (false, 0) => None,
        (false, _) => Some(format!(
            "upeg: loaded {loaded} project tool(s) from {path} ({failed} failed)"
        )),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_missing_working_directory_returns_an_error_before_loading() {
        let missing = std::env::temp_dir().join(format!(
            "upeg-missing-working-directory-{}",
            std::process::id()
        ));

        let error = super::apply_working_directory(Some(&missing))
            .expect_err("a missing working directory must be refused");

        assert!(error.contains("--working-directory"), "{error}");
        assert!(error.contains(&missing.display().to_string()), "{error}");
    }

    use super::{project_manifest_summary_line, skipped_tool_lines};
    use upeg_sources::project::ProjectManifestOrigin;

    fn manifest_path() -> std::path::PathBuf {
        std::path::PathBuf::from("/w/upeg.toml")
    }

    #[test]
    fn a_detected_manifest_folds_notice_and_tally_into_one_line() {
        let line = project_manifest_summary_line(
            ProjectManifestOrigin::Detected,
            &manifest_path(),
            3,
            1,
            0,
        )
        .expect("one line");
        assert_eq!(
            line,
            "upeg: loaded project manifest /w/upeg.toml (3 tool(s), 1 failed)"
        );
    }

    #[test]
    fn a_detected_manifest_with_no_tools_leaves_only_the_notice() {
        let line = project_manifest_summary_line(
            ProjectManifestOrigin::Detected,
            &manifest_path(),
            0,
            0,
            0,
        )
        .expect("one line");
        assert_eq!(line, "upeg: loaded project manifest /w/upeg.toml");
    }

    #[test]
    fn an_explicit_manifest_leaves_only_the_tally_without_the_consent_notice() {
        let line = project_manifest_summary_line(
            ProjectManifestOrigin::EnvOverride,
            &manifest_path(),
            2,
            0,
            0,
        )
        .expect("one line");
        assert_eq!(
            line,
            "upeg: loaded 2 project tool(s) from /w/upeg.toml (0 failed)"
        );
    }

    #[test]
    fn an_explicit_manifest_with_no_tools_leaves_no_line_at_all() {
        assert_eq!(
            project_manifest_summary_line(
                ProjectManifestOrigin::EnvOverride,
                &manifest_path(),
                0,
                0,
                0
            ),
            None
        );
    }

    #[test]
    fn a_detected_manifest_with_every_tool_skipped_still_leaves_the_tally_line() {
        // Even when every declared tool is skipped on this host, the
        // line must not drop to "no tools" — the following `~` lines
        // would lose the header they hang under.
        let line = project_manifest_summary_line(
            ProjectManifestOrigin::Detected,
            &manifest_path(),
            0,
            0,
            1,
        )
        .expect("one line");
        assert_eq!(
            line,
            "upeg: loaded project manifest /w/upeg.toml (0 tool(s), 0 failed)"
        );
    }

    #[test]
    fn a_skipped_tool_is_written_on_a_tilde_line_with_its_reason_not_as_a_failure() {
        let lines = skipped_tool_lines(&[upeg_loader::SkippedTool {
            id: "t.terminal".to_string(),
            reason: upeg_loader::LoadError::PtyUnsupportedOnHost,
        }]);

        let [line] = lines.as_slice() else {
            panic!("must be exactly one line: {lines:?}");
        };
        assert!(
            line.starts_with("upeg:   ~ t.terminal: skipped ("),
            "{line}"
        );
        assert!(
            !line.contains('✗'),
            "a skip is not a failure — it must not carry the failure marker: {line}"
        );
        assert!(
            line.contains(&upeg_loader::LoadError::PtyUnsupportedOnHost.to_string()),
            "a line without the reason leaves only a hole: {line}"
        );
    }

    #[test]
    fn no_skipped_tools_means_no_tilde_lines() {
        assert!(skipped_tool_lines(&[]).is_empty());
    }

    #[test]
    fn an_explicit_manifest_with_every_tool_skipped_still_leaves_the_tally_line() {
        let line = project_manifest_summary_line(
            ProjectManifestOrigin::EnvOverride,
            &manifest_path(),
            0,
            0,
            2,
        )
        .expect("one line");
        assert_eq!(
            line,
            "upeg: loaded 0 project tool(s) from /w/upeg.toml (0 failed)"
        );
    }
}
