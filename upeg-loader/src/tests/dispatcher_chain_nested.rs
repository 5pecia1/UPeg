//! Chain-inside-a-chain: what an inner run hands back to the outer one.
//!
//! A nested chain is the only step whose success arrives carrying engine
//! bookkeeping of its own, and the only one whose args decide an
//! authorization question further down. Both are covered here rather than
//! in `dispatcher_chain.rs` so the nesting story reads in one place.

use super::{call_dispatcher, call_dispatcher_result, tool_success};
use crate::ToolToml;
use crate::dispatcher::chain_dispatcher_for;
use upeg_core::{OutputEntry, OutputKind, OutputValue, ToolResult, ToolSuccess};

/// The step every fixture chain ends on unless it names another tool.
const 안쪽_도구: &str = "test.nested.echo";

/// Step id of the gated step inside the inner chain.
const 승인_단계_ID: &str = "gate";

fn 실패(result: ToolResult) -> upeg_core::ToolError {
    match result {
        ToolResult::Failure(failure) => failure.error,
        ToolResult::Success(success) => panic!("실패를 기대했으나 성공: {success:?}"),
    }
}

fn 요약_상태(success: &ToolSuccess, id: &str) -> String {
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == "steps")
        .expect("chain 성공 봉투는 steps 행을 싣는다");
    let OutputValue::Json(serde_json::Value::Array(rows)) = &entry.value else {
        panic!("steps 행은 JSON 배열이어야 한다");
    };
    rows.iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("step `{id}` 요약 행이 없다: {rows:?}"))["status"]
        .as_str()
        .expect("status는 문자열이다")
        .to_string()
}

/// 체인 하나를 파싱해 런타임 dispatcher로 등록한다. 중첩 체인을 만들려면
/// 안쪽 체인이 바깥 체인과 같은 registry에 있어야 한다.
fn 체인_등록(id: &'static str, body: &str) {
    let parsed = toml::from_str::<ToolToml>(&format!(
        "id = \"{id}\"\ntoolkit = \"test\"\ninvoker = \"Chain\"\n{body}"
    ))
    .expect("유효한 체인 fixture");
    let dispatcher = chain_dispatcher_for(&parsed).expect("dispatcher built");
    upeg_runtime::register_runtime_dispatcher(id, dispatcher);
}

fn 안쪽_도구_등록() {
    upeg_runtime::register_single_text_runtime_dispatcher(안쪽_도구, |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("done")
            .to_string())
    });
}

fn 바깥_체인(id: &str, 안쪽: &str, args: Option<&str>) -> ToolToml {
    let 인자 = args.map_or_else(String::new, |args| format!("args = '{args}'\n"));
    toml::from_str::<ToolToml>(&format!(
        r#"id = "test.nested.{id}"
            toolkit = "test"
            invoker = "Chain"

            [[steps]]
            id = "inner"
            tool = "{안쪽}"
            {인자}
        "#
    ))
    .expect("유효한 바깥 체인 fixture")
}

#[test]
fn 마지막_단계가_체인이어도_step_요약_충돌로_실패하지_않는다() {
    // 안쪽 체인의 성공 봉투에는 엔진이 붙인 `steps` 행이 이미 실려 있다.
    // 그 행을 그대로 물려받으면 바깥 체인은 자기 행을 놓을 자리가 없어
    // 작성자가 쓴 적도 없는 이름으로 `chain_step_summary_conflict`를 냈다.
    안쪽_도구_등록();
    체인_등록(
        "test.nested.inner_plain",
        &format!("output = \"inner\"\n[[steps]]\nid = \"leaf\"\ntool = \"{안쪽_도구}\"\n"),
    );
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_plain",
        "test.nested.inner_plain",
        None,
    ))
    .expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(
        upeg_runtime::tool_success_primary_text(&success),
        "inner",
        "바깥 체인의 결과는 안쪽 체인의 출력이다"
    );
    assert_eq!(
        요약_상태(&success, "inner"),
        "ran",
        "바깥 요약은 중첩 체인을 step 하나로 기록한다"
    );
}

#[test]
fn 마지막_단계가_출력없는_체인이면_바깥_요약이_primary가_된다() {
    // 안쪽 체인이 자기 출력이 없으면 엔진 행이 그 체인의 primary였다.
    // 그 행을 걷어내면 primary도 함께 사라져야 정본 봉투가 성립한다.
    upeg_runtime::register_runtime_dispatcher("test.nested.action_only", |_| {
        ToolResult::Success(ToolSuccess::new(None, Vec::new()).expect("action-only 성공"))
    });
    체인_등록(
        "test.nested.inner_action",
        "[[steps]]\nid = \"act\"\ntool = \"test.nested.action_only\"\n",
    );
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_action",
        "test.nested.inner_action",
        None,
    ))
    .expect("dispatcher built");

    let success = tool_success(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(success.primary_output_id.as_deref(), Some("steps"));
    assert_eq!(요약_상태(&success, "inner"), "ran");
}

#[test]
fn 체인이_아닌_마지막_단계의_steps_출력은_여전히_충돌이다() {
    // 엔진 행만 접히고 사용자 행은 접히지 않는다는 뜻이다 — 요리법의 단계
    // 목록을 `steps`로 내는 도구는 조용히 버려지는 대신 보고되어야 한다.
    upeg_runtime::register_runtime_dispatcher("test.nested.user_steps", |_| {
        ToolResult::Success(
            ToolSuccess::new(
                Some("steps".to_string()),
                vec![OutputEntry {
                    id: "steps".to_string(),
                    label: None,
                    kind: OutputKind::Json,
                    value: OutputValue::Json(serde_json::json!(["preheat", "bake"])),
                }],
            )
            .expect("사용자 steps 출력"),
        )
    });
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_user_steps",
        "test.nested.user_steps",
        None,
    ))
    .expect("dispatcher built");

    let error = 실패(call_dispatcher_result(&f, serde_json::json!({})));

    assert_eq!(error.code, "chain_step_summary_conflict");
}

/// `requires_approval` 한 단계짜리 안쪽 체인. 승인은 호출 surface가
/// 인가된 경우에만 인정되므로, 중첩 호출이 surface를 물려받는지 여기서
/// 드러난다.
fn 승인_안쪽_체인_등록(id: &'static str) {
    안쪽_도구_등록();
    체인_등록(
        id,
        &format!(
            "[[steps]]\nid = \"{승인_단계_ID}\"\ntool = \"{안쪽_도구}\"\nrequires_approval = true\nargs = '{{\"input\":\"approved\"}}'\n"
        ),
    );
}

#[test]
fn 중첩_체인은_바깥_호출의_surface를_물려받아_cli에서_승인된다() {
    승인_안쪽_체인_등록("test.nested.inner_gate");
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_gate",
        "test.nested.inner_gate",
        Some(r#"{"approve":true}"#),
    ))
    .expect("dispatcher built");

    let out = call_dispatcher(&f, serde_json::json!({ "_upeg": { "surface": "cli" } }))
        .expect("cli에서 시작한 호출은 중첩 체인의 승인 단계도 통과해야 한다");

    assert_eq!(out, "approved");
}

#[test]
fn 중첩_체인은_바깥_호출의_surface를_물려받아_mcp에서는_거부된다() {
    승인_안쪽_체인_등록("test.nested.inner_gate_denied");
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_gate_denied",
        "test.nested.inner_gate_denied",
        Some(r#"{"approve":true}"#),
    ))
    .expect("dispatcher built");

    let error = 실패(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "mcp" } }),
    ));

    assert_eq!(error.code, "approval_denied_for_surface");
    assert!(
        error.message.contains("surface `mcp`"),
        "중첩 단계도 신원 없는 호출이 아니라 진짜 호출 surface로 판정되어야 한다: {}",
        error.message
    );
}

#[test]
fn step_args가_지어낸_surface는_호출의_surface로_덮인다() {
    // 템플릿에 `{{input.*}}`로 스며든 호출자 텍스트가 `_upeg.surface`를
    // 위조해 중첩 체인의 승인 장벽을 열려는 시도다.
    승인_안쪽_체인_등록("test.nested.inner_spoof");
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_spoof",
        "test.nested.inner_spoof",
        Some(r#"{"approve":true,"_upeg":{"surface":"cli"}}"#),
    ))
    .expect("dispatcher built");

    let error = 실패(call_dispatcher_result(
        &f,
        serde_json::json!({ "_upeg": { "surface": "mcp" } }),
    ));

    assert_eq!(
        error.code, "approval_denied_for_surface",
        "step args가 각인을 위조할 수 있으면 승인 장벽은 장식이다"
    );
    assert!(error.message.contains("surface `mcp`"), "{}", error.message);
}

#[test]
fn step_args는_호출의_보드_맥락을_물려받는다() {
    // surface만 물려주면 `_upeg.board`/`boardEnv`를 읽는 step(External의
    // 환경변수 주입)이 조용히 보드 밖에서 돌게 된다.
    upeg_runtime::register_single_text_runtime_dispatcher("test.nested.context_probe", |args| {
        Ok(args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .and_then(|context| context.get("board"))
            .and_then(|board| board.as_str())
            .unwrap_or("<none>")
            .to_string())
    });
    let f = chain_dispatcher_for(&바깥_체인(
        "outer_board",
        "test.nested.context_probe",
        Some(r#"{"input":"x"}"#),
    ))
    .expect("dispatcher built");

    let out = call_dispatcher(
        &f,
        serde_json::json!({ "_upeg": { "surface": "cli", "board": "dev" } }),
    )
    .expect("보드 맥락을 읽는 step은 성공해야 한다");

    assert_eq!(out, "dev");
}
