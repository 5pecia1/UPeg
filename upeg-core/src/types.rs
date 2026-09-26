//! Pure-data enums shared by every surface: PinKind, PegboardUnits,
//! Invoker, Surface, and IoType.
//!
//! These types are deliberately small and dependency-free — UI surfaces
//! and runtime toolbox code imports them without dragging in toolbox
//! state, schema, or inventory metadata. The closed I/O type set
//! ([`IoType`] / input/output kinds) is documented in [`crate::input`]
//! module docs.

use crate::args_preset::ArgsPreset;
use crate::pin_span::PinSpan;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

const PIN_COLOR_PREFIX: char = '#';
const PIN_COLOR_HEX_DIGITS: usize = 6;
const PIN_COLOR_HEX_LEN: usize = 1 + PIN_COLOR_HEX_DIGITS;

// ─── PinKind ────────────────────────────────────────────────
// Lexicon §4: the display pattern shown at the UI point. Pascal-case labels appear on UI badges.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PinKind {
    Inline,
    Launcher,
    Live,
    Action,
    /// Passive embed — the external website IS the tool. User
    /// interacts with the webview directly. Pairs with
    /// [`Invoker::Static`].
    Embed,
    /// Controlled embed — the app drives the external website by
    /// writing form values into DOM via CSS selectors. Pairs with
    /// [`Invoker::Embed`] and requires non-empty
    /// `controlled_embed.bindings`.
    ControlledEmbed,
    Chain,
    Llm,
}

impl PinKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inline => "Inline",
            Self::Launcher => "Launcher",
            Self::Live => "Live",
            Self::Action => "Action",
            Self::Embed => "Embed",
            Self::ControlledEmbed => "ControlledEmbed",
            Self::Chain => "Chain",
            Self::Llm => "Llm",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "inline" => Self::Inline,
            "launcher" => Self::Launcher,
            "live" => Self::Live,
            "action" => Self::Action,
            "embed" => Self::Embed,
            "controlledembed" => Self::ControlledEmbed,
            "chain" => Self::Chain,
            "llm" => Self::Llm,
            _ => return None,
        })
    }
}

// ─── BindingRole ────────────────────────────────────────────────
// Controlled Embed design (2026-05-29): every `selector_binding`
// participates in the pipeline in exactly one role.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BindingRole {
    /// Write the corresponding `input_field` value into the DOM
    /// element matched by `selector` (sets `.value`, fires `input`
    /// event). `field` names the input.
    #[default]
    Input,
    /// Fire a click event on the DOM element matched by `selector`
    /// (e.g., a page's submit button). `field` is unused.
    Trigger,
    /// Read the DOM element matched by `selector` (`textContent` or
    /// `.value`) after the trigger fires and surface it as an output
    /// with key `field`.
    Output,
}

impl BindingRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Trigger => "trigger",
            Self::Output => "output",
        }
    }

    /// Parse a role label. Case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "input" => Self::Input,
            "trigger" => Self::Trigger,
            "output" => Self::Output,
            _ => return None,
        })
    }
}

// ─── Controlled Embed browser settings ──────────────────────────

pub const CONTROLLED_EMBED_MOBILE_VIEWPORT: (u16, u16) = (390, 844);
pub const CONTROLLED_EMBED_TABLET_VIEWPORT: (u16, u16) = (768, 1024);
pub const CONTROLLED_EMBED_DESKTOP_VIEWPORT: (u16, u16) = (1366, 768);
pub const CONTROLLED_EMBED_VIEWPORT_MIN: u16 = 1;
pub const CONTROLLED_EMBED_VIEWPORT_MAX: u16 = 4096;
pub const DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS: u64 = 5000;
pub const DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS: u64 = 0;
pub const DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS: u64 = 50;
pub const MAX_CONTROLLED_EMBED_WAIT_MS: u64 = 60000;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ControlledEmbedSettings {
    pub user_agent: Option<ControlledEmbedUserAgent>,
    pub viewport: Option<ControlledEmbedViewport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ControlledEmbedUserAgent {
    Default,
    MobileSafari,
    Custom(String),
}

impl ControlledEmbedUserAgent {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::MobileSafari => "mobile_safari",
            Self::Custom(_) => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "default" => Self::Default,
            "mobile_safari" => Self::MobileSafari,
            "custom" => Self::Custom(String::new()),
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ControlledEmbedViewport {
    Preset(ControlledEmbedViewportPreset),
    Custom { width: u16, height: u16 },
}

impl ControlledEmbedViewport {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Preset(preset) => preset.label(),
            Self::Custom { .. } => "custom",
        }
    }

    pub const fn dimensions(&self) -> (u16, u16) {
        match self {
            Self::Preset(preset) => preset.dimensions(),
            Self::Custom { width, height } => (*width, *height),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ControlledEmbedViewportPreset {
    Mobile,
    Tablet,
    Desktop,
}

impl ControlledEmbedViewportPreset {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mobile => "mobile",
            Self::Tablet => "tablet",
            Self::Desktop => "desktop",
        }
    }

    pub const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Mobile => CONTROLLED_EMBED_MOBILE_VIEWPORT,
            Self::Tablet => CONTROLLED_EMBED_TABLET_VIEWPORT,
            Self::Desktop => CONTROLLED_EMBED_DESKTOP_VIEWPORT,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "mobile" => Self::Mobile,
            "tablet" => Self::Tablet,
            "desktop" => Self::Desktop,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ControlledEmbedTriggerAction {
    #[default]
    Click,
    Enter,
}

impl ControlledEmbedTriggerAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Enter => "enter",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "click" => Self::Click,
            "enter" => Self::Enter,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BindingWaitCondition {
    #[default]
    Exists,
    Visible,
}

impl BindingWaitCondition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::Visible => "visible",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "exists" => Self::Exists,
            "visible" => Self::Visible,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BindingWaitOnTimeout {
    #[default]
    Fail,
    Continue,
}

impl BindingWaitOnTimeout {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Continue => "continue",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "fail" => Self::Fail,
            "continue" => Self::Continue,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BindingWait {
    pub for_selector: Option<String>,
    pub condition: BindingWaitCondition,
    pub timeout_ms: u64,
    pub settle_ms: u64,
    pub on_timeout: BindingWaitOnTimeout,
}

impl Default for BindingWait {
    fn default() -> Self {
        Self {
            for_selector: None,
            condition: BindingWaitCondition::Exists,
            timeout_ms: DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS,
            settle_ms: DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS,
            on_timeout: BindingWaitOnTimeout::Fail,
        }
    }
}

/// One CSS selector binding for a Controlled Embed tool. Owned-string
/// data. Lives in `upeg-core` so validators and runtime registries
/// share a single definition.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SelectorBinding {
    pub role: BindingRole,
    pub field: String,
    pub selector: String,
    pub trigger_action: ControlledEmbedTriggerAction,
    pub wait: Option<BindingWait>,
}

#[cfg(test)]
mod controlled_embed_tests {
    use super::*;

    #[test]
    fn controlled_embed_settings_have_no_browser_overrides_by_default() {
        let settings = ControlledEmbedSettings::default();
        assert_eq!(settings.user_agent, None);
        assert_eq!(settings.viewport, None);
    }

    #[test]
    fn controlled_embed_user_agent_labels_use_fixed_strings() {
        assert_eq!(ControlledEmbedUserAgent::Default.label(), "default");
        assert_eq!(
            ControlledEmbedUserAgent::MobileSafari.label(),
            "mobile_safari"
        );
        assert_eq!(
            ControlledEmbedUserAgent::Custom("UA".into()).label(),
            "custom"
        );
    }

    #[test]
    fn controlled_embed_viewport_presets_use_fixed_dimensions() {
        assert_eq!(
            ControlledEmbedViewportPreset::Mobile.dimensions(),
            (390, 844)
        );
        assert_eq!(
            ControlledEmbedViewportPreset::Tablet.dimensions(),
            (768, 1024)
        );
        assert_eq!(
            ControlledEmbedViewportPreset::Desktop.dimensions(),
            (1366, 768)
        );
    }

    #[test]
    fn controlled_embed_viewport_constants_use_fixed_bounds_and_dimensions() {
        assert_eq!(CONTROLLED_EMBED_MOBILE_VIEWPORT, (390, 844));
        assert_eq!(CONTROLLED_EMBED_TABLET_VIEWPORT, (768, 1024));
        assert_eq!(CONTROLLED_EMBED_DESKTOP_VIEWPORT, (1366, 768));
        assert_eq!(CONTROLLED_EMBED_VIEWPORT_MIN, 1);
        assert_eq!(CONTROLLED_EMBED_VIEWPORT_MAX, 4096);
    }

    #[test]
    fn controlled_embed_viewport_label_distinguishes_preset_from_custom() {
        assert_eq!(
            ControlledEmbedViewport::Preset(ControlledEmbedViewportPreset::Mobile).label(),
            "mobile"
        );
        assert_eq!(
            ControlledEmbedViewport::Custom {
                width: 320,
                height: 640,
            }
            .label(),
            "custom"
        );
    }

    #[test]
    fn controlled_embed_trigger_action_defaults_to_click() {
        assert_eq!(
            ControlledEmbedTriggerAction::default(),
            ControlledEmbedTriggerAction::Click
        );
    }

    #[test]
    fn controlled_embed_trigger_action_parses_click_and_enter() {
        assert_eq!(
            ControlledEmbedTriggerAction::parse("click"),
            Some(ControlledEmbedTriggerAction::Click)
        );
        assert_eq!(
            ControlledEmbedTriggerAction::parse("enter"),
            Some(ControlledEmbedTriggerAction::Enter)
        );
    }

    #[test]
    fn controlled_embed_trigger_action_returns_none_for_unknown_labels() {
        assert_eq!(ControlledEmbedTriggerAction::parse("submit"), None);
        assert_eq!(ControlledEmbedTriggerAction::parse(""), None);
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct PinColorHex(String);

impl PinColorHex {
    pub fn parse(s: &str) -> Result<Self, PinColorError> {
        if !s.starts_with(PIN_COLOR_PREFIX) {
            return Err(PinColorError::MissingHash);
        }
        if s.len() != PIN_COLOR_HEX_LEN {
            return Err(PinColorError::InvalidLength { actual: s.len() });
        }

        let mut normalized = String::with_capacity(PIN_COLOR_HEX_LEN);
        normalized.push(PIN_COLOR_PREFIX);
        for (index, ch) in s[1..].chars().enumerate() {
            if !ch.is_ascii_hexdigit() {
                return Err(PinColorError::InvalidDigit {
                    index: index + 1,
                    digit: ch,
                });
            }
            normalized.push(ch.to_ascii_uppercase());
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PinColorHex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PinColorHex {
    type Err = PinColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(feature = "serde")]
impl Serialize for PinColorHex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for PinColorHex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum PinColorError {
    #[error("pin color must start with #")]
    MissingHash,
    #[error("pin color must be #RRGGBB (7 bytes), got {actual}")]
    InvalidLength { actual: usize },
    #[error("pin color contains non-hex digit `{digit}` at byte {index}")]
    InvalidDigit { index: usize, digit: char },
}

// ─── PegboardUnits ───────────────────────────────────────────────
// Shared compact-cell footprint for pegboard surfaces. Desktop/PWA map this
// directly to CSS grid spans; TUI maps the same units to character-cell rects.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PegboardUnits {
    /// One base cell: 1 column × 1 row.
    U1,
    /// Two columns × 1 row.
    U2,
    /// One column × 2 rows.
    U2T,
}

impl PegboardUnits {
    pub const fn label(self) -> &'static str {
        match self {
            Self::U1 => "U1",
            Self::U2 => "U2",
            Self::U2T => "U2T",
        }
    }

    pub const fn grid_span(self) -> (u16, u16) {
        match self {
            Self::U1 => (1, 1),
            Self::U2 => (2, 1),
            Self::U2T => (1, 2),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "u1" => Self::U1,
            "u2" => Self::U2,
            "u2t" => Self::U2T,
            _ => return None,
        })
    }
}

// ─── Pegboard placement grid ────────────────────────────────────
// Shared coordinate grid for user pegboard arrangements. Storage is
// canonical: the same (x, y) means the same cell on every surface.
// Per-surface code maps a cell to its native size (TUI chars / Desktop
// px); the column count is invariant.

/// Number of columns in the canonical pegboard grid.
pub const BOARD_COLS: u16 = 6;

/// User-controlled placement of a tool on a board. Position is owned by
/// the user; size comes from the tool's manifest (`pegboard_units`)
/// unless the user set a per-pin [`PinSpan`] override in `span`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Placement {
    pub tool_id: String,
    pub x: u16,
    pub y: u16,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub color: Option<PinColorHex>,
    /// User-set size override. `None` keeps the manifest footprint.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub span: Option<PinSpan>,
    /// User-saved argument preset applied when invoking from this pin.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub args_preset: Option<ArgsPreset>,
}

impl Placement {
    pub fn new(tool_id: impl Into<String>, x: u16, y: u16) -> Self {
        Self {
            tool_id: tool_id.into(),
            x,
            y,
            color: None,
            span: None,
            args_preset: None,
        }
    }

    pub fn with_color(mut self, color: Option<PinColorHex>) -> Self {
        self.color = color;
        self
    }

    pub fn with_span(mut self, span: Option<PinSpan>) -> Self {
        self.span = span;
        self
    }

    pub fn with_args_preset(mut self, args_preset: Option<ArgsPreset>) -> Self {
        self.args_preset = args_preset;
        self
    }
}

// ─── Invoker ───────────────────────────────────────────────────
// Lexicon §4 + Lexicon §7 (Invoker::Embed vs PinKind::Embed):
// the *call mechanism*, orthogonal to how the cell is rendered.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Invoker {
    Function, // pure Rust function in this binary
    External, // external binary (subprocess)
    Http,     // remote HTTP call
    /// No invocation — the tool is self-presenting (e.g. a Passive
    /// Embed renders its webview, and the webview IS the tool). Listed
    /// in the catalogue, dispatch is a no-op. Pairs with
    /// [`PinKind::Embed`] only.
    Static,
    /// WebView selector adapter — writes form values into DOM via
    /// CSS selectors. Pairs with [`PinKind::ControlledEmbed`] only.
    Embed,
    Chain, // declarative composition of other tools
    Llm,   // LLM prompt/pattern invocation
    Wasm,  // upeg WASM plugin invocation
}

impl Invoker {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::External => "external",
            Self::Http => "http",
            Self::Static => "static",
            Self::Embed => "embed",
            Self::Chain => "chain",
            Self::Llm => "llm",
            Self::Wasm => "wasm",
        }
    }

    /// Parse an invoker label. Case-insensitive — accepts both the
    /// lowercase form from [`Invoker::label`] and the `PascalCase` form
    /// TOML/WASM authors typically write. Padded values (`"External "`)
    /// are rejected; manifests must be canonical.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "function" => Self::Function,
            "external" => Self::External,
            "http" => Self::Http,
            "static" => Self::Static,
            "embed" => Self::Embed,
            "chain" => Self::Chain,
            "llm" => Self::Llm,
            "wasm" => Self::Wasm,
            _ => return None,
        })
    }
}

// ─── Surface ───────────────────────────────────────────────────
// PRD v2.1 §6.2: the seven surfaces a Tool can be exposed on.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Surface {
    Cli,
    Tui,
    Desktop,
    Pwa,
    Ext,
    Mcp,
    Http,
}

impl Surface {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Tui => "tui",
            Self::Desktop => "desktop",
            Self::Pwa => "pwa",
            Self::Ext => "ext",
            Self::Mcp => "mcp",
            Self::Http => "http",
        }
    }

    /// Parse a surface label. Case-insensitive. Padded values are rejected.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "cli" => Self::Cli,
            "tui" => Self::Tui,
            "desktop" => Self::Desktop,
            "pwa" => Self::Pwa,
            "ext" => Self::Ext,
            "mcp" => Self::Mcp,
            "http" => Self::Http,
            _ => return None,
        })
    }

    /// Returns `true` for surfaces that support headless dispatch (CLI, TUI, MCP, HTTP).
    /// These surfaces can dispatch tools without a GUI surface.
    pub const fn supports_headless_dispatch(self) -> bool {
        matches!(self, Self::Cli | Self::Tui | Self::Mcp | Self::Http)
    }

    /// Returns `true` for surfaces that require a GUI to dispatch tools
    /// (Desktop, PWA, Chrome Extension).
    pub const fn requires_gui_surface(self) -> bool {
        matches!(self, Self::Desktop | Self::Pwa | Self::Ext)
    }
}

/// All seven surfaces — the default for a typical pure-function Tool.
pub const ALL_SURFACES: &[Surface] = &[
    Surface::Cli,
    Surface::Tui,
    Surface::Desktop,
    Surface::Pwa,
    Surface::Ext,
    Surface::Mcp,
    Surface::Http,
];

/// GUI surfaces only — local visual surfaces without headless CLI/MCP/HTTP
/// dispatch contracts.
pub const GUI_SURFACES: &[Surface] = &[Surface::Desktop, Surface::Pwa, Surface::Ext];

/// GUI surfaces that can host an Embed Tool (PRD §4.3 LIMIT: PWA Embed =
/// bookmark only; daemon required for desktop/ext).
pub const EMBED_SURFACES: &[Surface] = GUI_SURFACES;

// ─── Closed I/O type system ────────────────────────────────────
// PRD v2.1 §5.6: manifests may declare only the cross-surface types all
// renderers know how to display/serialize. JSON Schema is still the external
// shape, but these labels are the closed domain vocabulary behind it.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IoType {
    String,
    Number,
    Boolean,
    Options,
    MultiOptions,
    Markdown,
    Json,
    Datetime,
    FilePath,
    Url,
    File,
    EmbeddedView,
}

impl IoType {
    pub const fn label(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Options => "options",
            Self::MultiOptions => "multi_options",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::Datetime => "datetime",
            Self::FilePath => "file_path",
            Self::Url => "url",
            Self::File => "file",
            Self::EmbeddedView => "embedded_view",
        }
    }

    /// Parse a manifest I/O type label. Case-insensitive. `integer` is
    /// accepted as a JSON Schema spelling of PRD's `number` so existing
    /// schema authors can keep integer constraints without escaping the
    /// closed vocabulary.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "string" => Self::String,
            "number" | "integer" => Self::Number,
            "boolean" => Self::Boolean,
            "options" => Self::Options,
            "multi_options" => Self::MultiOptions,
            "markdown" => Self::Markdown,
            "json" => Self::Json,
            "datetime" => Self::Datetime,
            "file_path" => Self::FilePath,
            "url" => Self::Url,
            "file" => Self::File,
            "embedded_view" => Self::EmbeddedView,
            _ => return None,
        })
    }
}

pub const CLOSED_IO_TYPES: &[&str] = &[
    IoType::String.label(),
    IoType::Number.label(),
    "integer",
    IoType::Boolean.label(),
    IoType::Options.label(),
    IoType::MultiOptions.label(),
    IoType::Markdown.label(),
    IoType::Json.label(),
    IoType::Datetime.label(),
    IoType::FilePath.label(),
    IoType::Url.label(),
    IoType::File.label(),
    IoType::EmbeddedView.label(),
];

/// Comma-separated form of [`CLOSED_IO_TYPES`] used in error messages.
/// Single source of truth = [`IoType`]'s `label()` outputs plus the
/// JSON Schema alias `"integer"` for `Number`.
pub const IO_TYPE_LIST: &str = "string, number, integer, boolean, options, multi_options, markdown, json, datetime, file_path, url, file, embedded_view";

#[cfg(test)]
mod pin_color_tests {
    use super::*;

    const LOWERCASE_COLOR: &str = "#a1b2c3";
    const UPPERCASE_COLOR: &str = "#A1B2C3";

    #[test]
    fn pin_color_normalizes_valid_hex_to_uppercase() {
        let color = PinColorHex::parse(LOWERCASE_COLOR).expect("valid color");

        assert_eq!(color.as_str(), UPPERCASE_COLOR);
        assert_eq!(color.to_string(), UPPERCASE_COLOR);
    }

    #[test]
    fn pin_color_rejects_short_bad_digit_or_missing_hash() {
        const INVALID_COLORS: &[&str] = &["#12345", "#12GG45", "123456"];

        for candidate in INVALID_COLORS {
            assert!(
                PinColorHex::parse(candidate).is_err(),
                "{candidate} must be rejected",
            );
        }
    }
}

#[cfg(test)]
mod io_type_tests {
    use super::*;

    #[test]
    fn io_type_labels_parse_roundtrip() {
        for ty in [
            IoType::String,
            IoType::Number,
            IoType::Boolean,
            IoType::Options,
            IoType::MultiOptions,
            IoType::Markdown,
            IoType::Json,
            IoType::Datetime,
            IoType::FilePath,
            IoType::Url,
            IoType::File,
            IoType::EmbeddedView,
        ] {
            let label = ty.label();
            assert_eq!(
                IoType::parse(label),
                Some(ty),
                "label `{label}` must roundtrip through IoType::parse",
            );
        }
    }

    #[test]
    fn io_type_file_and_embedded_view_labels_are_distinct() {
        assert_eq!(IoType::File.label(), "file");
        assert_eq!(IoType::EmbeddedView.label(), "embedded_view");
    }

    #[test]
    fn io_type_parse_is_case_insensitive() {
        assert_eq!(IoType::parse("FILE"), Some(IoType::File));
        assert_eq!(IoType::parse("Embedded_View"), Some(IoType::EmbeddedView));
    }

    #[test]
    fn io_type_parse_returns_none_for_unknown_values() {
        assert_eq!(IoType::parse("blob"), None);
        assert_eq!(IoType::parse(""), None);
    }

    #[test]
    fn closed_io_types_include_the_new_variants() {
        assert!(CLOSED_IO_TYPES.contains(&"file"));
        assert!(CLOSED_IO_TYPES.contains(&"embedded_view"));
    }

    #[test]
    fn io_type_list_string_includes_the_new_variants() {
        assert!(IO_TYPE_LIST.contains("file"));
        assert!(IO_TYPE_LIST.contains("embedded_view"));
    }
}

#[cfg(test)]
mod surface_tests {
    use super::*;

    #[test]
    fn surface_headless_dispatch_support_count_is_four() {
        let headless_count = ALL_SURFACES
            .iter()
            .filter(|s| s.supports_headless_dispatch())
            .count();
        assert_eq!(
            headless_count, 4,
            "CLI, TUI, MCP, HTTP support headless dispatch"
        );
    }

    #[test]
    fn surface_gui_requirement_count_is_three() {
        let gui_count = ALL_SURFACES
            .iter()
            .filter(|s| s.requires_gui_surface())
            .count();
        assert_eq!(gui_count, 3, "Desktop, PWA, Ext require a GUI surface");
    }

    #[test]
    fn surface_headless_and_gui_are_mutually_exclusive() {
        for s in ALL_SURFACES {
            assert!(
                !(s.supports_headless_dispatch() && s.requires_gui_surface()),
                "{s:?} surface cannot support both headless and GUI"
            );
        }
    }

    #[test]
    fn gui_surfaces_do_not_support_headless_dispatch() {
        for s in GUI_SURFACES {
            assert!(
                !s.supports_headless_dispatch(),
                "{s:?} surface is GUI-only so it cannot dispatch headless"
            );
        }
    }

    #[test]
    fn embed_surfaces_do_not_support_headless_dispatch() {
        for s in EMBED_SURFACES {
            assert!(
                !s.supports_headless_dispatch(),
                "{s:?} surface is embed-only so it cannot dispatch headless"
            );
        }
    }

    #[test]
    fn cli_surface_supports_headless_dispatch() {
        assert!(Surface::Cli.supports_headless_dispatch());
        assert!(!Surface::Cli.requires_gui_surface());
    }

    #[test]
    fn tui_surface_supports_headless_dispatch() {
        assert!(Surface::Tui.supports_headless_dispatch());
        assert!(!Surface::Tui.requires_gui_surface());
    }

    #[test]
    fn mcp_surface_supports_headless_dispatch() {
        assert!(Surface::Mcp.supports_headless_dispatch());
        assert!(!Surface::Mcp.requires_gui_surface());
    }

    #[test]
    fn http_surface_supports_headless_dispatch() {
        assert!(Surface::Http.supports_headless_dispatch());
        assert!(!Surface::Http.requires_gui_surface());
    }

    #[test]
    fn desktop_surface_supports_gui_only() {
        assert!(!Surface::Desktop.supports_headless_dispatch());
        assert!(Surface::Desktop.requires_gui_surface());
    }

    #[test]
    fn pwa_surface_supports_gui_only() {
        assert!(!Surface::Pwa.supports_headless_dispatch());
        assert!(Surface::Pwa.requires_gui_surface());
    }

    #[test]
    fn ext_surface_supports_gui_only() {
        assert!(!Surface::Ext.supports_headless_dispatch());
        assert!(Surface::Ext.requires_gui_surface());
    }
}
