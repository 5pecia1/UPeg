#[cfg(test)]
mod tests {
    use serde_json::json;
    use upeg_core::{
        FieldConstraints, InputAdapterError, InputFieldSpec, InputKind, InputName, InputSpec,
        InputSpecError, NumberConstraints,
    };

    fn 숫자_필드(
        이름: &str,
        종류: InputKind,
        숫자_제약: NumberConstraints,
    ) -> Result<InputFieldSpec, InputSpecError> {
        InputFieldSpec::with_constraints(
            InputName::new(이름)?,
            None,
            None,
            true,
            종류,
            FieldConstraints {
                number: Some(숫자_제약),
                string: None,
            },
        )
    }

    #[test]
    fn 숫자와_정수_제약은_json_schema에서_왕복된다() -> Result<(), InputAdapterError> {
        let 명세 = InputSpec::new(vec![
            숫자_필드(
                "ratio",
                InputKind::Number,
                NumberConstraints {
                    min: Some(0.25),
                    max: Some(4.5),
                    default: Some(1.5),
                },
            )?,
            숫자_필드(
                "max_output_bytes",
                InputKind::Integer,
                NumberConstraints {
                    min: Some(1.0),
                    max: Some(67_108_864.0),
                    default: Some(1_048_576.0),
                },
            )?,
        ])?;

        let json_schema = 명세.to_json_schema_value();
        assert_eq!(
            json_schema["properties"]["ratio"],
            json!({
                "type": "number",
                "minimum": 0.25,
                "maximum": 4.5,
                "default": 1.5,
            })
        );
        // Integer bounds must serialize as JSON integers. `20.0` is a
        // number `validate_integer_value` used to refuse, so a float
        // here would advertise a default the tool itself rejects.
        assert_eq!(
            json_schema["properties"]["max_output_bytes"],
            json!({
                "type": "integer",
                "minimum": 1,
                "maximum": 67_108_864,
                "default": 1_048_576,
            })
        );
        let 복원된_명세 = InputSpec::try_from(&json_schema)?;

        assert_eq!(복원된_명세, 명세);
        Ok(())
    }

    #[test]
    fn 정수_입력은_자신이_광고한_기본값을_스스로_받아들인다() -> Result<(), InputAdapterError> {
        // The bug this pins: `password_generate` advertised
        // `"default": 20.0` and then rejected 20.0 as "not an integer".
        let 명세 = InputSpec::new(vec![숫자_필드(
            "length",
            InputKind::Integer,
            NumberConstraints {
                min: Some(8.0),
                max: Some(64.0),
                default: Some(20.0),
            },
        )?])?;
        let 기본값 = 명세.to_json_schema_value()["properties"]["length"]["default"].clone();
        let 인자 = |값: serde_json::Value| {
            let mut map = serde_json::Map::new();
            map.insert("length".to_string(), 값);
            map
        };

        assert!(
            명세.validate_json_args(&인자(기본값)).is_ok(),
            "the advertised default must validate",
        );
        assert!(
            명세.validate_json_args(&인자(json!(20.0))).is_ok(),
            "JSON has one number type: 20.0 denotes the same integer as 20",
        );
        assert!(
            명세.validate_json_args(&인자(json!(20.5))).is_err(),
            "a genuine fraction is still not an integer",
        );
        Ok(())
    }

    #[test]
    fn 숫자가_아닌_json_schema_제약값은_거부된다() {
        let json_schema = json!({
            "type": "object",
            "properties": {
                "ratio": {
                    "type": "number",
                    "minimum": "zero"
                }
            }
        });

        assert!(matches!(
            InputSpec::try_from(&json_schema),
            Err(InputAdapterError::JsonSchemaNumericConstraintNotNumber {
                property,
                keyword: "minimum",
                kind: "string",
            }) if property == "ratio"
        ));
    }

    #[test]
    fn 숫자가_아닌_필드의_json_schema_숫자_제약은_거부된다() {
        let json_schema = json!({
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "default": 1
                }
            }
        });

        assert!(matches!(
            InputSpec::try_from(&json_schema),
            Err(InputAdapterError::JsonSchemaNumericConstraintUnsupportedKind {
                property,
                keyword: "default",
                kind: "boolean",
            }) if property == "enabled"
        ));
    }
}
