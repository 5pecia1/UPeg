//! `upeg plugin` command handlers: scaffold (`new`), validate-then-copy
//! (`install`), and enumerate (`list`) WASM guest plugins.
//!
//! `new` is pure scaffolding — it never touches `upeg-wasm` and stays
//! compiled regardless of the `wasm-plugin` cargo feature. `install`/
//! `list` load plugin manifests through `upeg-wasm` and are gated the
//! same way `upeg wasm load` is (see `run_wasm_action` above in this
//! crate).

use std::path::Path;
#[cfg(feature = "wasm-plugin")]
use std::path::PathBuf;

use crate::domain::plugin::{name::validate_plugin_name, scaffold};
use crate::error::CliError;
use crate::surfaces::cli::PluginAction;

/// `.wasm` file extension, factored out so the two places that filter
/// directory entries by it can't drift.
#[cfg(feature = "wasm-plugin")]
const WASM_EXTENSION: &str = "wasm";

pub(super) fn run_plugin_command(action: PluginAction) -> Result<String, CliError> {
    match action {
        PluginAction::New { name, dir, local } => {
            run_plugin_new(&name, dir.as_deref(), local.as_deref())
        }
        #[cfg(feature = "wasm-plugin")]
        PluginAction::Install { path, force } => run_plugin_install(&path, force),
        #[cfg(feature = "wasm-plugin")]
        PluginAction::List => run_plugin_list(),
    }
}

// ─── new ─────────────────────────────────────────────────────────────

fn run_plugin_new(
    name: &str,
    dir: Option<&Path>,
    local: Option<&Path>,
) -> Result<String, CliError> {
    validate_plugin_name(name).map_err(|e| CliError::tool_failed(e.to_string()))?;

    let parent = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| CliError::tool_failed(format!("plugin new: current directory: {e}")))?,
    };
    let target = parent.join(name);
    refuse_nonempty_dir(&target)?;

    let files = scaffold::render(name, local);
    let src_dir = target.join("src");
    std::fs::create_dir_all(&src_dir)
        .map_err(|e| CliError::tool_failed(format!("plugin new: `{}`: {e}", src_dir.display())))?;
    write_scaffold_file(&target.join("Cargo.toml"), &files.cargo_toml)?;
    write_scaffold_file(&src_dir.join("lib.rs"), &files.lib_rs)?;

    Ok(next_steps_message(name, &target))
}

fn write_scaffold_file(path: &Path, contents: &str) -> Result<(), CliError> {
    std::fs::write(path, contents)
        .map_err(|e| CliError::tool_failed(format!("plugin new: `{}`: {e}", path.display())))
}

/// Refuse to scaffold into an existing non-empty directory. A missing or
/// empty directory is fine — `new` creates/fills it.
fn refuse_nonempty_dir(target: &Path) -> Result<(), CliError> {
    match std::fs::read_dir(target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(CliError::tool_failed(format!(
                    "plugin new: `{}` already exists and is not empty",
                    target.display()
                )));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CliError::tool_failed(format!(
            "plugin new: `{}`: {e}",
            target.display()
        ))),
    }
}

fn next_steps_message(name: &str, target: &Path) -> String {
    format!(
        "scaffolded `{name}` at {}\n\nnext steps:\n  cd {}\n  cargo build --target wasm32-unknown-unknown --release\n  upeg plugin install target/wasm32-unknown-unknown/release/{name}.wasm\n",
        target.display(),
        target.display(),
    )
}

// ─── install / list (wasm-plugin only) ────────────────────────────────

#[cfg(feature = "wasm-plugin")]
fn run_plugin_install(path: &Path, force: bool) -> Result<String, CliError> {
    let wasm_path = resolve_wasm_artifact(path)?;
    let bytes = std::fs::read(&wasm_path).map_err(|e| {
        CliError::tool_failed(format!("plugin install: `{}`: {e}", wasm_path.display()))
    })?;

    // Validate BEFORE installing: `inspect_bytes` runs the same manifest
    // parse + per-decl checks `upeg_wasm::register_from_bytes` does, but
    // performs no registration — a bad plugin never touches the toolbox
    // or the filesystem.
    let inspection = upeg_wasm::inspect_bytes(&bytes).map_err(|e| {
        CliError::tool_failed(format!(
            "plugin install: `{}` failed validation: {e}",
            wasm_path.display()
        ))
    })?;

    let dest_dir = crate::infrastructure::paths::wasm_dir().ok_or_else(|| {
        CliError::tool_failed(
            "plugin install: could not resolve the wasm plugin directory (no HOME-class env)"
                .to_string(),
        )
    })?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| {
        CliError::tool_failed(format!("plugin install: `{}`: {e}", dest_dir.display()))
    })?;

    let file_stem = wasm_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            CliError::tool_failed(format!(
                "plugin install: `{}` has no usable file name",
                wasm_path.display()
            ))
        })?;
    let dest = dest_dir.join(format!("{file_stem}.{WASM_EXTENSION}"));

    if let Some(existing) = std::fs::read(&dest).ok()
        && existing != bytes
        && !force
    {
        return Err(CliError::tool_failed(format!(
            "plugin install: `{}` already exists with different content; pass --force to overwrite",
            dest.display()
        )));
    }
    std::fs::write(&dest, &bytes)
        .map_err(|e| CliError::tool_failed(format!("plugin install: `{}`: {e}", dest.display())))?;

    Ok(format_tool_ids(&inspection.tool_ids))
}

#[cfg(feature = "wasm-plugin")]
fn resolve_wasm_artifact(path: &Path) -> Result<PathBuf, CliError> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| CliError::tool_failed(format!("plugin install: `{}`: {e}", path.display())))?;
    if metadata.is_file() {
        return Ok(path.to_path_buf());
    }

    let release_dir = path
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release");
    let mut candidates = wasm_files_in(&release_dir).map_err(|e| {
        CliError::tool_failed(format!(
            "plugin install: no built `.wasm` artifact under `{}` ({e}); run \
             `cargo build --target wasm32-unknown-unknown --release` first",
            release_dir.display()
        ))
    })?;
    candidates.sort();

    match candidates.len() {
        0 => Err(CliError::tool_failed(format!(
            "plugin install: no `.wasm` file found under `{}`; run \
             `cargo build --target wasm32-unknown-unknown --release` first",
            release_dir.display()
        ))),
        1 => Ok(candidates.remove(0)),
        _ => {
            let list = candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(CliError::tool_failed(format!(
                "plugin install: multiple `.wasm` artifacts under `{}`; pass the exact file: {list}",
                release_dir.display()
            )))
        }
    }
}

#[cfg(feature = "wasm-plugin")]
fn wasm_files_in(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(WASM_EXTENSION))
        .collect())
}

#[cfg(feature = "wasm-plugin")]
fn run_plugin_list() -> Result<String, CliError> {
    let Some(dir) = crate::infrastructure::paths::wasm_dir() else {
        return Ok(empty_plugin_list_message(None));
    };
    let mut wasm_files = match wasm_files_in(&dir) {
        Ok(files) => files,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(empty_plugin_list_message(Some(&dir)));
        }
        Err(e) => {
            return Err(CliError::tool_failed(format!(
                "plugin list: `{}`: {e}",
                dir.display()
            )));
        }
    };
    if wasm_files.is_empty() {
        return Ok(empty_plugin_list_message(Some(&dir)));
    }
    wasm_files.sort();

    let mut out = String::new();
    for file in wasm_files {
        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let row = match std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| upeg_wasm::inspect_bytes(&bytes).map_err(|e| e.to_string()))
        {
            Ok(inspection) => format!(
                "{file_name}\t{}\t{}",
                inspection.toolkit,
                inspection.tool_ids.join(",")
            ),
            Err(e) => format!("{file_name}\terror\t{e}"),
        };
        out.push_str(&row);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(feature = "wasm-plugin")]
fn empty_plugin_list_message(dir: Option<&Path>) -> String {
    match dir {
        Some(dir) => format!("no plugins installed (looked in {})\n", dir.display()),
        None => "no plugins installed (no ~/.upeg config root available)\n".to_string(),
    }
}

#[cfg(feature = "wasm-plugin")]
fn format_tool_ids(ids: &[String]) -> String {
    let mut out = String::new();
    for id in ids {
        out.push_str(id);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonempty_directory_is_rejected() {
        let tmp = std::env::temp_dir().join(format!(
            "upeg_plugin_new_nonempty_{}_{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        std::fs::write(tmp.join("keep.txt"), b"already here").expect("seed file");

        let result = refuse_nonempty_dir(&tmp);

        let _ = std::fs::remove_dir_all(&tmp);
        assert!(result.is_err());
    }

    #[test]
    fn a_missing_or_empty_directory_is_allowed() {
        let tmp = std::env::temp_dir().join(format!(
            "upeg_plugin_new_missing_{}_{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&tmp);

        assert!(refuse_nonempty_dir(&tmp).is_ok());

        std::fs::create_dir_all(&tmp).expect("create empty tmp dir");
        assert!(refuse_nonempty_dir(&tmp).is_ok());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn a_single_wasm_file_picks_its_path() {
        let tmp = std::env::temp_dir().join(format!(
            "upeg_plugin_resolve_one_{}_{}",
            std::process::id(),
            line!()
        ));
        let release_dir = tmp.join("target/wasm32-unknown-unknown/release");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&release_dir).expect("create release dir");
        std::fs::write(release_dir.join("only.wasm"), b"fake").expect("write fake wasm");

        let resolved = resolve_wasm_artifact(&tmp);

        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(
            resolved.expect("single candidate resolves"),
            release_dir.join("only.wasm")
        );
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn multiple_wasm_files_return_an_ambiguity_error() {
        let tmp = std::env::temp_dir().join(format!(
            "upeg_plugin_resolve_many_{}_{}",
            std::process::id(),
            line!()
        ));
        let release_dir = tmp.join("target/wasm32-unknown-unknown/release");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&release_dir).expect("create release dir");
        std::fs::write(release_dir.join("a.wasm"), b"fake").expect("write fake wasm a");
        std::fs::write(release_dir.join("b.wasm"), b"fake").expect("write fake wasm b");

        let resolved = resolve_wasm_artifact(&tmp);

        let _ = std::fs::remove_dir_all(&tmp);
        assert!(resolved.is_err());
    }

    #[cfg(feature = "wasm-plugin")]
    #[test]
    fn no_built_artifact_returns_a_guidance_message() {
        let tmp = std::env::temp_dir().join(format!(
            "upeg_plugin_resolve_none_{}_{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create crate dir without a build");

        let resolved = resolve_wasm_artifact(&tmp);

        let _ = std::fs::remove_dir_all(&tmp);
        let err = resolved.expect_err("no build artifact should fail");
        assert!(
            err.message()
                .contains("cargo build --target wasm32-unknown-unknown")
        );
    }
}
