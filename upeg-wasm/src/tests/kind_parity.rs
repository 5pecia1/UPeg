use std::collections::BTreeSet;

use super::*;

// --- Kind-parity drift gate (Task A2, define-once roadmap) ---------------
//
// `upeg-loader` lowers Toolkit TOML `type = "…"` fields directly into
// `upeg_core::InputKind` / `upeg_core::OutputKind` — there is no
// separate loader-owned kind enum to drift against. So "the loader's
// kind vocabulary" *is* `upeg_core::{InputKind, OutputKind}`, and the
// grammar-parity question reduces to: does `PluginInputKind` /
// `PluginOutputKind`'s wire "type" tag name always match the matching
// core kind's `label()`? Both sides are read off the types' own
// Serialize / label() impls below — no hand-copied string list to fall
// out of sync.

fn single_choice_spec() -> ChoiceSpec {
    ChoiceSpec::new(vec![
        ChoiceOption::new("a", None, None).expect("non-empty choice value"),
    ])
    .expect("non-empty choice list")
}

fn single_plugin_choice() -> Vec<PluginChoiceOption> {
    vec![PluginChoiceOption::new("a")]
}

/// One instance of every `InputKind` variant, used only to read
/// `.label()` off each — payload contents are irrelevant to naming.
fn core_input_kind_samples() -> Vec<InputKind> {
    vec![
        InputKind::String,
        InputKind::Number,
        InputKind::Integer,
        InputKind::Boolean,
        InputKind::Options(single_choice_spec()),
        InputKind::MultiOptions(single_choice_spec()),
        InputKind::Markdown,
        InputKind::Json,
        InputKind::DateTime,
        InputKind::FilePath,
        InputKind::Url,
        InputKind::File(upeg_core::FileInputPolicy::default()),
    ]
}

/// One instance of every `PluginInputKind` variant, used only to read
/// the serialized `type` tag off each.
fn plugin_input_kind_samples() -> Vec<PluginInputKind> {
    vec![
        PluginInputKind::String,
        PluginInputKind::Number,
        PluginInputKind::Integer,
        PluginInputKind::Boolean,
        PluginInputKind::Options(single_plugin_choice()),
        PluginInputKind::MultiOptions(single_plugin_choice()),
        PluginInputKind::Markdown,
        PluginInputKind::Json,
        PluginInputKind::DateTime,
        PluginInputKind::FilePath,
        PluginInputKind::Url,
        PluginInputKind::File,
    ]
}

/// One instance of every `OutputKind` variant.
fn core_output_kind_samples() -> Vec<OutputKind> {
    vec![
        OutputKind::String,
        OutputKind::Number,
        OutputKind::Integer,
        OutputKind::Boolean,
        OutputKind::Options(single_choice_spec()),
        OutputKind::MultiOptions(single_choice_spec()),
        OutputKind::Markdown,
        OutputKind::Json,
        OutputKind::DateTime,
        OutputKind::FilePath,
        OutputKind::Url,
        OutputKind::File,
        OutputKind::EmbeddedView {
            url: "https://example.com/view".to_string(),
        },
    ]
}

/// One instance of every `PluginOutputKind` variant.
fn plugin_output_kind_samples() -> Vec<PluginOutputKind> {
    vec![
        PluginOutputKind::String,
        PluginOutputKind::Number,
        PluginOutputKind::Integer,
        PluginOutputKind::Boolean,
        PluginOutputKind::Options(single_plugin_choice()),
        PluginOutputKind::MultiOptions(single_plugin_choice()),
        PluginOutputKind::Markdown,
        PluginOutputKind::Json,
        PluginOutputKind::DateTime,
        PluginOutputKind::FilePath,
        PluginOutputKind::Url,
        PluginOutputKind::File,
        PluginOutputKind::EmbeddedView {
            url: "https://example.com/view".to_string(),
        },
    ]
}

fn plugin_input_kind_wire_name(kind: &PluginInputKind) -> String {
    serde_json::to_value(kind)
        .expect("PluginInputKind always serializes")
        .get("type")
        .and_then(serde_json::Value::as_str)
        .expect("tagged enum always carries a `type` field")
        .to_string()
}

fn plugin_output_kind_wire_name(kind: &PluginOutputKind) -> String {
    serde_json::to_value(kind)
        .expect("PluginOutputKind always serializes")
        .get("type")
        .and_then(serde_json::Value::as_str)
        .expect("tagged enum always carries a `type` field")
        .to_string()
}

#[test]
fn 플러그인_입력_kind는_loader_input_kind와_일치한다() {
    let plugin_names: BTreeSet<String> = plugin_input_kind_samples()
        .iter()
        .map(plugin_input_kind_wire_name)
        .collect();
    let plugin_names: BTreeSet<&str> = plugin_names.iter().map(String::as_str).collect();
    let core_names: BTreeSet<&'static str> = core_input_kind_samples()
        .iter()
        .map(InputKind::label)
        .collect();

    assert_eq!(
        plugin_names, core_names,
        "PluginInputKind's wire `type` names must exactly match \
         upeg_core::InputKind::label() — the Toolkit TOML loader lowers \
         directly into upeg_core::InputKind, so any mismatch here is a \
         grammar drift between the plugin DTO and TOML grammars"
    );
}

#[test]
fn 플러그인_출력_kind는_loader_output_kind와_일치한다() {
    let plugin_names: BTreeSet<String> = plugin_output_kind_samples()
        .iter()
        .map(plugin_output_kind_wire_name)
        .collect();
    let plugin_names: BTreeSet<&str> = plugin_names.iter().map(String::as_str).collect();
    let core_names: BTreeSet<&'static str> = core_output_kind_samples()
        .iter()
        .map(OutputKind::label)
        .collect();

    assert_eq!(
        plugin_names, core_names,
        "PluginOutputKind's wire `type` names must exactly match \
         upeg_core::OutputKind::label() — same grammar-drift gate as the \
         input side, extended to EmbeddedView"
    );
}
