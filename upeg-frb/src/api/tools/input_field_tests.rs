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
    InputName::new(raw).expect("테스트 입력 이름은 유효해야 한다")
}

fn choice(value: &str, label: Option<&str>, description: Option<&str>) -> ChoiceOption {
    ChoiceOption::new(
        value,
        label.map(str::to_string),
        description.map(str::to_string),
    )
    .expect("테스트 선택지는 유효해야 한다")
}

fn choices() -> ChoiceSpec {
    ChoiceSpec::new(vec![
        choice("fast", Some("Fast"), Some("lower quality")),
        choice("safe", None, None),
    ])
    .expect("테스트 선택지 목록은 유효해야 한다")
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
    .expect("테스트 입력 필드는 유효해야 한다")
}

#[test]
fn 입력_종류는_모든_변형이_고유한_dto로_매핑된다() {
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
            "입력 종류 `{}` 매핑이 어긋났다",
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
fn number와_integer는_서로_다른_dto_변형으로_남는다() {
    assert_ne!(
        InputFieldType::from(&InputKind::Number),
        InputFieldType::from(&InputKind::Integer),
        "Number와 Integer를 하나로 접으면 Dart가 정수 전용 키보드/파싱을 고를 수 없다",
    );
}

#[test]
fn 입력_필드_dto는_설명을_그대로_전달한다() {
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
fn 라벨이_없는_입력_필드는_이름으로_대체된다() {
    let spec = InputFieldSpec::new(input_name("input"), None, None, false, InputKind::String)
        .expect("테스트 입력 필드는 유효해야 한다");

    let dto = InputFieldDto::from(&spec);

    assert_eq!(dto.label, "input");
    assert_eq!(dto.description, None);
    assert!(!dto.required);
}

#[test]
fn 숫자_제약은_최소_최대_기본값을_함께_전달한다() {
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
fn 문자열_제약은_정규식_플레이스홀더_기본값을_함께_전달한다() {
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
fn 선택지_라벨과_설명은_dto까지_살아남는다() {
    let dto = InputFieldDto::from(&field(
        "mode",
        InputKind::Options(choices()),
        FieldConstraints::default(),
    ));

    let InputFieldType::Select { options } = dto.field_type else {
        panic!("Options 입력은 Select DTO로 변환되어야 한다");
    };
    assert_eq!(options, expected_choice_dtos());
}
