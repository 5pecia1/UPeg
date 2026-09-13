use std::fs;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::error::CliError;

#[derive(Subcommand, Debug)]
pub enum ToolkitSchemaCommand {
    /// Write the current deterministic Toolkit manifest schema and docs.
    Generate {
        /// Destination for deterministic JSON Schema.
        #[arg(long)]
        json: PathBuf,
        /// Destination for generated Markdown documentation.
        #[arg(long)]
        markdown: PathBuf,
    },
    /// Compare generated Toolkit manifest schema/docs against committed baselines.
    Check {
        /// Baseline deterministic JSON Schema to compare against.
        #[arg(long = "baseline-json")]
        baseline_json: PathBuf,
        /// Baseline generated Markdown documentation to compare against.
        #[arg(long = "baseline-markdown")]
        baseline_markdown: PathBuf,
        /// Destination for current deterministic JSON Schema.
        #[arg(long = "current-json")]
        current_json: PathBuf,
        /// Destination for current generated Markdown documentation.
        #[arg(long = "current-markdown")]
        current_markdown: PathBuf,
        /// Destination for the schema unified diff artifact.
        #[arg(long = "schema-diff-output")]
        schema_diff_output: PathBuf,
        /// Destination for the docs unified diff artifact.
        #[arg(long = "docs-diff-output")]
        docs_diff_output: PathBuf,
    },
}

struct ToolkitSchemaArtifacts {
    schema_json: String,
    markdown_docs: String,
}

pub(crate) fn run_toolkit_schema_command(action: ToolkitSchemaCommand) -> Result<String, CliError> {
    match action {
        ToolkitSchemaCommand::Generate { json, markdown } => {
            let artifacts = generate_toolkit_schema_artifacts()?;
            write_toolkit_schema_artifacts(&artifacts, &json, &markdown)?;
            Ok(format!(
                "wrote toolkit schema artifacts to {} and {}\n",
                json.display(),
                markdown.display()
            ))
        }
        ToolkitSchemaCommand::Check {
            baseline_json,
            baseline_markdown,
            current_json,
            current_markdown,
            schema_diff_output,
            docs_diff_output,
        } => check_toolkit_schema_artifacts(
            &baseline_json,
            &baseline_markdown,
            &current_json,
            &current_markdown,
            &schema_diff_output,
            &docs_diff_output,
        ),
    }
}

fn generate_toolkit_schema_artifacts() -> Result<ToolkitSchemaArtifacts, CliError> {
    let schema_json = upeg_loader::toolkit_schema_json().map_err(|err| {
        CliError::tool_failed(format!("failed to render toolkit JSON Schema: {err}"))
    })?;
    let markdown_docs = upeg_loader::toolkit_manifest_docs_markdown().map_err(|err| {
        CliError::tool_failed(format!("failed to render toolkit manifest docs: {err}"))
    })?;

    Ok(ToolkitSchemaArtifacts {
        schema_json,
        markdown_docs,
    })
}

fn write_toolkit_schema_artifacts(
    artifacts: &ToolkitSchemaArtifacts,
    json_path: &Path,
    markdown_path: &Path,
) -> Result<(), CliError> {
    write_text(json_path, &artifacts.schema_json)?;
    write_text(markdown_path, &artifacts.markdown_docs)
}

fn check_toolkit_schema_artifacts(
    baseline_json_path: &Path,
    baseline_markdown_path: &Path,
    current_json_path: &Path,
    current_markdown_path: &Path,
    schema_diff_output_path: &Path,
    docs_diff_output_path: &Path,
) -> Result<String, CliError> {
    let baseline_json = read_text(baseline_json_path)?;
    let baseline_markdown = read_text(baseline_markdown_path)?;
    let artifacts = generate_toolkit_schema_artifacts()?;

    write_toolkit_schema_artifacts(&artifacts, current_json_path, current_markdown_path)?;

    let schema_diff = unified_diff(
        &baseline_json_path.display().to_string(),
        &current_json_path.display().to_string(),
        &baseline_json,
        &artifacts.schema_json,
    )?;
    let docs_diff = unified_diff(
        &baseline_markdown_path.display().to_string(),
        &current_markdown_path.display().to_string(),
        &baseline_markdown,
        &artifacts.markdown_docs,
    )?;

    write_text(schema_diff_output_path, &schema_diff)?;
    write_text(docs_diff_output_path, &docs_diff)?;

    let schema_changed = !schema_diff.is_empty();
    let docs_changed = !docs_diff.is_empty();
    if schema_changed || docs_changed {
        return Err(CliError::tool_failed(format!(
            "toolkit schema/docs drift detected: schema={}, docs={}",
            drift_label(schema_changed),
            drift_label(docs_changed)
        )));
    }

    Ok("no toolkit schema/docs drift\n".to_string())
}

fn read_text(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path)
        .map_err(|err| CliError::tool_failed(format!("failed to read {}: {err}", path.display())))
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

fn unified_diff(
    baseline_label: &str,
    current_label: &str,
    baseline: &str,
    current: &str,
) -> Result<String, CliError> {
    if baseline == current {
        return Ok(String::new());
    }

    let baseline_line_count = diff_line_count(baseline);
    let current_line_count = diff_line_count(current);
    let mut diff = String::new();
    push_diff_header(
        &mut diff,
        baseline_label,
        current_label,
        baseline_line_count,
        current_line_count,
    )?;

    for line in baseline.lines() {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in current.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }

    Ok(diff)
}

fn push_diff_header(
    diff: &mut String,
    baseline_label: &str,
    current_label: &str,
    baseline_line_count: usize,
    current_line_count: usize,
) -> Result<(), CliError> {
    use std::fmt::Write as _;

    writeln!(diff, "--- {baseline_label}")
        .map_err(|err| CliError::tool_failed(format!("failed to format schema diff: {err}")))?;
    writeln!(diff, "+++ {current_label}")
        .map_err(|err| CliError::tool_failed(format!("failed to format schema diff: {err}")))?;
    writeln!(
        diff,
        "@@ -{},{} +{},{} @@",
        diff_start_line(baseline_line_count),
        baseline_line_count,
        diff_start_line(current_line_count),
        current_line_count
    )
    .map_err(|err| CliError::tool_failed(format!("failed to format schema diff: {err}")))
}

fn diff_line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

fn diff_start_line(line_count: usize) -> usize {
    usize::from(line_count != 0)
}

fn drift_label(changed: bool) -> &'static str {
    if changed { "changed" } else { "unchanged" }
}
