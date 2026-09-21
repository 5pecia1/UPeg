    use super::*;
    use crate::input::FileContent;

    #[test]
    fn output_spec_rejects_duplicate_names() {
        let spec = OutputSpec::new(vec![
            OutputFieldSpec {
                name: "x".to_string(),
                label: None,
                description: None,
                kind: OutputKind::String,
                constraints: FieldConstraints::default(),
            },
            OutputFieldSpec {
                name: "x".to_string(),
                label: None,
                description: None,
                kind: OutputKind::Number,
                constraints: FieldConstraints::default(),
            },
        ]);
        assert!(matches!(
            spec,
            Err(OutputSpecError::DuplicateName { name }) if name == "x"
        ));
    }

    #[test]
    fn output_spec_rejects_empty_names() {
        let spec = OutputSpec::new(vec![OutputFieldSpec {
            name: String::new(),
            label: None,
            description: None,
            kind: OutputKind::String,
            constraints: FieldConstraints::default(),
        }]);
        assert!(matches!(spec, Err(OutputSpecError::EmptyName)));
    }

    #[test]
    fn output_spec_rejects_padded_names() {
        let spec = OutputSpec::new(vec![OutputFieldSpec {
            name: " name".to_string(),
            label: None,
            description: None,
            kind: OutputKind::String,
            constraints: FieldConstraints::default(),
        }]);
        assert!(matches!(spec, Err(OutputSpecError::PaddedName { .. })));
    }

    #[test]
    fn static_output_spec_converts_to_owned() {
        static FIELDS: &[StaticOutputFieldSpec] = &[
            StaticOutputFieldSpec {
                name: "gwei",
                label: Some("Gwei"),
                description: None,
                kind: StaticOutputKind::Number,
                constraints: StaticFieldConstraints::empty(),
            },
            StaticOutputFieldSpec {
                name: "view",
                label: None,
                description: None,
                kind: StaticOutputKind::EmbeddedView {
                    url: "https://transform.tools/",
                },
                constraints: StaticFieldConstraints::empty(),
            },
        ];
        let owned = StaticOutputSpec { fields: FIELDS }
            .to_output_spec()
            .expect("conversion should succeed");
        assert_eq!(owned.fields.len(), 2);
        assert_eq!(owned.fields[0].name, "gwei");
        assert!(matches!(owned.fields[0].kind, OutputKind::Number));
        assert_eq!(owned.fields[1].name, "view");
        match &owned.fields[1].kind {
            OutputKind::EmbeddedView { url } => {
                assert_eq!(url, "https://transform.tools/");
            }
            other => panic!("expected EmbeddedView, got {other:?}"),
        }
    }

    #[test]
    fn output_spec_exports_as_json_schema_value() {
        let spec = OutputSpec::new(vec![
            OutputFieldSpec {
                name: "summary".to_string(),
                label: Some("Summary".to_string()),
                description: Some("short output".to_string()),
                kind: OutputKind::Markdown,
                constraints: FieldConstraints::default(),
            },
            OutputFieldSpec {
                name: "count".to_string(),
                label: None,
                description: None,
                kind: OutputKind::Integer,
                constraints: FieldConstraints::default(),
            },
            OutputFieldSpec {
                name: "view".to_string(),
                label: None,
                description: None,
                kind: OutputKind::EmbeddedView {
                    url: "https://example.com/".to_string(),
                },
                constraints: FieldConstraints::default(),
            },
        ])
        .expect("valid output spec");

        let schema = spec.to_json_schema_value();

        assert_eq!(schema["type"], serde_json::json!("object"));
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
        assert_eq!(
            schema["properties"]["summary"]["type"],
            serde_json::json!("string")
        );
        assert_eq!(
            schema["properties"]["summary"]["format"],
            serde_json::json!("markdown")
        );
        assert_eq!(
            schema["properties"]["summary"]["x-upeg-kind"],
            serde_json::json!("markdown")
        );
        assert_eq!(
            schema["properties"]["summary"]["title"],
            serde_json::json!("Summary")
        );
        assert_eq!(
            schema["properties"]["summary"]["description"],
            serde_json::json!("short output")
        );
        assert_eq!(
            schema["properties"]["count"]["type"],
            serde_json::json!("integer")
        );
        assert_eq!(
            schema["properties"]["view"]["type"],
            serde_json::json!("string")
        );
        assert_eq!(
            schema["properties"]["view"]["format"],
            serde_json::json!("uri")
        );
        assert_eq!(
            schema["properties"]["view"]["x-upeg-kind"],
            serde_json::json!("embedded_view")
        );
        assert_eq!(
            schema["properties"]["view"]["x-upeg-url"],
            serde_json::json!("https://example.com/")
        );
        assert!(
            schema["required"].as_array().is_some_and(Vec::is_empty),
            "output schema export keeps required empty until OutputSpec models requiredness"
        );
    }

    #[test]
    fn single_number_output_wraps_text_as_structured_content() {
        let spec = OutputSpec::new(vec![OutputFieldSpec {
            name: "result".to_string(),
            label: None,
            description: None,
            kind: OutputKind::Number,
            constraints: FieldConstraints::default(),
        }])
        .expect("valid output spec");

        assert_eq!(
            spec.structured_content_from_text("255"),
            Some(serde_json::json!({"result": 255}))
        );
    }

    #[test]
    fn json_output_parses_text_json_into_a_structured_content_value() {
        let spec = OutputSpec::new(vec![OutputFieldSpec {
            name: "formatted".to_string(),
            label: None,
            description: None,
            kind: OutputKind::Json,
            constraints: FieldConstraints::default(),
        }])
        .expect("valid output spec");

        assert_eq!(
            spec.structured_content_from_text(r#"{"ok":true}"#),
            Some(serde_json::json!({"formatted": {"ok": true}}))
        );
    }

    #[test]
    fn multiple_outputs_use_structured_content_only_for_object_text() {
        let spec = OutputSpec::new(vec![
            OutputFieldSpec {
                name: "left".to_string(),
                label: None,
                description: None,
                kind: OutputKind::String,
                constraints: FieldConstraints::default(),
            },
            OutputFieldSpec {
                name: "right".to_string(),
                label: None,
                description: None,
                kind: OutputKind::Integer,
                constraints: FieldConstraints::default(),
            },
        ])
        .expect("valid output spec");

        assert_eq!(
            spec.structured_content_from_text(r#"{"left":"a","right":1}"#),
            Some(serde_json::json!({"left": "a", "right": 1}))
        );
        assert_eq!(spec.structured_content_from_text("plain text"), None);
    }

    #[test]
    fn output_kind_labels_are_unique_for_every_variant() {
        let kinds = [
            OutputKind::String,
            OutputKind::Number,
            OutputKind::Integer,
            OutputKind::Boolean,
            OutputKind::Markdown,
            OutputKind::Json,
            OutputKind::DateTime,
            OutputKind::FilePath,
            OutputKind::Url,
            OutputKind::File,
            OutputKind::EmbeddedView {
                url: "x".to_string(),
            },
        ];
        let labels: HashSet<&str> = kinds.iter().map(OutputKind::label).collect();
        assert_eq!(labels.len(), kinds.len());
    }

    #[test]
    fn output_value_can_hold_files_and_view_embeds() {
        let file = OutputValue::File(FileValue {
            name: "x.bin".to_string(),
            content: FileContent::Bytes(vec![1, 2, 3]),
            mime: None,
        });
        let view = OutputValue::EmbeddedView("https://example.com/".to_string());
        assert!(matches!(file, OutputValue::File(_)));
        assert!(matches!(view, OutputValue::EmbeddedView(_)));
    }

    #[test]
    fn single_output_serializes_to_json_as_primary() {
        let success = ToolSuccess::new(
            Some("result".to_string()),
            vec![OutputEntry {
                id: "result".to_string(),
                label: Some("Result".to_string()),
                kind: OutputKind::String,
                value: OutputValue::String("ok".to_string()),
            }],
        )
        .expect("valid success");

        assert_eq!(
            ToolResult::Success(success).to_canonical_json(),
            serde_json::json!({
                "ok": true,
                "primary_output_id": "result",
                "outputs": [
                    {
                        "id": "result",
                        "label": "Result",
                        "kind": "string",
                        "value": "ok"
                    }
                ]
            })
        );
    }

    #[test]
    fn multiple_outputs_serialize_every_entry_to_json() {
        let success = ToolSuccess::new(
            Some("summary".to_string()),
            vec![
                OutputEntry {
                    id: "summary".to_string(),
                    label: Some("Summary".to_string()),
                    kind: OutputKind::Markdown,
                    value: OutputValue::Markdown("done".to_string()),
                },
                OutputEntry {
                    id: "count".to_string(),
                    label: None,
                    kind: OutputKind::Integer,
                    value: OutputValue::Integer(2),
                },
            ],
        )
        .expect("valid success");

        assert_eq!(
            success.to_canonical_json(),
            serde_json::json!({
                "ok": true,
                "primary_output_id": "summary",
                "outputs": [
                    {
                        "id": "summary",
                        "label": "Summary",
                        "kind": "markdown",
                        "value": "done"
                    },
                    {
                        "id": "count",
                        "label": null,
                        "kind": "integer",
                        "value": 2
                    }
                ]
            })
        );
    }

    #[test]
    fn without_outputs_primary_is_null() {
        let success = ToolSuccess::new(None, Vec::new()).expect("valid success");

        assert_eq!(
            success.to_canonical_json(),
            serde_json::json!({
                "ok": true,
                "primary_output_id": null,
                "outputs": []
            })
        );
    }

    #[test]
    fn multiple_outputs_without_primary_output_id_are_rejected() {
        let success = ToolSuccess::new(
            None,
            vec![
                OutputEntry {
                    id: "left".to_string(),
                    label: None,
                    kind: OutputKind::String,
                    value: OutputValue::String("left".to_string()),
                },
                OutputEntry {
                    id: "right".to_string(),
                    label: None,
                    kind: OutputKind::String,
                    value: OutputValue::String("right".to_string()),
                },
            ],
        );

        assert!(matches!(
            success,
            Err(ToolResultError::MissingPrimaryOutputId)
        ));
    }

    #[test]
    fn invalid_primary_output_id_is_rejected() {
        let success = ToolSuccess::new(
            Some("missing".to_string()),
            vec![OutputEntry {
                id: "result".to_string(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String("ok".to_string()),
            }],
        );

        assert!(matches!(
            success,
            Err(ToolResultError::InvalidPrimaryOutputId { id }) if id == "missing"
        ));
    }

    #[test]
    fn duplicate_output_ids_are_rejected() {
        let success = ToolSuccess::new(
            Some("result".to_string()),
            vec![
                OutputEntry {
                    id: "result".to_string(),
                    label: None,
                    kind: OutputKind::String,
                    value: OutputValue::String("first".to_string()),
                },
                OutputEntry {
                    id: "result".to_string(),
                    label: None,
                    kind: OutputKind::String,
                    value: OutputValue::String("second".to_string()),
                },
            ],
        );

        assert!(matches!(
            success,
            Err(ToolResultError::DuplicateOutputId { id }) if id == "result"
        ));
    }

    #[test]
    fn error_serializes_as_structured_json() {
        let failure = ToolResult::Failure(ToolFailure {
            error: ToolError {
                code: "tool.failed".to_string(),
                message: "tool execution failed".to_string(),
                details: Some(serde_json::json!({"exit_code": 2})),
            },
        });

        assert_eq!(
            failure.to_canonical_json(),
            serde_json::json!({
                "ok": false,
                "error": {
                    "code": "tool.failed",
                    "message": "tool execution failed",
                    "details": {"exit_code": 2}
                }
            })
        );
    }
