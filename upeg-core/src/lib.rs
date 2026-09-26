//! Core domain types for upeg.
//!
//! Names follow `docs/LEXICON.md` §4 — see that document before renaming or
//! adding new variants. PRD v2.1 §5.1 calls for a *single Tool abstraction*
//! whose manifest (`ToolMeta`) any surface (CLI/TUI/Desktop/PWA/Ext/MCP/HTTP)
//! consumes uniformly. This crate holds only the pure-data layer — UI
//! rendering, IPC, and runtime registries live in surface-specific crates.
//! The closed I/O type set lives in [`input`] module docs.

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

mod args_preset;
mod board_guidance;
mod board_key;
mod builtin_boards;
pub mod capability;
pub mod i18n;
mod identity;
pub mod input;
pub mod interface_inventory;
pub mod keyboard;
pub mod keyboard_catalog;
mod manifest;
mod manifest_validate;
pub mod output;
pub mod paths;
mod pin_span;
pub mod prefs;
pub mod presentation;
mod principal;
mod project_board;
pub mod search;
pub mod source;
mod toolbox;
mod types;
pub mod ux;

pub use args_preset::{ArgsPreset, ArgsPresetError};
pub use board_guidance::BoardGuidance;
pub use board_key::{BoardKey, BoardKeyError, PROJECT_BOARD_KEY_SEPARATOR};
pub use builtin_boards::{BUILTIN_BOARDS, BuiltinBoard, is_builtin_board};
pub use identity::{ToolId, ToolIdError, ToolIdentity, ToolKey};
pub use input::{
    ChoiceOption, ChoiceSpec, DEFAULT_FILE_MAX_COUNT, DraftInputValue, FieldConstraints,
    FieldValidation, FileContent, FileInputPolicy, FileInputPolicyError, FileInputPolicyParams,
    FileInputValueError, FileOutputPreflightError, FilePolicySchemaError, FileValue,
    FormFieldState, FormState, InputAdapterError, InputFieldSpec, InputKind, InputName, InputSpec,
    InputSpecError, InputValue, InputValueError, MAX_FILE_INPUT_COUNT,
    MAX_FILE_INPUT_METADATA_BYTES, MAX_FILE_INPUT_NODES, MAX_FILE_INPUT_RAW_BYTES,
    MAX_FILE_OUTPUT_METADATA_BYTES, MAX_FILE_OUTPUT_NESTING_DEPTH, MAX_FILE_OUTPUT_NODES,
    MAX_FILE_OUTPUT_RAW_BYTES, MAX_UNTRUSTED_OUTPUT_WIRE_BYTES, NumberConstraints,
    StaticChoiceOption, StaticFieldConstraints, StaticFileInputPolicy, StaticInputFieldSpec,
    StaticInputKind, StaticInputSpec, StaticNumberConstraints, StaticStringConstraints,
    StringConstraints, preflight_file_output_json, validate_file_output_tree,
};
pub use interface_inventory::{
    Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
    INTERFACE_INVENTORY_SCHEMA_VERSION, InterfaceEntry, InterfaceInventory,
    InterfaceInventoryError, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
};
pub use keyboard::{
    BoardSlot, Key, KeyModifiers, KeyStroke, KeyboardCommand, KeyboardContext, KeyboardScope,
    NavDirection, OrderDirection, PageDirection, key_from_label, key_stroke_from_label,
    resolve_key,
};
pub use keyboard_catalog::{
    ALL_KEYBOARD_SCOPES, BindingEntry, CatalogBinding, CatalogChord, ChordModifier, ScopeBindings,
    binding_catalog, scope_label_key,
};
pub use manifest::{
    ALL_SURFACES_EXCEPT_MCP, BoardExecutionContext, EXECUTION_CONTEXT_APPROVED_STEPS,
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_CWD, EXECUTION_CONTEXT_PRINCIPAL,
    EXECUTION_CONTEXT_SURFACE, PrimaryOutputIdError, StaticToolMeta, ToolMeta, ToolkitMeta,
    validate_primary_output_id,
};
pub use manifest_validate::{
    EmbedPairingError, validate_embed_binding_shape, validate_embed_pairing,
    validate_pin_invoker_pairing,
};
pub use output::{
    OutputEntry, OutputFieldSpec, OutputKind, OutputSpec, OutputSpecError, OutputValue,
    ProcessErrorDetails, ProcessErrorStreams, ProcessTermination, StaticOutputFieldSpec,
    StaticOutputKind, StaticOutputSpec, ToolError, ToolFailure, ToolResult, ToolResultError,
    ToolSuccess,
};
pub use pin_span::{ColSpan, PinSpan, PinSpanError, RowSpan};
pub use presentation::{
    ActionBinding, ActionScope, ActionSuccess, BindingResolution, PRESENTATION_VERSION_V1,
    PresentationAction, PresentationColumn, PresentationRow, RowsResolution, ToolEffect,
    ToolPresentation, resolve_bindings, resolve_rows, validate_json_pointer,
};
pub use principal::{
    ALL_PRINCIPAL_ROLES, PRINCIPAL_ROLE_KEY, PRINCIPAL_SURFACE_KEY, Principal, PrincipalRole,
};
pub use project_board::{
    BoardStoreKey, PROJECT_BOARD_KEY_PREFIX, ProjectBoardNamespace, StoredBoardKey,
};
pub use search::{
    PinnedSignal, RecentSignal, SearchField, SearchMatchTier, SearchQuery, SearchResult,
    SearchSignals, search_tools,
};
pub use source::{Source, StaticSource};
pub use toolbox::inventory;
pub use types::{
    ALL_SURFACES, BOARD_COLS, BindingRole, BindingWait, BindingWaitCondition, BindingWaitOnTimeout,
    CLOSED_IO_TYPES, CONTROLLED_EMBED_DESKTOP_VIEWPORT, CONTROLLED_EMBED_MOBILE_VIEWPORT,
    CONTROLLED_EMBED_TABLET_VIEWPORT, CONTROLLED_EMBED_VIEWPORT_MAX, CONTROLLED_EMBED_VIEWPORT_MIN,
    ControlledEmbedSettings, ControlledEmbedTriggerAction, ControlledEmbedUserAgent,
    ControlledEmbedViewport, ControlledEmbedViewportPreset, DEFAULT_CONTROLLED_EMBED_WAIT_POLL_MS,
    DEFAULT_CONTROLLED_EMBED_WAIT_SETTLE_MS, DEFAULT_CONTROLLED_EMBED_WAIT_TIMEOUT_MS,
    EMBED_SURFACES, GUI_SURFACES, IO_TYPE_LIST, Invoker, IoType, MAX_CONTROLLED_EMBED_WAIT_MS,
    PegboardUnits, PinColorError, PinColorHex, PinKind, Placement, SelectorBinding, Surface,
};

pub use capability::{
    DispatchCapability, RuntimeHost, UnsupportedReason, dispatch_capability,
    dispatch_capability_for_tool,
};

/// Re-exported static registration proc-macros from `upeg-macros`.
pub use upeg_macros::{tool, toolkit};

#[cfg(test)]
mod manifest_tests;
