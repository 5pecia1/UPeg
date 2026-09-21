use super::*;

#[test]
fn plugin_input_spec_to_core_accepts_all_plugin_kinds() {
    let choices = vec![
        PluginChoiceOption::new("a").with_label("A"),
        PluginChoiceOption::new("b").with_description("second"),
    ];
    let spec = PluginInputSpec::new([
        PluginInputField::required("text", PluginInputKind::String),
        PluginInputField::optional("number", PluginInputKind::Number),
        PluginInputField::optional("integer", PluginInputKind::Integer),
        PluginInputField::optional("flag", PluginInputKind::Boolean),
        PluginInputField::optional("one", PluginInputKind::Options(choices.clone())),
        PluginInputField::optional("many", PluginInputKind::MultiOptions(choices)),
        PluginInputField::optional("markdown", PluginInputKind::Markdown),
        PluginInputField::optional("json", PluginInputKind::Json),
        PluginInputField::optional("when", PluginInputKind::DateTime),
        PluginInputField::optional("path", PluginInputKind::FilePath),
        PluginInputField::optional("url", PluginInputKind::Url),
        PluginInputField::optional("upload", PluginInputKind::File),
    ]);

    let core = plugin_input_spec_to_core(&spec).expect("typed plugin spec lowers");

    assert_eq!(core.fields.len(), 12);
    assert_eq!(core.fields[0].name.as_str(), "text");
    assert_eq!(core.fields[0].kind, InputKind::String);
    assert!(matches!(core.fields[4].kind, InputKind::Options(_)));
    assert!(matches!(core.fields[5].kind, InputKind::MultiOptions(_)));
    assert_eq!(core.fields[10].kind, InputKind::Url);
    assert_eq!(core.fields[11].name.as_str(), "upload");
    assert_eq!(
        core.fields[11].kind,
        InputKind::File(upeg_core::FileInputPolicy::default())
    );
}

#[test]
fn decl_to_meta_rejects_empty_or_padded_input_names() {
    for (name, expected) in [
        ("", "must not be empty"),
        (" input", "leading or trailing whitespace"),
        ("input ", "leading or trailing whitespace"),
    ] {
        let decl = PluginToolDecl {
            id: "y.x".into(),
            toolkit: "y".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: Some(PluginInputSpec::new([PluginInputField::required(
                name,
                PluginInputKind::String,
            )])),
            output_spec: None,
            pin: None,
            pegboard_units: "U1".to_string(),
            surfaces: None,
            export: None,
        };
        match decl_to_meta(decl) {
            Err(LoadError::InvalidInputSpec { detail }) => assert!(
                detail.contains(expected),
                "name={name:?}: error should contain `{expected}`, got `{detail}`"
            ),
            other => panic!("name={name:?}: expected InvalidInputSpec, got {other:?}"),
        }
    }
}

#[test]
fn decl_to_meta_rejects_duplicate_input_names() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: Some(PluginInputSpec::new([
            PluginInputField::required("input", PluginInputKind::String),
            PluginInputField::optional("input", PluginInputKind::Integer),
        ])),
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };

    match decl_to_meta(decl) {
        Err(LoadError::InvalidInputSpec { detail }) => assert!(
            detail.contains("appears more than once"),
            "duplicate-name error should explain the conflict; got `{detail}`"
        ),
        other => panic!("expected duplicate input rejection, got {other:?}"),
    }
}

#[test]
fn decl_to_meta_rejects_duplicate_output_names() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: Some(PluginOutputSpec::new([
            PluginOutputField::new("result", PluginOutputKind::String),
            PluginOutputField::new("result", PluginOutputKind::Integer),
        ])),
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };

    match decl_to_meta(decl) {
        Err(LoadError::InvalidOutputSpec { detail }) => assert!(
            detail.contains("appears more than once"),
            "duplicate output error should explain the duplicate name, got `{detail}`"
        ),
        other => panic!("expected InvalidOutputSpec, got {other:?}"),
    }
}

#[test]
fn decl_to_meta_rejects_invalid_plugin_choice_spec() {
    let cases = [
        (
            PluginInputKind::Options(Vec::new()),
            "declare at least one option",
        ),
        (
            PluginInputKind::Options(vec![PluginChoiceOption::new("")]),
            "must not be empty",
        ),
        (
            PluginInputKind::MultiOptions(vec![
                PluginChoiceOption::new("a"),
                PluginChoiceOption::new("a"),
            ]),
            "appears more than once",
        ),
    ];
    for (kind, expected) in cases {
        let decl = PluginToolDecl {
            id: "y.x".into(),
            toolkit: "y".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: Some(PluginInputSpec::new([PluginInputField::required(
                "choice", kind,
            )])),
            output_spec: None,
            pin: None,
            pegboard_units: "U1".to_string(),
            surfaces: None,
            export: None,
        };
        match decl_to_meta(decl) {
            Err(LoadError::InvalidInputSpec { detail }) => assert!(
                detail.contains(expected),
                "choice error should contain `{expected}`, got `{detail}`"
            ),
            other => panic!("expected InvalidInputSpec, got {other:?}"),
        }
    }
}

#[test]
fn decl_without_input_spec_uses_empty_core_spec() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    let meta = decl_to_meta(decl).expect("omitted input_spec is no inputs");
    assert!(meta.input_spec.fields.is_empty());
}

#[test]
fn decl_to_meta_accepts_valid_input_spec() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: Some(PluginInputSpec::new([PluginInputField::required(
            "input",
            PluginInputKind::String,
        )])),
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    let meta = decl_to_meta(decl).expect("valid input spec must load");
    assert_eq!(meta.input_spec.fields.len(), 1);
    assert_eq!(meta.input_spec.fields[0].name.as_str(), "input");
    assert!(
        meta.input_schema_value().to_string().contains("\"input\""),
        "schema boundary value must be generated from the typed input spec"
    );
}
