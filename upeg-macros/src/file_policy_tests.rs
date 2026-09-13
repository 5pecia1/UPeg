use super::*;

#[test]
fn file_정책은_static_input_kind에_손실없이_방출된다() {
    let input = syn::parse_str::<ToolInput>(
        r#"required input: File(extensions=["png", "jpg"], max_count=3, max_file_bytes=1048576, max_total_bytes=2097152)"#,
    )
    .expect("File 정책을 파싱해야 한다");

    let tokens = build_static_input_spec_expr(&Some(vec![input]))
        .expect("File 정책의 정적 입력 명세를 생성해야 한다")
        .to_string();

    assert!(tokens.contains("StaticInputKind :: File"));
    assert!(tokens.contains("StaticFileInputPolicy"));
    assert!(tokens.contains("extensions : & [\"png\" , \"jpg\"]"));
    assert!(tokens.contains("max_count : 3"));
    assert!(tokens.contains("max_file_bytes : Some (1048576)"));
    assert!(tokens.contains("max_total_bytes : Some (2097152)"));
}

#[test]
fn 정책이_없는_file은_기본_static_정책을_방출한다() {
    let input = syn::parse_str::<ToolInput>("required input: File")
        .expect("정책 없는 File을 파싱해야 한다");

    let tokens = build_static_input_spec_expr(&Some(vec![input]))
        .expect("기본 File 정책의 정적 입력 명세를 생성해야 한다")
        .to_string();

    assert!(tokens.contains("StaticInputKind :: File"));
    assert!(tokens.contains("StaticFileInputPolicy"));
    assert!(tokens.contains("extensions : & []"));
    assert!(tokens.contains("max_count : 1"));
    assert!(tokens.contains("max_file_bytes : None"));
    assert!(tokens.contains("max_total_bytes : None"));
}

#[test]
fn file_정책은_output에서_명시적으로_거부된다() {
    let output = syn::parse_str::<ToolOutput>(r#"result: File(extensions=["png"], max_count=2)"#)
        .expect("공용 문법은 File 정책을 파싱해야 한다");

    let err = match build_static_output_spec_expr(&Some(vec![output])) {
        Ok(_) => panic!("입력 전용 File 정책을 output에서 거부해야 한다"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("`File(...)` policy is input-only"),
        "오류는 File 정책이 입력 전용임을 설명해야 한다: {err}"
    );
}

#[test]
fn 정책이_없는_file_output은_계속_허용된다() {
    let output = syn::parse_str::<ToolOutput>("result: File")
        .expect("정책 없는 File output을 파싱해야 한다");

    let tokens = build_static_output_spec_expr(&Some(vec![output]))
        .expect("정책 없는 File output 명세를 생성해야 한다")
        .to_string();

    assert!(tokens.contains("StaticOutputKind :: File"));
}
