use super::*;

static STATIC_CHOICES: &[StaticChoiceOption] = &[
    StaticChoiceOption {
        value: "fast",
        label: Some("Fast"),
        description: Some("fast path"),
    },
    StaticChoiceOption {
        value: "safe",
        label: Some("Safe"),
        description: Some("safe path"),
    },
];

static STATIC_FIELDS: &[StaticInputFieldSpec] = &[
    StaticInputFieldSpec {
        name: "string",
        label: Some("String"),
        description: Some("text input"),
        required: true,
        kind: StaticInputKind::String,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "number",
        label: None,
        description: None,
        required: true,
        kind: StaticInputKind::Number,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "integer",
        label: None,
        description: None,
        required: true,
        kind: StaticInputKind::Integer,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "boolean",
        label: None,
        description: None,
        required: true,
        kind: StaticInputKind::Boolean,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "options",
        label: None,
        description: None,
        required: true,
        kind: StaticInputKind::Options(STATIC_CHOICES),
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "multi_options",
        label: None,
        description: None,
        required: true,
        kind: StaticInputKind::MultiOptions(STATIC_CHOICES),
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "markdown",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::Markdown,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "json",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::Json,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "datetime",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::DateTime,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "file_path",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::FilePath,
        constraints: StaticFieldConstraints::empty(),
    },
    StaticInputFieldSpec {
        name: "url",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::Url,
        constraints: StaticFieldConstraints::empty(),
    },
];

fn input_name(raw: &str) -> InputName {
    InputName::new(raw).expect("test input name should be valid")
}

fn choice(value: &str) -> ChoiceOption {
    ChoiceOption::new(value, None, None).expect("test choice should be valid")
}

fn choices(values: &[&str]) -> ChoiceSpec {
    ChoiceSpec::new(values.iter().map(|value| choice(value)).collect())
        .expect("test choice spec should be valid")
}

fn field(name: &str, required: bool, kind: InputKind) -> InputFieldSpec {
    InputFieldSpec::new(input_name(name), None, None, required, kind)
        .expect("test field should be valid")
}

fn object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value
        .as_object()
        .expect("test value should be a json object")
        .clone()
}

fn all_kind_spec() -> InputSpec {
    InputSpec::new(vec![
        field("string", true, InputKind::String),
        field("number", true, InputKind::Number),
        field("integer", true, InputKind::Integer),
        field("boolean", true, InputKind::Boolean),
        field(
            "options",
            true,
            InputKind::Options(choices(&["fast", "safe"])),
        ),
        field(
            "multi_options",
            true,
            InputKind::MultiOptions(choices(&["fast", "safe"])),
        ),
        field("markdown", false, InputKind::Markdown),
        field("json", false, InputKind::Json),
        field("datetime", false, InputKind::DateTime),
        field("file_path", false, InputKind::FilePath),
        field("url", false, InputKind::Url),
    ])
    .expect("all kind spec should be valid")
}

#[test]
fn empty_input_spec_has_no_fields_and_a_valid_empty_state() {
    let spec = InputSpec::empty();

    assert!(spec.fields.is_empty());
    assert_eq!(spec.initial_form_state(), FormState { fields: Vec::new() });
    assert_eq!(spec.validate_json_args(&serde_json::Map::new()), Ok(()));
    assert_eq!(
        spec.args_from_form_state(&FormState { fields: Vec::new() })
            .expect("empty state should convert"),
        serde_json::json!({})
    );
}

#[test]
fn input_name_constructor_rejects_empty_and_padded_names() {
    assert_eq!(InputName::new(""), Err(InputSpecError::EmptyName));
    assert!(matches!(
        InputName::new(" name"),
        Err(InputSpecError::PaddedName { .. })
    ));
    assert!(matches!(
        InputName::new("name "),
        Err(InputSpecError::PaddedName { .. })
    ));

    let name = InputName::new("name").expect("canonical name should be accepted");
    assert_eq!(name.as_str(), "name");
    assert_eq!(String::from(name), "name");
}

#[test]
fn input_constructors_preserve_metadata() {
    let option = ChoiceOption::new(
        "fast",
        Some("Fast".to_string()),
        Some("fast path".to_string()),
    )
    .expect("choice option should construct");
    let choices = ChoiceSpec::new(vec![option.clone()]).expect("choice spec should construct");
    let field = InputFieldSpec::new(
        input_name("mode"),
        Some("Mode".to_string()),
        Some("run mode".to_string()),
        true,
        InputKind::Options(choices.clone()),
    )
    .expect("field should construct");

    assert_eq!(choices.options, vec![option]);
    assert_eq!(field.name.as_str(), "mode");
    assert_eq!(field.label.as_deref(), Some("Mode"));
    assert_eq!(field.description.as_deref(), Some("run mode"));
    assert!(field.required);
    assert_eq!(field.kind, InputKind::Options(choices));
}

#[test]
fn input_choice_validation_rejects_empty_and_duplicate_options() {
    assert_eq!(
        ChoiceSpec::new(Vec::new()),
        Err(InputSpecError::EmptyChoices)
    );
    assert_eq!(
        ChoiceSpec::new(vec![ChoiceOption {
            value: String::new(),
            label: None,
            description: None,
        }]),
        Err(InputSpecError::EmptyChoiceValue)
    );
    assert!(matches!(
        ChoiceSpec::new(vec![choice("fast"), choice("fast")]),
        Err(InputSpecError::DuplicateChoiceValue { value }) if value == "fast"
    ));
    assert_eq!(
        InputFieldSpec::new(
            input_name("mode"),
            None,
            None,
            true,
            InputKind::Options(ChoiceSpec {
                options: Vec::new()
            }),
        ),
        Err(InputSpecError::EmptyChoices)
    );
}

#[test]
fn new_input_spec_rejects_duplicate_names_and_preserves_order() {
    let spec = InputSpec::new(vec![
        field("second", false, InputKind::String),
        field("first", false, InputKind::Boolean),
    ])
    .expect("unique fields should construct");

    let names: Vec<_> = spec
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, vec!["second", "first"]);

    assert!(matches!(
        InputSpec::new(vec![
            field("duplicate", false, InputKind::String),
            field("duplicate", false, InputKind::Boolean),
        ]),
        Err(InputSpecError::DuplicateName { name }) if name.as_str() == "duplicate"
    ));
}

#[test]
fn static_input_fields_convert_every_static_kind() {
    let spec = StaticInputSpec {
        fields: STATIC_FIELDS,
    }
    .to_input_spec()
    .expect("static input spec should convert");

    let labels: Vec<_> = spec.fields.iter().map(|field| field.kind.label()).collect();
    assert_eq!(
        labels,
        vec![
            "string",
            "number",
            "integer",
            "boolean",
            "options",
            "multi_options",
            "markdown",
            "json",
            "datetime",
            "file_path",
            "url",
        ]
    );
    assert_eq!(spec.fields[0].label.as_deref(), Some("String"));
    assert_eq!(spec.fields[0].description.as_deref(), Some("text input"));
    assert!(spec.fields[0].required);
    match &spec.fields[4].kind {
        InputKind::Options(choices) => {
            assert_eq!(choices.allowed_values(), vec!["fast", "safe"]);
            assert_eq!(choices.options[0].label.as_deref(), Some("Fast"));
        }
        other => panic!("expected options kind, got {other:?}"),
    }
    match &spec.fields[5].kind {
        InputKind::MultiOptions(choices) => {
            assert_eq!(choices.allowed_values(), vec!["fast", "safe"]);
        }
        other => panic!("expected multi_options kind, got {other:?}"),
    }
}

#[test]
fn initial_form_state_uses_stable_order_and_draft_variants() {
    let spec = InputSpec::new(vec![
        field("text", false, InputKind::String),
        field("number", false, InputKind::Number),
        field("flag", false, InputKind::Boolean),
        field("choice", false, InputKind::Options(choices(&["yes"]))),
        field("many", false, InputKind::MultiOptions(choices(&["yes"]))),
    ])
    .expect("spec should construct");

    let state = spec.initial_form_state();
    let names: Vec<_> = state
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let drafts: Vec<_> = state
        .fields
        .iter()
        .map(|field| field.draft.clone())
        .collect();

    assert_eq!(names, vec!["text", "number", "flag", "choice", "many"]);
    assert_eq!(
        drafts,
        vec![
            DraftInputValue::Empty,
            DraftInputValue::Empty,
            DraftInputValue::Boolean(false),
            DraftInputValue::Options(None),
            DraftInputValue::MultiOptions(Vec::new()),
        ]
    );
    assert!(
        state
            .fields
            .iter()
            .all(|field| field.validation == FieldValidation::Unknown)
    );
}

#[test]
fn json_args_validation_accepts_every_input_kind_variant() {
    let spec = all_kind_spec();
    let args = object(serde_json::json!({
        "string": "hello",
        "number": 1.5,
        "integer": 42,
        "boolean": false,
        "options": "fast",
        "multi_options": ["fast", "safe"],
        "markdown": "# title",
        "json": { "ok": true },
        "datetime": "2026-05-20T00:00:00Z",
        "file_path": "/tmp/input.txt",
        "url": "https://example.com",
    }));

    assert_eq!(spec.validate_json_args(&args), Ok(()));

    let labels: Vec<_> = spec.fields.iter().map(|field| field.kind.label()).collect();
    assert_eq!(
        labels,
        vec![
            "string",
            "number",
            "integer",
            "boolean",
            "options",
            "multi_options",
            "markdown",
            "json",
            "datetime",
            "file_path",
            "url",
        ]
    );
}

#[test]
fn json_args_validation_rejects_missing_required_and_invalid_choices() {
    let spec = all_kind_spec();
    let missing_bool = object(serde_json::json!({
        "string": "hello",
        "number": 1,
        "integer": 1,
        "options": "fast",
        "multi_options": ["fast"],
    }));
    assert!(matches!(
        spec.validate_json_args(&missing_bool),
        Err(InputValueError::MissingRequired { name }) if name.as_str() == "boolean"
    ));

    let explicit_false = object(serde_json::json!({
        "string": "hello",
        "number": 1,
        "integer": 1,
        "boolean": false,
        "options": "fast",
        "multi_options": ["fast"],
    }));
    assert_eq!(spec.validate_json_args(&explicit_false), Ok(()));

    let bad_choice = object(serde_json::json!({
        "string": "hello",
        "number": 1,
        "integer": 1,
        "boolean": true,
        "options": "slow",
        "multi_options": ["fast"],
    }));
    assert!(matches!(
        spec.validate_json_args(&bad_choice),
        Err(InputValueError::InvalidChoice { name, allowed_values })
            if name.as_str() == "options" && allowed_values == vec!["fast", "safe"]
    ));

    let bad_multi_choice = object(serde_json::json!({
        "string": "hello",
        "number": 1,
        "integer": 1,
        "boolean": true,
        "options": "fast",
        "multi_options": ["slow"],
    }));
    assert!(matches!(
        spec.validate_json_args(&bad_multi_choice),
        Err(InputValueError::InvalidMultiChoice { name, invalid_value, .. })
            if name.as_str() == "multi_options" && invalid_value == "slow"
    ));
}

#[test]
fn json_args_validation_handles_null_by_kind_and_requiredness() {
    let spec = InputSpec::new(vec![
        field("optional_text", false, InputKind::String),
        field("required_text", true, InputKind::String),
        field("json_payload", true, InputKind::Json),
    ])
    .expect("spec should construct");

    let optional_null_and_json_null = object(serde_json::json!({
        "optional_text": null,
        "required_text": "present",
        "json_payload": null,
    }));
    assert_eq!(
        spec.validate_json_args(&optional_null_and_json_null),
        Ok(())
    );

    let required_null = object(serde_json::json!({
        "required_text": null,
        "json_payload": null,
    }));
    assert!(matches!(
        spec.validate_json_args(&required_null),
        Err(InputValueError::MissingRequired { name }) if name.as_str() == "required_text"
    ));
}

#[test]
fn json_args_validation_handles_empty_multi_options_by_requiredness() {
    let spec = InputSpec::new(vec![
        field(
            "required_flags",
            true,
            InputKind::MultiOptions(choices(&["dry", "verbose"])),
        ),
        field(
            "optional_flags",
            false,
            InputKind::MultiOptions(choices(&["dry", "verbose"])),
        ),
    ])
    .expect("spec should construct");

    let optional_empty = object(serde_json::json!({
        "required_flags": ["dry"],
        "optional_flags": [],
    }));
    assert_eq!(spec.validate_json_args(&optional_empty), Ok(()));

    let required_empty = object(serde_json::json!({
        "required_flags": [],
        "optional_flags": ["dry"],
    }));
    assert!(matches!(
        spec.validate_json_args(&required_empty),
        Err(InputValueError::MissingRequired { name }) if name.as_str() == "required_flags"
    ));
}

#[test]
fn args_from_form_state_parses_drafts_and_outputs_multi_select_arrays() {
    let spec = all_kind_spec();
    let state = FormState {
        fields: vec![
            FormFieldState {
                name: input_name("string"),
                draft: DraftInputValue::Text("hello".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("number"),
                draft: DraftInputValue::Text(" 1.5 ".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("integer"),
                draft: DraftInputValue::Text(" 42 ".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("boolean"),
                draft: DraftInputValue::Boolean(false),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("options"),
                draft: DraftInputValue::Options(Some("safe".to_string())),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("multi_options"),
                draft: DraftInputValue::MultiOptions(vec!["fast".to_string(), "safe".to_string()]),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("markdown"),
                draft: DraftInputValue::Text("# title".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("json"),
                draft: DraftInputValue::Text(r#"{ "nested": true }"#.to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("datetime"),
                draft: DraftInputValue::Text("2026-05-20T00:00:00Z".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("file_path"),
                draft: DraftInputValue::Text("/tmp/input.txt".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("url"),
                draft: DraftInputValue::Text("https://example.com".to_string()),
                validation: FieldValidation::Unknown,
            },
        ],
    };

    let args = spec
        .args_from_form_state(&state)
        .expect("form state should convert to args");

    assert_eq!(args["string"], serde_json::json!("hello"));
    assert_eq!(args["number"], serde_json::json!(1.5));
    assert_eq!(args["integer"], serde_json::json!(42));
    assert_eq!(args["boolean"], serde_json::json!(false));
    assert_eq!(args["options"], serde_json::json!("safe"));
    assert_eq!(args["multi_options"], serde_json::json!(["fast", "safe"]));
    assert_eq!(args["json"], serde_json::json!({ "nested": true }));
}

#[test]
fn args_from_form_state_omits_empty_optional_drafts() {
    let spec = InputSpec::new(vec![
        field("count", false, InputKind::Integer),
        field("payload", false, InputKind::Json),
        field("mode", false, InputKind::Options(choices(&["fast"]))),
        field("flags", false, InputKind::MultiOptions(choices(&["fast"]))),
    ])
    .expect("optional spec should construct");
    let state = FormState {
        fields: vec![
            FormFieldState {
                name: input_name("count"),
                draft: DraftInputValue::Text("   ".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("payload"),
                draft: DraftInputValue::Text(String::new()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("mode"),
                draft: DraftInputValue::Options(None),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("flags"),
                draft: DraftInputValue::MultiOptions(Vec::new()),
                validation: FieldValidation::Unknown,
            },
        ],
    };

    assert_eq!(
        spec.args_from_form_state(&state)
            .expect("empty optional drafts should convert"),
        serde_json::json!({})
    );
}

#[test]
fn args_from_form_state_rejects_wrong_draft_shapes() {
    let spec = InputSpec::new(vec![field("count", true, InputKind::Integer)])
        .expect("spec should construct");
    let wrong_kind = FormState {
        fields: vec![FormFieldState {
            name: input_name("count"),
            draft: DraftInputValue::Boolean(true),
            validation: FieldValidation::Unknown,
        }],
    };
    assert!(matches!(
        spec.args_from_form_state(&wrong_kind),
        Err(InputAdapterError::DraftKindMismatch { name, expected, actual })
            if name.as_str() == "count" && expected == "text" && actual == "boolean"
    ));

    let invalid_integer = FormState {
        fields: vec![FormFieldState {
            name: input_name("count"),
            draft: DraftInputValue::Text("not an integer".to_string()),
            validation: FieldValidation::Unknown,
        }],
    };
    assert!(matches!(
        spec.args_from_form_state(&invalid_integer),
        Err(InputAdapterError::InvalidInteger { name, value })
            if name.as_str() == "count" && value == "not an integer"
    ));
}

#[test]
fn args_from_form_state_rejects_unknown_duplicate_and_missing_required_fields() {
    let spec = InputSpec::new(vec![field("count", true, InputKind::Integer)])
        .expect("spec should construct");

    let unknown_field = FormState {
        fields: vec![FormFieldState {
            name: input_name("extra"),
            draft: DraftInputValue::Text("1".to_string()),
            validation: FieldValidation::Unknown,
        }],
    };
    assert!(matches!(
        spec.args_from_form_state(&unknown_field),
        Err(InputAdapterError::UnknownFormField { name }) if name.as_str() == "extra"
    ));

    let duplicate_field = FormState {
        fields: vec![
            FormFieldState {
                name: input_name("count"),
                draft: DraftInputValue::Text("1".to_string()),
                validation: FieldValidation::Unknown,
            },
            FormFieldState {
                name: input_name("count"),
                draft: DraftInputValue::Text("2".to_string()),
                validation: FieldValidation::Unknown,
            },
        ],
    };
    assert!(matches!(
        spec.args_from_form_state(&duplicate_field),
        Err(InputAdapterError::DuplicateFormField { name }) if name.as_str() == "count"
    ));

    let missing_required = FormState { fields: Vec::new() };
    assert!(matches!(
        spec.args_from_form_state(&missing_required),
        Err(InputAdapterError::Value(InputValueError::MissingRequired { name }))
            if name.as_str() == "count"
    ));
}

#[test]
fn file_input_kind_exposes_label_and_initial_draft_correctly() {
    let spec = InputSpec::new(vec![field(
        "attachment",
        false,
        InputKind::File(FileInputPolicy::default()),
    )])
    .expect("spec should construct");

    assert_eq!(spec.fields[0].kind.label(), "file");

    let state = spec.initial_form_state();
    assert_eq!(state.fields.len(), 1);
    assert!(matches!(state.fields[0].draft, DraftInputValue::File(None)));
}

#[test]
fn file_input_validation_accepts_only_structured_objects() {
    let spec = InputSpec::new(vec![field(
        "doc",
        true,
        InputKind::File(FileInputPolicy::default()),
    )])
    .expect("spec should construct");

    let valid = serde_json::json!({
        "name": "a.txt",
        "is_dir": false,
        "content": {"kind": "bytes", "bytes": ""},
    });
    let mut args = serde_json::Map::new();
    args.insert("doc".to_string(), valid);
    assert!(
        spec.validate_json_args(&args).is_ok(),
        "structured file object should pass"
    );

    let mut args = serde_json::Map::new();
    args.insert(
        "doc".to_string(),
        serde_json::Value::String("plain text".to_string()),
    );
    assert!(matches!(
        spec.validate_json_args(&args),
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { name, .. }
        )) if name.as_str() == "doc"
    ));
}

#[test]
fn file_input_validation_rejects_legacy_byte_arrays() {
    // Given
    let spec = InputSpec::new(vec![field(
        "doc",
        true,
        InputKind::File(FileInputPolicy::default()),
    )])
    .expect("must construct the input spec");
    let mut args = serde_json::Map::new();
    args.insert(
        "doc".to_string(),
        serde_json::json!({
            "name": "a.txt",
            "is_dir": false,
            "content": {"kind": "bytes", "bytes": []},
        }),
    );

    // When
    let result = spec.validate_json_args(&args);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { .. }
        ))
    ));
}

#[test]
fn file_input_validation_rejects_noncanonical_base64() {
    // Given
    let spec = InputSpec::new(vec![field(
        "doc",
        true,
        InputKind::File(FileInputPolicy::default()),
    )])
    .expect("must construct the input spec");
    let mut args = serde_json::Map::new();
    args.insert(
        "doc".to_string(),
        serde_json::json!({
            "name": "a.txt",
            "is_dir": false,
            "content": {"kind": "bytes", "bytes": "Zh=="},
        }),
    );

    // When
    let result = spec.validate_json_args(&args);

    // Then
    assert!(matches!(
        result,
        Err(InputValueError::File(
            FileInputValueError::InvalidStructure { .. }
        ))
    ));
}

#[test]
fn file_value_preserves_pathed_fields_in_json() {
    let file = FileValue {
        name: "report.md".to_string(),
        content: FileContent::Bytes(vec![97, 98, 99]),
        mime: Some("text/markdown".to_string()),
    };
    let json = InputValue::File(file).into_json_value();

    let object = json.as_object().expect("file json should be object");
    assert_eq!(
        object.get("name").and_then(|v| v.as_str()),
        Some("report.md")
    );
    assert_eq!(
        object.get("is_dir").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        object.get("mime").and_then(|v| v.as_str()),
        Some("text/markdown")
    );
    let content = object.get("content").and_then(|v| v.as_object()).unwrap();
    assert_eq!(content.get("kind").and_then(|v| v.as_str()), Some("bytes"));
    let bytes = content.get("bytes").and_then(|v| v.as_str()).unwrap();
    assert_eq!(bytes, "YWJj");
}

#[test]
fn directory_preserves_recursive_entries() {
    let inner = FileValue {
        name: "child.txt".to_string(),
        content: FileContent::Bytes(vec![]),
        mime: None,
    };
    let outer = FileValue {
        name: "folder".to_string(),
        content: FileContent::Directory(vec![inner]),
        mime: None,
    };
    let json = InputValue::File(outer).into_json_value();

    let object = json.as_object().unwrap();
    assert_eq!(
        object.get("is_dir").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let content = object.get("content").and_then(|v| v.as_object()).unwrap();
    assert_eq!(
        content.get("kind").and_then(|v| v.as_str()),
        Some("directory")
    );
    let entries = content.get("entries").and_then(|v| v.as_array()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]
            .as_object()
            .and_then(|o| o.get("name"))
            .and_then(|v| v.as_str()),
        Some("child.txt")
    );
}

#[test]
fn file_static_kind_converts_to_owned() {
    static FIELDS: &[StaticInputFieldSpec] = &[StaticInputFieldSpec {
        name: "f",
        label: None,
        description: None,
        required: false,
        kind: StaticInputKind::File(StaticFileInputPolicy::DEFAULT),
        constraints: StaticFieldConstraints::empty(),
    }];
    let spec = StaticInputSpec { fields: FIELDS };
    let owned = spec
        .to_input_spec()
        .expect("static to owned should succeed");
    assert!(matches!(owned.fields[0].kind, InputKind::File(_)));
}

#[test]
fn field_constraints_default_to_empty() {
    let constraints = FieldConstraints::default();
    assert!(constraints.is_empty());
    assert!(constraints.number.is_none());
    assert!(constraints.string.is_none());
}

#[test]
fn number_constraints_preserve_min_max_default() {
    let constraints = FieldConstraints {
        number: Some(NumberConstraints {
            min: Some(1.0),
            max: Some(65535.0),
            default: Some(8080.0),
        }),
        string: None,
    };
    assert!(!constraints.is_empty());
    let n = constraints.number.as_ref().unwrap();
    assert_eq!(n.min, Some(1.0));
    assert_eq!(n.max, Some(65535.0));
    assert_eq!(n.default, Some(8080.0));
}

#[test]
fn string_constraints_preserve_regex_placeholder_default() {
    let constraints = FieldConstraints {
        number: None,
        string: Some(StringConstraints {
            regex: Some("^[a-z]+$".to_string()),
            placeholder: Some("abc".to_string()),
            default: Some("xyz".to_string()),
        }),
    };
    let s = constraints.string.as_ref().unwrap();
    assert_eq!(s.regex.as_deref(), Some("^[a-z]+$"));
    assert_eq!(s.placeholder.as_deref(), Some("abc"));
    assert_eq!(s.default.as_deref(), Some("xyz"));
}

#[test]
fn static_constraints_convert_to_owned_exactly() {
    let static_c = StaticFieldConstraints {
        number: Some(StaticNumberConstraints {
            min: Some(0.0),
            max: Some(10.0),
            default: None,
        }),
        string: Some(StaticStringConstraints {
            regex: Some(r"\d+"),
            placeholder: None,
            default: Some("zero"),
        }),
    };
    let owned = static_c.to_owned();
    assert_eq!(
        owned.number.unwrap(),
        NumberConstraints {
            min: Some(0.0),
            max: Some(10.0),
            default: None,
        }
    );
    let s = owned.string.unwrap();
    assert_eq!(s.regex.as_deref(), Some(r"\d+"));
    assert_eq!(s.placeholder, None);
    assert_eq!(s.default.as_deref(), Some("zero"));
}

#[test]
fn constrained_input_field_constructs_via_with_constraints() {
    let constraints = FieldConstraints {
        number: Some(NumberConstraints {
            min: Some(0.0),
            max: Some(100.0),
            default: Some(50.0),
        }),
        string: None,
    };
    let field = InputFieldSpec::with_constraints(
        input_name("ratio"),
        None,
        None,
        true,
        InputKind::Number,
        constraints.clone(),
    )
    .expect("field should construct");
    assert_eq!(field.constraints, constraints);
}
