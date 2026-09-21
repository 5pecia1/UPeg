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
fn register_all_is_idempotent_with_single_path_per_builtin() {
    // Two calls should be cheap and behaviorally equivalent.
    register_all();
    register_all();

    let r = upeg_runtime::try_runtime_dispatch("num.hex_to_decimal", &json!({"input": "0x10"}));
    assert_eq!(success_text(r), "16");
}

#[test]
fn dispatch_registered_returns_not_found_for_unregistered_tool_id() {
    assert!(matches!(
        dispatch_registered("no.such.tool", &json!({})),
        RegisteredDispatch::NotFound
    ));
}

#[test]
fn dispatch_registered_runs_registered_tool() {
    match dispatch_registered("num.hex_to_decimal", &json!({"input": "0xff"})) {
        RegisteredDispatch::Ran(result) => {
            assert_eq!(success_text(Some(result)), "255");
        }
        other => panic!("expected Ran(Success), got {other:?}"),
    }
}

#[test]
fn dispatch_registered_returns_unimplemented_for_tool_without_dispatcher() {
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
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
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
fn gui_only_meta_is_excluded_from_dispatcher_requirement() {
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
fn all_builtin_tool_metas_have_a_dispatcher() {
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
fn builtin_dispatchers_use_macro_generated_id_constants() {
    let source = include_str!("../dispatch.rs");
    let raw_dispatch_literal = concat!("register_runtime_dispatcher", "(\"");
    assert!(
        !source.contains(raw_dispatch_literal),
        "built-in dispatcher ids must come from #[tool] macro-generated *_TOOL_ID constants"
    );
}

#[test]
fn file_input_deserialization_does_not_clone_json_value() {
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
fn single_output_tool_normalizes_to_meta_primary_output_id() {
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
fn json_single_output_tool_returns_json_entry_not_string() {
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
fn register_all_wires_media_dispatchers() {
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
fn images_convert_dispatcher_returns_zip_file_output() {
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
fn eth_gas_has_real_dispatcher_on_native() {
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
fn timer_live_tools_have_dispatchers() {
    // Timer + Live tools must have a registered runtime dispatcher.
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
fn time_epoch_dispatcher_returns_json_with_epoch_and_iso() {
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

    // The clock must be post-1970 and the ISO string must be ISO-8601
    // UTC with a trailing 'Z'.
    assert!(epoch > 0, "epoch must be post-1970, got {epoch}");
    assert!(
        iso.ends_with('Z'),
        "iso must be UTC (trailing 'Z'), got `{iso}`"
    );
}

// ─── read_str defensive helper pins ─────────────────────

#[test]
fn read_str_returns_string_value_verbatim() {
    let args = json!({"input": "hello world"});
    assert_eq!(read_str_from_value(args, "input"), "hello world");
}

#[test]
fn read_str_missing_key_returns_empty_string() {
    // Common path: dispatcher reads an unset field. Empty is
    // the "absent" signal that downstream tool functions handle
    // (e.g., `text.uppercase("")` → "" Ok).
    let args = json!({});
    assert_eq!(read_str_from_value(args, "input"), "");
}

#[test]
fn read_str_explicit_null_returns_empty_string() {
    // Same as missing — Null is "no value".
    let args = json!({"input": null});
    assert_eq!(read_str_from_value(args, "input"), "");
}

#[test]
fn read_str_defensively_returns_empty_string_for_non_string_input() {
    assert_eq!(read_str_from_value(json!({"input": 5}), "input"), "");
    assert_eq!(read_str_from_value(json!({"input": true}), "input"), "");
    assert_eq!(
        read_str_from_value(json!({"input": [1, 2, 3]}), "input"),
        ""
    );
}

// ─── read_f64 defensive helper pins ─────────────────────

#[test]
fn read_f64_uses_default_for_missing_or_null_key() {
    assert_eq!(read_f64_from_value(json!({}), "dpi", 144.0), Ok(144.0));
    assert_eq!(
        read_f64_from_value(json!({"dpi": null}), "dpi", 144.0),
        Ok(144.0)
    );
}

#[test]
fn read_f64_reads_numeric_values() {
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
fn read_f64_rejects_non_numeric_values() {
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
fn read_usize_uses_default_for_missing_or_null_key() {
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
fn read_usize_reads_non_negative_integers() {
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
fn read_usize_rejects_invalid_values() {
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
fn text_join_defaults_to_space_when_delim_absent() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("text.join", &json!({"input": r#"["a","b","c"]"#}));
    assert_eq!(
        success_text(r),
        "a b c",
        "absent delim must default to single space"
    );
}

#[test]
fn text_join_concatenates_when_delim_explicitly_empty() {
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
fn text_join_explicit_null_delim_uses_default() {
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
fn text_join_uses_string_delim_verbatim() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "text.join",
        &json!({"input": r#"["a","b","c"]"#, "separator": " - "}),
    );
    assert_eq!(success_text(r), "a - b - c");
}

#[test]
fn text_join_number_delim_rejected_by_schema_validation() {
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
fn text_join_bool_delim_rejected_by_schema_validation() {
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
fn string_schema_rejects_non_string_args_before_dispatch() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch("text.uppercase", &json!({"input": 5}));
    assert_eq!(
        failure_message(r),
        "input `input` expected string, got number",
        "Number(5) sent to a string-typed dispatcher must fail schema validation"
    );
}

#[test]
fn register_all_wires_random_bytes_default_size_dispatcher() {
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
fn register_all_wires_nanoid_dispatcher() {
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
fn register_all_wires_password_generate_default_dispatcher() {
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
fn register_all_wires_password_estimate_dispatcher() {
    register_all();
    let r = upeg_runtime::try_runtime_dispatch(
        "security.password_estimate",
        &json!({"input": "password123"}),
    );
    let s = success_text(r);
    assert!(s.contains("\"common_pattern\""), "got {s}");
}

#[test]
fn all_builtin_tool_input_schemas_are_closed_and_valid() {
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
