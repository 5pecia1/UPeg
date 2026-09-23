//! Declarative TOML loader for upeg Toolkits (PRD v2.1 §5.1, §2.3 #8).
//!
//! Lets users register Tools without writing Rust — the *zero-lines-of-code migration*
//! pillar. A TOML file like
//!
//! ```toml
//! id = "convert"
//! tags = ["pure", "text"]
//!
//! [[tools]]
//! id = "url_encode"
//! pin = "Inline"
//! pegboard_units = "U1"
//! invoker = "External"
//! surfaces = ["cli", "tui", "http"]
//! boards = ["dev"]
//! ```
//!
//! lowers into the same `ToolMeta` shape that `#[upeg::tool]` produces, and
//! joins the global registry through the paired metadata+dispatcher runtime
//! registration path.
//!
//! Execution semantics for declarative `External`, `Http`, `Chain`, `Llm`,
//! and `Wasm` invokers are registered by the loader. GUI `Embed` metadata is
//! registered as a sidecar for surfaces that render WebView/iframe surfaces.

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

mod dispatcher;
mod error;
mod error_metadata;
mod execution_requirements;
mod invoker_metadata;
mod loader;
mod manifest_docs;
mod manifest_examples;
mod manifest_origin;
mod model;
mod parse;
mod schema;

pub use dispatcher::{BOARD_ENV, PROJECT_MANIFEST_ENV, normalize_controlled_embed_result};
pub use error::LoadError;
pub use loader::{LoadOutcome, load_and_register_dir_verbose, load_and_register_file_verbose};
#[cfg(test)]
pub(crate) use loader::{load_and_register_dir, load_dir};
pub use manifest_docs::toolkit_manifest_docs_markdown;
pub use model::{
    ChainConnectionToml, ChainStepToml, CredentialRefToml, InputChoiceToml, InputDefaultToml,
    InputFieldToml, KeyValueToml, OutputFieldToml, SelectorBindingToml, ToolEntryToml,
    ToolSetupInstallToml, ToolSetupToml, ToolToml, ToolkitToml, TriggerToml,
};
pub use parse::{SkippedTool, ToolkitParse, parse_toolkit_full, parse_toolkit_with_skips};
pub use schema::{toolkit_schema_json, toolkit_schema_value};

#[cfg(test)]
mod tests;
