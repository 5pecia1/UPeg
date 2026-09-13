//! Tests for `dispatch` were extracted here to keep the parent module
//! under the 1000-line workspace budget. Mounted from `dispatch.rs` as a
//! `#[cfg(test)] mod tests;`, so `super::*` brings every item of
//! `dispatch` (including its `pub(crate)`/private helpers) into scope.

use super::*;
use serde_json::json;

const TEST_USIZE_DEFAULT: usize = 64;
const TEST_USIZE_VALUE: usize = 1_024;

fn read_str_from_value(value: Value, key: &str) -> String {
    read_str(
        upeg_runtime::DispatchArgs::parse(&value).expect("test args must be an object"),
        key,
    )
    .to_string()
}

fn read_f64_from_value(value: Value, key: &str, default: f64) -> Result<f64, String> {
    read_f64(
        upeg_runtime::DispatchArgs::parse(&value).expect("test args must be an object"),
        key,
        default,
    )
}

fn read_usize_from_value(value: Value, key: &str, default: usize) -> Result<usize, String> {
    read_usize(
        upeg_runtime::DispatchArgs::parse(&value).expect("test args must be an object"),
        key,
        default,
    )
}

fn success_text(result: Option<ToolResult>) -> String {
    match result {
        Some(ToolResult::Success(success)) => upeg_runtime::tool_success_primary_text(&success),
        Some(ToolResult::Failure(failure)) => {
            panic!("expected success, got failure: {:?}", failure.error)
        }
        None => panic!("expected registered dispatcher"),
    }
}

fn success_result(result: Option<ToolResult>) -> ToolSuccess {
    match result {
        Some(ToolResult::Success(success)) => success,
        Some(ToolResult::Failure(failure)) => {
            panic!("expected success, got failure: {:?}", failure.error)
        }
        None => panic!("expected registered dispatcher"),
    }
}

fn static_meta(id: &str) -> &'static upeg_core::StaticToolMeta {
    upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .find(|meta| meta.id == id)
        .expect("test tool metadata must exist")
}

fn failure_message(result: Option<ToolResult>) -> String {
    match result {
        Some(ToolResult::Failure(failure)) => failure.error.message,
        Some(ToolResult::Success(success)) => {
            panic!("expected failure, got success: {success:?}")
        }
        None => panic!("expected registered dispatcher"),
    }
}

#[test]
fn 등록_전체는_멱등적이고_내장마다_단일_경로를_가진다() {
    // Two calls should be cheap and behaviorally equivalent.
    register_all();
    register_all();

    let r = upeg_runtime::try_runtime_dispatch("num.hex_to_decimal", &json!({"input": "0x10"}));
    assert_eq!(success_text(r), "16");
}

#[test]
fn dispatch_registered는_미등록_tool_id에_not_found를_반환한다() {
    assert!(matches!(
        dispatch_registered("no.such.tool", &json!({})),
        RegisteredDispatch::NotFound
    ));
}

#[test]
fn dispatch_registered는_등록된_tool을_실행한다() {
    match dispatch_registered("num.hex_to_decimal", &json!({"input": "0xff"})) {
        RegisteredDispatch::Ran(result) => {
            assert_eq!(success_text(Some(result)), "255");
        }
        other => panic!("expected Ran(Success), got {other:?}"),
    }
}

#[test]
fn dispatch_registered는_dispatcher_없는_등록_tool에_unimplemented를_반환한다() {
    // A tool present in the toolbox but without a paired runtime
    // dispatcher must surface as `Unimplemented`, not `NotFound`.
    let id = "test.dispatch_registered.no_handler";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "test",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "test")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &[],
        display_label: "Test tool",
        description: "",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: upeg_core::ALL_SURFACES,
        boards: &[],
    });
    assert!(matches!(
        dispatch_registered(id, &json!({})),
        RegisteredDispatch::Unimplemented
    ));
}

#[test]
fn gui_전용_메타는_dispatcher_요구_대상에서_제외된다() {
    // GUI-only tools should NOT require dispatchers
    register_all();

    // memo.scratch is a GUI-only tool (has Desktop/PWA/Chrome-ext surfaces only)
    // It should NOT have a headless dispatcher
    let memo_scratch_has_dispatcher =
        upeg_runtime::try_runtime_dispatch("memo.scratch", &json!({})).is_some();
    assert!(
        !memo_scratch_has_dispatcher,
        "memo.scratch is GUI-only and should NOT have a headless dispatcher"
    );

    // num.hex_to_decimal has headless surfaces (CLI, etc.)
    // It SHOULD have a dispatcher
    let hex_to_dec_has_dispatcher =
        upeg_runtime::try_runtime_dispatch("num.hex_to_decimal", &json!({})).is_some();
    assert!(
        hex_to_dec_has_dispatcher,
        "num.hex_to_decimal has headless surfaces and MUST have a dispatcher"
    );
}

/// Helper to determine if a StaticToolMeta requires a built-in dispatcher.
/// Returns true if the tool is a Function invoker AND has at least one headless
/// dispatch surface (CLI, TUI, MCP, or HTTP).
fn requires_builtin_dispatcher(meta: &upeg_core::StaticToolMeta) -> bool {
    meta.invoker == upeg_core::Invoker::Function && meta.has_headless_dispatch_surface()
}

#[test]
fn 모든_내장_도구_메타는_하나의_dispatcher를_가진다() {
    register_all();

    let metas: Vec<_> = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
        .filter(|meta| requires_builtin_dispatcher(meta))
        .collect();
    assert!(
        !metas.is_empty(),
        "test must see #[tool] inventory entries from upeg-tools"
    );

    let missing: Vec<_> = metas
        .into_iter()
        .filter_map(|meta| {
            upeg_runtime::try_runtime_dispatch(meta.id, &json!({}))
                .is_none()
                .then_some(meta.id)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "built-in StaticToolMeta ids without dispatchers: {missing:?}"
    );
}

#[test]
fn 내장_dispatcher는_매크로가_생성한_id_상수를_사용한다() {
    let source = include_str!("../dispatch.rs");
    let raw_dispatch_literal = concat!("register_runtime_dispatcher", "(\"");
    assert!(
        !source.contains(raw_dispatch_literal),
        "built-in dispatcher ids must come from #[tool] macro-generated *_TOOL_ID constants"
    );
}

#[test]
fn file_입력_역직렬화는_json_value를_복제하지_않는다() {
    let source = include_str!("../dispatch.rs");
    let read_file = source
        .split_once("fn read_file")
        .and_then(|(_, tail)| tail.split_once("fn read_f64"))
        .map(|(body, _)| body)
        .expect("read_file must remain between read_file and read_f64");

    assert!(
        !read_file.contains(".clone()"),
        "read_file must deserialize the borrowed JSON value without cloning its byte arrays"
    );
}

#[test]
fn 단일_output_도구는_메타의_primary_output_id로_정규화된다() {
    register_all();

    let meta = static_meta(crate::HEX_TO_DECIMAL_TOOL_ID);
    let primary = meta
        .primary_output_id
        .expect("hex_to_decimal declares primary output");
    let success = success_result(upeg_runtime::try_runtime_dispatch(
        crate::HEX_TO_DECIMAL_TOOL_ID,
        &json!({"input": "0x10"}),
    ));

    assert_eq!(success.primary_output_id.as_deref(), Some(primary));
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == primary)
        .expect("primary output entry must exist");
    assert_eq!(entry.kind, OutputKind::Number);
    assert_eq!(
        entry.value,
        OutputValue::Number(serde_json::Number::from(16_u8))
    );
}

#[test]
fn json_단일_output_도구는_문자열이_아닌_json_entry를_반환한다() {
    register_all();

    let meta = static_meta(crate::REGEX_MATCH_TOOL_ID);
    let primary = meta
        .primary_output_id
        .expect("regex_match declares primary output");
    let success = success_result(upeg_runtime::try_runtime_dispatch(
        crate::REGEX_MATCH_TOOL_ID,
        &json!({"pattern": "[a-z]+", "input": "ab 12 cd"}),
    ));

    assert_eq!(success.primary_output_id.as_deref(), Some(primary));
    let entry = success
        .outputs
        .iter()
        .find(|entry| entry.id == primary)
        .expect("primary output entry must exist");
    assert_eq!(entry.kind, OutputKind::Json);
    assert_eq!(entry.value, OutputValue::Json(json!(["ab", "cd"])));
}

#[test]
fn 등록_전체는_미디어_dispatcher를_연결한다() {
    register_all();

    let image_result = upeg_runtime::try_runtime_dispatch(
        crate::IMAGE_TO_PDF_TOOL_ID,
        &json!({"images": "", "output": "out.pdf"}),
    );
    assert!(
        matches!(image_result, Some(ToolResult::Failure(_))),
        "media.image_to_pdf must have a registered dispatcher, got {image_result:?}"
    );

    let pdf_result = upeg_runtime::try_runtime_dispatch(
        crate::PDF_TO_IMAGES_TOOL_ID,
        &json!({"input": "missing.pdf", "output_dir": "."}),
    );
    assert!(
        matches!(pdf_result, Some(ToolResult::Failure(_))),
        "media.pdf_to_images must have a registered dispatcher, got {pdf_result:?}"
    );

    let batch_result = upeg_runtime::try_runtime_dispatch(
        crate::IMAGES_CONVERT_TOOL_ID,
        &json!({"images": "", "output_format": "png"}),
    );
    assert!(
        matches!(batch_result, Some(ToolResult::Failure(_))),
        "media.images_convert must have a registered dispatcher, got {batch_result:?}"
    );
}

#[test]
fn 이미지_일괄_변환_dispatcher는_zip_file_output을_반환한다() {
    register_all();
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, u8::MAX]));
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    let directory = FileValue {
        name: "photos".to_string(),
        mime: None,
        content: upeg_core::FileContent::Directory(vec![FileValue {
            name: "one.png".to_string(),
            mime: Some("image/png".to_string()),
            content: upeg_core::FileContent::Bytes(png),
        }]),
    };
    let success = success_result(upeg_runtime::try_runtime_dispatch(
        crate::IMAGES_CONVERT_TOOL_ID,
        &json!({
            "images": serde_json::to_value(directory).expect("directory should serialize"),
            "output_format": "png",
        }),
    ));

    let output = success
        .outputs
        .iter()
        .find(|entry| entry.id == "result")
        .expect("result output must exist");
    let file = match &output.value {
        OutputValue::File(file) => file,
        other => panic!("expected File output, got {other:?}"),
    };

    assert_eq!(file.name, "photos-images.zip");
    assert_eq!(file.mime.as_deref(), Some(crate::IMAGES_ZIP_MIME));
    assert!(
        matches!(&file.content, upeg_core::FileContent::Bytes(bytes) if bytes.starts_with(b"PK"))
    );
}

#[test]
fn eth_gas는_native에서_실제_dispatcher를_가진다() {
    // eth.gas was promoted from a gui_meta.rs `Invoker::Http` placeholder
    // (no working dispatcher at all) to a real `Invoker::Function` tool
    // with a native JSON-RPC dispatcher — mirrors `net.status`: the
    // `StaticToolMeta` compiles on every target, but the runtime
    // dispatcher is only wired up natively (`register_eth_dispatchers`).
    register_all();
    let meta = static_meta(crate::ETH_GAS_TOOL_ID);
    assert_eq!(meta.invoker, upeg_core::Invoker::Function);
    #[cfg(not(target_arch = "wasm32"))]
    {
        assert!(
            upeg_runtime::try_runtime_dispatch(crate::ETH_GAS_TOOL_ID, &json!({})).is_some(),
            "eth.gas must have a registered native dispatcher"
        );
    }
}

#[test]
fn timer_live_도구들은_dispatcher를_가진다() {
    // Timer + Live 도구들은 runtime dispatcher가 등록되어 있어야 함.
    register_all();

    // time.epoch - Timer source with dispatcher
    let time_epoch =
        upeg_runtime::try_runtime_dispatch(crate::gui_meta::TIME_EPOCH_TOOL_ID, &json!({}));
    assert!(
        time_epoch.is_some(),
        "time.epoch (Timer+Live) must have a registered dispatcher"
    );

    // net.status - Timer source with dispatcher (native only)
    #[cfg(not(target_arch = "wasm32"))]
    {
        let net_status =
            upeg_runtime::try_runtime_dispatch(crate::gui_meta::NET_STATUS_TOOL_ID, &json!({}));
        assert!(
            net_status.is_some(),
            "net.status (Timer+Live) must have a registered dispatcher"
        );
    }
}

#[test]
fn time_epoch_dispatcher는_epoch와_iso를_담은_json을_반환한다() {
    register_all();

    let outcome =
        upeg_runtime::try_runtime_dispatch(crate::gui_meta::TIME_EPOCH_TOOL_ID, &json!({}));
    let success = match outcome {
        Some(ToolResult::Success(success)) => success,
        other => panic!("expected time.epoch success, got {other:?}"),
    };
    assert_eq!(success.primary_output_id.as_deref(), Some("epoch"));
    let epoch = success
        .outputs
        .iter()
        .find(|entry| entry.id == "epoch")
        .and_then(|entry| match &entry.value {
            OutputValue::Number(number) => number.as_u64(),
            _ => None,
        })
        .expect("`epoch` is a u64");
    let iso = success
        .outputs
        .iter()
        .find(|entry| entry.id == "iso")
        .and_then(|entry| match &entry.value {
            OutputValue::DateTime(value) => Some(value.as_str()),
            _ => None,
        })
        .expect("`iso` is a string");

    // 시계는 1970년 이후, ISO 문자열은 'Z'로 끝나는 ISO-8601 UTC.
    assert!(epoch > 0, "epoch must be post-1970, got {epoch}");
    assert!(
        iso.ends_with('Z'),
        "iso must be UTC (trailing 'Z'), got `{iso}`"
    );
}

// ─── read_str defensive helper pins ─────────────────────

#[test]
fn 읽기_str는_문자열_값_있는그대로를_반환한다() {
    let args = json!({"input": "hello world"});
    assert_eq!(read_str_from_value(args, "input"), "hello world");
}

#[test]
fn 읽기_str_누락된_키는_빈_문자열을_반환한다() {
    // Common path: dispatcher reads an unset field. Empty is
    // the "absent" signal that downstream tool functions handle
    // (e.g., `text.uppercase("")` → "" Ok).
    let args = json!({});
    assert_eq!(read_str_from_value(args, "input"), "");
}

#[test]
fn 읽기_str_명시적_널은_빈_문자열을_반환한다() {
    // Same as missing — Null is "no value".
    let args = json!({"input": null});
    assert_eq!(read_str_from_value(args, "input"), "");
}

#[test]
fn read_str은_문자열이_아닌_입력에_방어적으로_빈_문자열을_반환한다() {
    assert_eq!(read_str_from_value(json!({"input": 5}), "input"), "");
    assert_eq!(read_str_from_value(json!({"input": true}), "input"), "");
    assert_eq!(
        read_str_from_value(json!({"input": [1, 2, 3]}), "input"),
        ""
    );
}

// ─── read_f64 defensive helper pins ─────────────────────

#[test]
fn 키_없는_또는_널이면_read_f64는_기본을_사용한다() {
    assert_eq!(read_f64_from_value(json!({}), "dpi", 144.0), Ok(144.0));
    assert_eq!(
        read_f64_from_value(json!({"dpi": null}), "dpi", 144.0),
        Ok(144.0)
    );
}

#[test]
fn read_f64는_숫자_값을_읽는다() {
    assert_eq!(
        read_f64_from_value(json!({"dpi": 72}), "dpi", 144.0),
        Ok(72.0)
    );
    assert_eq!(
        read_f64_from_value(json!({"dpi": 150.5}), "dpi", 144.0),
        Ok(150.5)
    );
}

#[test]
fn read_f64는_숫자가_아닌_값을_거부한다() {
    assert_eq!(
        read_f64_from_value(json!({"dpi": "144"}), "dpi", 144.0),
        Err("dpi must be a number".to_string())
    );
    assert_eq!(
        read_f64_from_value(json!({"dpi": true}), "dpi", 144.0),
        Err("dpi must be a number".to_string())
    );
}

#[test]
fn 키가_없거나_널이면_read_usize는_기본값을_사용한다() {
    assert_eq!(
        read_usize_from_value(json!({}), "limit", TEST_USIZE_DEFAULT),
        Ok(TEST_USIZE_DEFAULT)
    );
    assert_eq!(
        read_usize_from_value(json!({"limit": null}), "limit", TEST_USIZE_DEFAULT),
        Ok(TEST_USIZE_DEFAULT)
    );
}

#[test]
fn read_usize는_음수가_아닌_정수를_읽는다() {
    assert_eq!(
        read_usize_from_value(json!({"limit": 0}), "limit", TEST_USIZE_DEFAULT),
        Ok(0)
    );
    assert_eq!(
        read_usize_from_value(
            json!({"limit": TEST_USIZE_VALUE}),
            "limit",
            TEST_USIZE_DEFAULT
        ),
        Ok(TEST_USIZE_VALUE)
    );
}

#[test]
fn read_usize는_잘못된_값을_거부한다() {
    for invalid in [json!(-1), json!(1.5), json!("64"), json!(true)] {
        assert_eq!(
            read_usize_from_value(json!({"limit": invalid}), "limit", TEST_USIZE_DEFAULT),
            Err("limit must be a non-negative integer".to_string())
        );
    }
}

// ─── text.join delim contract pins  ───────────
//
// text.join distinguishes "delim absent" (default " ") from
// "delim explicitly empty" (no separator). The stringify-non-
// string contract propagates without collapsing that distinction.

#[test]
fn 구분자가_없으면_텍스트_결합은_공백을_기본으로_사용한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("text.join", &json!({"input": r#"["a","b","c"]"#}));
    assert_eq!(
        success_text(r),
        "a b c",
        "absent delim must default to single space"
    );
}

#[test]
fn 명시적_빈_구분자_텍스트_결합은_이어붙인다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": ""}),
    );
    assert_eq!(
        success_text(r),
        "abc",
        "explicit empty delim must concatenate with no separator"
    );
}

#[test]
fn 텍스트_결합_명시적_널_구분자는_기본을_사용한다() {
    // Null is treated as absent (consistent with read_str's
    // null-carve-out), so the default " " still applies via
    // `as_str()` returning None.
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": null}),
    );
    assert_eq!(
        success_text(r),
        "a b c",
        "explicit null delim must default to single space (Null = absent)"
    );
}

#[test]
fn 텍스트_결합은_문자열_구분자를_그대로_사용한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": " - "}),
    );
    assert_eq!(success_text(r), "a - b - c");
}

#[test]
fn 텍스트_결합의_숫자_구분자는_schema_검증에서_거부된다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": 3}),
    );
    assert_eq!(
        failure_message(r),
        "input `separator` expected string, got number",
        "Number delim must fail before dispatcher coercion"
    );
}

#[test]
fn 텍스트_결합의_불리언_구분자는_schema_검증에서_거부된다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": true}),
    );
    assert_eq!(
        failure_message(r),
        "input `separator` expected string, got boolean",
        "Bool delim must fail before dispatcher coercion"
    );
}

#[test]
fn 문자열_schema는_문자열이_아닌_인자를_dispatcher_전에_거부한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("text.uppercase", &json!({"input": 5}));
    assert_eq!(
        failure_message(r),
        "input `input` expected string, got number",
        "Number(5) sent to a string-typed dispatcher must fail schema validation"
    );
}

#[test]
fn 등록_전체는_랜덤_바이트의_기본_크기_dispatcher를_연결한다() {
    // count omitted → 16 bytes → 32 hex chars. Pinning here so the
    // built-in's default never silently changes.
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("security.bytes_generate", &json!({}));
    let s = success_text(r);
    assert_eq!(s.len(), 32, "default count=16 → 32 hex chars, got {s:?}");
    assert!(
        s.chars().all(|c| c.is_ascii_hexdigit()),
        "must be lowercase hex, got {s:?}"
    );
}

#[test]
fn 등록_전체는_nanoid_dispatcher를_연결한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(crate::NANOID_TOOL_ID, &json!({}));
    let s = success_text(r);
    assert_eq!(s.len(), 21, "NanoID must stay at 21 chars, got {s:?}");
    assert!(
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "NanoID must stay URL-safe, got {s:?}"
    );
}

#[test]
fn 등록_전체는_비밀번호_생성_기본값_dispatcher를_연결한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("security.password_generate", &json!({}));
    let s = success_text(r);
    assert_eq!(s.len(), crate::PASSWORD_DEFAULT_LENGTH);
    assert!(
        s.chars().all(|c| c.is_ascii_alphanumeric()),
        "omitted include_symbols must match the schema form default, got {s:?}"
    );
}

#[test]
fn 등록_전체는_비밀번호_강도_dispatcher를_연결한다() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "security.password_estimate",
        &json!({"input": "password123"}),
    );
    let s = success_text(r);
    assert!(s.contains("\"common_pattern\""), "got {s}");
}

#[test]
fn 모든_내장_도구의_입력_schema는_닫혀_있고_유효하다() {
    let mut bad = Vec::new();
    for meta in upeg_core::inventory::iter::<upeg_core::StaticToolMeta>() {
        if let Err(error) = upeg_core::ToolMeta::from_static(meta) {
            bad.push(format!("{}: {error}", meta.id));
        }
    }
    assert!(
        bad.is_empty(),
        "built-in input specs generated from typed macro inputs must validate:\n{}",
        bad.join("\n")
    );
}
