//! Companion to the `input_tests` module — collects only form-state /
//! arg-coercion regression tests. Split out for the workspace 1000-LoC
//! file-size budget.

use super::*;

#[test]
fn tui_form_state_initializes_from_input_spec() {
    let form = form_from_fields(vec![
        input_field("input", InputKind::String, true),
        input_field("size", InputKind::Integer, false),
    ]);
    assert_eq!(form.len(), 2);
    assert_eq!(form.fields[0].name.as_str(), "input");
    assert_eq!(form.fields[0].draft, DraftInputValue::Text(String::new()));
    let input = form.spec_for_field(&form.fields[0]).unwrap();
    assert!(input.required);
    assert!(matches!(input.kind, InputKind::String));
    let size = form.spec_for_field(&form.fields[1]).unwrap();
    assert!(!size.required);
    assert!(matches!(size.kind, InputKind::Integer));
}

#[test]
fn tui_form_args_coerce_to_typed_values() {
    let mut form = form_from_fields(vec![
        input_field("s", InputKind::String, false),
        input_field("n", InputKind::Number, false),
        input_field("i", InputKind::Integer, false),
        input_field("b", InputKind::Boolean, false),
    ]);
    set_form_text(&mut form, 0, "hi");
    set_form_text(&mut form, 1, "2.5");
    set_form_text(&mut form, 2, "42");
    form.fields[3].draft = DraftInputValue::Boolean(true);

    let args = form.args().expect("typed TUI args");
    assert_eq!(args["s"], "hi");
    assert_eq!(args["n"], 2.5);
    assert_eq!(args["i"], 42);
    assert_eq!(args["b"], true);
}

#[test]
fn tui_form_args_invalid_number_is_validation_error() {
    let mut form = form_from_fields(vec![input_field("n", InputKind::Number, false)]);
    set_form_text(&mut form, 0, "not-a-number");
    let err = form.args().unwrap_err();
    assert!(err.contains("invalid number"), "got: {err}");
}

#[test]
fn tui_form_args_omit_empty_integer_so_dispatcher_default_applies() {
    // Iter-108 contract (TUI parity): an empty integer field must be
    // omitted so a dispatcher like `text.repeat` applies its declared
    // default instead of erroring with "n must be a non-negative
    // integer". Iter 99 sent Null here and broke the default path.
    // Pinned at the TUI surface so a regression shows up loudly even if
    // the upeg-core helper tests alone lapse.
    let mut form = form_from_fields(vec![
        input_field("input", InputKind::String, true),
        input_field("n", InputKind::Integer, false),
    ]);
    set_form_text(&mut form, 0, "x");
    let args = form.args().expect("typed TUI args");
    assert_eq!(args["input"], "x");
    assert!(
        args.get("n").is_none(),
        "an empty integer must omit the key. got: {args}"
    );
}

#[test]
fn tui_form_args_trim_whitespace_around_numbers() {
    // Iter-109 contract (TUI parity): pasting a number with surrounding
    // whitespace must still coerce cleanly — " 42 " → 42, "\t-7\n" → -7,
    // " 0.5 " → 0.5. Without trim the parser rejects it and the
    // dispatcher errors.
    let mut form = form_from_fields(vec![
        input_field("i", InputKind::Integer, false),
        input_field("j", InputKind::Integer, false),
        input_field("f", InputKind::Number, false),
    ]);
    set_form_text(&mut form, 0, "  42  ");
    set_form_text(&mut form, 1, "\t-7\n");
    set_form_text(&mut form, 2, " 0.5 ");
    let args = form.args().expect("typed TUI args");
    assert_eq!(args["i"], 42);
    assert_eq!(args["j"], -7);
    assert_eq!(args["f"], 0.5);
}
