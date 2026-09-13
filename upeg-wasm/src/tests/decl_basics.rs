use super::*;

#[test]
fn 선언을_메타로_바꾸면_최소_선언은_기본값을_사용한다() {
    let decl = PluginToolDecl {
        id: "test.wasm.minimal".into(),
        toolkit: "test".into(),
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
    let meta = decl_to_meta(decl).unwrap();
    assert_eq!(meta.id, "test.wasm.minimal");
    assert_eq!(meta.pin, PinKind::Inline);
    assert_eq!(meta.pegboard_units, PegboardUnits::U1);
    assert_eq!(meta.invoker, Invoker::Wasm);
    assert_eq!(meta.surfaces.len(), 7); // ALL_SURFACES default
    assert_eq!(meta.description, "");
}

#[test]
fn 선언을_메타로_바꾸면_설명이_정적_필드로_누출된다() {
    // Iter 120: pin the leak path so a future refactor that
    // swaps Box::leak for some smarter scheme can't silently
    // drop the description (which would render the tool's
    // `tool show` panel blank).
    let decl = PluginToolDecl {
        id: "test.wasm.with_desc".into(),
        toolkit: "test".into(),
        tags: None,
        display_label: None,
        description: Some("does the thing".into()),
        input_spec: None,
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    let meta = decl_to_meta(decl).unwrap();
    assert_eq!(meta.description, "does the thing");
}

#[test]
fn 입력_명세가_있는_선언은_코어_명세로_낮춰진다() {
    // Plugin manifests now carry typed DTOs. Pin the host boundary so
    // a future refactor can't silently produce zero-input forms for
    // plugin tools that did declare an input spec.
    let decl = PluginToolDecl {
        id: "test.wasm.with_schema".into(),
        toolkit: "test".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: Some(PluginInputSpec::new([PluginInputField::required(
            "x",
            PluginInputKind::String,
        )])),
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    let meta = decl_to_meta(decl).unwrap();
    assert_eq!(meta.input_spec.fields.len(), 1);
    assert_eq!(meta.input_spec.fields[0].name.as_str(), "x");
    assert_eq!(meta.input_spec.fields[0].kind, InputKind::String);
    assert!(
        meta.input_schema_value().to_string().contains("\"x\""),
        "boundary JSON schema must still be generated from typed metadata; got {:?}",
        meta.input_schema_value()
    );
}

#[test]
fn 출력_명세가_있는_선언은_코어_명세로_낮춰진다() {
    let decl = PluginToolDecl {
        id: "test.wasm.with_outputs".into(),
        toolkit: "test".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: Some(PluginOutputSpec::new([
            PluginOutputField::new("result", PluginOutputKind::String)
                .with_label("Result")
                .with_description("Rendered result"),
            PluginOutputField::new(
                "view",
                PluginOutputKind::EmbeddedView {
                    url: "https://example.com/view".to_string(),
                },
            ),
        ])),
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };

    let meta = decl_to_meta(decl).unwrap();

    assert_eq!(meta.output_spec.fields.len(), 2);
    assert_eq!(meta.primary_output_id, Some("result"));
    assert_eq!(meta.output_spec.fields[0].name, "result");
    assert_eq!(meta.output_spec.fields[0].label.as_deref(), Some("Result"));
    assert_eq!(
        meta.output_spec.fields[0].description.as_deref(),
        Some("Rendered result")
    );
    assert_eq!(meta.output_spec.fields[0].kind, OutputKind::String);
    assert_eq!(
        meta.output_spec.fields[1].kind,
        OutputKind::EmbeddedView {
            url: "https://example.com/view".to_string()
        }
    );
}

#[test]
fn 출력_명세의_primary_output_id가_없으면_선언은_거부된다() {
    let decl = PluginToolDecl {
        id: "test.wasm.missing_primary".into(),
        toolkit: "test".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: Some(PluginOutputSpec {
            fields: vec![PluginOutputField::new("result", PluginOutputKind::String)],
            primary_output_id: None,
        }),
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };

    match decl_to_meta(decl) {
        Err(LoadError::InvalidOutputSpec { detail }) => {
            assert!(detail.contains("primary_output_id required"), "{detail}");
        }
        other => panic!("missing plugin primary_output_id must fail, got {other:?}"),
    }
}

#[test]
fn 명시적_표면이_있는_선언을_메타로_바꾼다() {
    let decl = PluginToolDecl {
        id: "test.wasm.surf".into(),
        toolkit: "test".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: Some(vec!["mcp".into(), "http".into()]),
        export: None,
    };
    let meta = decl_to_meta(decl).unwrap();
    assert_eq!(meta.surfaces, &[Surface::Mcp, Surface::Http]);
}

#[test]
fn 선언을_메타로_바꾸면_페그보드_단위를_허용한다() {
    for (label, expected) in [
        ("U1", PegboardUnits::U1),
        ("U2", PegboardUnits::U2),
        ("U2T", PegboardUnits::U2T),
    ] {
        let decl = PluginToolDecl {
            id: format!("test.wasm.units.{label}").into(),
            toolkit: "test".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: None,
            pegboard_units: label.to_string(),
            surfaces: None,
            export: None,
        };
        let meta = decl_to_meta(decl).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(meta.pegboard_units, expected);
    }
}

#[test]
fn 선언을_메타로_바꾸면_빈값이나_알수없는_페그보드_단위를_거부한다() {
    for (label, expected) in [
        ("", "EmptyPegboardUnits"),
        ("  ", "EmptyPegboardUnits"),
        ("U3", "UnknownPegboardUnits"),
    ] {
        let decl = PluginToolDecl {
            id: "test.wasm.bad_units".into(),
            toolkit: "test".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: None,
            pegboard_units: label.to_string(),
            surfaces: None,
            export: None,
        };
        match (decl_to_meta(decl), expected) {
            (Err(LoadError::EmptyPegboardUnits), "EmptyPegboardUnits") => {}
            (Err(LoadError::UnknownPegboardUnits(units)), "UnknownPegboardUnits") => {
                assert_eq!(units, "U3");
            }
            (got, _) => panic!("pegboard_units={label:?}: expected {expected}, got {got:?}"),
        }
    }
}

#[test]
fn 선언을_메타로_바꾸면_알수없는_pin_종류를_거부한다() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: None,
        pin: Some("Mauve".into()),
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    match decl_to_meta(decl) {
        Err(LoadError::UnknownPinKind(k)) => assert_eq!(k, "Mauve"),
        other => panic!("expected UnknownPinKind, got {other:?}"),
    }
}

#[test]
fn 알수없는_pin_종류_메시지는_유효한_집합을_나열한다() {
    // Iter 136: error message must include the valid options so
    // plugin authors don't have to grep the upeg source. Format
    // matches upeg-loader's `LoadError::UnknownPinKind`.
    let err = LoadError::UnknownPinKind("Mauve".into());
    let msg = format!("{err}");
    assert!(
        msg.contains("`Mauve`"),
        "must echo the user's input; got {msg:?}"
    );
    assert!(
        msg.contains("Inline/Launcher/Live/Action/Embed"),
        "must list the valid set; got {msg:?}"
    );
}

#[test]
fn 선언을_메타로_바꾸면_알수없는_표면을_거부한다() {
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
        surfaces: Some(vec!["fax".into()]),
        export: None,
    };
    match decl_to_meta(decl) {
        Err(LoadError::UnknownSurface(s)) => assert_eq!(s, "fax"),
        other => panic!("expected UnknownSurface, got {other:?}"),
    }
}

#[test]
fn 알수없는_표면_메시지는_유효한_집합을_나열한다() {
    // Iter 136: parity with upeg-loader's `UnknownSurface`.
    let err = LoadError::UnknownSurface("fax".into());
    let msg = format!("{err}");
    assert!(
        msg.contains("`fax`"),
        "must echo the user's input; got {msg:?}"
    );
    assert!(
        msg.contains("cli/tui/desktop/pwa/ext/mcp/http"),
        "must list the valid set; got {msg:?}"
    );
}

#[test]
fn 선언을_메타로_바꾸면_모든_pin_종류_변형을_허용한다() {
    // Iter 189: companion to iter 188's loader-side coverage,
    // applied to upeg-wasm's `decl_to_meta`. Pre-iter-189 only
    // Inline (default) was tested here — Launcher, Live, Action,
    // Embed all flowed through `PinKind::parse` (iter 137)
    // without being end-to-end exercised at this layer. A future
    // regression that drops a variant from the wasm decl path
    // wouldn't fail any wasm-crate test in isolation.
    for (label, expected) in [
        ("Inline", PinKind::Inline),
        ("Launcher", PinKind::Launcher),
        ("Live", PinKind::Live),
        ("Action", PinKind::Action),
        ("Embed", PinKind::Embed),
    ] {
        let decl = PluginToolDecl {
            id: format!("test.wasm.iter189.pin.{label}").into(),
            toolkit: "test".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: Some(label.into()),
            pegboard_units: "U1".to_string(),
            surfaces: None,
            export: None,
        };
        let meta = decl_to_meta(decl).unwrap_or_else(|e| panic!("`{label}`: {e:?}"));
        assert_eq!(
            meta.pin, expected,
            "decl `pin: \"{label}\"` must parse to {expected:?}"
        );
    }
}

#[test]
fn 선언을_메타로_바꾸면_모든_표면_변형을_허용한다() {
    // Iter 189: end-to-end coverage of every Surface label through
    // `decl_to_meta`. Pre-iter-189 only Mcp + Http were named
    // (in `decl_to_meta_with_explicit_surfaces`); the other 5
    // surfaces flowed through `parse_surface` without test cover.
    for (label, expected) in [
        ("cli", Surface::Cli),
        ("tui", Surface::Tui),
        ("desktop", Surface::Desktop),
        ("pwa", Surface::Pwa),
        ("ext", Surface::Ext),
        ("mcp", Surface::Mcp),
        ("http", Surface::Http),
    ] {
        let decl = PluginToolDecl {
            id: format!("test.wasm.iter189.surface.{label}").into(),
            toolkit: "test".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: None,
            pegboard_units: "U1".to_string(),
            surfaces: Some(vec![label.into()]),
            export: None,
        };
        let meta = decl_to_meta(decl).unwrap_or_else(|e| panic!("`{label}`: {e:?}"));
        assert_eq!(
            meta.surfaces,
            &[expected],
            "decl `surfaces: [\"{label}\"]` must parse to [{expected:?}]"
        );
    }
}
