use super::*;

#[test]
fn decl_to_meta_rejects_empty_id_and_toolkit() {
    // Iter 198: parallel to upeg-loader's iter 196 fix. Plugin
    // manifests with empty id/toolkit would silently register
    // unidentifiable / mis-grouped tools.
    for (id, toolkit, expected) in [
        ("", "ok", "EmptyId"),
        ("   ", "ok", "EmptyId"),
        ("\t\n", "ok", "EmptyId"),
        ("ok.", "ok", "EmptyId"),
        (" ok.x ", "ok", "NonCanonicalId"),
        ("ok", "", "EmptyToolkit"),
        ("ok", "  ", "EmptyToolkit"),
        ("ok.x", " ok ", "NonCanonicalToolkit"),
    ] {
        let decl = PluginToolDecl {
            id: id.into(),
            toolkit: toolkit.into(),
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
        match (decl_to_meta(decl), expected) {
            (Err(LoadError::EmptyId), "EmptyId") => {}
            (Err(LoadError::EmptyToolkit), "EmptyToolkit") => {}
            (Err(LoadError::NonCanonicalId(_)), "NonCanonicalId") => {}
            (Err(LoadError::NonCanonicalToolkit(_)), "NonCanonicalToolkit") => {}
            (got, _) => {
                panic!("id={id:?} toolkit={toolkit:?}: expected {expected}, got {got:?}")
            }
        }
    }
}

#[test]
fn decl_to_meta_accepts_dotted_toolkit_and_dotted_local_name() {
    let meta = decl_to_meta(PluginToolDecl {
        id: "github.com.admin.tools.list".into(),
        toolkit: "github.com".into(),
        tags: None,
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    })
    .expect("dotted toolkit and dotted local name should be accepted");

    assert_eq!(meta.toolkit_id(), "github.com");
    assert_eq!(meta.tool_id(), "admin.tools.list");
}

#[test]
fn decl_to_meta_rejects_padded_tags() {
    let decl = PluginToolDecl {
        id: "y.x".into(),
        toolkit: "y".into(),
        tags: Some(vec![" dev ".into()]),
        display_label: None,
        description: None,
        input_spec: None,
        output_spec: None,
        pin: None,
        pegboard_units: "U1".to_string(),
        surfaces: None,
        export: None,
    };
    match decl_to_meta(decl) {
        Err(LoadError::NonCanonicalTag { position, tag }) => {
            assert_eq!(position, 0);
            assert_eq!(tag, " dev ");
        }
        other => panic!("padded plugin tags must be rejected, got {other:?}"),
    }
}

#[test]
fn id_shadowing_builtin_message_explains_rename_path() {
    // Iter 249/252: parallel to upeg-loader iter-249 + mcp_import
    // iter-250 message-format pins. Plugin authors and TOML authors
    // must see equally clear guidance on the rename remedy.
    let err = LoadError::IdShadowsBuiltIn("num.hex_to_decimal".into());
    let msg = format!("{err}");
    assert!(
        msg.contains("num.hex_to_decimal"),
        "message must echo the colliding id; got `{msg}`"
    );
    assert!(
        msg.contains("shadows a built-in"),
        "message must explain the failure mode; got `{msg}`"
    );
    assert!(
        msg.contains("Rename"),
        "message must hint at the remedy; got `{msg}`"
    );
    assert!(
        msg.contains("inventory"),
        "message should explain WHERE the collision lives (link-time inventory); got `{msg}`"
    );
}

#[test]
fn empty_id_and_toolkit_messages_match_loader() {
    // Iter 198: pin parity with upeg-loader's matching messages so
    // plugin authors and TOML authors see identical guidance.
    // Iter-136 set this same parity contract for the unknown-XXX
    // messages.
    let id_msg = format!("{}", LoadError::EmptyId);
    let toolkit_msg = format!("{}", LoadError::EmptyToolkit);
    assert!(
        id_msg.contains("`id`") && id_msg.contains("non-empty"),
        "id message must mention field + non-empty contract; got `{id_msg}`"
    );
    assert!(
        toolkit_msg.contains("`toolkit`") && toolkit_msg.contains("non-empty"),
        "toolkit message must mention field + non-empty contract; got `{toolkit_msg}`"
    );
}

#[test]
fn decl_to_meta_rejects_padded_id_and_toolkit() {
    let decl = PluginToolDecl {
        id: "  iter241.padded_plugin_id  ".into(),
        toolkit: " iter241\t".into(),
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
    assert!(
        matches!(
            decl_to_meta(decl),
            Err(LoadError::NonCanonicalToolkit(_) | LoadError::NonCanonicalId(_))
        ),
        "padded plugin identities must be rejected instead of normalized"
    );
}
