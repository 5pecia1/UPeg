//! Display-only External setup guidance parsed from a manifest.

use schemars::JsonSchema;
use serde::Deserialize;

/// Optional, display-only setup guidance for an External tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 2)
)]
pub struct ToolSetupToml {
    /// HTTP(S) guide for obtaining or configuring the required command.
    #[serde(default)]
    pub guide_url: Option<String>,
    /// Plain-language operator guidance. UPeg never executes this text.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Display-only commands, selected by the host operating system.
    #[serde(default)]
    pub install: Option<ToolSetupInstallToml>,
}

/// Per-operating-system display-only install commands.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    deny_unknown_fields,
    extend("x-doc-reference" = true, "x-doc-order" = 2)
)]
pub struct ToolSetupInstallToml {
    #[serde(default)]
    pub linux: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
    #[serde(default)]
    pub windows: Vec<String>,
}
