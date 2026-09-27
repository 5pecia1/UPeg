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
//!
//! # Manifest contract (`model`)
//!
//! One manifest defines one non-callable Toolkit and one or more
//! callable Tools. `[[tools]].id` is a local id — repeating the
//! `{toolkit}.` prefix is rejected. `pegboard_units` (`U1`/`U2`/`U2T`)
//! is required and shared by every pegboard surface. Inputs/outputs use
//! only the closed `upeg_core` I/O type set. `category` is rejected —
//! use tags.
//!
//! | Invoker | Required fields | Rules |
//! |---|---|---|
//! | `External` | `command` | optional `args_template`/`cwd`/`env`/`timeout_ms`/`color`/`pty`/`setup`; credential values reach the child as env at spawn only |
//! | `Http` | `url` | header/body templates reference credential *names* only; the built-in adapter covers `http://` and `mock://echo` — for TLS wrap in `External` |
//! | `Embed` | `embed_url`, `controlled_embed.bindings` | selector mappings are user-verified values |
//! | `Chain` | `steps` | step args may use `{{steps.<id>.output}}` expressions |
//! | `Llm` | `prompt` | `provider = "echo"` is the offline default, `provider = "tool:<id>"` delegates to a provider Tool; an unknown provider fails explicitly |
//! | `Wasm` | `wasm_path` or a loaded WASM Toolkit declaration | the host validates the exported manifest before registering |
//!
//! Credentials are references, never values: a `credentials[]` entry
//! says only *where* a secret resolves at run time — `value`,
//! `secret_value`, and inline literal secrets are all invalid, and
//! secret bytes appear in no manifest, log, or HTTP/MCP listing.
//!
//! The generated field reference is `docs/TOOL_MANIFEST.md` (canonical,
//! derived from these Rust types; the JSON Schema for editors/CI is
//! `fixtures/toolkit.schema.json` via `just toolkit-schema`). The schema
//! proves only the TOML→JSON shape; `upeg tool validate` validates
//! declarations (invoker fields, typed inputs, chain structure, HTTP(S)
//! guide URLs) — not runtime credentials, reachability, or executable
//! availability, which is a separate non-executing check
//! (`upeg_runtime::readiness`). The `External` child-process contract
//! (cwd resolution against the manifest's directory, `/dev/null` stdin,
//! `color`/`pty`, timeout process-group kill, cancellation,
//! `args_template` tokens, failure envelope, in-progress streaming) is
//! enforced by `dispatcher`'s External invoker.

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
mod project;
mod schema;

pub use dispatcher::{
    BOARD_ENV, PROJECT_MANIFEST_ENV, PROJECT_ROOT_ENV, SURFACE_ENV,
    normalize_controlled_embed_result,
};
pub use error::LoadError;
pub use loader::{
    LoadOutcome, ProjectToolkitInspection, inspect_project_toolkit, load_and_register_dir_verbose,
    load_project_toolkit_file_verbose,
};
#[cfg(test)]
pub(crate) use loader::{load_and_register_dir, load_dir};
pub use manifest_docs::toolkit_manifest_docs_markdown;
pub use model::{
    ChainConnectionToml, ChainStepToml, CredentialRefToml, InputChoiceToml, InputDefaultToml,
    InputFieldToml, KeyValueToml, OutputFieldToml, SelectorBindingToml, ToolEntryToml,
    ToolSetupInstallToml, ToolSetupToml, ToolToml, ToolkitToml, TriggerToml,
};
pub use parse::{SkippedTool, ToolkitParse, parse_toolkit_full, parse_toolkit_with_skips};
pub use project::{ProjectConfig, ProjectConfigError, parse_project_config};
pub use schema::{toolkit_schema_json, toolkit_schema_value};

#[cfg(test)]
mod tests;
