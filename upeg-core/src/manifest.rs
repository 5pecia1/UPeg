//! `ToolMeta`, `ToolkitMeta`, and `BoardExecutionContext` — the manifest
//! data layer for the upeg toolbox.
//!
//! Lexicon §4. The `render` binding for a specific surface lives in the
//! surface-specific layer (see `flutter_app/lib/src/widgets/pin_renderers/`
//! for the desktop/PWA renderer registry).

use crate::identity::{ToolIdentity, ToolKey, canonical_tool_id_in_toolkit, canonical_toolkit_id};
use crate::input::{InputSpec, InputSpecError, StaticInputSpec};
use crate::output::{OutputFieldSpec, OutputSpec, OutputSpecError, StaticOutputSpec};
use crate::presentation::{ToolEffect, ToolPresentation};
use crate::source::{Source, StaticSource};
use crate::types::{Invoker, PegboardUnits, PinKind, Surface};

#[derive(Clone, Copy, Debug)]
pub struct StaticToolMeta {
    /// Canonical full identifier: `{toolkit}.{tool}`.
    pub id: &'static str,
    /// Owning Toolkit identifier. Toolkit is a grouping/distribution unit;
    /// the callable unit is always the Tool identified by [`StaticToolMeta::id`].
    pub toolkit: &'static str,
    /// Tool identifier inside its owning Toolkit.
    ///
    /// This is part of the structured toolbox key. It is stored separately
    /// from [`StaticToolMeta::id`] so code does not infer ownership by splitting
    /// the full id on `.`.
    pub local_id: &'static str,
    /// Effective PRD v2.1 tags for this Tool. Declarative/WASM sources
    /// pre-merge Toolkit-level and Tool-level tags before constructing
    /// `ToolMeta`, so consumers read one canonical tag set.
    pub tags: &'static [&'static str],
    /// Canonical short human-readable label for presentation surfaces.
    ///
    /// This is populated by each Tool source (Rust macro, TOML loader, or
    /// WASM manifest lowering) so surfaces never need a central hardcoded
    /// id-to-label lookup table.
    pub display_label: &'static str,
    /// One-line human-readable description. Surfaced by MCP `tools/list`,
    /// HTTP `/v1/tools`, and the TUI detail panel. Empty string is the
    /// default so existing tools without a description still compile.
    pub description: &'static str,
    /// Inventory-safe typed input metadata emitted by compile-time registration.
    pub input_spec: StaticInputSpec,
    /// Inventory-safe typed output metadata. Mirror of `input_spec`.
    pub output_spec: StaticOutputSpec,
    /// Canonical output field id rendered as the primary result.
    pub primary_output_id: Option<&'static str>,
    pub effect: ToolEffect,
    /// How the Pin's execution begins. GUI-only hint; non-GUI surfaces
    /// ignore the variant and call the tool function directly.
    pub source: StaticSource,
    pub pin: PinKind,
    pub pegboard_units: PegboardUnits,
    pub invoker: Invoker,
    pub surfaces: &'static [Surface],
    pub boards: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct ToolMeta {
    /// Canonical full identifier: `{toolkit}.{tool}`.
    pub id: &'static str,
    /// Owning Toolkit identifier. Toolkit is a grouping/distribution unit;
    /// the callable unit is always the Tool identified by [`ToolMeta::id`].
    pub toolkit: &'static str,
    /// Tool identifier inside its owning Toolkit.
    ///
    /// This is part of the structured toolbox key. It is stored separately
    /// from [`ToolMeta::id`] so code does not infer ownership by splitting the
    /// full id on `.`.
    pub local_id: &'static str,
    /// Effective PRD v2.1 tags for this Tool. Declarative/WASM sources
    /// pre-merge Toolkit-level and Tool-level tags before constructing
    /// `ToolMeta`, so consumers read one canonical tag set.
    pub tags: &'static [&'static str],
    /// Canonical short human-readable label for presentation surfaces.
    pub display_label: &'static str,
    /// One-line human-readable description. Surfaced by MCP `tools/list`,
    /// HTTP `/v1/tools`, and the TUI detail panel.
    pub description: &'static str,
    /// Canonical owned typed input metadata.
    pub input_spec: InputSpec,
    /// Canonical owned typed output metadata. Mirror of `input_spec`.
    pub output_spec: OutputSpec,
    /// Canonical output field id rendered as the primary result.
    pub primary_output_id: Option<&'static str>,
    pub effect: ToolEffect,
    pub presentation: Option<ToolPresentation>,
    /// Trigger source for this Tool.
    pub source: Source,
    pub pin: PinKind,
    pub pegboard_units: PegboardUnits,
    pub invoker: Invoker,
    pub surfaces: &'static [Surface],
    pub boards: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub struct ToolkitMeta {
    /// Toolkit identifier: group/distribution unit, never directly callable.
    pub id: &'static str,
    /// Toolkit-level tags inherited by every Tool in the Toolkit.
    pub tags: &'static [&'static str],
    /// One-line Toolkit description for discovery surfaces.
    pub description: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardExecutionContext {
    /// Board key whose context is being applied.
    pub board: String,
    /// Environment entries scoped to this Board. External dispatchers receive
    /// these as real process environment variables and all dispatchers also
    /// receive them under the reserved `_upeg.boardEnv` request field.
    pub env: std::collections::BTreeMap<String, String>,
    /// Auto-detected project manifest path for this execution, when present.
    pub project_manifest: Option<String>,
}

impl ToolkitMeta {
    pub fn assert_valid(&self) {
        canonical_toolkit_id(self.id);
        for tag in self.tags {
            assert!(!tag.trim().is_empty(), "ToolkitMeta tags must be non-empty");
            assert_eq!(
                *tag,
                tag.trim(),
                "ToolkitMeta tags must be canonical and unpadded"
            );
        }
    }
}

impl BoardExecutionContext {
    pub fn new(board: impl Into<String>) -> Self {
        Self {
            board: board.into(),
            env: std::collections::BTreeMap::new(),
            project_manifest: None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "board": self.board,
            "boardEnv": self.env,
            "projectManifest": self.project_manifest,
        })
    }
}

pub const EXECUTION_CONTEXT_ARG: &str = "_upeg";

/// Key inside the [`EXECUTION_CONTEXT_ARG`] envelope carrying the
/// caller's working directory.
///
/// A dispatch can originate from a shell in a subdirectory, the host
/// daemon, an MCP client, or a GUI pin — none of which share upeg's own
/// process working directory. Callers that know where the user "is" put
/// an **absolute** path here; the External invoker honors it unless the
/// tool declares its own `cwd`.
pub const EXECUTION_CONTEXT_CWD: &str = "cwd";

/// Key inside the [`EXECUTION_CONTEXT_ARG`] envelope carrying the label
/// of the [`Surface`] the call originated from (`cli`, `mcp`, `http`, …).
///
/// Stamped by the surface itself, and a caller-supplied value under this
/// name is always discarded by the runtime's reserved-block wipe. That
/// makes it the one piece of caller identity a dispatcher may trust,
/// which is why Chain-step approval authorization is keyed on it
/// (`docs/architecture/chain.md`).
pub const EXECUTION_CONTEXT_SURFACE: &str = "surface";

/// Key inside the [`EXECUTION_CONTEXT_ARG`] envelope carrying the
/// caller's Chain-step approval allow-list (step ids or tool ids).
///
/// Unlike [`EXECUTION_CONTEXT_SURFACE`] this one is *carried from the
/// caller* — the surface has no value of its own to stamp here — so it
/// states intent, never authority. Whether that intent is honored is
/// decided against [`EXECUTION_CONTEXT_SURFACE`].
pub const EXECUTION_CONTEXT_APPROVED_STEPS: &str = "approvedSteps";

/// Key inside the [`EXECUTION_CONTEXT_ARG`] envelope carrying the
/// stamped [`crate::Principal`] block (`{ role, surface }`).
///
/// Authoritative-from-surface, exactly like [`EXECUTION_CONTEXT_SURFACE`]:
/// the runtime writes it and the reserved-block wipe discards whatever a
/// caller put under this name. It is what lets an authorization decision
/// say "this *caller*" instead of only "this door"
/// (`docs/architecture/call-envelope.md`).
pub const EXECUTION_CONTEXT_PRINCIPAL: &str = "principal";

/// Every surface except MCP — the default for tools imported *from* an
/// upstream MCP server. Re-exposing an imported tool back over upeg's
/// own MCP surface is an explicit opt-in (`reexport = true` in the
/// import config); without it, an import chain cannot silently loop a
/// server's tools back out over MCP.
pub const ALL_SURFACES_EXCEPT_MCP: &[Surface] = &[
    Surface::Cli,
    Surface::Tui,
    Surface::Desktop,
    Surface::Pwa,
    Surface::Ext,
    Surface::Http,
];

impl StaticToolMeta {
    #[allow(
        clippy::expect_used,
        reason = "self-validation entry point; panic is the contract on invalid StaticToolMeta"
    )]
    pub fn assert_valid(&self) {
        let identity = canonical_tool_id_in_toolkit(self.id, self.toolkit);
        assert_eq!(
            identity.local(),
            self.local_id,
            "StaticToolMeta.local_id must match the full id's local Tool id"
        );
        ToolKey::parse_canonical(self.toolkit, self.local_id)
            .expect("StaticToolMeta structured key must be canonical and unpadded");
        InputSpec::from_static_fields(self.input_spec.fields)
            .expect("StaticToolMeta.input_spec must be valid");
        let output_spec = self
            .output_spec
            .to_output_spec()
            .expect("StaticToolMeta.output_spec must be valid");
        validate_primary_output_id(&output_spec.fields, self.primary_output_id)
            .expect("StaticToolMeta.primary_output_id must reference output_spec.fields");
        assert!(
            !self.display_label.trim().is_empty(),
            "StaticToolMeta.display_label must be non-empty"
        );
        assert_eq!(
            self.display_label,
            self.display_label.trim(),
            "StaticToolMeta.display_label must be canonical and unpadded"
        );
        for tag in self.tags {
            assert!(
                !tag.trim().is_empty(),
                "StaticToolMeta tags must be non-empty"
            );
            assert_eq!(
                *tag,
                tag.trim(),
                "StaticToolMeta tags must be canonical and unpadded"
            );
        }
        for board in self.boards {
            assert!(
                !board.trim().is_empty(),
                "StaticToolMeta boards must be non-empty"
            );
            assert_eq!(
                *board,
                board.trim(),
                "StaticToolMeta boards must be canonical and unpadded"
            );
        }
        // Embed pin/invoker pairing — Passive (pin=Embed) requires
        // invoker=Static, Controlled (pin=ControlledEmbed) requires
        // invoker=Embed. Binding-shape rules need the parsed bindings
        // which `assert_valid` doesn't have; the loader runs that
        // second pass after registering bindings.
        crate::validate_pin_invoker_pairing(self.id, self.pin, self.invoker)
            .expect("StaticToolMeta embed pairing must be valid");
    }

    pub fn identity(&self) -> ToolIdentity<'_> {
        canonical_tool_id_in_toolkit(self.id, self.toolkit)
    }

    /// Structured toolbox key for this Tool.
    #[allow(
        clippy::expect_used,
        reason = "StaticToolMeta fields are produced by compile-time-canonical metadata; parse cannot fail at runtime"
    )]
    pub fn key(&self) -> ToolKey<'_> {
        ToolKey::parse_canonical(self.toolkit, self.local_id)
            .expect("StaticToolMeta structured key must be canonical and unpadded")
    }

    /// Toolkit id for this Tool.
    pub const fn toolkit_id(&self) -> &str {
        self.toolkit
    }

    /// Tool id within its Toolkit.
    pub const fn tool_id(&self) -> &str {
        self.local_id
    }

    /// Returns `true` if this Tool supports headless dispatch on any of its
    /// surfaces (CLI, TUI, MCP, or HTTP).
    pub fn has_headless_dispatch_surface(&self) -> bool {
        self.surfaces.iter().any(|s| s.supports_headless_dispatch())
    }
}

// Identity for `PartialEq` is the structured Toolkit/local key. The full id is
// a transport/display string and must not be parsed to infer ownership.
impl PartialEq for ToolMeta {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for ToolMeta {}
impl std::hash::Hash for ToolMeta {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

/// Conversion error when materializing a [`ToolMeta`] from a
/// [`StaticToolMeta`]. Wraps either an input-side or output-side spec error.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolMetaError {
    #[error(transparent)]
    Input(#[from] InputSpecError),
    #[error(transparent)]
    Output(#[from] OutputSpecError),
    #[error(transparent)]
    PrimaryOutput(#[from] PrimaryOutputIdError),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrimaryOutputIdError {
    #[error("primary_output_id required when outputs are non-empty")]
    MissingPrimaryOutputId,
    #[error("primary_output_id `{id}` does not reference any output field name")]
    InvalidPrimaryOutputId { id: String },
    #[error("primary_output_id must be absent when outputs are empty")]
    UnexpectedPrimaryOutputId,
}

pub fn validate_primary_output_id(
    fields: &[OutputFieldSpec],
    primary_output_id: Option<&str>,
) -> Result<(), PrimaryOutputIdError> {
    if fields.is_empty() {
        return if primary_output_id.is_some() {
            Err(PrimaryOutputIdError::UnexpectedPrimaryOutputId)
        } else {
            Ok(())
        };
    }

    let Some(primary_output_id) = primary_output_id else {
        return Err(PrimaryOutputIdError::MissingPrimaryOutputId);
    };

    if fields.iter().any(|field| field.name == primary_output_id) {
        Ok(())
    } else {
        Err(PrimaryOutputIdError::InvalidPrimaryOutputId {
            id: primary_output_id.to_string(),
        })
    }
}

impl ToolMeta {
    pub fn from_static(meta: &'static StaticToolMeta) -> Result<Self, ToolMetaError> {
        let output_spec = meta.output_spec.to_output_spec()?;
        validate_primary_output_id(&output_spec.fields, meta.primary_output_id)?;
        Ok(Self {
            id: meta.id,
            toolkit: meta.toolkit,
            local_id: meta.local_id,
            tags: meta.tags,
            display_label: meta.display_label,
            description: meta.description,
            input_spec: InputSpec::from_static_fields(meta.input_spec.fields)?,
            output_spec,
            primary_output_id: meta.primary_output_id,
            effect: meta.effect,
            presentation: None,
            source: meta.source.to_owned_source(),
            pin: meta.pin,
            pegboard_units: meta.pegboard_units,
            invoker: meta.invoker,
            surfaces: meta.surfaces,
            boards: meta.boards,
        })
    }

    #[allow(
        clippy::expect_used,
        reason = "self-validation entry point; panic is the contract on invalid ToolMeta"
    )]
    pub fn assert_valid(&self) {
        let identity = canonical_tool_id_in_toolkit(self.id, self.toolkit);
        assert_eq!(
            identity.local(),
            self.local_id,
            "ToolMeta.local_id must match the full id's local Tool id"
        );
        ToolKey::parse_canonical(self.toolkit, self.local_id)
            .expect("ToolMeta structured key must be canonical and unpadded");
        InputSpec::new(self.input_spec.fields.clone()).expect("ToolMeta.input_spec must be valid");
        OutputSpec::new(self.output_spec.fields.clone())
            .expect("ToolMeta.output_spec must be valid");
        validate_primary_output_id(&self.output_spec.fields, self.primary_output_id)
            .expect("ToolMeta.primary_output_id must reference output_spec.fields");
        assert!(
            !self.display_label.trim().is_empty(),
            "ToolMeta.display_label must be non-empty"
        );
        assert_eq!(
            self.display_label,
            self.display_label.trim(),
            "ToolMeta.display_label must be canonical and unpadded"
        );
        for tag in self.tags {
            assert!(!tag.trim().is_empty(), "ToolMeta tags must be non-empty");
            assert_eq!(
                *tag,
                tag.trim(),
                "ToolMeta tags must be canonical and unpadded"
            );
        }
        for board in self.boards {
            assert!(
                !board.trim().is_empty(),
                "ToolMeta boards must be non-empty"
            );
            assert_eq!(
                *board,
                board.trim(),
                "ToolMeta boards must be canonical and unpadded"
            );
        }
        // Embed pin/invoker pairing — Passive (pin=Embed) requires
        // invoker=Static, Controlled (pin=ControlledEmbed) requires
        // invoker=Embed. Binding-shape rules run in the loader after
        // selector_bindings have been parsed.
        crate::validate_pin_invoker_pairing(self.id, self.pin, self.invoker)
            .expect("ToolMeta embed pairing must be valid");
    }

    pub fn identity(&self) -> ToolIdentity<'_> {
        canonical_tool_id_in_toolkit(self.id, self.toolkit)
    }

    /// Structured toolbox key for this Tool.
    #[allow(
        clippy::expect_used,
        reason = "ToolMeta fields are produced by compile-time-canonical metadata; parse cannot fail at runtime"
    )]
    pub fn key(&self) -> ToolKey<'_> {
        ToolKey::parse_canonical(self.toolkit, self.local_id)
            .expect("ToolMeta structured key must be canonical and unpadded")
    }

    /// Toolkit id for this Tool.
    pub const fn toolkit_id(&self) -> &str {
        self.toolkit
    }

    /// Tool id within its Toolkit. For `num.hex_to_decimal`, this returns
    /// `hex_to_decimal`.
    pub const fn tool_id(&self) -> &str {
        self.local_id
    }

    /// Format the Surface list for compact UI footers:
    /// `cli · tui · desktop · pwa · ext · mcp · http`.
    pub fn surfaces_label(&self) -> String {
        self.surface_labels().join(" · ")
    }

    /// Generated `input_schema` value for API surfaces.
    ///
    /// `input_spec` remains the canonical owned runtime metadata; JSON Schema is
    /// only the boundary representation.
    pub fn input_schema_value(&self) -> serde_json::Value {
        self.input_spec.to_json_schema_value()
    }

    /// Each surface as its `label()` — the JSON-array form used by
    /// HTTP `/v1/tools`, HTTP `/v1/openapi.json` `x-surfaces`, CLI
    /// `tool list --json`, and CLI `tool show --json`.
    pub fn surface_labels(&self) -> Vec<&'static str> {
        self.surfaces.iter().map(|s| s.label()).collect()
    }

    pub fn is_on_board(&self, board: &str) -> bool {
        self.boards.contains(&board)
    }

    /// `true` when this Tool is meant to appear on (and be dispatchable from)
    /// the given surface. Each binary entry-point (CLI, MCP server, HTTP,
    /// TUI, …) filters by its own surface so a tool declared
    /// `surfaces = ["mcp"]` doesn't leak into `upeg tool list` or `upeg call`.
    pub fn is_on_surface(&self, s: Surface) -> bool {
        self.surfaces.contains(&s)
    }

    /// Returns `true` if this Tool supports headless dispatch on any of its
    /// surfaces (CLI, TUI, MCP, or HTTP).
    pub fn has_headless_dispatch_surface(&self) -> bool {
        self.surfaces.iter().any(|s| s.supports_headless_dispatch())
    }
}

#[cfg(test)]
mod headless_dispatch_tests {
    use super::*;
    use crate::types::{ALL_SURFACES, EMBED_SURFACES, GUI_SURFACES};

    /// Fixtures for testing mixed-capability tools.
    fn mixed_capability_static_tool() -> &'static StaticToolMeta {
        static TOOL: StaticToolMeta = StaticToolMeta {
            id: "test.mixed_capability",
            toolkit: "test",
            local_id: "mixed_capability",
            tags: &[],
            display_label: "Mixed Capability Tool",
            description: "A tool with both CLI and Desktop surfaces",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Cli, Surface::Desktop],
            boards: &[],
        };
        &TOOL
    }

    fn gui_only_static_tool() -> &'static StaticToolMeta {
        static TOOL: StaticToolMeta = StaticToolMeta {
            id: "test.gui_only",
            toolkit: "test",
            local_id: "gui_only",
            tags: &[],
            display_label: "GUI Only Tool",
            description: "A tool with Desktop surface only",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Launcher,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Desktop],
            boards: &[],
        };
        &TOOL
    }

    fn headless_only_static_tool() -> &'static StaticToolMeta {
        static TOOL: StaticToolMeta = StaticToolMeta {
            id: "test.headless_only",
            toolkit: "test",
            local_id: "headless_only",
            tags: &[],
            display_label: "Headless Only Tool",
            description: "A tool with CLI surface only",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Cli],
            boards: &[],
        };
        &TOOL
    }

    #[test]
    fn mixed_capability_tool_supports_headless_dispatch() {
        let tool = mixed_capability_static_tool();
        assert!(
            tool.has_headless_dispatch_surface(),
            "Tool with [Cli, Desktop] surfaces should support headless dispatch"
        );
    }

    #[test]
    fn gui_only_tool_does_not_support_headless_dispatch() {
        let tool = gui_only_static_tool();
        assert!(
            !tool.has_headless_dispatch_surface(),
            "Tool with [Desktop] surface should NOT support headless dispatch"
        );
    }

    #[test]
    fn headless_only_tool_supports_headless_dispatch() {
        let tool = headless_only_static_tool();
        assert!(
            tool.has_headless_dispatch_surface(),
            "Tool with [Cli] surface should support headless dispatch"
        );
    }

    #[test]
    fn gui_surfaces_have_no_headless_dispatch_surface() {
        // Test each GUI surface individually
        let desktop_tool = StaticToolMeta {
            id: "test.desktop",
            toolkit: "test",
            local_id: "desktop",
            tags: &[],
            display_label: "Desktop",
            description: "",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Desktop],
            boards: &[],
        };
        assert!(
            !desktop_tool.has_headless_dispatch_surface(),
            "Desktop tool should NOT support headless dispatch"
        );

        let pwa_tool = StaticToolMeta {
            id: "test.pwa",
            toolkit: "test",
            local_id: "pwa",
            tags: &[],
            display_label: "PWA",
            description: "",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Pwa],
            boards: &[],
        };
        assert!(
            !pwa_tool.has_headless_dispatch_surface(),
            "PWA tool should NOT support headless dispatch"
        );

        let ext_tool = StaticToolMeta {
            id: "test.ext",
            toolkit: "test",
            local_id: "ext",
            tags: &[],
            display_label: "Ext",
            description: "",
            input_spec: StaticInputSpec { fields: &[] },
            output_spec: StaticOutputSpec { fields: &[] },
            primary_output_id: None,
            effect: ToolEffect::Unknown,
            source: StaticSource::Static,
            pin: PinKind::Inline,
            pegboard_units: PegboardUnits::U1,
            invoker: Invoker::Function,
            surfaces: &[Surface::Ext],
            boards: &[],
        };
        assert!(
            !ext_tool.has_headless_dispatch_surface(),
            "Ext tool should NOT support headless dispatch"
        );
    }

    #[test]
    fn all_surfaces_partition_into_4_headless_and_3_gui() {
        let headless_count = ALL_SURFACES
            .iter()
            .filter(|s| s.supports_headless_dispatch())
            .count();
        let gui_count = ALL_SURFACES
            .iter()
            .filter(|s| s.requires_gui_surface())
            .count();
        assert_eq!(headless_count, 4, "Expected 4 headless surfaces");
        assert_eq!(gui_count, 3, "Expected 3 GUI surfaces");
        assert_eq!(
            headless_count + gui_count,
            7,
            "All 7 surfaces should be accounted for"
        );
    }

    #[test]
    fn gui_surfaces_const_does_not_support_headless_dispatch() {
        // Using GUI_SURFACES constant
        for s in GUI_SURFACES {
            assert!(
                !(*s).supports_headless_dispatch(),
                "{s:?} should not support headless dispatch"
            );
        }
    }

    #[test]
    fn embed_surfaces_const_does_not_support_headless_dispatch() {
        // Using EMBED_SURFACES constant
        for s in EMBED_SURFACES {
            assert!(
                !(*s).supports_headless_dispatch(),
                "{s:?} should not support headless dispatch"
            );
        }
    }

    #[test]
    fn mixed_capability_toolmeta_also_supports_headless_dispatch() {
        let static_tool = mixed_capability_static_tool();
        let tool_meta =
            ToolMeta::from_static(static_tool).expect("valid fixture should convert to ToolMeta");
        assert!(
            tool_meta.has_headless_dispatch_surface(),
            "ToolMeta with [Cli, Desktop] should support headless dispatch"
        );
    }
}
