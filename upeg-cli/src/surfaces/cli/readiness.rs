//! `upeg tool check` rendering for local and attached host inspections.

use std::fmt::Write as _;

use upeg_runtime::readiness::{ToolPlatform, ToolReadiness, ToolReadinessStatus};

use crate::CliError;

pub fn format_tool_readiness(readiness: &ToolReadiness, json: bool) -> Result<String, CliError> {
    if json {
        return pretty_json(readiness);
    }
    let mut output = format!(
        "status        {}\nplatform      {}\n",
        readiness_status_label(readiness.status),
        platform_label(readiness.platform),
    );
    for (value, label) in [
        (readiness.command.as_deref(), "command"),
        (
            readiness
                .working_directory
                .as_ref()
                .map(|path| path.to_string_lossy())
                .as_deref(),
            "directory",
        ),
        (
            readiness
                .executable
                .as_ref()
                .map(|path| path.to_string_lossy())
                .as_deref(),
            "executable",
        ),
    ] {
        if let Some(value) = value {
            let _ = writeln!(output, "{label:<13}{value}");
        }
    }
    let setup = readiness
        .setup
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| CliError::tool_failed(format!("readiness: {error}")))?;
    render_setup(&mut output, setup)?;
    Ok(output)
}

pub fn format_tool_readiness_json(
    readiness: &serde_json::Value,
    json: bool,
) -> Result<String, CliError> {
    if json {
        return pretty_json(readiness);
    }
    if readiness.is_null() {
        return Ok(
            "not applicable: process prerequisites are not declared for this tool\n".to_string(),
        );
    }
    let mut output = format!(
        "status        {}\nplatform      {}\n",
        readiness["status"].as_str().unwrap_or("unknown"),
        readiness["platform"].as_str().unwrap_or("unknown")
    );
    for (field, label) in [
        ("command", "command"),
        ("working_directory", "directory"),
        ("executable", "executable"),
    ] {
        if let Some(value) = readiness[field].as_str() {
            let _ = writeln!(output, "{label:<13}{value}");
        }
    }
    render_setup(&mut output, readiness.get("setup").cloned())?;
    Ok(output)
}

fn pretty_json(value: &impl serde::Serialize) -> Result<String, CliError> {
    let mut output = serde_json::to_string_pretty(value)
        .map_err(|error| CliError::tool_failed(format!("readiness: {error}")))?;
    output.push('\n');
    Ok(output)
}

fn render_setup(output: &mut String, setup: Option<serde_json::Value>) -> Result<(), CliError> {
    let Some(setup) = setup else {
        return Ok(());
    };
    for (field, label) in [("instructions", "instructions"), ("guide_url", "guide")] {
        if let Some(value) = setup[field].as_str() {
            let _ = writeln!(output, "{label:<13}{value}");
        }
    }
    for command in setup["install"]["commands"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        let _ = writeln!(output, "install      {command}");
    }
    Ok(())
}

const fn readiness_status_label(status: ToolReadinessStatus) -> &'static str {
    match status {
        ToolReadinessStatus::Ready => "ready",
        ToolReadinessStatus::MissingExecutable => "missing_executable",
        ToolReadinessStatus::MissingWorkingDirectory => "missing_working_directory",
        ToolReadinessStatus::UncheckedCredentialPath => "unchecked_credential_path",
    }
}
const fn platform_label(platform: ToolPlatform) -> &'static str {
    match platform {
        ToolPlatform::Linux => "linux",
        ToolPlatform::Macos => "macos",
        ToolPlatform::Windows => "windows",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_not_applicable_is_valid_null() {
        assert_eq!(
            format_tool_readiness_json(&serde_json::Value::Null, true).expect("format"),
            "null\n"
        );
    }
}
