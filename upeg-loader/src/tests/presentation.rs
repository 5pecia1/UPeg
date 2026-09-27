use crate::LoadError;
use crate::parse::parse_manifest;
use upeg_core::{ActionBinding, ActionScope, ToolEffect};

fn manifest(presentation: &str) -> String {
    format!(
        r#"id = "demo"

[[tools]]
id = "list"
pegboard_units = "U1"
invoker = "External"
command = "echo"
effect = "read"
primary_output_id = "result"

[[tools.outputs]]
name = "result"
type = "json"

{presentation}
"#
    )
}

#[test]
fn presentation_v1_lowers_collection_and_row_binding() {
    let parsed = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1
output = "result"
rows = "/items"
row_key = "/id"
columns = [{ label = "Name", pointer = "/name" }]

[[tools.presentation.actions]]
id = "open"
scope = "row"
label = "Open"
target_tool = "demo.detail"

[tools.presentation.actions.bindings]
id = { from = "row", pointer = "/id" }
"#,
    ))
    .expect("presentation parses");
    let meta = &parsed.tools[0].0;
    assert_eq!(meta.effect, ToolEffect::Read);
    let presentation = meta.presentation.as_ref().expect("presentation lowered");
    assert_eq!(presentation.actions[0].scope, ActionScope::Row);
    assert!(matches!(
        presentation.actions[0].bindings["id"],
        ActionBinding::Row { .. }
    ));
}

#[test]
fn presentation_rejects_reserved_binding_target() {
    let error = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1

[[tools.presentation.actions]]
id = "bad"
scope = "result"
label = "Bad"
target_tool = "demo.detail"

[tools.presentation.actions.bindings]
_upeg = { from = "constant", value = true }
"#,
    ))
    .expect_err("reserved target must fail");
    assert!(
        matches!(error, LoadError::InvalidPresentation(message) if message.contains("reserved"))
    );
}

#[test]
fn presentation_rejects_an_undeclared_collection_output() {
    let error = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1
output = "missing"
rows = "/items"
row_key = "/id"
columns = [{ label = "Name", pointer = "/name" }]
"#,
    ))
    .expect_err("collection output must name a declared JSON output");
    assert!(
        matches!(error, LoadError::InvalidPresentation(message) if message.contains("not declared"))
    );
}

#[test]
fn presentation_rejects_duplicate_action_ids() {
    let error = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1

[[tools.presentation.actions]]
id = "open"
scope = "result"
label = "First"
target_tool = "demo.first"

[[tools.presentation.actions]]
id = "open"
scope = "result"
label = "Second"
target_tool = "demo.second"
"#,
    ))
    .expect_err("action ids select actions and must be unique");
    assert!(matches!(error, LoadError::InvalidPresentation(message) if message.contains("unique")));
}

#[test]
fn optional_output_only_fields_preserve_version_one() {
    let parsed = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1
output = "result"
title_pointer = "/view/title"
summary = [{ label = "Changes", pointer = "/view/changes" }]

[tools.presentation.status]
label_pointer = "/view/status/label"
tone_pointer = "/view/status/tone"

[[tools.presentation.actions]]
id = "apply"
scope = "result"
label = "Apply"
target_tool = "demo.apply"
enabled_pointer = "/view/actions/apply/enabled"
disabled_reason_pointer = "/view/actions/apply/reason"
"#,
    ))
    .expect("additive v1 fields load");
    let presentation = parsed.tools[0].0.presentation.as_ref().unwrap();
    assert_eq!(presentation.version, 1);
    assert_eq!(presentation.title_pointer.as_deref(), Some("/view/title"));
    assert_eq!(
        presentation.actions[0].enabled_pointer.as_deref(),
        Some("/view/actions/apply/enabled")
    );
}

#[test]
fn output_alone_without_display_fields_is_still_rejected() {
    let error = parse_manifest(&manifest(
        "[tools.presentation]\nversion = 1\noutput = \"result\"\n",
    ))
    .expect_err("bare output is not a display contract");
    assert!(matches!(error, LoadError::InvalidPresentation(_)));
}

#[test]
fn optional_display_pointer_must_be_valid_json_pointer() {
    let error = parse_manifest(&manifest(
        "[tools.presentation]\nversion = 1\noutput = \"result\"\ntitle_pointer = \"view/title\"\n",
    ))
    .expect_err("invalid display pointer must fail at load time");
    assert!(
        matches!(error, LoadError::InvalidPresentation(message) if message.contains("JSON Pointer"))
    );
}

#[test]
fn condition_is_limited_to_result_actions() {
    let error = parse_manifest(&manifest(
        r#"[tools.presentation]
version = 1
output = "result"
rows = "/items"
row_key = "/id"
columns = [{ label = "Name", pointer = "/name" }]

[[tools.presentation.actions]]
id = "open"
scope = "row"
label = "Open"
target_tool = "demo.detail"
enabled_pointer = "/view/enabled"
"#,
    ))
    .expect_err("row conditions are outside this contract");
    assert!(
        matches!(error, LoadError::InvalidPresentation(message) if message.contains("result action"))
    );
}
