use super::*;

#[test]
fn decl_to_meta_rejects_empty_pin_kind() {
    // Iter 206: parallel to upeg-loader iter 205. Present-but-empty
    // pin gets EmptyPinKind instead of UnknownPinKind.
    for empty in ["", "  ", "\t\n"] {
        let decl = PluginToolDecl {
            id: "y.x".into(),
            toolkit: "y".into(),
            tags: None,
            display_label: None,
            description: None,
            input_spec: None,
            output_spec: None,
            pin: Some(empty.into()),
            pegboard_units: "U1".to_string(),
            surfaces: None,
            export: None,
        };
        match decl_to_meta(decl) {
            Err(LoadError::EmptyPinKind) => {}
            other => panic!("pin={empty:?}: expected EmptyPinKind, got {other:?}"),
        }
    }
}

#[test]
fn decl_to_meta_rejects_empty_surface_entry() {
    // Iter 207: parallel to upeg-loader iter 207. Empty surface
    // entries get EmptyInSurfaces with position info.
    for (surfaces_list, expected_pos) in [
        (vec![String::new(), "cli".into()], 0),
        (vec!["cli".into(), String::new(), "http".into()], 1),
        (vec!["cli".into(), "  ".into()], 1),
    ] {
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
            surfaces: Some(surfaces_list.clone()),
            export: None,
        };
        match decl_to_meta(decl) {
            Err(LoadError::EmptyInSurfaces { position }) => {
                assert_eq!(
                    position, expected_pos,
                    "surfaces {surfaces_list:?}: expected position {expected_pos}, got {position}"
                );
            }
            other => {
                panic!("surfaces {surfaces_list:?}: expected EmptyInSurfaces, got {other:?}")
            }
        }
    }
}

#[test]
fn omitted_pin_kind_still_uses_default() {
    // Pin the omitted-vs-present distinction. Omitting pin
    // (None) must still default to Inline cleanly.
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
    let meta = decl_to_meta(decl).unwrap();
    assert_eq!(meta.pin, PinKind::Inline);
}

#[test]
fn decl_to_meta_rejects_padded_pin_kind() {
    // Plugin manifest enum values are canonical strings; padded non-empty
    // values must not be silently normalized.
    for label in ["Inline ", " Embed", "Live\n", "\tLauncher"] {
        let decl = PluginToolDecl {
            id: "y.x".into(),
            toolkit: "y".into(),
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
        match decl_to_meta(decl) {
            Err(LoadError::UnknownPinKind(found)) => assert_eq!(found, label),
            other => panic!("pin=`{label}`: expected UnknownPinKind, got {other:?}"),
        }
    }
}

#[test]
fn decl_to_meta_rejects_padded_surface_entry() {
    let cases = [
        vec!["cli ".to_string(), "tui".into()],
        vec!["cli".to_string(), "\tmcp".into()],
        vec!["http\n".to_string()],
    ];
    for surfaces in cases {
        let snapshot = surfaces.clone();
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
            surfaces: Some(surfaces),
            export: None,
        };
        match decl_to_meta(decl) {
            Err(LoadError::UnknownSurface(_)) => {}
            other => panic!("surfaces={snapshot:?}: expected UnknownSurface, got {other:?}"),
        }
    }
}

#[test]
fn register_from_bytes_rejects_garbage() {
    // Raw bytes that aren't a valid wasm module — extism::Plugin::new
    // refuses; we surface that as `LoadError::Extism`.
    let result = register_from_bytes(b"not a wasm module");
    match result {
        Err(LoadError::Extism(_)) => {}
        other => panic!("expected Extism load error, got {other:?}"),
    }
}
