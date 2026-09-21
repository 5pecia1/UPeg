//! Built-in upeg Tools.
//!
//! Each function is a *pure* Tool implementation annotated with
//! `#[upeg::tool]`. The macro registers a `StaticToolMeta` into the global
//! `inventory` registry; surfaces (CLI/TUI/Desktop/MCP/HTTP/Ext) discover
//! every Tool here without per-surface plumbing — PRD v2.1 §5.1.
//!
//! These functions return `Result<T, &'static str>` so different surfaces
//! can format errors their own way (CLI → exit 1 + stderr, Desktop → "—"
//! placeholder, MCP/HTTP → JSON error). UI wrappers live in surface crates.
//!
//! Implementations are organized one file per toolkit under
//! [`toolkits`]; this `lib.rs` only declares the toolkit metadata,
//! re-exports tool functions to the crate root for stable consumer
//! paths (e.g. `upeg_tools::sha256_hex`), and exposes
//! [`register_all`].

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
// Crate-root re-exports below preserve the stable public API
// (`upeg_tools::hex_to_decimal`, etc.) without per-symbol plumbing as new
// tools join each toolkit module.
#![allow(
    clippy::wildcard_imports,
    reason = "intentional public-surface re-export from each toolkit module"
)]

use upeg_core::toolkit;

mod dispatch;
mod toolkits;

pub use dispatch::{RegisteredDispatch, dispatch_registered, register_all};

pub use toolkits::color::*;
pub use toolkits::convert::*;
pub use toolkits::csv::*;
// `devcontainer`'s functions compile on every target (their `StaticToolMeta`
// must stay discoverable on wasm32 too — see `toolkits::devcontainer`'s module
// doc), so this re-export is unconditional, exactly like `eth`/`weather`.
pub use toolkits::devcontainer::*;
// `eth`'s functions compile on every target (their `StaticToolMeta` must
// stay discoverable on wasm32 too — see `toolkits::eth`'s module doc), so
// this re-export is unconditional, unlike `net` below.
pub use toolkits::eth::*;
pub use toolkits::hash::*;
pub use toolkits::id::*;
#[allow(
    unused_imports,
    reason = "wasm build leaves some helpers unused; native build exercises them"
)]
pub use toolkits::media::*;
#[cfg(not(target_arch = "wasm32"))]
pub use toolkits::net::*;
pub use toolkits::num::*;
pub use toolkits::qr::*;
pub use toolkits::security::*;
pub use toolkits::text::*;
pub use toolkits::time::*;
// `weather`'s functions compile on every target (their `StaticToolMeta` must
// stay discoverable on wasm32 too — see `toolkits::weather`'s module doc), so
// this re-export is unconditional, exactly like `eth` above.
pub use toolkits::weather::*;

#[toolkit(
    id = "convert",
    tags = ["pure", "encoding"],
    description = "Encoding, decoding, and serialization utilities."
)]
const _: () = ();

#[toolkit(
    id = "id",
    tags = ["pure", "identifier", "generator"],
    description = "Identifier generators."
)]
const _: () = ();

#[toolkit(
    id = "text",
    tags = ["pure", "text"],
    description = "Text processing utilities."
)]
const _: () = ();

#[toolkit(
    id = "hash",
    tags = ["pure", "hash"],
    description = "Hashing utilities."
)]
const _: () = ();

#[toolkit(
    id = "time",
    tags = ["pure", "time"],
    description = "Timestamp utilities."
)]
const _: () = ();

#[toolkit(
    id = "color",
    tags = ["pure", "color"],
    description = "Color conversion utilities."
)]
const _: () = ();

#[toolkit(
    id = "security",
    tags = ["pure", "security", "generator"],
    description = "Security helper utilities."
)]
const _: () = ();

#[toolkit(
    id = "media",
    tags = ["media", "file", "pdf", "image", "native"],
    description = "Image and PDF file conversion utilities."
)]
const _: () = ();

#[toolkit(
    id = "qr",
    tags = ["pure", "visual"],
    description = "QR code generation and decoding."
)]
const _: () = ();

#[toolkit(
    id = "csv",
    tags = ["pure", "data"],
    description = "CSV row-level diffing, CSV-to-JSON, and column selection."
)]
const _: () = ();

#[toolkit(
    id = "num",
    tags = ["pure", "numeric"],
    description = "Numeric base conversion utilities (hex/decimal/binary)."
)]
const _: () = ();

// `eth`'s dispatcher is native-only (see `toolkits::eth`'s module doc),
// but the `ToolkitMeta` grouping label itself carries no per-target
// behavior, so it is declared unconditionally like the toolkit's own
// `#[tool]` functions.
#[toolkit(
    id = "eth",
    tags = ["network", "crypto"],
    description = "Minimal Ethereum JSON-RPC reads (gas price, address balance)."
)]
const _: () = ();

// `weather`'s dispatcher is native-only (see `toolkits::weather`'s module
// doc), but the `ToolkitMeta` grouping label carries no per-target behavior,
// so it is declared unconditionally like `eth` above.
#[toolkit(
    id = "weather",
    tags = ["network", "weather"],
    description = "Current weather and multi-day forecast lookups by city (Open-Meteo)."
)]
const _: () = ();

// `devcontainer`'s dispatcher is native-only (see `toolkits::devcontainer`'s
// module doc), but the `ToolkitMeta` grouping label carries no per-target
// behavior, so it is declared unconditionally like `eth`/`weather` above.
#[toolkit(
    id = "devcontainer",
    tags = ["file", "vscode", "native"],
    description = "Locate VS Code/Cursor Dev Container workspaces from workspaceStorage."
)]
const _: () = ();

// GUI-only metadata lives outside the built-in dispatcher modules because
// these tools are inspectable/pinnable but not headless-dispatchable.
pub mod gui_meta;
#[cfg(test)]
mod embed_pairing_inventory_tests {
    use upeg_core::{StaticToolMeta, inventory, validate_pin_invoker_pairing};

    /// Every static `StaticToolMeta` in the global inventory must pass
    /// the embed pin/invoker pairing validator. Catches drift if a
    /// future `inventory::submit!` author forgets the rule. Static
    /// tools never carry selector_bindings, so the binding-shape
    /// rules (loader-side) don't apply here.
    #[test]
    fn every_static_tool_meta_follows_embed_pairing_rules() {
        let mut violations = Vec::<String>::new();
        for meta in inventory::iter::<StaticToolMeta>() {
            if let Err(e) = validate_pin_invoker_pairing(meta.id, meta.pin, meta.invoker) {
                violations.push(format!("{}: {}", meta.id, e));
            }
        }
        assert!(
            violations.is_empty(),
            "static inventory violates embed-pairing rules:\n{}",
            violations.join("\n")
        );
    }
}

#[cfg(test)]
mod consistency_drift_tests {
    //! W5 tool-consistency drift gates. Each iterates the static inventory
    //! and pins one rule from the consistency audit so a future author who
    //! adds a tool that breaks the convention gets a red test, not silent
    //! drift. Vocabulary and allow-lists are named consts (no magic strings).
    use upeg_core::{Invoker, StaticOutputKind, StaticToolMeta, ToolkitMeta, inventory};

    // ─── id grammar vocabulary (R1/R3) ──────────────────────────
    /// Closed verb vocabulary allowed as a local id's last (or only)
    /// underscore-segment. Every entry is the imperative a tool performs.
    const ALLOWED_ID_VERBS: &[&str] = &[
        "encode",
        "decode",
        "parse",
        "format",
        "minify",
        "match",
        "diff",
        "lowercase",
        "uppercase",
        "count",
        "contains",
        "replace",
        "repeat",
        "split",
        "join",
        "trim",
        "reverse",
        "slugify",
        "generate",
        "estimate",
        "now",
        "create",
        "lookup",
        "select",
        "forecast",
        "extract",
        "inspect",
        "convert",
        "list",
    ];
    /// Whole-local-id allow-list for bare nouns / algorithm names that
    /// carry no verb (hash algos, uuid variants, live-value nouns).
    const ALLOWED_BARE_IDS: &[&str] = &[
        "sha256",
        "md5",
        "sha1",
        "sha512",
        "crc32",
        "uuid_v7",
        "uuid_v4",
        "nanoid",
        "contrast",
        "gas",
        "status",
        "epoch",
        "scratch",
        "transform_tools",
        "to_json",
        "nfc",
        "nfd",
        // `<subject>_extract_images` — the trailing `images` noun keeps the
        // grammar's last-segment verb gate from matching `extract`, so these
        // compound ids are allow-listed whole, like `transform_tools` above.
        "pdf_extract_images",
        "pptx_extract_images",
    ];
    /// Local id syntax: lowercase alphanumeric segments joined by single
    /// underscores.
    const ID_LOCAL_SYNTAX: &str = "^[a-z0-9]+(_[a-z0-9]+)*$";
    /// `<x>_to_<y>` conversion id shape.
    const ID_X_TO_Y: &str = "^[a-z0-9]+_to_[a-z0-9]+$";

    /// Toolkits whose tools are all generators (carry the `generator`
    /// toolkit tag). Used by the equivalence-group gate.
    const GENERATOR_TOOLKIT_TAG: &str = "generator";
    /// Canonical name every single declared output must use (R9/R10).
    const CANONICAL_SINGLE_OUTPUT: &str = "result";

    /// Per-tool expected single-output kind (R4/R10). Pins that each
    /// JSON/Number/Boolean-returning tool declares the honest kind.
    const EXPECTED_SINGLE_OUTPUT_KINDS: &[(&str, StaticOutputKind)] = &[
        ("num.hex_to_decimal", StaticOutputKind::Number),
        ("num.binary_to_decimal", StaticOutputKind::Number),
        ("convert.json_format", StaticOutputKind::Json),
        ("convert.url_query_parse", StaticOutputKind::Json),
        ("text.regex_match", StaticOutputKind::Json),
        ("text.word_count", StaticOutputKind::Number),
        ("text.char_count", StaticOutputKind::Number),
        ("text.line_count", StaticOutputKind::Number),
        ("text.contains", StaticOutputKind::Boolean),
        ("text.split", StaticOutputKind::Json),
        ("security.password_estimate", StaticOutputKind::Json),
        ("color.contrast", StaticOutputKind::Json),
        ("csv.diff", StaticOutputKind::Json),
        ("csv.to_json", StaticOutputKind::Json),
        ("eth.gas", StaticOutputKind::Number),
        ("weather.lookup", StaticOutputKind::Json),
        ("weather.forecast", StaticOutputKind::Json),
        ("devcontainer.list", StaticOutputKind::Json),
        ("devcontainer.lookup", StaticOutputKind::Json),
    ];

    /// The three id-generators are one equivalence group (R13/R14).
    const EQUIVALENCE_GROUP_IDS: &[&str] = &["id.uuid_v7", "id.uuid_v4", "id.nanoid"];

    fn all_tools() -> Vec<&'static StaticToolMeta> {
        inventory::iter::<StaticToolMeta>().collect()
    }

    fn find_tool(id: &str) -> &'static StaticToolMeta {
        inventory::iter::<StaticToolMeta>()
            .find(|meta| meta.id == id)
            .unwrap_or_else(|| panic!("tool `{id}` must exist in the static inventory"))
    }

    fn last_segment(local: &str) -> &str {
        local.rsplit('_').next().unwrap_or(local)
    }

    #[test]
    fn tool_ids_follow_grammar_rules() {
        let syntax = regex::Regex::new(ID_LOCAL_SYNTAX).expect("valid syntax regex");
        let x_to_y = regex::Regex::new(ID_X_TO_Y).expect("valid x_to_y regex");
        let mut violations = Vec::<String>::new();
        for meta in all_tools() {
            let local = meta.local_id;
            if !syntax.is_match(local) {
                violations.push(format!("{}: local id `{local}` is not snake_case", meta.id));
                continue;
            }
            let passes = x_to_y.is_match(local)
                || ALLOWED_ID_VERBS.contains(&last_segment(local))
                || ALLOWED_BARE_IDS.contains(&local);
            if !passes {
                violations.push(format!(
                    "{}: local id `{local}` is neither `<x>_to_<y>`, nor ends in an allowed verb {ALLOWED_ID_VERBS:?}, nor an allow-listed bare id",
                    meta.id
                ));
            }
        }
        assert!(
            violations.is_empty(),
            "tool id grammar violations:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn output_declarations_follow_rules() {
        let mut violations = Vec::<String>::new();
        // (a) Any headless single declared output must be named `result`.
        for meta in all_tools() {
            if meta.has_headless_dispatch_surface() && meta.output_spec.fields.len() == 1 {
                let name = meta.output_spec.fields[0].name;
                if name != CANONICAL_SINGLE_OUTPUT {
                    violations.push(format!(
                        "{}: single output is `{name}`, must be `{CANONICAL_SINGLE_OUTPUT}`",
                        meta.id
                    ));
                }
            }
        }
        // (b) Each JSON/Number/Boolean-returning tool declares the kind.
        for (id, expected) in EXPECTED_SINGLE_OUTPUT_KINDS {
            let meta = find_tool(id);
            let fields = meta.output_spec.fields;
            match fields {
                [field] if field.kind == *expected => {}
                [field] => violations.push(format!(
                    "{id}: declared output kind `{:?}`, expected `{expected:?}`",
                    field.kind
                )),
                other => violations.push(format!(
                    "{id}: expected exactly one declared output of kind `{expected:?}`, found {}",
                    other.len()
                )),
            }
        }
        assert!(
            violations.is_empty(),
            "output declaration violations:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn tags_follow_rules() {
        let toolkits: Vec<&ToolkitMeta> = inventory::iter::<ToolkitMeta>().collect();
        let toolkit_tags = |toolkit_id: &str| -> Vec<&'static str> {
            toolkits
                .iter()
                .find(|tk| tk.id == toolkit_id)
                .map(|tk| tk.tags.to_vec())
                .unwrap_or_default()
        };

        let mut violations = Vec::<String>::new();

        // (a) No tool-level tag re-declares a toolkit tag.
        for meta in all_tools() {
            let owner_tags = toolkit_tags(meta.toolkit);
            for tag in meta.tags {
                if owner_tags.contains(tag) {
                    violations.push(format!(
                        "{}: tool tag `{tag}` duplicates its toolkit `{}` tag",
                        meta.id, meta.toolkit
                    ));
                }
            }
            // (b) GUI-only metas must carry non-empty tags.
            if !meta.has_headless_dispatch_surface() && meta.tags.is_empty() {
                violations.push(format!("{}: GUI-only meta must declare tags", meta.id));
            }
        }

        // (c) No two toolkits share an identical tag set.
        for (i, a) in toolkits.iter().enumerate() {
            for b in &toolkits[i + 1..] {
                let mut ta = a.tags.to_vec();
                let mut tb = b.tags.to_vec();
                ta.sort_unstable();
                tb.sort_unstable();
                if ta == tb {
                    violations.push(format!(
                        "toolkits `{}` and `{}` share identical tag set {ta:?}",
                        a.id, b.id
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "tag rule violations:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn equivalence_group_metas_are_uniform() {
        let mut violations = Vec::<String>::new();

        // (a) uuid_v7 / uuid_v4 / nanoid: identical units, source, boards.
        let group: Vec<&StaticToolMeta> = EQUIVALENCE_GROUP_IDS
            .iter()
            .map(|id| find_tool(id))
            .collect();
        let head = group[0];
        let mut head_boards = head.boards.to_vec();
        head_boards.sort_unstable();
        for meta in &group[1..] {
            if meta.pegboard_units != head.pegboard_units {
                violations.push(format!(
                    "{}: pegboard_units {:?} != {} {:?}",
                    meta.id, meta.pegboard_units, head.id, head.pegboard_units
                ));
            }
            if meta.source != head.source {
                violations.push(format!(
                    "{}: source {:?} != {} {:?}",
                    meta.id, meta.source, head.id, head.source
                ));
            }
            let mut boards = meta.boards.to_vec();
            boards.sort_unstable();
            if boards != head_boards {
                violations.push(format!(
                    "{}: boards {:?} != {} {:?}",
                    meta.id, boards, head.id, head_boards
                ));
            }
        }

        // (b) Every tool in a `generator`-tagged toolkit is a Function invoker.
        let generator_toolkits: Vec<&'static str> = inventory::iter::<ToolkitMeta>()
            .filter(|tk| tk.tags.contains(&GENERATOR_TOOLKIT_TAG))
            .map(|tk| tk.id)
            .collect();
        assert!(
            !generator_toolkits.is_empty(),
            "expected at least one `generator`-tagged toolkit"
        );
        for meta in all_tools() {
            if generator_toolkits.contains(&meta.toolkit) && meta.invoker != Invoker::Function {
                violations.push(format!(
                    "{}: generator-toolkit tool must be Invoker::Function, got {:?}",
                    meta.id, meta.invoker
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "equivalence-group violations:\n{}",
            violations.join("\n")
        );
    }

    /// R6: every real toolkit (one with a `ToolkitMeta` registration —
    /// GUI-only groupings like `memo`/`embed`/`net`/`time` in
    /// `gui_meta.rs` never register one, so they're outside this gate's
    /// scope) owns at least 2 tool metas. Catches the single-tool-toolkit
    /// shape `qr`/`csv` used to carry as a documented, intentional
    /// exception before `qr.decode`/`csv.to_json`+`csv.select` shipped.
    #[test]
    fn every_toolkit_has_at_least_two_tools() {
        let toolkits: Vec<&ToolkitMeta> = inventory::iter::<ToolkitMeta>().collect();
        assert!(
            !toolkits.is_empty(),
            "expected at least one registered ToolkitMeta"
        );

        let mut violations = Vec::<String>::new();
        for toolkit in &toolkits {
            let tool_count = all_tools()
                .into_iter()
                .filter(|meta| meta.toolkit == toolkit.id)
                .count();
            if tool_count < 2 {
                violations.push(format!(
                    "toolkit `{}` owns {tool_count} tool(s), expected >= 2",
                    toolkit.id
                ));
            }
        }
        assert!(
            violations.is_empty(),
            "R6 toolkit-size violations:\n{}",
            violations.join("\n")
        );
    }
}

#[cfg(test)]
mod collision_policy_tests {
    use upeg_runtime::manifest::{CollisionError, validate_tool_identity};

    #[test]
    fn builtin_shadowing_uses_runtime_manifest_validator() {
        let key = upeg_core::ToolKey::parse_canonical("num", "hex_to_decimal")
            .expect("built-in key is canonical");

        assert!(matches!(
            validate_tool_identity(&key, &[]),
            Err(CollisionError::ShadowsBuiltIn { id, .. }) if id == "num.hex_to_decimal"
        ));
    }
}
