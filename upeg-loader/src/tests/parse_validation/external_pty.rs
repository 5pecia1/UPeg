//! `pty` is an External-invoker field. Declaring it on another invoker
//! is a manifest bug; declaring it on a host with no pseudoterminal is
//! not — that costs the one tool and leaves the rest of the file alone.

use crate::dispatcher::external::pty::HOST_SUPPORTS_PTY;
use crate::parse::{HostCapabilities, LoweredToolkit, toolkit_to_meta_and_tools};
use crate::{LoadError, ToolkitToml};

use super::super::parse_single_tool;

/// The two answers a host can give, named so the assertions below read
/// as the contract rather than as two bare booleans.
const PTY를_여는_호스트: HostCapabilities = HostCapabilities::with_pty(true);
const PTY가_없는_호스트: HostCapabilities = HostCapabilities::with_pty(false);

const 터미널_도구: &str = "y.terminal";
const 평범한_도구: &str = "y.plain";

/// One manifest carrying both kinds of tool, so "the rest of the file
/// still loads" is something the assertions can actually observe rather
/// than something the prose claims.
fn 두_도구_매니페스트(pty: &str) -> ToolkitToml {
    toml::from_str(&format!(
        r#"
id = "y"

[[tools]]
id = "terminal"
pegboard_units = "U1"
invoker = "External"
command = "git"
pty = {pty}

[[tools]]
id = "plain"
pegboard_units = "U1"
invoker = "External"
command = "git"
"#
    ))
    .expect("고정 매니페스트는 파싱된다")
}

fn 로드된_아이디(lowered: &LoweredToolkit) -> Vec<&'static str> {
    lowered.tools.iter().map(|(meta, _toml)| meta.id).collect()
}

fn 낮추기(pty: &str, host: HostCapabilities) -> LoweredToolkit {
    toolkit_to_meta_and_tools(&두_도구_매니페스트(pty), host).expect("적법한 매니페스트다")
}

#[test]
fn pty를_여는_호스트는_선언을_그대로_받아들인다() {
    // Given/When
    let lowered = 낮추기("true", PTY를_여는_호스트);

    // Then
    assert_eq!(로드된_아이디(&lowered), [터미널_도구, 평범한_도구]);
    assert!(lowered.skipped.is_empty(), "건너뛸 이유가 없다");
}

#[test]
fn pty가_없는_호스트는_그_도구_하나만_건너뛴다() {
    // Not a file-level rejection: the manifest is correct, and the other
    // tools in it never asked for a terminal.
    let lowered = 낮추기("true", PTY가_없는_호스트);

    assert_eq!(
        로드된_아이디(&lowered),
        [평범한_도구],
        "터미널을 요구하지 않은 도구는 그대로 실린다"
    );
    let [건너뛴_도구] = lowered.skipped.as_slice() else {
        panic!("건너뛴 도구는 하나여야 한다: {:?}", lowered.skipped);
    };
    assert_eq!(건너뛴_도구.id, 터미널_도구);
    assert!(
        matches!(건너뛴_도구.reason, LoadError::PtyUnsupportedOnHost),
        "실제 사유: {:?}",
        건너뛴_도구.reason
    );
}

#[test]
fn pty_false는_어느_호스트에서도_건너뛰지_않는다() {
    // `pty = false` says nothing the default did not already say, so no
    // host has anything to refuse.
    for host in [PTY를_여는_호스트, PTY가_없는_호스트] {
        let lowered = 낮추기("false", host);

        assert_eq!(로드된_아이디(&lowered), [터미널_도구, 평범한_도구]);
        assert!(lowered.skipped.is_empty());
    }
}

#[test]
fn pty_사유_메시지는_건너뛴다는_사실과_대안을_알려준다() {
    let message = LoadError::PtyUnsupportedOnHost.to_string();

    assert!(message.contains("pty = true"), "{message}");
    assert!(
        message.contains("skipped"),
        "무엇이 사라졌는지 말하지 않는 사유는 구멍만 남긴다: {message}"
    );
    assert!(
        message.contains("color = \"force\""),
        "대안이 없는 거부는 막다른 길이다: {message}"
    );
}

#[test]
fn external이_아닌_도구의_pty_선언은_거부된다() {
    let error = parse_single_tool(
        r#"id = "y.x"
           toolkit = "y"
           invoker = "Http"
           url = "https://example.com"
           method = "GET"
           pty = true"#,
    )
    .expect_err("pty는 External 전용 필드다");

    assert!(
        matches!(
            error,
            LoadError::InvokerFieldConflict {
                field: "pty",
                expected_invoker: "External",
                ..
            }
        ),
        "실제 오류: {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn external_도구의_pty_선언은_그대로_실린다() {
    use super::super::parse_full_single_tool;

    let (_meta, toml) = parse_full_single_tool(
        r#"id = "y.x"
           toolkit = "y"
           invoker = "External"
           command = "sh"
           pty = true"#,
    )
    .expect("Unix 호스트에서 pty 선언은 적법하다");

    assert_eq!(toml.pty, Some(true));
}

#[test]
fn 공개_파서는_호스트의_건너뜀_판정을_그대로_넘긴다() {
    // `parse_toolkit_full`은 `skipped`를 버린다. 그 자리에서 사라진
    // 정보는 어떤 표면도 되살릴 수 없으므로, 보고를 하는 호출자가
    // 쓸 수 있는 통로가 하나는 있어야 한다.
    let 결과 = crate::parse_toolkit_with_skips(
        r#"
id = "y"

[[tools]]
id = "terminal"
pegboard_units = "U1"
invoker = "External"
command = "git"
pty = true
"#,
    )
    .expect("적법한 매니페스트다");

    let 실린_아이디: Vec<&'static str> = 결과.tools.iter().map(|(meta, _)| meta.id).collect();
    if HOST_SUPPORTS_PTY {
        assert_eq!(실린_아이디, [터미널_도구]);
        assert!(결과.skipped.is_empty());
    } else {
        assert!(실린_아이디.is_empty());
        let [건너뛴_도구] = 결과.skipped.as_slice() else {
            panic!("건너뛴 도구는 하나여야 한다: {:?}", 결과.skipped);
        };
        assert_eq!(건너뛴_도구.id, 터미널_도구);
        assert!(matches!(
            건너뛴_도구.reason,
            LoadError::PtyUnsupportedOnHost
        ));
    }
}
