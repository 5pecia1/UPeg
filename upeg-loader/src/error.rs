use crate::dispatcher::{DEFAULT_APPROVAL_SURFACES, surface_label_list};
use thiserror::Error;
use upeg_core::{ALL_SURFACES, InputSpecError, OutputSpecError, PrimaryOutputIdError, ToolIdError};
use upeg_runtime::manifest::{CollisionError, ManifestError};
use upeg_runtime::{ScheduleConditionError, TRIGGER_SOURCES};

/// Separator between wire strings when a message spells out the valid trigger
/// sources. The list itself is derived from [`TRIGGER_SOURCES`] so the error
/// text cannot drift from the enum.
const TRIGGER_SOURCE_SEPARATOR: &str = "/";

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("invalid result presentation: {0}")]
    InvalidPresentation(String),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unknown pin `{0}` (Inline/Launcher/Live/Action/Embed/Chain/Llm)")]
    UnknownPinKind(String),

    #[error("unknown invoker `{0}` (Function/External/Http/Embed/Chain/Llm/Wasm)")]
    UnknownInvoker(String),

    #[error("unknown surface `{0}` (cli/tui/desktop/pwa/ext/mcp/http)")]
    UnknownSurface(String),

    #[error("unknown trigger source `{0}` ({valid})", valid = TRIGGER_SOURCES.join(TRIGGER_SOURCE_SEPARATOR))]
    UnknownTriggerSource(String),

    #[error("`triggers[{position}].condition` is not a valid schedule expression — {error}")]
    InvalidScheduleCondition {
        position: usize,
        #[source]
        error: ScheduleConditionError,
    },

    // `trigger_source` rather than `source`: thiserror reserves a field named
    // `source` for the error cause chain, and this one is a wire string.
    #[error(
        "`triggers[{position}].condition` is required for `source = \"{trigger_source}\"` — expected {expectation}"
    )]
    MissingTriggerCondition {
        position: usize,
        trigger_source: &'static str,
        expectation: &'static str,
    },

    #[error(
        "`triggers[{position}].condition` is not accepted for `source = \"{trigger_source}\"` — this source carries no condition"
    )]
    UnexpectedTriggerCondition {
        position: usize,
        trigger_source: &'static str,
    },

    #[error("`{field}` is retired on {scope} manifests — use `{replacement}` instead")]
    RetiredField {
        field: String,
        scope: &'static str,
        replacement: &'static str,
    },

    #[error("`steps` must list at least one chain step")]
    EmptyChain,

    #[error("`steps[{position}]` is empty — every chain step must be a non-empty tool id")]
    EmptyChainStep { position: usize },

    #[error("`steps[{position}].id = \"{id}\"` duplicates another chain node id")]
    DuplicateChainStepId { position: usize, id: String },

    #[error(
        "`connections[{position}].{field}` is empty — every chain connection endpoint must name a node id"
    )]
    EmptyChainConnection {
        position: usize,
        field: &'static str,
    },

    #[error("`connections[{position}]` references unknown chain node `{step}`")]
    UnknownChainConnectionStep { position: usize, step: String },

    #[error(
        "`connections` must be acyclic; model repeated execution with an explicit loop-capable tool instead of a silent cycle"
    )]
    ChainConnectionCycle,

    #[error(
        "`approval_surfaces` is empty — a chain that authorizes no surface can never satisfy a `requires_approval` step; drop the field to accept the default ({default}) or name the surfaces that may approve",
        default = surface_label_list(DEFAULT_APPROVAL_SURFACES)
    )]
    EmptyApprovalSurfaces,

    #[error(
        "`approval_surfaces[{position}]` is empty — every entry must name a surface ({valid})",
        valid = surface_label_list(ALL_SURFACES)
    )]
    EmptyInApprovalSurfaces { position: usize },

    #[error(
        "`approval_surfaces[{position}] = \"{surface}\"` is not a surface ({valid})",
        valid = surface_label_list(ALL_SURFACES)
    )]
    UnknownApprovalSurface { position: usize, surface: String },

    #[error(
        "`approval_surfaces` only means something for `invoker = \"Chain\"` — it authorizes the surfaces that may satisfy a chain step's `requires_approval` barrier"
    )]
    ApprovalSurfacesWithoutChain,

    #[error(
        "`approval_surfaces` ({approval}) names no surface this tool is exposed on ({exposed}) — a `requires_approval` step could never be approved, because no call can arrive from a surface that may approve it; put one of {approval} in `surfaces`, or approve from a surface the tool is on"
    )]
    UnreachableApprovalSurfaces { approval: String, exposed: String },

    #[error(
        "`outputs[{position}].name = \"{name}\"` is reserved on a Chain tool — every chain result carries an engine-owned `{name}` row listing each step's status; rename the declared output"
    )]
    ReservedChainOutputName { position: usize, name: &'static str },

    #[error("`id` must be a non-empty tool identifier")]
    EmptyId,

    #[error("`toolkit` must be a non-empty Toolkit id")]
    EmptyToolkit,

    #[error("`tools` must contain at least one `[[tools]]` entry for the Toolkit")]
    EmptyToolkitTools,

    #[error(
        "`[[tools]].id = \"{id}\"` must be local to the Toolkit (for example `hex_to_decimal`, not `num.hex_to_decimal`)"
    )]
    ToolIdContainsToolkit { id: String },

    #[error("`id = \"{id}\"` must start with its owning Toolkit prefix `{toolkit}.`")]
    ToolkitMismatch { id: String, toolkit: String },

    #[error("`id = \"{0}\"` must be canonical and unpadded")]
    NonCanonicalId(String),

    #[error("`toolkit = \"{0}\"` must be canonical and unpadded")]
    NonCanonicalToolkit(String),

    #[error("`boards[{position}].id = \"{board}\"` is not a valid board id — {error}")]
    InvalidProjectBoardId {
        position: usize,
        board: String,
        #[source]
        error: upeg_core::BoardKeyError,
    },

    #[error(
        "`boards[{position}].id = \"{board}\"` shadows a built-in board — pick a project-specific id"
    )]
    BuiltinProjectBoardId { position: usize, board: String },

    #[error("`boards[{position}].id = \"{board}\"` is declared twice")]
    DuplicateProjectBoardId { position: usize, board: String },

    #[error(
        "top-level `[[boards]]` is only accepted in a Project Manifest (`upeg.toml`); a toolkits-directory manifest has no project to scope a board to"
    )]
    BoardsOutsideProjectManifest,

    #[error("`tags[{position}]` is empty — every tag must be a non-empty discovery key")]
    EmptyTag { position: usize },

    #[error("`tags[{position}] = \"{tag}\"` must be canonical and unpadded")]
    NonCanonicalTag { position: usize, tag: String },

    #[error(
        "`invoker` is empty — set a non-empty runtime invoker, or omit it only when `steps` is present to infer `Chain`"
    )]
    EmptyInvoker,

    #[error(
        "`invoker` is required for runtime Toolkit tools unless `steps` is present to infer `Chain`; there is no implicit `Function` dispatcher for TOML"
    )]
    MissingInvoker,

    #[error(
        "`pin` is empty — omit the field entirely to default to `Inline`, or set a non-empty value"
    )]
    EmptyPinKind,

    #[error("`pegboard_units` is required — set one of U1, U2, or U2T")]
    MissingPegboardUnits,

    #[error("`pegboard_units` is empty — set one of U1, U2, or U2T")]
    EmptyPegboardUnits,

    #[error("unknown pegboard_units `{0}` (U1/U2/U2T)")]
    UnknownPegboardUnits(String),

    #[error("`boards[{position}]` is empty — every entry must be a non-empty board key")]
    EmptyBoard { position: usize },

    #[error("`boards[{position}] = \"{board}\"` must be canonical and unpadded")]
    NonCanonicalBoard { position: usize, board: String },

    #[error(
        "`controlled_embed.{field}` moved under `controlled_embed.browser.{field}` — use `controlled_embed.browser` for browser settings"
    )]
    DeprecatedControlledEmbedBrowserField { field: &'static str },

    #[error(
        "cannot mix `controlled_embed.browser` with old `controlled_embed.{field}` — use only `controlled_embed.browser.{field}`"
    )]
    ConflictingControlledEmbedBrowser { field: &'static str },

    #[error(
        "`controlled_embed.bindings[{position}].{missing}` is empty — Input/Output roles \
         need a `field`, all roles need a `selector`"
    )]
    EmptySelectorBinding {
        position: usize,
        missing: &'static str,
    },

    #[error(
        "`controlled_embed.bindings[{position}].role = \"{role}\"` is unknown — \
         use \"input\", \"trigger\", or \"output\""
    )]
    UnknownBindingRole { position: usize, role: String },

    #[error(
        "`controlled_embed.bindings[{position}].action = \"{action}\"` is unknown — use \"click\" or \"enter\""
    )]
    UnknownSelectorBindingAction { position: usize, action: String },

    #[error(
        "`controlled_embed.bindings[{position}].action = \"{action}\"` is only valid on trigger bindings; role `{role}` may use only \"click\""
    )]
    NonTriggerSelectorBindingAction {
        position: usize,
        role: String,
        action: String,
    },

    #[error(
        "`controlled_embed.bindings[{position}].wait.condition = \"{condition}\"` is unknown — use \"exists\" or \"visible\""
    )]
    InvalidBindingWaitCondition { position: usize, condition: String },

    #[error(
        "`controlled_embed.bindings[{position}].wait.on_timeout = \"{on_timeout}\"` is unknown — use \"fail\" or \"continue\""
    )]
    InvalidBindingWaitOnTimeout { position: usize, on_timeout: String },

    #[error(
        "`controlled_embed.bindings[{position}].wait.{field} = {value}` exceeds the allowed maximum {max}"
    )]
    OverMaxBindingWaitMs {
        position: usize,
        field: &'static str,
        value: u64,
        max: u64,
    },

    #[error(
        "`controlled_embed.bindings[{position}].wait.{key}` is unknown — use for_selector, condition, timeout_ms, settle_ms, or on_timeout"
    )]
    UnknownBindingWaitKey { position: usize, key: String },

    #[error(
        "`controlled_embed.bindings[{position}].wait.for_selector` must not be empty after trimming"
    )]
    InvalidBindingWait { position: usize },

    #[error(
        "`controlled_embed.browser.user_agent = \"{user_agent}\"` is unknown — use \"default\", \"mobile_safari\", or \"custom\""
    )]
    UnknownControlledEmbedUserAgent { user_agent: String },

    #[error(
        "`controlled_embed.browser.custom_user_agent` is required when `controlled_embed.browser.user_agent = \"custom\"`"
    )]
    MissingControlledEmbedCustomUserAgent,

    #[error("`controlled_embed.browser.custom_user_agent` must not be empty when trimmed")]
    EmptyControlledEmbedCustomUserAgent,

    #[error(
        "`controlled_embed.browser.custom_user_agent` is allowed only when `controlled_embed.browser.user_agent = \"custom\"`"
    )]
    UnexpectedControlledEmbedCustomUserAgent,

    #[error(
        "`controlled_embed.browser.viewport = \"{viewport}\"` is unknown — use \"mobile\", \"tablet\", \"desktop\", or \"custom\""
    )]
    UnknownControlledEmbedViewport { viewport: String },

    #[error(
        "`controlled_embed.browser.{field}` is required when `controlled_embed.browser.viewport = \"custom\"`"
    )]
    MissingControlledEmbedViewportDimension { field: &'static str },

    #[error(
        "`controlled_embed.browser.{field}` is allowed only when `controlled_embed.browser.viewport = \"custom\"`"
    )]
    UnexpectedControlledEmbedViewportDimension { field: &'static str },

    #[error(
        "`controlled_embed.browser.{field} = {value}` is outside the allowed range {min}..={max}"
    )]
    ControlledEmbedViewportDimensionOutOfRange {
        field: &'static str,
        value: u16,
        min: u16,
        max: u16,
    },

    #[error("`triggers[{position}].source` is empty — every trigger needs a source adapter label")]
    EmptyTriggerSource { position: usize },

    #[error("`credentials[{position}].{field}` is empty or required by its credential store")]
    EmptyCredentialField {
        position: usize,
        field: &'static str,
    },

    #[error("`credentials[{position}].store = \"{store}\"` is unknown — use `env` or `keychain`")]
    UnknownCredentialStore { position: usize, store: String },

    #[error(
        "`credentials[{position}].{field}` is forbidden — credential manifests store typed references only; put secret values in env or OS keychain"
    )]
    SecretField { position: usize, field: String },

    #[error(
        "`surfaces[{position}]` is empty — every surface must be one of cli/tui/desktop/pwa/ext/mcp/http"
    )]
    EmptyInSurfaces { position: usize },

    #[error("`inputs` is invalid: {detail}")]
    InvalidInputSpec { detail: String },

    #[error("`inputs` field `{name}` has unknown type `{kind}` (use one of: {expected})")]
    UnknownInputType {
        name: String,
        kind: String,
        expected: &'static str,
    },

    #[error(
        "`inputs` field `{name}` declares `options`, but `type = \"{kind}\"` is not `options` or `multi_options`"
    )]
    UnexpectedInputOptions { name: String, kind: String },

    #[error(
        "`inputs` field `{name}` declares `default`, but `type = \"{kind}\"` has no default slot (only number/integer and the text-shaped types do)"
    )]
    UnsupportedInputDefault { name: String, kind: String },

    #[error(
        "`inputs` field `{name}` declares a `default` of the wrong TOML type for `type = \"{kind}\"`"
    )]
    InvalidInputDefault { name: String, kind: String },

    #[error(
        "`inputs` field `{name}` declares `{field}`, but file policy is allowed only for `type = \"file\"`, not `type = \"{kind}\"`"
    )]
    UnexpectedInputFilePolicy {
        name: String,
        kind: String,
        field: &'static str,
    },

    #[error("`outputs` is invalid: {detail}")]
    InvalidOutputSpec { detail: String },

    #[error("`outputs` field `{name}` has unknown type `{kind}` (use one of: {expected})")]
    UnknownOutputType {
        name: String,
        kind: String,
        expected: &'static str,
    },

    #[error(
        "`outputs` field `{name}` declares `options`, but `type = \"{kind}\"` is not `options` or `multi_options`"
    )]
    UnexpectedOutputOptions { name: String, kind: String },

    #[error(
        "`outputs` field `{name}` declares `url`, but `type = \"{kind}\"` is not `embedded_view`"
    )]
    UnexpectedOutputUrl { name: String, kind: String },

    #[error("`outputs` field `{name}` with `type = \"embedded_view\"` requires a `url` field")]
    MissingEmbeddedViewUrl { name: String },

    #[error("`outputs` field `{name}` has an empty embedded view `url`")]
    EmptyEmbeddedViewUrl { name: String },

    #[error(
        "`outputs` field `{name}` embedded view `url = \"{url}\"` must be canonical and unpadded"
    )]
    NonCanonicalEmbeddedViewUrl { name: String, url: String },

    #[error(
        "`invoker = \"External\"` requires a non-empty `command` field naming the program to spawn (e.g. `command = \"git\"`)"
    )]
    ExternalRequiresCommand,

    #[error(
        "`args_template[{position}]` substitutes `{{{key}}}`, but no `inputs` field is named `{key}`"
    )]
    UnknownArgTemplateInput { position: usize, key: String },

    #[error(
        "`args_template[{position}]` contains an empty `{{}}` placeholder — name a declared `inputs` field"
    )]
    EmptyArgTemplatePlaceholder { position: usize },

    #[error("`invoker = \"{invoker}\"` requires a non-empty `{field}` field")]
    MissingInvokerField {
        invoker: &'static str,
        field: &'static str,
    },

    #[error(
        "`invoker = \"{invoker}\"` is only valid for link-time `#[tool]` functions; runtime Toolkit TOML must use External, Http, Embed, Chain, Llm, or Wasm"
    )]
    UnsupportedRuntimeInvoker { invoker: &'static str },

    #[error(
        "`id = \"{id}\"` declares `invoker = \"{invoker}\"`, but the loader has no dispatcher builder for that invoker"
    )]
    MissingDispatcher { id: String, invoker: String },

    #[error(transparent)]
    EmbedPairing(#[from] upeg_core::EmbedPairingError),

    #[error("`timeout_ms` must be greater than zero — omit it entirely for no time limit")]
    ZeroExternalTimeout,

    #[error("`env[{position}].name` is empty")]
    EmptyEnvName { position: usize },

    #[error("`color = \"{value}\"` is not a known color policy — expected one of: {allowed}")]
    UnknownExternalColor { value: String, allowed: String },

    #[error(
        "`pty = true` needs a host that can open a pseudoterminal — this build has none (Windows and wasm do not), so this tool is skipped and the rest of its manifest still loads. For a tool that must run here too, use `color = \"force\"` or the tool's own flag (`git -c color.ui=always`)"
    )]
    PtyUnsupportedOnHost,

    #[error(
        "`{field}` belongs to `invoker = \"{expected_invoker}\"`, but this tool declares `invoker = \"{invoker}\"`"
    )]
    InvokerFieldConflict {
        invoker: String,
        field: &'static str,
        expected_invoker: &'static str,
    },

    #[error(
        "`embed_url` is empty — omit the field entirely so the desktop UI shows its \"no URL configured\" empty-state, or set a real URL"
    )]
    EmptyEmbedUrl,

    #[error(
        "`id = \"{0}\"` shadows a built-in tool (link-time `#[upeg::tool]` inventory entry). Rename to a non-colliding id (e.g. `my.{0}`) — pre-iter-249 the runtime override silently broke `tool show` (showed built-in meta) and `dispatch_tool` (ran your dispatcher)"
    )]
    IdShadowsBuiltIn(String),
}

impl From<InputSpecError> for LoadError {
    fn from(error: InputSpecError) -> Self {
        Self::InvalidInputSpec {
            detail: error.to_string(),
        }
    }
}

impl From<OutputSpecError> for LoadError {
    fn from(error: OutputSpecError) -> Self {
        Self::InvalidOutputSpec {
            detail: error.to_string(),
        }
    }
}

impl From<PrimaryOutputIdError> for LoadError {
    fn from(error: PrimaryOutputIdError) -> Self {
        Self::InvalidOutputSpec {
            detail: error.to_string(),
        }
    }
}

impl From<ManifestError> for LoadError {
    fn from(error: ManifestError) -> Self {
        match error {
            ManifestError::ToolId {
                source,
                id,
                toolkit,
            } => load_error_for_tool_id(source, &id, &toolkit),
            ManifestError::Collision(CollisionError::ShadowsBuiltIn { id, .. }) => {
                Self::IdShadowsBuiltIn(id)
            }
            ManifestError::Collision(CollisionError::DuplicateTool { id, .. }) => {
                Self::IdShadowsBuiltIn(id)
            }
            ManifestError::EmptyTag { position } => Self::EmptyTag { position },
            ManifestError::NonCanonicalTag { position, tag } => {
                Self::NonCanonicalTag { position, tag }
            }
            ManifestError::EmptyDisplayLabel => Self::EmptyPinKind,
            ManifestError::EmptyPinKind => Self::EmptyPinKind,
            ManifestError::UnknownPinKind(kind) => Self::UnknownPinKind(kind),
            ManifestError::MissingPegboardUnits => Self::MissingPegboardUnits,
            ManifestError::EmptyPegboardUnits => Self::EmptyPegboardUnits,
            ManifestError::UnknownPegboardUnits(units) => Self::UnknownPegboardUnits(units),
            ManifestError::EmptyInvoker => Self::EmptyInvoker,
            ManifestError::MissingInvoker => Self::MissingInvoker,
            ManifestError::UnknownInvoker(invoker) => Self::UnknownInvoker(invoker),
            ManifestError::EmptyInSurfaces { position } => Self::EmptyInSurfaces { position },
            ManifestError::UnknownSurface(surface) => Self::UnknownSurface(surface),
            ManifestError::EmptyBoard { position } => Self::EmptyBoard { position },
            ManifestError::NonCanonicalBoard { position, board } => {
                Self::NonCanonicalBoard { position, board }
            }
            ManifestError::PrimaryOutputId(error) => error.into(),
        }
    }
}

fn load_error_for_tool_id(error: ToolIdError, id_value: &str, toolkit_value: &str) -> LoadError {
    match error {
        ToolIdError::EmptyId | ToolIdError::EmptyLocal => LoadError::EmptyId,
        ToolIdError::EmptyToolkit => LoadError::EmptyToolkit,
        ToolIdError::NonCanonicalId { id } => LoadError::NonCanonicalId(id),
        ToolIdError::NonCanonicalToolkit { toolkit } => LoadError::NonCanonicalToolkit(toolkit),
        ToolIdError::MissingSeparator => LoadError::ToolkitMismatch {
            id: id_value.to_string(),
            toolkit: toolkit_value.to_string(),
        },
        ToolIdError::ToolkitMismatch { id, toolkit } => LoadError::ToolkitMismatch { id, toolkit },
    }
}
