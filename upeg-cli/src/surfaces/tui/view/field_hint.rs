//! Constraint hint line for one TUI form field.
//!
//! The form already renders label, required marker, kind and description.
//! What it used to drop on the floor is the field's declared constraints,
//! so a user typing into `count` had no way to learn the accepted range
//! without reading the tool's prose. This renders the machine-readable
//! part of that contract — numeric range, string placeholder — next to
//! the description.

use upeg_core::{InputFieldSpec, NumberConstraints, StringConstraints};

const MIN_HINT_PREFIX: &str = "min ";
const MAX_HINT_PREFIX: &str = "max ";
const PLACEHOLDER_HINT_PREFIX: &str = "e.g. ";
const HINT_SEPARATOR: &str = " · ";

/// Machine-readable constraint summary, or `None` when the field
/// declares nothing worth showing.
pub(super) fn constraint_hint(spec: &InputFieldSpec) -> Option<String> {
    let number_hint = spec.constraints.number.as_ref().and_then(number_range_hint);
    let string_hint = spec.constraints.string.as_ref().and_then(placeholder_hint);
    match (number_hint, string_hint) {
        (Some(number), Some(string)) => Some(format!("{number}{HINT_SEPARATOR}{string}")),
        (Some(hint), None) | (None, Some(hint)) => Some(hint),
        (None, None) => None,
    }
}

fn number_range_hint(constraints: &NumberConstraints) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(min) = constraints.min {
        parts.push(format!("{MIN_HINT_PREFIX}{min}"));
    }
    if let Some(max) = constraints.max {
        parts.push(format!("{MAX_HINT_PREFIX}{max}"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join(HINT_SEPARATOR))
}

fn placeholder_hint(constraints: &StringConstraints) -> Option<String> {
    let placeholder = constraints
        .placeholder
        .as_deref()
        .filter(|value| !value.is_empty())?;
    Some(format!("{PLACEHOLDER_HINT_PREFIX}{placeholder}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use upeg_core::{FieldConstraints, InputKind, InputName};

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

    fn number(min: Option<f64>, max: Option<f64>) -> FieldConstraints {
        FieldConstraints {
            number: Some(upeg_core::NumberConstraints {
                min,
                max,
                default: None,
            }),
            string: None,
        }
    }

    #[test]
    fn 숫자_범위는_최소와_최대를_함께_보여준다() {
        let hint = constraint_hint(&field(InputKind::Integer, number(Some(8.0), Some(128.0))));

        assert_eq!(hint.as_deref(), Some("min 8 · max 128"));
    }

    #[test]
    fn 한쪽만_선언된_범위는_그쪽만_보여준다() {
        assert_eq!(
            constraint_hint(&field(InputKind::Number, number(Some(0.0), None))).as_deref(),
            Some("min 0"),
        );
        assert_eq!(
            constraint_hint(&field(InputKind::Number, number(None, Some(1.0)))).as_deref(),
            Some("max 1"),
        );
    }

    #[test]
    fn 기본값만_선언된_숫자_필드는_힌트가_없다() {
        let constraints = FieldConstraints {
            number: Some(upeg_core::NumberConstraints {
                min: None,
                max: None,
                default: Some(20.0),
            }),
            string: None,
        };

        assert_eq!(
            constraint_hint(&field(InputKind::Integer, constraints)),
            None
        );
    }

    #[test]
    fn 플레이스홀더는_예시로_보여준다() {
        let constraints = FieldConstraints {
            number: None,
            string: Some(StringConstraints {
                regex: Some("^[a-z-]+$".to_string()),
                placeholder: Some("my-post-title".to_string()),
                default: None,
            }),
        };

        assert_eq!(
            constraint_hint(&field(InputKind::String, constraints)).as_deref(),
            Some("e.g. my-post-title"),
        );
    }

    #[test]
    fn 제약이_없으면_힌트도_없다() {
        assert_eq!(
            constraint_hint(&field(InputKind::String, FieldConstraints::default())),
            None
        );
    }
}
