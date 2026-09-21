use super::*;

#[test]
fn file_policy_emits_losslessly_into_static_input_kind() {
    let input = syn::parse_str::<ToolInput>(
        r#"required input: File(extensions=["png", "jpg"], max_count=3, max_file_bytes=1048576, max_total_bytes=2097152)"#,
    )
    .expect("must parse the File policy");

    let tokens = build_static_input_spec_expr(&Some(vec![input]))
        .expect("must generate the static input spec for the File policy")
        .to_string();

    assert!(tokens.contains("StaticInputKind :: File"));
    assert!(tokens.contains("StaticFileInputPolicy"));
    assert!(tokens.contains("extensions : & [\"png\" , \"jpg\"]"));
    assert!(tokens.contains("max_count : 3"));
    assert!(tokens.contains("max_file_bytes : Some (1048576)"));
    assert!(tokens.contains("max_total_bytes : Some (2097152)"));
}

#[test]
fn file_without_policy_emits_default_static_policy() {
    let input = syn::parse_str::<ToolInput>("required input: File")
        .expect("must parse File without a policy");

    let tokens = build_static_input_spec_expr(&Some(vec![input]))
        .expect("must generate the static input spec for the default File policy")
        .to_string();

    assert!(tokens.contains("StaticInputKind :: File"));
    assert!(tokens.contains("StaticFileInputPolicy"));
    assert!(tokens.contains("extensions : & []"));
    assert!(tokens.contains("max_count : 1"));
    assert!(tokens.contains("max_file_bytes : None"));
    assert!(tokens.contains("max_total_bytes : None"));
}

#[test]
fn file_policy_is_explicitly_rejected_on_output() {
    let output = syn::parse_str::<ToolOutput>(r#"result: File(extensions=["png"], max_count=2)"#)
        .expect("the shared grammar must parse the File policy");

    let err = match build_static_output_spec_expr(&Some(vec![output])) {
        Ok(_) => panic!("input-only File policy must be rejected on output"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("`File(...)` policy is input-only"),
        "the error must explain the File policy is input-only: {err}"
    );
}

#[test]
fn file_output_without_policy_is_still_allowed() {
    let output = syn::parse_str::<ToolOutput>("result: File")
        .expect("must parse File output without a policy");

    let tokens = build_static_output_spec_expr(&Some(vec![output]))
        .expect("must generate the File output spec without a policy")
        .to_string();

    assert!(tokens.contains("StaticOutputKind :: File"));
}
