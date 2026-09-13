//! `render_tests` 모듈의 짝 — 선언된 입력 제약(기본값/범위/플레이스홀더)이
//! Form 보기까지 도달하는지만 확인한다. 워크스페이스 1000-LoC 파일 크기
//! 예산 때문에 분리했다.

use super::*;
use upeg_core::{FieldConstraints, NumberConstraints, StringConstraints};

fn constrained_field(
    name: &str,
    description: Option<&str>,
    kind: InputKind,
    constraints: FieldConstraints,
) -> InputFieldSpec {
    InputFieldSpec::with_constraints(
        InputName::new(name).expect("테스트 입력 이름"),
        None,
        description.map(str::to_string),
        false,
        kind,
        constraints,
    )
    .expect("테스트 입력 필드")
}

fn form_state(fields: Vec<InputFieldSpec>) -> TuiFormState {
    TuiFormState::new(InputSpec::new(fields).expect("테스트 입력 명세"))
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
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("테스트 터미널");
    let tools = list_tools();
    terminal
        .draw(|f| render(f, &state, &tools))
        .expect("렌더 성공");
    format!("{:?}", terminal.backend().buffer())
}

#[test]
fn form_보기는_선언된_기본값을_초기_draft로_보여준다() {
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
        "Form 보기는 필드 이름을 보여줘야 한다. got: {buf}"
    );
    assert!(
        buf.contains("20"),
        "선언된 기본값이 초기 draft에 들어가야 한다. got: {buf}"
    );
}

#[test]
fn form_보기는_설명_아래에_최소_최대를_보여준다() {
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
        "필드 설명이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("min 8"),
        "선언된 최소값이 보여야 한다. got: {buf}"
    );
    assert!(
        buf.contains("max 128"),
        "선언된 최대값이 보여야 한다. got: {buf}"
    );
}

#[test]
fn form_보기는_플레이스홀더를_예시로_보여준다() {
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
        "선언된 플레이스홀더가 예시로 보여야 한다. got: {buf}"
    );
}
