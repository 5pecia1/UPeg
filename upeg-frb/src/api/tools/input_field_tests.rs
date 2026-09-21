//! Mapping pins for [`super::input_field`].
//!
//! Every [`InputKind`] variant is asserted explicitly: the previous
//! `Number | Integer => Number` fold and the choice `label`/`description`
//! drop both shipped untested, so this module exists to make any future
//! lossy fold fail a test instead of quietly reaching the Dart form.

use super::{
    ChoiceOptionDto, FieldConstraintsDto, InputFieldDto, InputFieldType, NumberConstraintsDto,
    StringConstraintsDto,
};
use upeg_core::{
    ChoiceOption, ChoiceSpec, FieldConstraints, FileInputPolicy, InputFieldSpec, InputKind,
    InputName, NumberConstraints, StringConstraints,
};

fn input_name(raw: &str) -> InputName {
    InputName::new(raw).expect("test input name must be valid")
}

fn choice(value: &str, label: Option<&str>, description: Option<&str>) -> ChoiceOption {
    ChoiceOption::new(
        value,
        label.map(str::to_string),
        description.map(str::to_string),
    )
    .expect("test choice must be valid")
}

fn choices() -> ChoiceSpec {
    ChoiceSpec::new(vec![
        choice("fast", Some("Fast"), Some("lower quality")),
        choice("safe", None, None),
    ])
    .expect("test choice list must be valid")
}

fn expected_choice_dtos() -> Vec<ChoiceOptionDto> {
    vec![
        ChoiceOptionDto {
            value: "fast".to_string(),
            label: "Fast".to_string(),
            description: Some("lower quality".to_string()),
        },
        ChoiceOptionDto {
            value: "safe".to_string(),
            // No declared label — falls back to the value.
            label: "safe".to_string(),
            description: None,
        },
    ]
}

fn field(name: &str, kind: InputKind, constraints: FieldConstraints) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        input_name(name),
        Some(format!("{name} label")),
        Some(format!("{name} helper")),
        true,
        kind,
        constraints,
    )
    .expect("test input field must be valid")
}

#[test]
fn input_kind_maps_every_variant_to_a_distinct_dto() {
    let cases: Vec<(InputKind, InputFieldType)> = vec![
        (InputKind::String, InputFieldType::Text),
        (InputKind::Number, InputFieldType::Number),
        (InputKind::Integer, InputFieldType::Integer),
        (InputKind::Boolean, InputFieldType::Boolean),
        (InputKind::Markdown, InputFieldType::Markdown),
        (InputKind::Json, InputFieldType::Multiline),
        (InputKind::DateTime, InputFieldType::DateTime),
        (InputKind::FilePath, InputFieldType::FilePath),
        (InputKind::Url, InputFieldType::Url),
        (
            InputKind::Options(choices()),
            InputFieldType::Select {
                options: expected_choice_dtos(),
            },
        ),
        (
            InputKind::MultiOptions(choices()),
            InputFieldType::MultiOptions {
                options: expected_choice_dtos(),
            },
        ),
    ];

    for (kind, expected) in cases {
        assert_eq!(
            InputFieldType::from(&kind),
            expected,
            "input kind `{}` mapped to the wrong DTO",
            kind.label(),
        );
    }

    // File carries its policy, so it is asserted by shape rather than by
    // an inline literal (policy equality is covered next door).
    assert!(matches!(
        InputFieldType::from(&InputKind::File(FileInputPolicy::default())),
        InputFieldType::File { .. }
    ));
}

#[test]
fn number_and_integer_stay_distinct_dto_variants() {
    assert_ne!(
        InputFieldType::from(&InputKind::Number),
        InputFieldType::from(&InputKind::Integer),
        "folding Number and Integer together would stop Dart from choosing an integer-only keyboard/parser",
    );
}

#[test]
fn input_field_dto_passes_description_through() {
    let dto = InputFieldDto::from(&field(
        "count",
        InputKind::Integer,
        FieldConstraints::default(),
    ));

    assert_eq!(dto.key, "count");
    assert_eq!(dto.label, "count label");
    assert_eq!(dto.description.as_deref(), Some("count helper"));
    assert!(dto.required);
    assert_eq!(dto.constraints, None);
}

#[test]
fn input_field_without_label_falls_back_to_name() {
    let spec = InputFieldSpec::new(input_name("input"), None, None, false, InputKind::String)
        .expect("test input field must be valid");

    let dto = InputFieldDto::from(&spec);

    assert_eq!(dto.label, "input");
    assert_eq!(dto.description, None);
    assert!(!dto.required);
}

#[test]
fn number_constraints_carry_min_max_default() {
    let dto = InputFieldDto::from(&field(
        "count",
        InputKind::Integer,
        FieldConstraints {
            number: Some(NumberConstraints {
                min: Some(8.0),
                max: Some(128.0),
                default: Some(20.0),
            }),
            string: None,
        },
    ));

    assert_eq!(
        dto.constraints,
        Some(FieldConstraintsDto {
            number: Some(NumberConstraintsDto {
                min: Some(8.0),
                max: Some(128.0),
                default: Some(20.0),
            }),
            string: None,
        })
    );
}

#[test]
fn string_constraints_carry_regex_placeholder_default() {
    let dto = InputFieldDto::from(&field(
        "slug",
        InputKind::String,
        FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: Some("^[a-z-]+$".to_string()),
                placeholder: Some("my-post-title".to_string()),
                default: Some("hello-world".to_string()),
            }),
        },
    ));

    assert_eq!(
        dto.constraints,
        Some(FieldConstraintsDto {
            number: None,
            string: Some(StringConstraintsDto {
                regex: Some("^[a-z-]+$".to_string()),
                placeholder: Some("my-post-title".to_string()),
                default: Some("hello-world".to_string()),
            }),
        })
    );
}

#[test]
fn choice_labels_and_descriptions_survive_to_dto() {
    let dto = InputFieldDto::from(&field(
        "mode",
        InputKind::Options(choices()),
        FieldConstraints::default(),
    ));

    let InputFieldType::Select { options } = dto.field_type else {
        panic!("Options input must convert to a Select DTO");
    };
    assert_eq!(options, expected_choice_dtos());
}
