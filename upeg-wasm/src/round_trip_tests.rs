//! 3-grammar round-trip drift gate (Task A2, define-once roadmap).
//!
//! A tool defined once should lower to the *same* `upeg_core::ToolMeta`
//! shape no matter which grammar authored it. This module defines one
//! equivalent tool as ① Toolkit TOML (parsed through
//! `upeg_loader::parse_toolkit_full`) and ② a wasm plugin manifest JSON
//! (parsed and lowered through this crate's own `decl_to_meta`, the exact
//! function `register_from_bytes` calls after asking a real `.wasm`
//! plugin for its `manifest` export) and asserts the two `ToolMeta`s
//! agree on id, input fields (name/kind/required), output declaration,
//! `pegboard_units`, and the invoker-visible surface set.
//!
//! The fixture's `doc` input uses `PluginInputKind::File` /
//! TOML `type = "file"` — the variant this task adds — so a regression
//! that drops the new variant's lowering breaks this test, not just the
//! narrower unit tests in `tests.rs`.
//!
//! ③ Macro side (`#[upeg::tool]`) is intentionally not included: built-in
//! tools compiled through the macro don't have a byte-identical
//! equivalent of this synthetic fixture (no built-in exists with this
//! exact id/shape), and fabricating one would just re-test the macro's
//! own existing inventory tests. The TOML↔plugin pair already exercises
//! the two *declarative* grammars, which is where the plugin `File`
//! parity gap actually lived.

use std::collections::BTreeSet;

use upeg_core::{InputKind, OutputKind, Surface, ToolMeta};

use crate::decl_to_meta;
use upeg_plugin_api::PluginManifest;

const TOOLKIT_TOML: &str = r#"
id = "roundtrip"

[[tools]]
id = "echo"
pin = "Inline"
pegboard_units = "U1"
invoker = "External"
command = "echo"
surfaces = ["cli", "http"]
primary_output_id = "result"

[[tools.inputs]]
name = "text"
type = "string"
required = true

[[tools.inputs]]
name = "when"
type = "datetime"

[[tools.inputs]]
name = "doc"
type = "file"
max_count = 3
extensions = ["png", "tar.gz"]
max_file_bytes = 5000000
max_total_bytes = 10000000

[[tools.outputs]]
name = "result"
type = "string"
"#;

const PLUGIN_MANIFEST_JSON: &str = r#"
{
    "id": "roundtrip",
    "tools": [
        {
            "id": "roundtrip.echo",
            "toolkit": "roundtrip",
            "pin": "Inline",
            "pegboard_units": "U1",
            "surfaces": ["cli", "http"],
            "input_spec": {
                "fields": [
                    {"name": "text", "required": true, "kind": {"type": "string"}},
                    {"name": "when", "required": false, "kind": {"type": "datetime"}},
                    {
                        "name": "doc",
                        "required": false,
                        "kind": {"type": "file"},
                        "file_policy": {
                            "max_count": 3,
                            "extensions": ["png", "tar.gz"],
                            "max_file_bytes": 5000000,
                            "max_total_bytes": 10000000
                        }
                    }
                ]
            },
            "output_spec": {
                "fields": [
                    {"name": "result", "kind": {"type": "string"}}
                ],
                "primary_output_id": "result"
            }
        }
    ]
}
"#;

fn toml_side_meta() -> ToolMeta {
    let (_toolkit_meta, mut tools) =
        upeg_loader::parse_toolkit_full(TOOLKIT_TOML).expect("fixture TOML must parse");
    assert_eq!(tools.len(), 1, "fixture declares exactly one tool");
    tools.remove(0).0
}

fn plugin_side_meta() -> ToolMeta {
    let manifest: PluginManifest =
        serde_json::from_str(PLUGIN_MANIFEST_JSON).expect("fixture manifest JSON must parse");
    assert_eq!(manifest.tools.len(), 1, "fixture declares exactly one tool");
    let decl = manifest
        .tools
        .into_iter()
        .next()
        .expect("checked len == 1 above");
    decl_to_meta(decl).expect("fixture plugin decl must lower to ToolMeta")
}

/// `(name, kind label, required)` triples, sorted by name so field
/// declaration order doesn't matter for the comparison.
fn input_shape(meta: &ToolMeta) -> Vec<(String, &'static str, bool)> {
    let mut shape: Vec<(String, &'static str, bool)> = meta
        .input_spec
        .fields
        .iter()
        .map(|field| {
            (
                field.name.as_str().to_string(),
                field.kind.label(),
                field.required,
            )
        })
        .collect();
    shape.sort_by(|a, b| a.0.cmp(&b.0));
    shape
}

/// `(name, kind label)` pairs, sorted by name.
fn output_shape(meta: &ToolMeta) -> Vec<(String, &'static str)> {
    let mut shape: Vec<(String, &'static str)> = meta
        .output_spec
        .fields
        .iter()
        .map(|field| (field.name.clone(), field.kind.label()))
        .collect();
    shape.sort_by(|a, b| a.0.cmp(&b.0));
    shape
}

fn surface_labels(meta: &ToolMeta) -> BTreeSet<&'static str> {
    meta.surfaces.iter().copied().map(Surface::label).collect()
}

#[test]
fn toml_and_plugin_manifests_converge_to_same_tool_meta() {
    let toml_meta = toml_side_meta();
    let plugin_meta = plugin_side_meta();

    assert_eq!(
        toml_meta.id, plugin_meta.id,
        "both grammars must produce the same canonical tool id"
    );
    assert_eq!(toml_meta.toolkit, plugin_meta.toolkit);
    assert_eq!(toml_meta.local_id, plugin_meta.local_id);

    assert_eq!(
        input_shape(&toml_meta),
        input_shape(&plugin_meta),
        "input field name+kind+required must agree, including the new `file` kind"
    );

    assert_eq!(
        output_shape(&toml_meta),
        output_shape(&plugin_meta),
        "output field name+kind must agree"
    );
    assert_eq!(
        toml_meta.primary_output_id, plugin_meta.primary_output_id,
        "primary output id must agree"
    );

    assert_eq!(
        toml_meta.pegboard_units, plugin_meta.pegboard_units,
        "pegboard footprint must agree"
    );

    assert_eq!(
        surface_labels(&toml_meta),
        surface_labels(&plugin_meta),
        "invoker-visible surface set must agree"
    );

    // Sanity: the `doc` field really did lower to the new File kind on
    // both sides, not silently fall back to FilePath or get dropped.
    let doc_kind_toml = toml_meta
        .input_spec
        .fields
        .iter()
        .find(|f| f.name.as_str() == "doc")
        .map(|f| &f.kind);
    let doc_kind_plugin = plugin_meta
        .input_spec
        .fields
        .iter()
        .find(|f| f.name.as_str() == "doc")
        .map(|f| &f.kind);
    assert_eq!(doc_kind_toml, doc_kind_plugin);
    let Some(InputKind::File(policy)) = doc_kind_plugin else {
        panic!("the plugin doc input must carry a File policy");
    };
    assert_eq!(policy.max_count(), 3);
    assert_eq!(policy.extensions().collect::<Vec<_>>(), ["png", "tar.gz"]);
    assert_eq!(policy.max_file_bytes(), Some(5_000_000));
    assert_eq!(policy.max_total_bytes(), Some(10_000_000));

    // And the output side truly resolved to a plain string, not e.g.
    // an EmbeddedView, confirming `output_shape`'s label comparison
    // isn't hiding a structural mismatch.
    assert_eq!(toml_meta.output_spec.fields[0].kind, OutputKind::String);
}
