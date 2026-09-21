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
