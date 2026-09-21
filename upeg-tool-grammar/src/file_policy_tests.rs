use super::*;

#[test]
fn file_inline_policy_parses_all_fields() {
    let parsed = syn::parse_str::<ToolInput>(
        r#"required input: File(max_total_bytes=2097152, extensions=["png", "jpg"], max_file_bytes=1048576, max_count=3)"#,
    )
    .expect("must parse every field of the File inline policy");

    assert_eq!(parsed.ty.to_string(), "File");
    assert!(
        parsed.params.is_some(),
        "the File inline policy must be preserved as KindParams"
    );
}

#[test]
fn file_inline_policy_rejects_duplicate_parameters() {
    let err = match syn::parse_str::<ToolInput>(
        r#"required input: File(max_count=2, extensions=["png"], max_count=3)"#,
    ) {
        Ok(_) => panic!("duplicate File parameters must be rejected"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("duplicate file parameter `max_count`"),
        "the error must identify the duplicate File parameter: {err}"
    );
}

#[test]
fn file_inline_policy_rejects_unknown_parameters() {
    let err = match syn::parse_str::<ToolInput>(
        r#"required input: File(extensions=["png"], content_types=["image/png"])"#,
    ) {
        Ok(_) => panic!("unknown File parameters must be rejected"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("unknown file parameter `content_types`"),
        "the error must identify the unknown File parameter: {err}"
    );
}

#[test]
fn file_extensions_rejects_non_string_array() {
    let err = match syn::parse_str::<ToolInput>(r#"required input: File(extensions="png")"#) {
        Ok(_) => panic!("a single string value for extensions must be rejected"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("`extensions` expects an array of string literals"),
        "the error must explain the extensions string-array contract: {err}"
    );
}

#[test]
fn file_integer_limits_reject_non_unsigned_integers() {
    for parameter in ["max_count", "max_file_bytes", "max_total_bytes"] {
        let source = format!(r#"required input: File({parameter}="unbounded")"#);
        let err = match syn::parse_str::<ToolInput>(&source) {
            Ok(_) => panic!("a string value for a File integer limit must be rejected"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains(&format!(
                "`{parameter}` expects an unsigned integer literal"
            )),
            "the error must explain the unsigned-integer contract of {parameter}: {err}"
        );
    }
}
