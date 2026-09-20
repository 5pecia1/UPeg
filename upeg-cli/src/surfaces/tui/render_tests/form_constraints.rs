//! Companion to the `render_tests` module — checks only that declared
//! input constraints (defaults/ranges/placeholders) reach the Form
//! view. Split out for the workspace 1000-LoC file-size budget.

use super::*;
use upeg_core::{FieldConstraints, NumberConstraints, StringConstraints};

fn constrained_field(
    name: &str,
    description: Option<&str>,
    kind: InputKind,
    constraints: FieldConstraints,
) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        InputName::new(name).expect("test input name"),
        None,
        description.map(str::to_string),
        false,
        kind,
        constraints,
    )
    .expect("test input field")
}

fn form_state(fields: Vec<InputFieldSpec>) -> TuiFormState {
    TuiFormState::new(InputSpec::new(fields).expect("test input spec"))
}

fn rendered_form(form: TuiFormState) -> String {
    use ratatui::backend::TestBackend;

    let state = State {
        view: View::Form {
            tool_id: "security.password_generate",
            form,
        },
        ..State::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    let tools = list_tools();
    terminal
        .draw(|f| render(f, &state, &tools))
        .expect("render success");
    format!("{:?}", terminal.backend().buffer())
}

#[test]
fn form_view_shows_declared_default_as_initial_draft() {
    let buf = rendered_form(form_state(vec![constrained_field(
        "count",
        Some("Password length"),
        InputKind::Integer,
        FieldConstraints {
            number: Some(NumberConstraints {
                min: Some(8.0),
                max: Some(128.0),
                default: Some(20.0),
            }),
            string: None,
        },
    )]));

    assert!(
        buf.contains("count"),
        "Form view must show the field name. got: {buf}"
    );
    assert!(
        buf.contains("20"),
        "the declared default must enter the initial draft. got: {buf}"
    );
}

#[test]
fn form_view_shows_min_max_below_description() {
    let buf = rendered_form(form_state(vec![constrained_field(
        "count",
        Some("Password length"),
        InputKind::Integer,
        FieldConstraints {
            number: Some(NumberConstraints {
                min: Some(8.0),
                max: Some(128.0),
                default: None,
            }),
            string: None,
        },
    )]));

    assert!(
        buf.contains("Password length"),
        "the field description must be visible. got: {buf}"
    );
    assert!(
        buf.contains("min 8"),
        "the declared minimum must be visible. got: {buf}"
    );
    assert!(
        buf.contains("max 128"),
        "the declared maximum must be visible. got: {buf}"
    );
}

#[test]
fn form_view_shows_placeholder_as_example() {
    let buf = rendered_form(form_state(vec![constrained_field(
        "slug",
        None,
        InputKind::String,
        FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: None,
                placeholder: Some("my-post-title".to_string()),
                default: None,
            }),
        },
    )]));

    assert!(
        buf.contains("e.g. my-post-title"),
        "the declared placeholder must appear as an example. got: {buf}"
    );
}
