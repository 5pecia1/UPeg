//! End-to-end proof that `color = "force"` reaches the child as
//! environment, and that omitting it changes nothing.
//!
//! Every assertion is a **delta against the parent's own environment**,
//! never "the child saw nothing". A CI runner that exports `FORCE_COLOR`
//! for its own reasons is a legal environment to run this suite in, and
//! an `inherit` child there is *supposed* to see that value — asserting
//! an empty string would have made the ambient environment part of the
//! contract.

use serde_json::json;
use upeg_core::ToolResult;
use upeg_runtime::{DispatchArgs, tool_success_primary_text};

use crate::ToolToml;
use crate::dispatcher::external_dispatcher_for;

use super::color::{CLICOLOR_FORCE_ENV, COLOR_ENABLED_VALUE, FORCE_COLOR_ENV, NO_COLOR_ENV};

/// Field separator in the probe's output — one that cannot appear in an
/// environment variable name and is unlikely in these values.
const 구분자: &str = "|";

/// The variables the probe reports, in the order it prints them.
const 관찰_변수: &[&str] = &[CLICOLOR_FORCE_ENV, FORCE_COLOR_ENV, NO_COLOR_ENV];

/// Prints the three color variables the policy touches, so the
/// assertion reads the child's own view of its environment rather than
/// upeg's.
const 색상_변수_출력: &str = r#"printf '%s|%s|%s' "$CLICOLOR_FORCE" "$FORCE_COLOR" "$NO_COLOR""#;

fn 도구(color: Option<&str>) -> ToolToml {
    ToolToml {
        id: "test.color_probe".to_string(),
        toolkit: "test".to_string(),
        invoker: Some("External".to_string()),
        command: Some("sh".to_string()),
        args_template: Some(vec!["-c".to_string(), 색상_변수_출력.to_string()]),
        color: color.map(str::to_string),
        ..ToolToml::default()
    }
}

fn 실행(tool: &ToolToml) -> String {
    let dispatcher = external_dispatcher_for(tool, None).expect("External dispatcher");
    let args = json!({});
    let parsed = DispatchArgs::parse(&args).expect("빈 객체 인자");
    match dispatcher(parsed) {
        ToolResult::Success(success) => tool_success_primary_text(&success),
        ToolResult::Failure(failure) => panic!("자식 실행 실패: {}", failure.error.message),
    }
}

/// What the probe would print if it inherited this process's
/// environment untouched — the baseline every `inherit` assertion
/// compares against.
fn 부모_환경() -> String {
    관찰_변수
        .iter()
        .map(|name| std::env::var(name).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(구분자)
}

/// What the probe prints under `color = "force"`: both force variables
/// set, `NO_COLOR` gone.
fn 강제_환경() -> String {
    format!("{COLOR_ENABLED_VALUE}{구분자}{COLOR_ENABLED_VALUE}{구분자}")
}

#[test]
fn color_force는_자식에게_색상_환경변수를_넘긴다() {
    assert_eq!(실행(&도구(Some("force"))), 강제_환경());
}

#[test]
fn color_force는_상속된_no_color를_지운다() {
    // The trailing field is `NO_COLOR`, and `force` must leave it empty
    // whatever the parent exported — no-color.org ranks it above the two
    // force variables, so an inherited one would cancel the whole opt-in.
    let 출력 = 실행(&도구(Some("force")));
    let no_color = 출력.rsplit(구분자).next().expect("NO_COLOR 칸");
    assert_eq!(no_color, "", "force 아래에서 NO_COLOR는 비어 있어야 한다");
}

#[test]
fn color_선언이_없으면_환경은_그대로다() {
    assert_eq!(실행(&도구(None)), 부모_환경());
}

#[test]
fn color_inherit은_선언이_없을_때와_같다() {
    assert_eq!(실행(&도구(Some("inherit"))), 부모_환경());
}

#[test]
fn 선언된_env는_color_정책을_덮어쓴다() {
    // The policy sets defaults; an explicit `env` entry is the author
    // saying they know better, so it must win.
    let mut tool = 도구(Some("force"));
    tool.env = Some(vec![crate::model::KeyValueToml {
        name: FORCE_COLOR_ENV.to_string(),
        value: "0".to_string(),
    }]);

    assert_eq!(
        실행(&tool),
        format!("{COLOR_ENABLED_VALUE}{구분자}0{구분자}")
    );
}

#[test]
fn 선언된_no_color는_force의_제거를_이긴다() {
    // Same ordering rule as the line above: the removal is a default,
    // and a tool that re-declares `NO_COLOR` is opting back out.
    let mut tool = 도구(Some("force"));
    tool.env = Some(vec![crate::model::KeyValueToml {
        name: NO_COLOR_ENV.to_string(),
        value: COLOR_ENABLED_VALUE.to_string(),
    }]);

    assert_eq!(
        실행(&tool),
        format!("{COLOR_ENABLED_VALUE}{구분자}{COLOR_ENABLED_VALUE}{구분자}{COLOR_ENABLED_VALUE}")
    );
}
