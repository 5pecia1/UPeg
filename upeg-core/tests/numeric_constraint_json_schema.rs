#[cfg(test)]
mod tests {
    use serde_json::json;
    use upeg_core::{
        FieldConstraints, InputAdapterError, InputFieldSpec, InputKind, InputName, InputSpec,
        InputSpecError, NumberConstraints,
    };

    fn numeric_field(
        name: &str,
        kind: InputKind,
        number_constraints: NumberConstraints,
    ) -> Result<InputFieldSpec, InputSpecError> {
        InputFieldSpec::with_constraints(
            InputName::new(name)?,
            None,
            None,
            true,
            kind,
            FieldConstraints {
                number: Some(number_constraints),
                string: None,
            },
        )
    }

    #[test]
    fn number_and_integer_constraints_roundtrip_through_json_schema()
    -> Result<(), InputAdapterError> {
        let spec = InputSpec::new(vec![
            numeric_field(
                "ratio",
                InputKind::Number,
                NumberConstraints {
                    min: Some(0.25),
                    max: Some(4.5),
                    default: Some(1.5),
                },
            )?,
            numeric_field(
                "max_output_bytes",
                InputKind::Integer,
                NumberConstraints {
                    min: Some(1.0),
                    max: Some(67_108_864.0),
                    default: Some(1_048_576.0),
                },
            )?,
        ])?;

        let json_schema = spec.to_json_schema_value();
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
        let restored_spec = InputSpec::try_from(&json_schema)?;

        assert_eq!(restored_spec, spec);
        Ok(())
    }

    #[test]
    fn integer_input_accepts_the_default_it_advertises() -> Result<(), InputAdapterError> {
        // The bug this pins: `password_generate` advertised
        // `"default": 20.0` and then rejected 20.0 as "not an integer".
        let spec = InputSpec::new(vec![numeric_field(
            "length",
            InputKind::Integer,
            NumberConstraints {
                min: Some(8.0),
                max: Some(64.0),
                default: Some(20.0),
            },
        )?])?;
        let default_value = spec.to_json_schema_value()["properties"]["length"]["default"].clone();
        let args = |value: serde_json::Value| {
            let mut map = serde_json::Map::new();
            map.insert("length".to_string(), value);
            map
        };

        assert!(
            spec.validate_json_args(&args(default_value)).is_ok(),
            "the advertised default must validate",
        );
        assert!(
            spec.validate_json_args(&args(json!(20.0))).is_ok(),
            "JSON has one number type: 20.0 denotes the same integer as 20",
        );
        assert!(
            spec.validate_json_args(&args(json!(20.5))).is_err(),
            "a genuine fraction is still not an integer",
        );
        Ok(())
    }

    #[test]
    fn non_numeric_json_schema_constraint_values_are_rejected() {
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
    fn json_schema_numeric_constraints_on_non_numeric_fields_are_rejected() {
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
