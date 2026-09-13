//! Shared WASM plugin manifest contract for upeg.
//!
//! This crate is intentionally small and has no host-runtime dependencies.
//! Guest plugins can depend on it to build their `manifest` export from Rust
//! types, while `upeg-wasm` depends on the same types when deserializing the
//! JSON returned by that export.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::get_unwrap,
        clippy::tests_outside_test_module,
        clippy::print_stdout,
        clippy::unreachable,
        clippy::string_add,
        clippy::manual_let_else,
        reason = "tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
    )
)]

use serde::{Deserialize, Serialize};

mod file_input_policy;
mod guest;

pub use file_input_policy::{DEFAULT_PLUGIN_FILE_MAX_COUNT, PluginFileInputPolicy};
pub use guest::{FromPluginArg, NO_VALUE_ERROR, ToPluginOutput};

macro_rules! string_newtype {
    ($(#[$meta:meta])* pub struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Borrow the underlying manifest string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the typed value and return the underlying string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

string_newtype! {
    /// Toolkit identifier in a plugin manifest.
    pub struct ToolkitId;
}

string_newtype! {
    /// Full Tool identifier in a plugin manifest.
    pub struct ToolId;
}

string_newtype! {
    /// Wasm export name in a plugin manifest.
    pub struct ExportName;
}

/// Typed plugin input specification carried in a Tool declaration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInputSpec {
    /// Ordered input fields displayed and validated by host surfaces.
    pub fields: Vec<PluginInputField>,
}

impl PluginInputSpec {
    /// Create an input spec from ordered fields.
    #[must_use]
    pub fn new(fields: impl IntoIterator<Item = PluginInputField>) -> Self {
        Self {
            fields: fields.into_iter().collect(),
        }
    }

    /// Create an empty no-input specification.
    #[must_use]
    pub fn empty() -> Self {
        Self { fields: Vec::new() }
    }
}

/// One typed input field in a plugin manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInputField {
    /// Canonical argument key passed to the plugin export.
    pub name: String,
    /// Optional short presentation label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional field help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether callers must provide a value.
    pub required: bool,
    /// Closed input kind vocabulary.
    pub kind: PluginInputKind,
    /// Raw File-input policy. Omission means Core's default policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_policy: Option<PluginFileInputPolicy>,
}

impl PluginInputField {
    /// Create an input field.
    #[must_use]
    pub fn new(name: impl Into<String>, required: bool, kind: PluginInputKind) -> Self {
        Self {
            name: name.into(),
            label: None,
            description: None,
            required,
            kind,
            file_policy: None,
        }
    }

    /// Create a required input field.
    #[must_use]
    pub fn required(name: impl Into<String>, kind: PluginInputKind) -> Self {
        Self::new(name, true, kind)
    }

    /// Create an optional input field.
    #[must_use]
    pub fn optional(name: impl Into<String>, kind: PluginInputKind) -> Self {
        Self::new(name, false, kind)
    }

    /// Add a short presentation label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add field help text.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a raw File-input policy for host-side validation and lowering.
    #[must_use]
    pub fn with_file_policy(mut self, file_policy: PluginFileInputPolicy) -> Self {
        self.file_policy = Some(file_policy);
        self
    }
}

/// Closed plugin input kind vocabulary mirroring `upeg_core::InputKind`.
///
/// Wire "type" tag names must stay identical to `upeg_core::InputKind::label()`
/// (and, transitively, the Toolkit TOML `type = "…"` strings the loader
/// accepts) — the three definition grammars converge on one closed
/// vocabulary. `DateTime` carries an explicit `rename` because
/// `#[serde(rename_all = "snake_case")]` would otherwise produce
/// `date_time`, one word short of the `datetime` every other grammar uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "options")]
pub enum PluginInputKind {
    String,
    Number,
    Integer,
    Boolean,
    Options(Vec<PluginChoiceOption>),
    MultiOptions(Vec<PluginChoiceOption>),
    Markdown,
    Json,
    #[serde(rename = "datetime")]
    DateTime,
    FilePath,
    Url,
    /// File or directory body, mirroring `upeg_core::InputKind::File`.
    File,
}

/// Typed plugin output specification carried in a Tool declaration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginOutputSpec {
    /// Ordered output fields rendered by host surfaces.
    pub fields: Vec<PluginOutputField>,
    /// Output field id rendered as the primary result when fields are present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_output_id: Option<String>,
}

impl PluginOutputSpec {
    /// Create an output spec from ordered fields.
    #[must_use]
    pub fn new(fields: impl IntoIterator<Item = PluginOutputField>) -> Self {
        let fields = fields.into_iter().collect::<Vec<_>>();
        let primary_output_id = fields.first().map(|field| field.name.clone());
        Self {
            fields,
            primary_output_id,
        }
    }

    /// Create an empty no-output specification.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            fields: Vec::new(),
            primary_output_id: None,
        }
    }

    /// Set the output field id rendered as the primary result.
    #[must_use]
    pub fn with_primary_output_id(mut self, primary_output_id: impl Into<String>) -> Self {
        self.primary_output_id = Some(primary_output_id.into());
        self
    }
}

/// One typed output field in a plugin manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginOutputField {
    /// Canonical output key emitted by the plugin export.
    pub name: String,
    /// Optional short presentation label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional output help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Closed output kind vocabulary.
    pub kind: PluginOutputKind,
}

impl PluginOutputField {
    /// Create an output field.
    #[must_use]
    pub fn new(name: impl Into<String>, kind: PluginOutputKind) -> Self {
        Self {
            name: name.into(),
            label: None,
            description: None,
            kind,
        }
    }

    /// Add a short presentation label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add output help text.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Closed plugin output kind vocabulary mirroring `upeg_core::OutputKind`.
///
/// Same wire-name parity contract as [`PluginInputKind`]; see its doc
/// comment for the `DateTime` rename rationale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "options")]
pub enum PluginOutputKind {
    String,
    Number,
    Integer,
    Boolean,
    Options(Vec<PluginChoiceOption>),
    MultiOptions(Vec<PluginChoiceOption>),
    Markdown,
    Json,
    #[serde(rename = "datetime")]
    DateTime,
    FilePath,
    Url,
    File,
    EmbeddedView {
        url: String,
    },
}

/// One selectable plugin choice option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginChoiceOption {
    /// Stable value sent to the plugin export.
    pub value: String,
    /// Optional presentation label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional choice help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PluginChoiceOption {
    /// Create a choice option.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: None,
            description: None,
        }
    }

    /// Add a presentation label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add choice help text.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A tool declared by a plugin's `manifest` export.
///
/// Field names are the JSON wire shape consumed by `upeg-wasm`. Host-side
/// validation still lowers this declaration into `upeg_core::ToolMeta`,
/// where canonical IDs, tags, surfaces, and input specs are checked before
/// registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginToolDecl {
    /// Canonical full Tool id, normally `{toolkit}.{tool}`.
    pub id: ToolId,
    /// Owning Toolkit id.
    pub toolkit: ToolkitId,
    /// Tool-level discovery tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Short human-readable display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_label: Option<String>,
    /// One-line human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Typed input metadata for host-side validation and surface rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_spec: Option<PluginInputSpec>,
    /// Typed output metadata for host-side rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_spec: Option<PluginOutputSpec>,
    /// Optional upeg pin kind label, for example `Inline` or `Launcher`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// Compact pegboard footprint label: `U1`, `U2`, or `U2T`.
    pub pegboard_units: String,
    /// Optional surface labels, for example `cli` or `mcp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surfaces: Option<Vec<String>>,
    /// Wasm export name. Defaults to `id` in the host when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export: Option<ExportName>,
}

impl PluginToolDecl {
    /// Create a Tool declaration that explicitly maps a upeg Tool id to a wasm
    /// export name.
    #[must_use]
    pub fn new(
        toolkit: impl Into<ToolkitId>,
        id: impl Into<ToolId>,
        export: impl Into<ExportName>,
    ) -> Self {
        Self {
            id: id.into(),
            toolkit: toolkit.into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: None,
            pegboard_units: "U1".to_string(),
            surfaces: None,
            export: Some(export.into()),
        }
    }

    /// Add discovery tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Add a one-line description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a short display label.
    #[must_use]
    pub fn with_display_label(mut self, display_label: impl Into<String>) -> Self {
        self.display_label = Some(display_label.into());
        self
    }

    /// Add a typed input specification.
    #[must_use]
    pub fn with_input_spec(mut self, input_spec: PluginInputSpec) -> Self {
        self.input_spec = Some(input_spec);
        self
    }

    /// Add a typed output specification.
    #[must_use]
    pub fn with_output_spec(mut self, output_spec: PluginOutputSpec) -> Self {
        self.output_spec = Some(output_spec);
        self
    }

    /// Set the compact pegboard footprint label (`U1`, `U2`, or `U2T`).
    #[must_use]
    pub fn with_pegboard_units(mut self, pegboard_units: impl Into<String>) -> Self {
        self.pegboard_units = pegboard_units.into();
        self
    }
}

/// JSON document returned by a plugin's `manifest` export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Toolkit id.
    pub id: ToolkitId,
    /// Toolkit-level tags inherited by every Tool in the Toolkit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// One-line Toolkit description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tool declarations exported by this plugin.
    #[serde(default)]
    pub tools: Vec<PluginToolDecl>,
}

impl PluginManifest {
    /// Create an empty plugin manifest for a Toolkit id.
    #[must_use]
    pub fn new(id: impl Into<ToolkitId>) -> Self {
        Self {
            id: id.into(),
            tags: None,
            description: None,
            tools: Vec::new(),
        }
    }

    /// Add Toolkit-level discovery tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = Some(tags.into_iter().map(Into::into).collect());
        self
    }

    /// Add a one-line Toolkit description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add one Tool declaration.
    #[must_use]
    pub fn with_tool(mut self, tool: PluginToolDecl) -> Self {
        self.tools.push(tool);
        self
    }

    /// Serialize this manifest to the JSON string a plugin's `manifest`
    /// export returns.
    ///
    /// # Errors
    ///
    /// Returns any `serde_json` serialization error.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 최소_manifest는_역직렬화한다() {
        let json = r#"{"id":"x","tools":[{"id":"x.echo","toolkit":"x","pegboard_units":"U1"}]}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.id.as_str(), "x");
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].id.as_str(), "x.echo");
        assert_eq!(manifest.tools[0].toolkit.as_str(), "x");
        assert_eq!(manifest.tools[0].pegboard_units, "U1");
        assert!(manifest.tools[0].export.is_none());
    }

    #[test]
    fn 전체_manifest는_역직렬화한다() {
        let json = r#"{"id":"demo","tags":["wasm"],"tools":[{
            "id": "demo.greet",
            "toolkit": "demo",
            "description": "Say hi.",
            "input_spec": {"fields":[{"name":"name","required":true,"kind":{"type":"string"}}]},
            "output_spec": {"fields":[{"name":"greeting","kind":{"type":"string"}}],"primary_output_id":"greeting"},
            "pin": "Launcher",
            "pegboard_units": "U2",
            "surfaces": ["cli","mcp"],
            "export": "demo_greet"
        }]}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();

        assert_eq!(manifest.tags.as_deref(), Some(&["wasm".to_string()][..]));
        let tool = &manifest.tools[0];
        assert_eq!(tool.description.as_deref(), Some("Say hi."));
        assert_eq!(
            tool.input_spec.as_ref().map(|spec| spec.fields.len()),
            Some(1)
        );
        assert_eq!(
            tool.input_spec.as_ref().map(|spec| &spec.fields[0].kind),
            Some(&PluginInputKind::String)
        );
        assert_eq!(
            tool.output_spec.as_ref().map(|spec| spec.fields.len()),
            Some(1)
        );
        assert_eq!(
            tool.output_spec
                .as_ref()
                .and_then(|spec| spec.primary_output_id.as_deref()),
            Some("greeting")
        );
        assert_eq!(
            tool.output_spec.as_ref().map(|spec| &spec.fields[0].kind),
            Some(&PluginOutputKind::String)
        );
        assert_eq!(tool.pin.as_deref(), Some("Launcher"));
        assert_eq!(tool.pegboard_units, "U2");
        assert_eq!(
            tool.surfaces.as_deref(),
            Some(&["cli".to_string(), "mcp".to_string()][..])
        );
        assert_eq!(
            tool.export.as_ref().map(ExportName::as_str),
            Some("demo_greet")
        );
    }

    #[test]
    fn 빌더는_널_옵션_필드_없이_직렬화한다() {
        let manifest = PluginManifest::new("demo").with_tool(
            PluginToolDecl::new("demo", "demo.greet", "demo_greet")
                .with_description("Say hi.")
                .with_input_spec(PluginInputSpec::new([PluginInputField::required(
                    "name",
                    PluginInputKind::String,
                )]))
                .with_output_spec(PluginOutputSpec::new([PluginOutputField::new(
                    "greeting",
                    PluginOutputKind::String,
                )])),
        );

        let json = manifest.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["id"], "demo");
        assert_eq!(value["tools"][0]["toolkit"], "demo");
        assert_eq!(value["tools"][0]["id"], "demo.greet");
        assert_eq!(value["tools"][0]["input_spec"]["fields"][0]["name"], "name");
        assert_eq!(
            value["tools"][0]["input_spec"]["fields"][0]["kind"]["type"],
            "string"
        );
        assert_eq!(
            value["tools"][0]["output_spec"]["fields"][0]["kind"]["type"],
            "string"
        );
        assert_eq!(
            value["tools"][0]["output_spec"]["primary_output_id"],
            "greeting"
        );
        assert_eq!(value["tools"][0]["pegboard_units"], "U1");
        assert_eq!(value["tools"][0]["export"], "demo_greet");
        assert!(
            value.get("tags").is_none(),
            "omitted optional manifest fields should not serialize as null: {json}"
        );
        assert!(
            value["tools"][0].get("surfaces").is_none(),
            "omitted optional tool fields should not serialize as null: {json}"
        );
    }
}
