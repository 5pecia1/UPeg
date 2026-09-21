//! End-to-end proof that `color = "force"` reaches the child as
//! environment, and that omitting it changes nothing.
//!
//! Every assertion is a **delta against the parent's own environment**,
//! never "the child saw nothing". A CI runner that exports `FORCE_COLOR`
//! for its own reasons is a legal environment to run this suite in, and
//! an `inherit` child there is *supposed* to see that value — asserting
//! an empty string would have made the ambient environment part of the
//! contract.

use serde_json::json;
use upeg_core::ToolResult;
use upeg_runtime::{DispatchArgs, tool_success_primary_text};

use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;

use super::color::{CLICOLOR_FORCE_ENV, COLOR_ENABLED_VALUE, FORCE_COLOR_ENV, NO_COLOR_ENV};

/// Field separator in the probe's output — one that cannot appear in an
/// environment variable name and is unlikely in these values.
const SEPARATOR: &str = "|";

/// The variables the probe reports, in the order it prints them.
const OBSERVED_VARS: &[&str] = &[CLICOLOR_FORCE_ENV, FORCE_COLOR_ENV, NO_COLOR_ENV];

/// Prints the three color variables the policy touches, so the
/// assertion reads the child's own view of its environment rather than
/// upeg's.
const COLOR_VARS_PRINT: &str = r#"printf '%s|%s|%s' "$CLICOLOR_FORCE" "$FORCE_COLOR" "$NO_COLOR""#;

fn tool(color: Option<&str>) -> ToolToml {
    ToolToml {
        id: "test.color_probe".to_string(),
        toolkit: "test".to_string(),
        invoker: Some("External".to_string()),
        command: Some("sh".to_string()),
        args_template: Some(vec!["-c".to_string(), COLOR_VARS_PRINT.to_string()]),
        color: color.map(str::to_string),
        ..ToolToml::default()
    }
}

fn run(tool: &ToolToml) -> String {
    let dispatcher = external_dispatcher_for(tool, None).expect("External dispatcher");
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("empty object args");
    match dispatcher(parsed) {
        ToolResult::Success(success) => tool_success_primary_text(&success),
        ToolResult::Failure(failure) => panic!("child run failed: {}", failure.error.message),
    }
}

/// What the probe would print if it inherited this process's
/// environment untouched — the baseline every `inherit` assertion
/// compares against.
fn parent_env() -> String {
    OBSERVED_VARS
        .iter()
        .map(|name| std::env::var(name).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

/// What the probe prints under `color = "force"`: both force variables
/// set, `NO_COLOR` gone.
fn forced_env() -> String {
    format!("{COLOR_ENABLED_VALUE}{SEPARATOR}{COLOR_ENABLED_VALUE}{SEPARATOR}")
}

#[test]
fn color_force_passes_color_env_vars_to_child() {
    assert_eq!(run(&tool(Some("force"))), forced_env());
}

#[test]
fn color_force_removes_inherited_no_color() {
    // The trailing field is `NO_COLOR`, and `force` must leave it empty
    // whatever the parent exported — no-color.org ranks it above the two
    // force variables, so an inherited one would cancel the whole opt-in.
    let output = run(&tool(Some("force")));
    let no_color = output.rsplit(SEPARATOR).next().expect("NO_COLOR field");
    assert_eq!(no_color, "", "NO_COLOR must be empty under force");
}

#[test]
fn without_color_declaration_env_is_unchanged() {
    assert_eq!(run(&tool(None)), parent_env());
}

#[test]
fn color_inherit_matches_no_declaration() {
    assert_eq!(run(&tool(Some("inherit"))), parent_env());
}

#[test]
fn declared_env_overrides_color_policy() {
    // The policy sets defaults; an explicit `env` entry is the author
    // saying they know better, so it must win.
    let mut tool = tool(Some("force"));
    tool.env = Some(vec![crate::model::KeyValueToml {
        name: FORCE_COLOR_ENV.to_string(),
        value: "0".to_string(),
    }]);

    assert_eq!(
        run(&tool),
        format!("{COLOR_ENABLED_VALUE}{SEPARATOR}0{SEPARATOR}")
    );
}

#[test]
fn declared_no_color_beats_force_removal() {
    // Same ordering rule as the line above: the removal is a default,
    // and a tool that re-declares `NO_COLOR` is opting back out.
    let mut tool = tool(Some("force"));
    tool.env = Some(vec![crate::model::KeyValueToml {
        name: NO_COLOR_ENV.to_string(),
        value: COLOR_ENABLED_VALUE.to_string(),
    }]);

    assert_eq!(
        run(&tool),
        format!(
            "{COLOR_ENABLED_VALUE}{SEPARATOR}{COLOR_ENABLED_VALUE}{SEPARATOR}{COLOR_ENABLED_VALUE}"
        )
    );
}
