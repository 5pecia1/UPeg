//! Typed inventory for cross-surface interface contracts.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::ALL_SURFACES;

mod surface_set;

pub use surface_set::SurfaceSet;

/// Current on-disk/interface inventory schema version.
///
/// v3 collapsed the per-surface entry explosion of v2: one contract is one
/// entry carrying the [`SurfaceSet`] it is advertised on.
pub const INTERFACE_INVENTORY_SCHEMA_VERSION: u32 = 3;

/// Root interface inventory document.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InterfaceInventory {
    /// JSON root field `schemaVersion`; currently only version 3 is valid.
    #[cfg_attr(feature = "serde", serde(rename = "schemaVersion"))]
    pub schema_version: u32,
    pub entries: Vec<InterfaceEntry>,
}

impl InterfaceInventory {
    /// Sort entries into a deterministic `(first surface, kind, id)` order.
    pub fn sort_entries(&mut self) {
        self.entries.sort();
    }

    /// Return entries sorted into deterministic `(first surface, kind, id)` order.
    #[must_use]
    pub fn sorted_entries(&self) -> Vec<InterfaceEntry> {
        let mut entries = self.entries.clone();
        entries.sort();
        entries
    }

    /// Validate schema version, required values, duplicate keys, and local paths.
    ///
    /// # Errors
    ///
    /// Returns [`InterfaceInventoryError`] when the document is not schema v3,
    /// an entry omits required data, an inventory key is duplicated, or a local
    /// path reference does not exist relative to the current working directory.
    pub fn validate(&self) -> Result<(), InterfaceInventoryError> {
        if self.schema_version != INTERFACE_INVENTORY_SCHEMA_VERSION {
            return Err(InterfaceInventoryError::UnsupportedSchemaVersion {
                expected: INTERFACE_INVENTORY_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }

        let mut seen = HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            entry.validate()?;

            let key = (entry.kind, entry.id.clone());
            if !seen.insert(key) {
                return Err(InterfaceInventoryError::DuplicateEntry {
                    kind: entry.kind,
                    id: entry.id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate base inventory rules plus complete coverage for every surface.
    ///
    /// # Errors
    ///
    /// Returns [`InterfaceInventoryError`] when base validation fails or any
    /// surface in [`ALL_SURFACES`] has no inventory entry.
    pub fn validate_surface_coverage(&self) -> Result<(), InterfaceInventoryError> {
        self.validate()?;

        for surface in ALL_SURFACES {
            if !self
                .entries
                .iter()
                .any(|entry| entry.surfaces.contains(surface))
            {
                return Err(InterfaceInventoryError::MissingSurfaceCoverage {
                    surface: surface.label().to_string(),
                });
            }
        }

        Ok(())
    }

    /// Deterministic JSON value with sorted entries and stable field names.
    #[must_use]
    pub fn to_deterministic_json(&self) -> serde_json::Value {
        let entries = self
            .sorted_entries()
            .iter()
            .map(InterfaceEntry::to_json)
            .collect::<Vec<_>>();

        serde_json::json!({
            "schemaVersion": self.schema_version,
            "entries": entries,
        })
    }
}

/// One exported interface contract and the surfaces it is advertised on.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InterfaceEntry {
    pub id: String,
    pub surfaces: SurfaceSet,
    pub kind: InterfaceKind,
    pub contract: ContractShape,
    pub version: String,
    pub compatibility: Compatibility,
    pub owner: OwnerRef,
    pub docs: DocRef,
    pub source: SourceRef,
    pub tests: TestMapping,
}

impl InterfaceEntry {
    fn sort_key(&self) -> (u8, u8, &str) {
        (self.surfaces.rank(), self.kind.rank(), self.id.as_str())
    }

    fn validate(&self) -> Result<(), InterfaceInventoryError> {
        validate_required("entry.id", &self.id)?;
        validate_surfaces(self.kind, &self.id, &self.surfaces)?;
        validate_required("entry.version", &self.version)?;
        self.contract.validate()?;
        self.owner.validate("entry.owner")?;
        self.docs.validate("entry.docs")?;
        self.source.validate("entry.source")?;
        self.tests.validate()?;

        Ok(())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "surfaces": self.surfaces.labels(),
            "kind": self.kind.label(),
            "contract": self.contract.to_json(),
            "version": self.version,
            "compatibility": self.compatibility.label(),
            "owner": self.owner.to_json(),
            "docs": self.docs.to_json(),
            "source": self.source.to_json(),
            "tests": self.tests.to_json(),
        })
    }
}

impl Ord for InterfaceEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for InterfaceEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Inventory entry kind, spanning product concepts and concrete surface shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum InterfaceKind {
    Tool,
    Toolkit,
    CliCommand,
    HttpRoute,
    McpMethod,
    McpTool,
    McpImport,
    TuiInteraction,
    DesktopComponent,
    PwaComponent,
    ChromeExtension,
    ManifestSchema,
}

impl InterfaceKind {
    const fn rank(self) -> u8 {
        match self {
            Self::Tool => 0,
            Self::Toolkit => 1,
            Self::CliCommand => 2,
            Self::HttpRoute => 3,
            Self::McpMethod => 4,
            Self::McpTool => 5,
            Self::McpImport => 6,
            Self::TuiInteraction => 7,
            Self::DesktopComponent => 8,
            Self::PwaComponent => 9,
            Self::ChromeExtension => 10,
            Self::ManifestSchema => 11,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Toolkit => "toolkit",
            Self::CliCommand => "cliCommand",
            Self::HttpRoute => "httpRoute",
            Self::McpMethod => "mcpMethod",
            Self::McpTool => "mcpTool",
            Self::McpImport => "mcpImport",
            Self::TuiInteraction => "tuiInteraction",
            Self::DesktopComponent => "desktopComponent",
            Self::PwaComponent => "pwaComponent",
            Self::ChromeExtension => "chromeExtension",
            Self::ManifestSchema => "manifestSchema",
        }
    }
}

/// Structured contract metadata for one side of an interface boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ContractIo {
    pub kind: ContractIoKind,
    pub schema: Option<serde_json::Value>,
    #[cfg_attr(feature = "serde", serde(rename = "schemaRef"))]
    pub schema_ref: Option<String>,
    pub description: Option<String>,
    pub declared: bool,
}

impl ContractIo {
    #[must_use]
    pub fn declared(
        kind: ContractIoKind,
        schema: Option<serde_json::Value>,
        schema_ref: Option<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            schema,
            schema_ref,
            description: Some(description.into()),
            declared: true,
        }
    }

    #[must_use]
    pub fn none(description: impl Into<String>) -> Self {
        Self {
            kind: ContractIoKind::None,
            schema: None,
            schema_ref: None,
            description: Some(description.into()),
            declared: true,
        }
    }

    #[must_use]
    pub fn not_declared(description: impl Into<String>) -> Self {
        Self {
            kind: ContractIoKind::NotDeclared,
            schema: None,
            schema_ref: None,
            description: Some(description.into()),
            declared: false,
        }
    }

    fn validate(&self, field: &'static str) -> Result<(), InterfaceInventoryError> {
        if let Some(schema_ref) = &self.schema_ref {
            validate_required(field, schema_ref)?;
        }
        if let Some(description) = &self.description {
            validate_required(field, description)?;
        }
        if self.kind == ContractIoKind::NotDeclared && self.declared {
            return Err(InterfaceInventoryError::MissingRequiredField {
                field: format!("{field}.declared"),
            });
        }
        if self.kind != ContractIoKind::NotDeclared && !self.declared {
            return Err(InterfaceInventoryError::MissingRequiredField {
                field: format!("{field}.kind"),
            });
        }
        Ok(())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": self.kind.label(),
            "schema": self.schema,
            "schemaRef": self.schema_ref,
            "description": self.description,
            "declared": self.declared,
        })
    }
}

/// Contract I/O kind labels serialized in schema v3 inventories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ContractIoKind {
    JsonSchema,
    CommandArgs,
    JsonRpcParams,
    HttpRequest,
    HttpResponse,
    UiInteraction,
    None,
    NotDeclared,
}

impl ContractIoKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::JsonSchema => "jsonSchema",
            Self::CommandArgs => "commandArgs",
            Self::JsonRpcParams => "jsonRpcParams",
            Self::HttpRequest => "httpRequest",
            Self::HttpResponse => "httpResponse",
            Self::UiInteraction => "uiInteraction",
            Self::None => "none",
            Self::NotDeclared => "notDeclared",
        }
    }
}

/// Structured address fields for locating an interface on a concrete surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ContractLocator {
    #[cfg_attr(feature = "serde", serde(rename = "commandPath"))]
    pub command_path: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "httpMethod"))]
    pub http_method: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "httpPath"))]
    pub http_path: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "jsonrpcMethod"))]
    pub jsonrpc_method: Option<String>,
}

impl ContractLocator {
    #[must_use]
    pub fn command_path(path: impl Into<String>) -> Self {
        Self {
            command_path: Some(path.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn http(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            http_method: Some(method.into()),
            http_path: Some(path.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn jsonrpc(method: impl Into<String>) -> Self {
        Self {
            jsonrpc_method: Some(method.into()),
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<(), InterfaceInventoryError> {
        for (field, value) in [
            ("entry.contract.locator.commandPath", &self.command_path),
            ("entry.contract.locator.httpMethod", &self.http_method),
            ("entry.contract.locator.httpPath", &self.http_path),
            ("entry.contract.locator.jsonrpcMethod", &self.jsonrpc_method),
        ] {
            if let Some(value) = value {
                validate_required(field, value)?;
            }
        }
        Ok(())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "commandPath": self.command_path,
            "httpMethod": self.http_method,
            "httpPath": self.http_path,
            "jsonrpcMethod": self.jsonrpc_method,
        })
    }
}

/// Schema v3 contract shape for interface-specific locators and I/O contracts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ContractShape {
    pub locator: ContractLocator,
    pub input: Option<ContractIo>,
    pub output: Option<ContractIo>,
    pub errors: Vec<ContractIo>,
}

impl ContractShape {
    #[must_use]
    pub fn new(locator: ContractLocator, input: ContractIo, output: ContractIo) -> Self {
        Self {
            locator,
            input: Some(input),
            output: Some(output),
            errors: Vec::new(),
        }
    }

    fn validate(&self) -> Result<(), InterfaceInventoryError> {
        self.locator.validate()?;
        let input =
            self.input
                .as_ref()
                .ok_or_else(|| InterfaceInventoryError::MissingRequiredField {
                    field: "entry.contract.input".to_string(),
                })?;
        input.validate("entry.contract.input")?;
        let output =
            self.output
                .as_ref()
                .ok_or_else(|| InterfaceInventoryError::MissingRequiredField {
                    field: "entry.contract.output".to_string(),
                })?;
        output.validate("entry.contract.output")?;
        for error in &self.errors {
            error.validate("entry.contract.errors")?;
        }
        Ok(())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "locator": self.locator.to_json(),
            "input": self.input.as_ref().map(ContractIo::to_json),
            "output": self.output.as_ref().map(ContractIo::to_json),
            "errors": self.errors.iter().map(ContractIo::to_json).collect::<Vec<_>>(),
        })
    }
}

/// Compatibility promise for an interface entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Compatibility {
    Stable,
    Experimental,
    Deprecated,
}

impl Compatibility {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Experimental => "experimental",
            Self::Deprecated => "deprecated",
        }
    }
}

/// Owning team/component reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OwnerRef {
    pub path: Option<String>,
    pub url: Option<String>,
}

impl OwnerRef {
    fn validate(&self, field: &'static str) -> Result<(), InterfaceInventoryError> {
        validate_ref(field, self.path.as_deref(), self.url.as_deref())
    }

    fn to_json(&self) -> serde_json::Value {
        ref_to_json(self.path.as_deref(), self.url.as_deref())
    }
}

/// Documentation reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DocRef {
    pub path: Option<String>,
    pub url: Option<String>,
}

impl DocRef {
    fn validate(&self, field: &'static str) -> Result<(), InterfaceInventoryError> {
        validate_ref(field, self.path.as_deref(), self.url.as_deref())
    }

    fn to_json(&self) -> serde_json::Value {
        ref_to_json(self.path.as_deref(), self.url.as_deref())
    }
}

/// Source implementation reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SourceRef {
    pub path: Option<String>,
    pub url: Option<String>,
}

impl SourceRef {
    fn validate(&self, field: &'static str) -> Result<(), InterfaceInventoryError> {
        validate_ref(field, self.path.as_deref(), self.url.as_deref())
    }

    fn to_json(&self) -> serde_json::Value {
        ref_to_json(self.path.as_deref(), self.url.as_deref())
    }
}

/// JSON value of [`TestMapping::Uncovered`]'s `path`/`testName` keys.
/// Spelled once so "there is no test" reads the same in the document as
/// it does in the type.
const UNCOVERED_TEST_JSON: Option<&str> = None;

/// How an interface entry is covered by tests.
///
/// [`Self::Uncovered`] is a first-class answer, not an absence: an entry
/// with no test yet says so *with its reason*, instead of aiming its
/// `tests` pointer at some unrelated-but-existing file so the row looks
/// populated. That dishonest shape is exactly what the
/// `upeg-cli/src/inventory/tests.rs` honesty check rejects, and a
/// declaration site cannot satisfy the check by lying once the gap is
/// expressible.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TestMapping {
    /// A real test file — optionally a specific test inside it — covers
    /// this entry.
    Covered {
        path: String,
        test_name: Option<String>,
    },
    /// Nothing covers this entry yet. `reason` is what a reader needs to
    /// judge the gap, and it travels into the generated inventory.
    Uncovered { reason: String },
}

impl TestMapping {
    /// Cover this entry with `path` (repo-relative) and an optional test
    /// name inside it.
    #[must_use]
    pub fn covered(path: impl Into<String>, test_name: Option<String>) -> Self {
        Self::Covered {
            path: path.into(),
            test_name,
        }
    }

    /// Declare this entry uncovered, recording why.
    #[must_use]
    pub fn uncovered(reason: impl Into<String>) -> Self {
        Self::Uncovered {
            reason: reason.into(),
        }
    }

    /// Repo-relative path of the covering test file, or `None` when the
    /// entry is [`Self::Uncovered`].
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Covered { path, .. } => Some(path.as_str()),
            Self::Uncovered { .. } => None,
        }
    }

    /// Name of the covering test, when one was named.
    #[must_use]
    pub fn test_name(&self) -> Option<&str> {
        match self {
            Self::Covered { test_name, .. } => test_name.as_deref(),
            Self::Uncovered { .. } => None,
        }
    }

    /// Why this entry has no test, or `None` when it has one.
    #[must_use]
    pub fn uncovered_reason(&self) -> Option<&str> {
        match self {
            Self::Covered { .. } => None,
            Self::Uncovered { reason } => Some(reason.as_str()),
        }
    }

    fn validate(&self) -> Result<(), InterfaceInventoryError> {
        match self {
            Self::Covered { path, test_name } => {
                validate_required("entry.tests.path", path)?;
                validate_existing_path("entry.tests.path", path)?;
                if let Some(test_name) = test_name {
                    validate_required("entry.tests.test_name", test_name)?;
                }
            }
            Self::Uncovered { reason } => validate_required("entry.tests.uncovered", reason)?,
        }

        Ok(())
    }

    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Covered { path, test_name } => serde_json::json!({
                "path": path,
                "testName": test_name,
            }),
            Self::Uncovered { reason } => serde_json::json!({
                "path": UNCOVERED_TEST_JSON,
                "testName": UNCOVERED_TEST_JSON,
                "uncovered": reason,
            }),
        }
    }
}

/// Validation errors for interface inventories.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InterfaceInventoryError {
    #[error("unsupported interface inventory schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    #[error("missing required interface inventory field {field}")]
    MissingRequiredField { field: String },
    #[error("duplicate interface inventory entry {kind:?}/{id}")]
    DuplicateEntry { kind: InterfaceKind, id: String },
    #[error("interface inventory entry {kind:?}/{id} advertises no surface")]
    EmptySurfaces { kind: InterfaceKind, id: String },
    #[error("missing interface inventory entries for surface {surface}")]
    MissingSurfaceCoverage { surface: String },
    #[error("local interface inventory path does not exist for {field}: {path}")]
    MissingLocalPath { field: String, path: String },
}

fn validate_surfaces(
    kind: InterfaceKind,
    id: &str,
    surfaces: &SurfaceSet,
) -> Result<(), InterfaceInventoryError> {
    if surfaces.is_empty() {
        return Err(InterfaceInventoryError::EmptySurfaces {
            kind,
            id: id.to_string(),
        });
    }

    Ok(())
}

fn validate_required(field: &'static str, value: &str) -> Result<(), InterfaceInventoryError> {
    if value.trim().is_empty() || value != value.trim() {
        return Err(InterfaceInventoryError::MissingRequiredField {
            field: field.to_string(),
        });
    }

    Ok(())
}

fn validate_ref(
    field: &'static str,
    path: Option<&str>,
    url: Option<&str>,
) -> Result<(), InterfaceInventoryError> {
    match (path, url) {
        (None, None) => Err(InterfaceInventoryError::MissingRequiredField {
            field: field.to_string(),
        }),
        (path, url) => {
            if let Some(path) = path {
                validate_required(field, path)?;
                validate_existing_path(field, path)?;
            }
            if let Some(url) = url {
                validate_required(field, url)?;
            }
            Ok(())
        }
    }
}

fn validate_existing_path(field: &'static str, path: &str) -> Result<(), InterfaceInventoryError> {
    if !local_inventory_path(path).exists() {
        return Err(InterfaceInventoryError::MissingLocalPath {
            field: field.to_string(),
            path: path.to_string(),
        });
    }

    Ok(())
}

fn local_inventory_path(path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }

    repo_root().join(path)
}

fn repo_root() -> &'static Path {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir)
}

fn ref_to_json(path: Option<&str>, url: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "path": path,
        "url": url,
    })
}

#[cfg(test)]
#[path = "interface_inventory/tests.rs"]
mod tests;
