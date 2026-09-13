//! `input_tests` 모듈의 짝 — 폼 상태/인자 강제 변환 회귀 테스트만
//! 모은다. 워크스페이스 1000-LoC 파일 크기 예산 때문에 분리했다.

use super::*;

#[test]
fn tui_폼_상태는_입력_명세에서_초기화된다() {
    let form = form_from_fields(vec![
        input_field("input", InputKind::String, true),
        input_field("size", InputKind::Integer, false),
    ]);
    assert_eq!(form.len(), 2);
    assert_eq!(form.fields[0].name.as_str(), "input");
    assert_eq!(form.fields[0].draft, DraftInputValue::Text(String::new()));
    let input = form.spec_for_field(&form.fields[0]).unwrap();
    assert!(input.required);
    assert!(matches!(input.kind, InputKind::String));
    let size = form.spec_for_field(&form.fields[1]).unwrap();
    assert!(!size.required);
    assert!(matches!(size.kind, InputKind::Integer));
}

#[test]
fn tui_폼_인자는_타입_있는_값으로_강제_변환된다() {
    let mut form = form_from_fields(vec![
        input_field("s", InputKind::String, false),
        input_field("n", InputKind::Number, false),
        input_field("i", InputKind::Integer, false),
        input_field("b", InputKind::Boolean, false),
    ]);
    set_form_text(&mut form, 0, "hi");
    set_form_text(&mut form, 1, "2.5");
    set_form_text(&mut form, 2, "42");
    form.fields[3].draft = DraftInputValue::Boolean(true);

    let args = form.args().expect("타입 있는 TUI 인자");
    assert_eq!(args["s"], "hi");
    assert_eq!(args["n"], 2.5);
    assert_eq!(args["i"], 42);
    assert_eq!(args["b"], true);
}

#[test]
fn tui_폼_인자의_잘못된_숫자는_검증_오류이다() {
    let mut form = form_from_fields(vec![input_field("n", InputKind::Number, false)]);
    set_form_text(&mut form, 0, "not-a-number");
    let err = form.args().unwrap_err();
    assert!(err.contains("invalid number"), "받은 값: {err}");
}

#[test]
fn tui_폼_인자는_빈_정수를_생략해_dispatcher_기본값을_사용하게_한다() {
    // 108회차 계약(TUI 동등성): 빈 정수 필드는 반드시 생략되어야 한다.
    // 그래야 `text.repeat` 같은 디스패처가 "n must be a non-negative integer"
    // 오류 대신 선언된 기본값을 적용한다. 99회차에서는 여기에 Null을 보내
    // 기본값 경로를 깨뜨렸다. 이후 upeg-core 헬퍼 테스트만 빠지더라도
    // 회귀가 크게 드러나도록 TUI 표면에서 고정한다.
    let mut form = form_from_fields(vec![
        input_field("input", InputKind::String, true),
        input_field("n", InputKind::Integer, false),
    ]);
    set_form_text(&mut form, 0, "x");
    let args = form.args().expect("타입 있는 TUI 인자");
    assert_eq!(args["input"], "x");
    assert!(
        args.get("n").is_none(),
        "빈 정수는 키를 생략해야 한다. 받은 값: {args}"
    );
}

#[test]
fn tui_폼_인자는_숫자_주변_공백을_잘라낸다() {
    // 109회차 계약(TUI 동등성): 앞뒤 공백이 붙은 숫자를 붙여넣어도
    // 깔끔하게 강제 변환되어야 한다. " 42 " → 42, "\t-7\n" → -7,
    // " 0.5 " → 0.5. trim이 없으면 파서가 거부하고 디스패처가 오류를 낸다.
    let mut form = form_from_fields(vec![
        input_field("i", InputKind::Integer, false),
        input_field("j", InputKind::Integer, false),
        input_field("f", InputKind::Number, false),
    ]);
    set_form_text(&mut form, 0, "  42  ");
    set_form_text(&mut form, 1, "\t-7\n");
    set_form_text(&mut form, 2, " 0.5 ");
    let args = form.args().expect("타입 있는 TUI 인자");
    assert_eq!(args["i"], 42);
    assert_eq!(args["j"], -7);
    assert_eq!(args["f"], 0.5);
}
