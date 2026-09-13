//! Built-in tool dispatchers.
//!
//! Each `#[upeg::tool]` annotation in `lib.rs` registers a *meta* into
//! the static `inventory` table — that lets every surface enumerate the
//! built-in catalogue at compile time. Running a tool is a separate
//! concern: surfaces look up `(id, args) → ToolResult` via
//! `upeg_runtime::try_runtime_dispatch`, which is populated at runtime
//! through `register_runtime_dispatcher`.
//!
//! The split exists so chain Tools, WASM plugins, and TOML-loaded
//! External tools share one dispatch path with built-ins.
//! Registration lives here (not in `upeg-cli`) so the Flutter desktop
//! surface (via `upeg-frb`, wasm32 / no axum / no extism) can wire
//! built-ins without dragging in CLI transitives.
//!
//! Calling `register_all()` is idempotent — the `OnceLock` gate makes
//! repeat calls cheap and safe across surfaces and tests.

use serde::Deserialize as _;
use serde_json::Value;
use std::sync::OnceLock;
use upeg_core::{FileValue, OutputEntry, OutputKind, OutputValue, ToolResult, ToolSuccess};
use upeg_runtime::{
    DispatchArgs, register_runtime_dispatcher, single_text_result, tool_failure, toolbox_tool,
    try_runtime_dispatch,
};

static REGISTERED: OnceLock<()> = OnceLock::new();

/// Outcome of routing a `(tool_id, args)` pair through the shared registry.
///
/// This is the common denominator of the register + existence-check +
/// dispatch dance that every surface (CLI/FRB/…) used to inline. Each
/// surface maps these variants onto its own protocol response, so the
/// wrapper stays free of surface-specific error codes and messages.
#[derive(Debug)]
pub enum RegisteredDispatch {
    /// `tool_id` is not present in the registry at all.
    NotFound,
    /// `tool_id` is registered but has no paired runtime dispatcher.
    /// Reachable only when meta is registered without a dispatcher.
    Unimplemented,
    /// The tool ran and produced a canonical result (success or failure).
    Ran(ToolResult),
}

/// Ensure built-ins are registered, confirm `tool_id` exists, then dispatch.
///
/// Lookup order mirrors the historical per-surface implementations:
///   1. [`register_all`] — idempotent built-in registration.
///   2. [`toolbox_tool`] existence check — missing → [`RegisteredDispatch::NotFound`].
///   3. [`try_runtime_dispatch`] — `None` (registered, no dispatcher) →
///      [`RegisteredDispatch::Unimplemented`]; otherwise [`RegisteredDispatch::Ran`].
pub fn dispatch_registered(tool_id: &str, args: &Value) -> RegisteredDispatch {
    register_all();
    if toolbox_tool(tool_id).is_none() {
        return RegisteredDispatch::NotFound;
    }
    match try_runtime_dispatch(tool_id, args) {
        Some(result) => RegisteredDispatch::Ran(result),
        None => RegisteredDispatch::Unimplemented,
    }
}

/// Register a canonical runtime dispatcher for every built-in tool exposed by
/// this crate. Idempotent: subsequent calls are O(1) no-ops.
///
/// Surfaces should call this once at startup. The CLI's `dispatch_tool`
/// fires it lazily on first use; the desktop UI calls it from
/// `App::use_hook` so the form-driven \[F1\] run path can resolve built-ins.
pub fn register_all() {
    REGISTERED.get_or_init(register_inner);
}

/// Read a single string argument from the dispatch args object.
///
/// Dispatch is schema-validated before these closures run, so non-string
/// values for string-typed built-in fields are rejected upstream. Missing or
/// null optional fields still read as the local default sentinel `""`.
fn read_str<'a>(args: DispatchArgs<'a>, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or("")
}

fn register_single_output_dispatcher<F>(id: &'static str, f: F)
where
    F: for<'a> Fn(DispatchArgs<'a>) -> Result<String, String> + Send + Sync + 'static,
{
    register_runtime_dispatcher(id, move |args| single_text_result(id, f(args)));
}

fn register_inner() {
    register_convert_dispatchers();
    register_text_dispatchers();
    register_id_dispatchers();
    register_security_dispatchers();
    register_color_dispatchers();
    register_hash_dispatchers();
    register_time_dispatchers();
    register_media_dispatchers();
    #[cfg(not(target_arch = "wasm32"))]
    register_net_dispatchers();
    register_qr_dispatchers();
    register_csv_dispatchers();
    register_num_dispatchers();
    #[cfg(not(target_arch = "wasm32"))]
    register_eth_dispatchers();
    #[cfg(not(target_arch = "wasm32"))]
    register_weather_dispatchers();
    #[cfg(not(target_arch = "wasm32"))]
    register_devcontainer_dispatchers();
}

fn register_convert_dispatchers() {
    register_single_output_dispatcher(crate::BASE64_ENCODE_TOOL_ID, |args| {
        Ok(crate::base64_encode(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::BASE64_DECODE_TOOL_ID, |args| {
        crate::base64_decode(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::BASE32_ENCODE_TOOL_ID, |args| {
        Ok(crate::base32_encode(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::BASE32_DECODE_TOOL_ID, |args| {
        crate::base32_decode(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::URL_ENCODE_TOOL_ID, |args| {
        Ok(crate::url_encode(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::URL_DECODE_TOOL_ID, |args| {
        crate::url_decode(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::URL_QUERY_PARSE_TOOL_ID, |args| {
        crate::url_query_parse(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::URL_QUERY_FORMAT_TOOL_ID, |args| {
        crate::url_query_format(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::HTML_ENCODE_TOOL_ID, |args| {
        Ok(crate::html_encode(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::HTML_DECODE_TOOL_ID, |args| {
        crate::html_decode(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::JSON_FORMAT_TOOL_ID, |args| {
        crate::json_format(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::JSON_MINIFY_TOOL_ID, |args| {
        crate::json_minify(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::CONVERT_NFC_TOOL_ID, |args| {
        Ok(crate::convert_nfc(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::CONVERT_NFD_TOOL_ID, |args| {
        Ok(crate::convert_nfd(read_str(args, "input")))
    });
}

fn register_text_dispatchers() {
    register_single_output_dispatcher(crate::TEXT_DIFF_TOOL_ID, |args| {
        Ok(crate::text_diff(
            read_str(args, "left"),
            read_str(args, "right"),
        ))
    });
    register_single_output_dispatcher(crate::REGEX_MATCH_TOOL_ID, |args| {
        crate::regex_match(read_str(args, "pattern"), read_str(args, "input"))
            .and_then(|matches| serde_json::to_string(&matches).map_err(|_| "format failed"))
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::TEXT_LOWERCASE_TOOL_ID, |args| {
        Ok(crate::text_lowercase(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::TEXT_UPPERCASE_TOOL_ID, |args| {
        Ok(crate::text_uppercase(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::TEXT_WORD_COUNT_TOOL_ID, |args| {
        Ok(crate::text_word_count(read_str(args, "input")).to_string())
    });
    register_single_output_dispatcher(crate::TEXT_CHAR_COUNT_TOOL_ID, |args| {
        Ok(crate::text_char_count(read_str(args, "input")).to_string())
    });
    register_single_output_dispatcher(crate::TEXT_LINE_COUNT_TOOL_ID, |args| {
        Ok(crate::text_line_count(read_str(args, "input")).to_string())
    });
    register_single_output_dispatcher(crate::TEXT_SLUGIFY_TOOL_ID, |args| {
        Ok(crate::text_slugify(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::TEXT_REPLACE_TOOL_ID, |args| {
        crate::text_replace(
            read_str(args, "input"),
            read_str(args, "from"),
            read_str(args, "to"),
        )
        .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::TEXT_CONTAINS_TOOL_ID, |args| {
        crate::text_contains(read_str(args, "input"), read_str(args, "pattern"))
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::TEXT_REPEAT_TOOL_ID, |args| {
        crate::text_repeat(read_str(args, "input"), read_usize(args, "count", 1)?)
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::TEXT_SPLIT_TOOL_ID, |args| {
        Ok(crate::text_split(
            read_str(args, "input"),
            read_str(args, "separator"),
        ))
    });
    register_single_output_dispatcher(crate::TEXT_JOIN_TOOL_ID, |args| {
        let separator = match args.get("separator") {
            None | Some(Value::Null) => " ",
            Some(_) => read_str(args, "separator"),
        };
        crate::text_join(read_str(args, "input"), separator)
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::TEXT_REVERSE_TOOL_ID, |args| {
        Ok(crate::text_reverse(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::TEXT_TRIM_TOOL_ID, |args| {
        Ok(crate::text_trim(read_str(args, "input")))
    });
}

fn register_id_dispatchers() {
    register_single_output_dispatcher(crate::UUID_V7_TOOL_ID, |_args| Ok(crate::uuid_v7()));
    register_single_output_dispatcher(crate::UUID_V4_TOOL_ID, |_args| Ok(crate::uuid_v4()));
    register_single_output_dispatcher(crate::NANOID_TOOL_ID, |_args| {
        crate::nanoid().map_err(std::string::ToString::to_string)
    });
}

fn register_security_dispatchers() {
    register_single_output_dispatcher(crate::BYTES_GENERATE_TOOL_ID, |args| {
        crate::bytes_generate(read_usize(
            args,
            "count",
            crate::BYTES_GENERATE_DEFAULT_COUNT,
        )?)
        .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::PASSWORD_GENERATE_TOOL_ID, |args| {
        let include_symbols = args
            .get("include_symbols")
            .and_then(Value::as_bool)
            .unwrap_or(crate::PASSWORD_DEFAULT_INCLUDE_SYMBOLS);
        crate::password_generate(
            read_usize(args, "count", crate::PASSWORD_DEFAULT_LENGTH)?,
            include_symbols,
        )
    });
    register_single_output_dispatcher(crate::PASSWORD_ESTIMATE_TOOL_ID, |args| {
        Ok(crate::password_estimate(read_str(args, "input")))
    });
}

fn register_color_dispatchers() {
    register_single_output_dispatcher(crate::COLOR_HEX_TO_RGB_TOOL_ID, |args| {
        crate::color_hex_to_rgb(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::COLOR_RGB_TO_HEX_TOOL_ID, |args| {
        crate::color_rgb_to_hex(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::COLOR_CONTRAST_TOOL_ID, |args| {
        crate::color_contrast(read_str(args, "foreground"), read_str(args, "background"))
            .map_err(std::string::ToString::to_string)
    });
}

fn register_qr_dispatchers() {
    register_single_output_dispatcher(crate::QR_ENCODE_TOOL_ID, |args| {
        crate::qr_encode(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::QR_DECODE_TOOL_ID, |args| {
        crate::qr_decode(&read_file(args, "input")?)
    });
}

fn register_csv_dispatchers() {
    register_single_output_dispatcher(crate::CSV_DIFF_TOOL_ID, |args| {
        crate::csv_diff(read_str(args, "left"), read_str(args, "right"))
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::CSV_TO_JSON_TOOL_ID, |args| {
        crate::csv_to_json(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::CSV_SELECT_TOOL_ID, |args| {
        crate::csv_select(read_str(args, "input"), read_str(args, "columns"))
    });
}

fn register_num_dispatchers() {
    register_single_output_dispatcher(crate::HEX_TO_DECIMAL_TOOL_ID, |args| {
        crate::hex_to_decimal(read_str(args, "input"))
            .map(|n| n.to_string())
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::DECIMAL_TO_HEX_TOOL_ID, |args| {
        crate::decimal_to_hex(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::DECIMAL_TO_BINARY_TOOL_ID, |args| {
        crate::decimal_to_binary(read_str(args, "input")).map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::BINARY_TO_DECIMAL_TOOL_ID, |args| {
        crate::binary_to_decimal(read_str(args, "input"))
            .map(|n| n.to_string())
            .map_err(std::string::ToString::to_string)
    });
}

/// `eth.*` dispatchers. Native-only registration (unlike `qr`/`csv`/`num`,
/// which register unconditionally): the tool functions themselves compile
/// on every target (so their `StaticToolMeta` stays discoverable
/// everywhere — see `toolkits::eth`'s module doc), but only the native
/// build gets a working runtime dispatcher, mirroring `net.status`
/// exactly (`register_net_dispatchers` below).
#[cfg(not(target_arch = "wasm32"))]
fn register_eth_dispatchers() {
    register_single_output_dispatcher(crate::ETH_GAS_TOOL_ID, |args| {
        crate::eth_gas(read_str(args, "endpoint"))
    });
    register_single_output_dispatcher(crate::ETH_ADDRESS_LOOKUP_TOOL_ID, |args| {
        crate::eth_address_lookup(read_str(args, "address"), read_str(args, "endpoint"))
    });
}

/// `weather.*` dispatchers. Native-only registration, mirroring
/// `register_eth_dispatchers`: the tool functions compile on every target (so
/// their `StaticToolMeta` stays discoverable everywhere — see
/// `toolkits::weather`'s module doc), but only the native build gets a working
/// runtime dispatcher.
#[cfg(not(target_arch = "wasm32"))]
fn register_weather_dispatchers() {
    register_single_output_dispatcher(crate::WEATHER_LOOKUP_TOOL_ID, |args| {
        crate::weather_lookup(read_str(args, "city"))
    });
    register_single_output_dispatcher(crate::WEATHER_FORECAST_TOOL_ID, |args| {
        crate::weather_forecast(
            read_str(args, "city"),
            read_usize(args, "days", crate::WEATHER_DEFAULT_FORECAST_DAYS)?,
        )
    });
}

/// `devcontainer.*` dispatchers. Native-only registration, mirroring
/// `register_eth_dispatchers`/`register_weather_dispatchers`: the tool
/// functions compile on every target (so their `StaticToolMeta` stays
/// discoverable everywhere — see `toolkits::devcontainer`'s module doc), but
/// only the native build gets a working runtime dispatcher (filesystem +
/// SQLite).
#[cfg(not(target_arch = "wasm32"))]
fn register_devcontainer_dispatchers() {
    register_single_output_dispatcher(crate::DEVCONTAINER_LIST_TOOL_ID, |args| {
        crate::devcontainer_list(read_str(args, "app"))
    });
    register_single_output_dispatcher(crate::DEVCONTAINER_LOOKUP_TOOL_ID, |args| {
        crate::devcontainer_lookup(
            read_str(args, "pattern"),
            read_str(args, "field"),
            read_str(args, "app"),
        )
    });
}

fn register_hash_dispatchers() {
    register_single_output_dispatcher(crate::SHA256_HEX_TOOL_ID, |args| {
        Ok(crate::sha256_hex(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::MD5_HEX_TOOL_ID, |args| {
        Ok(crate::md5_hex(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::SHA1_HEX_TOOL_ID, |args| {
        Ok(crate::sha1_hex(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::SHA512_HEX_TOOL_ID, |args| {
        Ok(crate::sha512_hex(read_str(args, "input")))
    });
    register_single_output_dispatcher(crate::CRC32_HEX_TOOL_ID, |args| {
        Ok(crate::crc32_hex(read_str(args, "input")))
    });
}

fn register_time_dispatchers() {
    register_single_output_dispatcher(crate::EPOCH_NOW_TOOL_ID, |_args| {
        crate::epoch_now()
            .map(|n| n.to_string())
            .map_err(std::string::ToString::to_string)
    });
    register_single_output_dispatcher(crate::ISO_NOW_TOOL_ID, |_args| {
        crate::iso_now().map_err(std::string::ToString::to_string)
    });
    register_runtime_dispatcher(crate::gui_meta::TIME_EPOCH_TOOL_ID, |_args| {
        let secs = match crate::epoch_now().map_err(std::string::ToString::to_string) {
            Ok(secs) => secs,
            Err(message) => return tool_failure("tool_error", message),
        };
        let iso = match crate::iso_now().map_err(std::string::ToString::to_string) {
            Ok(iso) => iso,
            Err(message) => return tool_failure("tool_error", message),
        };
        time_epoch_success(secs, iso)
    });
}

/// `net.status` Live+Timer dispatcher. Native-only — wasm32 has no
/// `std::net`. Runs a TCP probe to a fixed anycast endpoint with a
/// 1-second timeout (see `toolkits/net.rs`) and returns the JSON shape
/// `gui_meta.rs` declares: `{status, ping_ms, ipv6}`.
#[cfg(not(target_arch = "wasm32"))]
fn register_net_dispatchers() {
    register_runtime_dispatcher(crate::gui_meta::NET_STATUS_TOOL_ID, |_args| {
        let snapshot = crate::toolkits::net::net_status();
        net_status_success(snapshot.status.label(), snapshot.ping_ms, snapshot.ipv6)
    });
}

fn time_epoch_success(secs: u64, iso: String) -> ToolResult {
    structured_success(
        crate::gui_meta::TIME_EPOCH_EPOCH_OUTPUT_ID,
        vec![
            OutputEntry {
                id: crate::gui_meta::TIME_EPOCH_EPOCH_OUTPUT_ID.into(),
                label: None,
                kind: OutputKind::Number,
                value: OutputValue::Number(serde_json::Number::from(secs)),
            },
            OutputEntry {
                id: crate::gui_meta::TIME_EPOCH_ISO_OUTPUT_ID.into(),
                label: None,
                kind: OutputKind::DateTime,
                value: OutputValue::DateTime(iso),
            },
        ],
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn net_status_success(status: &str, ping_ms: u64, ipv6: bool) -> ToolResult {
    structured_success(
        crate::gui_meta::NET_STATUS_STATUS_OUTPUT_ID,
        vec![
            OutputEntry {
                id: crate::gui_meta::NET_STATUS_STATUS_OUTPUT_ID.into(),
                label: None,
                kind: OutputKind::String,
                value: OutputValue::String(status.into()),
            },
            OutputEntry {
                id: crate::gui_meta::NET_STATUS_PING_MS_OUTPUT_ID.into(),
                label: None,
                kind: OutputKind::Number,
                value: OutputValue::Number(serde_json::Number::from(ping_ms)),
            },
            OutputEntry {
                id: crate::gui_meta::NET_STATUS_IPV6_OUTPUT_ID.into(),
                label: None,
                kind: OutputKind::Boolean,
                value: OutputValue::Boolean(ipv6),
            },
        ],
    )
}

#[allow(
    clippy::expect_used,
    reason = "callers guarantee primary_output_id exists in outputs; panic is the contract"
)]
fn structured_success(primary_output_id: &str, outputs: Vec<OutputEntry>) -> ToolResult {
    ToolResult::Success(
        ToolSuccess::new(Some(primary_output_id.to_string()), outputs)
            .expect("structured built-in outputs must match their primary output id"),
    )
}

fn register_media_dispatchers() {
    register_single_output_dispatcher(crate::IMAGE_CONVERT_TOOL_ID, |args| {
        crate::image_convert(
            &read_file(args, "input")?,
            read_str(args, "output_format"),
            read_usize(args, "jpeg_quality", crate::IMAGE_CONVERT_DEFAULT_QUALITY)?,
            args.get("background")
                .and_then(Value::as_str)
                .unwrap_or(crate::IMAGE_CONVERT_DEFAULT_BACKGROUND),
            read_usize(args, "svg_width", crate::IMAGE_CONVERT_DEFAULT_SVG_WIDTH)?,
            read_usize(args, "max_output_bytes", crate::MAX_MEDIA_OUTPUT_BYTES)?,
        )
    });
    register_single_output_dispatcher(crate::PDF_INSPECT_TOOL_ID, |args| {
        crate::pdf_inspect(&read_file(args, "input")?)
    });
    register_runtime_dispatcher(crate::PDF_TO_MARKDOWN_TOOL_ID, |args| {
        let result = read_file(args, "input").and_then(|input| crate::pdf_to_markdown(&input));
        let result = match result {
            Ok(result) => result,
            Err(message) => return tool_failure("tool_error", message),
        };
        let report = match serde_json::to_value(result.report) {
            Ok(report) => report,
            Err(error) => return tool_failure("tool_error", error.to_string()),
        };
        structured_success(
            crate::PDF_MARKDOWN_OUTPUT_ID,
            vec![
                OutputEntry {
                    id: crate::PDF_MARKDOWN_OUTPUT_ID.into(),
                    label: None,
                    kind: OutputKind::Markdown,
                    value: OutputValue::Markdown(result.markdown),
                },
                OutputEntry {
                    id: crate::PDF_REPORT_OUTPUT_ID.into(),
                    label: None,
                    kind: OutputKind::Json,
                    value: OutputValue::Json(report),
                },
            ],
        )
    });
    register_single_output_dispatcher(crate::IMAGES_CONVERT_TOOL_ID, |args| {
        crate::images_convert(
            &read_file(args, "images")?,
            read_str(args, "output_format"),
            read_usize(args, "jpeg_quality", crate::IMAGE_CONVERT_DEFAULT_QUALITY)?,
            args.get("background")
                .and_then(Value::as_str)
                .unwrap_or(crate::IMAGE_CONVERT_DEFAULT_BACKGROUND),
            read_usize(args, "svg_width", crate::IMAGE_CONVERT_DEFAULT_SVG_WIDTH)?,
            read_usize(
                args,
                "max_output_bytes",
                crate::IMAGES_CONVERT_DEFAULT_MAX_OUTPUT_BYTES,
            )?,
        )
    });
    register_single_output_dispatcher(crate::IMAGE_TO_PDF_TOOL_ID, |args| {
        crate::image_to_pdf(
            &read_file(args, "input")?,
            read_usize(args, "max_output_bytes", crate::MAX_MEDIA_OUTPUT_BYTES)?,
        )
    });
    register_single_output_dispatcher(crate::PDF_TO_IMAGES_TOOL_ID, |args| {
        crate::pdf_to_images(
            &read_file(args, "input")?,
            read_f64(args, "dpi", crate::PDF_TO_IMAGES_DEFAULT_DPI)?,
            read_usize(args, "max_output_bytes", crate::MAX_MEDIA_OUTPUT_BYTES)?,
        )
    });
    register_single_output_dispatcher(crate::PDF_EXTRACT_IMAGES_TOOL_ID, |args| {
        crate::pdf_extract_images(&read_file(args, "input")?)
    });
    register_single_output_dispatcher(crate::PPTX_EXTRACT_IMAGES_TOOL_ID, |args| {
        crate::pptx_extract_images(&read_file(args, "input")?)
    });
}

/// Read a `File` argument from the dispatch args object, deserializing the
/// JSON `FileValue` shape (name / is_dir / content / mime). Symmetric with
/// [`read_str`]; used by tools that take byte inputs (e.g.
/// `media.pptx_extract_images`). Missing or malformed values yield a clear
/// error rather than a silent default, since a file input has no sensible
/// empty sentinel.
fn read_file(args: DispatchArgs<'_>, key: &str) -> Result<FileValue, String> {
    let value = args
        .get(key)
        .ok_or_else(|| format!("missing required file input `{key}`"))?;
    FileValue::deserialize(value).map_err(|e| format!("input `{key}` is not a valid file: {e}"))
}

fn read_f64(args: DispatchArgs<'_>, key: &str, default: f64) -> Result<f64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_f64().ok_or_else(|| format!("{key} must be a number")),
    }
}

fn read_usize(args: DispatchArgs<'_>, key: &str, default: usize) -> Result<usize, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => {
            let value = value
                .as_u64()
                .ok_or_else(|| format!("{key} must be a non-negative integer"))?;
            usize::try_from(value).map_err(|_| format!("{key} is too large"))
        }
    }
}

#[cfg(test)]
mod tests;
