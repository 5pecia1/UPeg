use super::*;

#[test]
fn file_인라인_정책은_모든_필드를_파싱한다() {
    let parsed = syn::parse_str::<ToolInput>(
        r#"required input: File(max_total_bytes=2097152, extensions=["png", "jpg"], max_file_bytes=1048576, max_count=3)"#,
    )
    .expect("File 인라인 정책의 모든 필드를 파싱해야 한다");

    assert_eq!(parsed.ty.to_string(), "File");
    assert!(
        parsed.params.is_some(),
        "File 인라인 정책은 KindParams로 보존되어야 한다"
    );
}

#[test]
fn file_인라인_정책은_중복된_파라미터를_거부한다() {
    let err = match syn::parse_str::<ToolInput>(
        r#"required input: File(max_count=2, extensions=["png"], max_count=3)"#,
    ) {
        Ok(_) => panic!("중복된 File 파라미터를 거부해야 한다"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("duplicate file parameter `max_count`"),
        "오류는 중복된 File 파라미터를 식별해야 한다: {err}"
    );
}

#[test]
fn file_인라인_정책은_알_수_없는_파라미터를_거부한다() {
    let err = match syn::parse_str::<ToolInput>(
        r#"required input: File(extensions=["png"], content_types=["image/png"])"#,
    ) {
        Ok(_) => panic!("알 수 없는 File 파라미터를 거부해야 한다"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("unknown file parameter `content_types`"),
        "오류는 알 수 없는 File 파라미터를 식별해야 한다: {err}"
    );
}

#[test]
fn file_extensions는_문자열_배열이_아니면_거부한다() {
    let err = match syn::parse_str::<ToolInput>(r#"required input: File(extensions="png")"#) {
        Ok(_) => panic!("extensions의 문자열 단일 값을 거부해야 한다"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("`extensions` expects an array of string literals"),
        "오류는 extensions의 문자열 배열 계약을 설명해야 한다: {err}"
    );
}

#[test]
fn file_정수_제한은_unsigned_정수가_아니면_거부한다() {
    for parameter in ["max_count", "max_file_bytes", "max_total_bytes"] {
        let source = format!(r#"required input: File({parameter}="unbounded")"#);
        let err = match syn::parse_str::<ToolInput>(&source) {
            Ok(_) => panic!("File 정수 제한의 문자열 값을 거부해야 한다"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains(&format!(
                "`{parameter}` expects an unsigned integer literal"
            )),
            "오류는 {parameter}의 unsigned 정수 계약을 설명해야 한다: {err}"
        );
    }
}
