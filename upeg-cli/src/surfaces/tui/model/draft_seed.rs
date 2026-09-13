//! Initial [`DraftInputValue`] for one TUI form field.
//!
//! A declared default (`Number(default=…)` / `Integer(default=…)` /
//! `String(default=…)`) is the tool author saying "start here", so the
//! TUI seeds the draft with it instead of an empty box — the same
//! contract the Flutter generic form honours. A field with no declared
//! default keeps the neutral empty draft.

use upeg_core::{DraftInputValue, InputFieldSpec, InputKind};

pub(super) fn initial_tui_draft(spec: &InputFieldSpec) -> DraftInputValue {
    match &spec.kind {
        InputKind::Boolean => DraftInputValue::Boolean(false),
        InputKind::Options(_) => DraftInputValue::Options(None),
        InputKind::File(_) => DraftInputValue::File(None),
        InputKind::Number | InputKind::Integer => {
            DraftInputValue::Text(seeded_number_text(spec).unwrap_or_default())
        }
        InputKind::String => DraftInputValue::Text(seeded_string_text(spec).unwrap_or_default()),
        InputKind::MultiOptions(_)
        | InputKind::Markdown
        | InputKind::Json
        | InputKind::DateTime
        | InputKind::FilePath
        | InputKind::Url => DraftInputValue::Text(String::new()),
    }
}

/// `f64`'s `Display` already prints the shortest round-tripping form, so
/// a `default=20` constraint renders as `20`, not `20.0`.
fn seeded_number_text(spec: &InputFieldSpec) -> Option<String> {
    let default = spec.constraints.number.as_ref()?.default?;
    Some(default.to_string())
}

fn seeded_string_text(spec: &InputFieldSpec) -> Option<String> {
    spec.constraints.string.as_ref()?.default.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::{FieldConstraints, InputName, NumberConstraints, StringConstraints};

    fn field(kind: InputKind, constraints: FieldConstraints) -> InputFieldSpec {
        InputFieldSpec::with_constraints(
            InputName::new("value").expect("테스트 입력 이름은 유효해야 한다"),
            None,
            None,
            false,
            kind,
            constraints,
        )
        .expect("테스트 입력 필드는 유효해야 한다")
    }

    fn number_constraints(default: Option<f64>) -> FieldConstraints {
        FieldConstraints {
            number: Some(NumberConstraints {
                min: None,
                max: None,
                default,
            }),
            string: None,
        }
    }

    #[test]
    fn 정수_기본값은_초기_draft에_소수점_없이_들어간다() {
        let draft = initial_tui_draft(&field(InputKind::Integer, number_constraints(Some(20.0))));

        assert_eq!(draft, DraftInputValue::Text("20".to_string()));
    }

    #[test]
    fn 숫자_기본값은_초기_draft에_들어간다() {
        let draft = initial_tui_draft(&field(InputKind::Number, number_constraints(Some(1.5))));

        assert_eq!(draft, DraftInputValue::Text("1.5".to_string()));
    }

    #[test]
    fn 기본값이_없는_숫자_필드는_빈_draft로_시작한다() {
        let draft = initial_tui_draft(&field(InputKind::Number, number_constraints(None)));

        assert_eq!(draft, DraftInputValue::Text(String::new()));
    }

    #[test]
    fn 문자열_기본값은_초기_draft에_들어간다() {
        let draft = initial_tui_draft(&field(
            InputKind::String,
            FieldConstraints {
                number: None,
                string: Some(StringConstraints {
                    regex: None,
                    placeholder: Some("my-post-title".to_string()),
                    default: Some("hello-world".to_string()),
                }),
            },
        ));

        assert_eq!(draft, DraftInputValue::Text("hello-world".to_string()));
    }

    #[test]
    fn 플레이스홀더는_draft를_채우지_않는다() {
        let draft = initial_tui_draft(&field(
            InputKind::String,
            FieldConstraints {
                number: None,
                string: Some(StringConstraints {
                    regex: None,
                    placeholder: Some("my-post-title".to_string()),
                    default: None,
                }),
            },
        ));

        assert_eq!(draft, DraftInputValue::Text(String::new()));
    }

    #[test]
    fn 제약이_없는_종류들은_기존_초기값을_유지한다() {
        assert_eq!(
            initial_tui_draft(&field(InputKind::Boolean, FieldConstraints::default())),
            DraftInputValue::Boolean(false)
        );
        assert_eq!(
            initial_tui_draft(&field(InputKind::Markdown, FieldConstraints::default())),
            DraftInputValue::Text(String::new())
        );
    }
}
