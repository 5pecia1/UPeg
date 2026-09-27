use super::common::parse;
use crate::domain::execution::dispatch;
use crate::*;

mod kv_args;

#[cfg(not(target_arch = "wasm32"))]
fn cli_media_temp_dir() -> std::path::PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "upeg_cli_media_{}_{}",
        std::process::id(),
        timestamp
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn cli_media_test_png_bytes() -> Vec<u8> {
    let pixel = image::Rgba([255_u8, 0, 0, 255]);
    let image = image::ImageBuffer::from_pixel(1, 1, pixel);
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("PNG fixture should encode");
    bytes
}

/// Build a zip archive holding one PNG image — the `media.image_to_pdf` input
/// contract (a zip of images).
#[cfg(not(target_arch = "wasm32"))]
fn write_cli_media_test_image_zip(path: &std::path::Path) {
    let png = cli_media_test_png_bytes();
    let file = std::fs::File::create(path).expect("zip fixture should be created");
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("image.png", zip::write::SimpleFileOptions::default())
        .expect("zip entry should start");
    std::io::Write::write_all(&mut zip, &png).expect("zip entry should write");
    zip.finish().expect("zip fixture should finalize");
}

#[test]
fn no_command_without_a_tty_prints_a_help_hint() {
    let out = run_no_command_with_terminal(None, None, false).expect("no-args should not fail");
    assert!(out.contains("upeg"));
    assert!(out.contains("--help"));
}

#[test]
fn no_command_without_a_tty_prints_the_selected_board_context() {
    let out = run_no_command_with_terminal(Some("dev"), None, false).expect("no-args board");
    assert!(out.contains("Board `dev` selected"), "got:\n{out}");
}

#[test]
fn no_command_without_a_tty_prints_the_selected_board_and_tag_context() {
    let out = run_no_command_with_terminal(Some("dev"), Some("pure"), false).expect("no-args tag");
    assert!(
        out.contains("Board `dev` + Tag `pure` selected"),
        "got:\n{out}"
    );
}

#[test]
fn completions_alias_generates_the_documented_bash_script() {
    let out = run(parse(&["upeg", "completions", "bash"])).expect("completion alias");
    assert!(
        out.contains("upeg") && out.contains("complete"),
        "bash completion output should be generated, got:\n{out}"
    );
}

#[test]
fn cli_lists_builtin_hex_to_decimal_and_dispatches_directly_and_through_call() {
    let list = run(parse(&["upeg", "tool", "list"])).expect("tool list");
    assert!(
        list.lines()
            .any(|line| line.starts_with("num.hex_to_decimal\tnum\tInline")),
        "tool list must expose num.hex_to_decimal as a CLI Inline tool; got:\n{list}"
    );

    let direct = run(parse(&["upeg", "num", "hex-to-decimal", "0xff"])).expect("direct call");
    assert_eq!(direct, "255\n");

    let generic = run(parse(&[
        "upeg",
        "call",
        "num.hex_to_decimal",
        "-a",
        "input=0xff",
    ]))
    .expect("generic call");
    assert_eq!(generic, "255\n");
}

#[test]
fn cli_tool_list_includes_media_tools() {
    let list = run(parse(&["upeg", "tool", "list"])).expect("tool list");
    assert!(
        list.lines()
            .any(|line| line == "media.image_to_pdf\tmedia\tLauncher"),
        "tool list must expose media.image_to_pdf, got:\n{list}"
    );
    assert!(
        list.lines()
            .any(|line| line == "media.pdf_to_images\tmedia\tLauncher"),
        "tool list must expose media.pdf_to_images, got:\n{list}"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cli_media_image_to_pdf_call_creates_a_pdf() {
    let dir = cli_media_temp_dir();
    std::fs::create_dir_all(&dir).expect("media test dir should be created");
    let input = dir.join("images.zip");
    let output = dir.join("output.pdf");
    write_cli_media_test_image_zip(&input);

    // `@path` File input + `--out` File(pdf) output, the F3 contract.
    let input_arg = format!("input=@{}", input.display());
    let out_flag = output.display().to_string();
    let out = run(parse(&[
        "upeg",
        "call",
        "media.image_to_pdf",
        "-a",
        input_arg.as_str(),
        "--out",
        out_flag.as_str(),
    ]))
    .expect("media image_to_pdf call should succeed");

    let bytes = std::fs::read(&output).expect("output PDF should be readable");
    assert!(bytes.starts_with(b"%PDF-"), "output must be a PDF");
    assert!(
        out.contains("wrote"),
        "CLI should report the written path, got: {out}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dynamic_toolkit_tool_route_dispatches_runtime_tools() {
    let id = "dyn.echo";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "dyn",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "dyn")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Dynamic route echo",
        input_spec: upeg_core::InputSpec::try_from(&serde_json::json!({
            "type": "object",
            "properties": { "input": { "type": "string" } },
            "additionalProperties": false,
        }))
        .expect("test input spec should import"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    });

    let out = run(parse(&["upeg", "dyn", "echo", "hello"])).expect("dynamic dispatch");
    assert_eq!(out, "hello\n");
}

#[test]
fn dynamic_toolkit_tool_route_uses_structured_keys_for_dotted_segments() {
    let id = "github.com.admin.tools.echo_cli";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "github.com",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "github.com")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Dotted dynamic route echo",
        input_spec: upeg_core::InputSpec::try_from(&serde_json::json!({
            "type": "object",
            "properties": { "input": { "type": "string" } },
            "additionalProperties": false,
        }))
        .expect("test input spec should import"),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        Ok(args
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    });

    let out = run(parse(&[
        "upeg",
        "github.com",
        "admin.tools.echo_cli",
        "hello",
    ]))
    .expect("dotted structured dynamic dispatch");
    assert_eq!(out, "hello\n");
}

#[test]
fn dynamic_toolkit_tool_route_injects_board_context() {
    let id = "dyn.ctx";
    let mut env = std::collections::BTreeMap::new();
    env.insert("PROFILE".to_string(), "cli".to_string());
    upeg_runtime::register_board_context(upeg_core::BoardExecutionContext {
        board: "cli-board".into(),
        env,
        project_manifest: Some("/tmp/upeg-cli-board/upeg.toml".into()),
    });
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "dyn",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "dyn")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Dynamic route context echo",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &["cli-board"],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        let context = args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| "missing board context".to_string())?;
        Ok(format!(
            "{}:{}:{}",
            context["board"].as_str().unwrap_or(""),
            context["boardEnv"]["PROFILE"].as_str().unwrap_or(""),
            context["projectManifest"].as_str().unwrap_or("")
        ))
    });

    let out = run(parse(&["upeg", "--board", "cli-board", "dyn", "ctx"])).expect("board dispatch");
    assert_eq!(out, "cli-board:cli:/tmp/upeg-cli-board/upeg.toml\n");
}

#[test]
fn execution_log_records_cli_dispatch_metadata_without_args_or_output() {
    let record = crate::adapters::execution_log::record_from_dispatch(
        "num.hex_to_decimal",
        &serde_json::json!({
            "input": "ARGUMENT_SENTINEL_DO_NOT_LOG",
            "_upeg": {"surface": "cli", "board": "dev"}
        }),
        &dispatch::Outcome::Success(dispatch::text_success("OUTPUT_SENTINEL_DO_NOT_LOG")),
        std::time::Duration::from_millis(3),
    );
    let encoded = serde_json::to_string(&record).unwrap();
    assert!(encoded.contains("num.hex_to_decimal"));
    assert!(!encoded.contains("ARGUMENT_SENTINEL_DO_NOT_LOG"));
    assert!(!encoded.contains("OUTPUT_SENTINEL_DO_NOT_LOG"));
    assert_eq!(record.surface, "cli");
    assert_eq!(record.board.as_deref(), Some("dev"));
}

#[test]
fn log_since_parser_accepts_relative_windows_and_epoch_milliseconds() {
    assert_eq!(parse_log_since("123456789").unwrap(), 123456789);
    let cutoff = parse_log_since("1h").unwrap();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    assert!(
        now_ms.saturating_sub(cutoff) <= 3_601_000,
        "1h cutoff must be close to now-1h, got cutoff={cutoff}, now={now_ms}"
    );
    assert!(parse_log_since("1w").is_err());
}

#[test]
fn credential_add_stores_only_references() {
    let path = std::env::temp_dir().join("upeg_cli_credential_add.json");
    let _ = std::fs::remove_file(&path);
    let record =
        credentials::add_reference_at(&path, "etherscan_api_key", None, Some("Authorization"))
            .expect("add credential reference");
    assert_eq!(record.env, "UPEG_CREDENTIAL_ETHERSCAN_API_KEY");
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("secret"));
    assert!(!raw.contains("api-key-value"));
    let _ = std::fs::remove_file(&path);
}

/// Registers a tool whose single text output echoes `surface:trigger` from the
/// reserved execution context, so a caller can assert exactly what a dispatch
/// stamped into `_upeg`.
fn register_context_echo_tool(id: &'static str, toolkit: &'static str) {
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit,
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, toolkit)
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Trigger context echo",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        let context = args
            .get(upeg_core::EXECUTION_CONTEXT_ARG)
            .ok_or_else(|| "missing execution context".to_string())?;
        Ok(format!(
            "{}:{}",
            context["surface"].as_str().unwrap_or(""),
            context["trigger"].as_str().unwrap_or("")
        ))
    });
}

#[test]
fn trigger_fire_stamps_the_first_declared_binding_label() {
    // `_upeg.trigger` carries the fired trigger, never the tool id — the tool
    // already knows its own id. `fire` names a tool, so the first declared
    // binding wins (`upeg_runtime::execution` module docs).
    const ID: &str = "trigfire.labelled";
    register_context_echo_tool(ID, "trigfire");
    upeg_runtime::set_trigger_bindings(
        ID,
        vec![
            upeg_runtime::TriggerBinding {
                tool_id: ID,
                source: upeg_runtime::TriggerSource::File.as_str().into(),
                condition: Some("/tmp/first.txt".into()),
            },
            upeg_runtime::TriggerBinding {
                tool_id: ID,
                source: upeg_runtime::TriggerSource::Clipboard.as_str().into(),
                condition: None,
            },
        ],
    );

    let out = run(parse(&["upeg", "trigger", "fire", ID])).expect("trigger fire");
    assert_eq!(out, "cli:file:/tmp/first.txt\n");
    upeg_runtime::set_trigger_bindings(ID, Vec::new());
}

#[test]
fn trigger_fire_without_bindings_does_not_stamp_a_label() {
    // No trigger declared means no trigger fired: a human did. Stamping a
    // synthetic source would put a value in the execution log's `trigger`
    // column that no tool could ever declare.
    const ID: &str = "trigfire.bare";
    register_context_echo_tool(ID, "trigfire");

    let out = run(parse(&["upeg", "trigger", "fire", ID])).expect("trigger fire");
    assert_eq!(out, "cli:\n");
}

#[test]
fn trigger_runtime_runs_supported_schedules_and_reports_host_diagnostics() {
    let id = "trig.runtime";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "trig",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "trig")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Trigger runtime echo",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |_args| Ok("ran".to_string()));
    upeg_runtime::set_trigger_bindings(
        id,
        vec![
            upeg_runtime::TriggerBinding {
                tool_id: id,
                source: "schedule".into(),
                condition: Some("now".into()),
            },
            upeg_runtime::TriggerBinding {
                tool_id: id,
                source: "hotkey".into(),
                condition: Some("ctrl+shift+u".into()),
            },
        ],
    );

    let list = run(parse(&["upeg", "trigger", "list", "--json"])).expect("trigger list");
    assert!(list.contains("\"runtimeSupported\": true"), "{list}");
    assert!(list.contains("platform global-hotkey adapter"), "{list}");

    let out = run(parse(&["upeg", "trigger", "run"])).expect("trigger run");
    assert!(out.contains("trig.runtime\tschedule\tok\tran"), "{out}");
    assert!(out.contains("trig.runtime\thotkey\tunsupported"), "{out}");
}

#[test]
fn trigger_runtime_rejects_tools_not_on_the_cli_surface() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    CALLS.store(0, Ordering::SeqCst);

    let id = "trig.runtime_http_only";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "trig",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "trig")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "HTTP-only trigger runtime guard",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |_args| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok("should-not-run".to_string())
    });
    upeg_runtime::set_trigger_bindings(
        id,
        vec![upeg_runtime::TriggerBinding {
            tool_id: id,
            source: "schedule".into(),
            condition: Some("now".into()),
        }],
    );

    let out = run(parse(&["upeg", "trigger", "run"])).expect("trigger run");
    assert!(
        out.contains("trig.runtime_http_only\tschedule\ttool_error"),
        "{out}"
    );
    assert!(!out.contains("should-not-run"), "{out}");
    assert_eq!(CALLS.load(Ordering::SeqCst), 0, "must not dispatch");
}

#[test]
fn trigger_runtime_runs_file_and_directory_path_adapters() {
    let root = std::env::temp_dir().join(format!("upeg_trigger_paths_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("input.txt");
    std::fs::write(&file, "payload").unwrap();

    let id = "trig.path";
    upeg_runtime::toolbox_add_tool(upeg_core::ToolMeta {
        id,
        toolkit: "trig",
        local_id: upeg_core::ToolId::parse_canonical_in_toolkit(id, "trig")
            .expect("test ToolMeta id must be canonical")
            .local(),
        tags: &["test"],
        display_label: "Test tool",
        description: "Trigger path echo",
        input_spec: upeg_core::InputSpec::empty(),
        output_spec: upeg_core::OutputSpec::empty(),
        primary_output_id: None,
        effect: upeg_core::ToolEffect::Unknown,
        presentation: None,
        source: upeg_core::Source::UserInput,
        pin: upeg_core::PinKind::Inline,
        pegboard_units: upeg_core::PegboardUnits::U1,
        invoker: upeg_core::Invoker::Function,
        surfaces: &[upeg_core::Surface::Cli],
        boards: &[],
    });
    upeg_runtime::register_single_text_runtime_dispatcher(id, |args| {
        Ok(args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    });
    upeg_runtime::set_trigger_bindings(
        id,
        vec![
            upeg_runtime::TriggerBinding {
                tool_id: id,
                source: "file".into(),
                condition: Some(file.display().to_string()),
            },
            upeg_runtime::TriggerBinding {
                tool_id: id,
                source: "directory".into(),
                condition: Some(root.display().to_string()),
            },
        ],
    );

    let out = run(parse(&["upeg", "trigger", "run"])).expect("trigger run");
    assert!(out.contains("trig.path\tfile\tok"), "{out}");
    assert!(out.contains(file.to_string_lossy().as_ref()), "{out}");
    assert!(out.contains("trig.path\tdirectory\tok"), "{out}");
    assert!(out.contains(root.to_string_lossy().as_ref()), "{out}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tool_list_includes_hex_to_decimal_and_is_sorted_by_id() {
    let out = run(parse(&["upeg", "tool", "list"])).expect("tool list");
    assert!(
        out.contains("num.hex_to_decimal"),
        "expected hex_to_decimal in output, got:\n{out}"
    );
    // Should be sorted by id — first column lexicographic.
    let first_ids: Vec<&str> = out
        .lines()
        .filter_map(|line| line.split('\t').next())
        .collect();
    let mut sorted = first_ids.clone();
    sorted.sort_unstable();
    assert_eq!(first_ids, sorted, "tool list must be sorted by id");
}

#[test]
fn toolkit_list_groups_tools_with_effective_tags() {
    let out = run(parse(&["upeg", "toolkit", "list"])).expect("toolkit list");
    let convert = out
        .lines()
        .find(|line| line.starts_with("convert\t"))
        .expect("convert toolkit row");
    assert!(
        convert.contains('[') && convert.contains("pure") && convert.contains("tools"),
        "toolkit list row must expose effective tags and tool count, got:\n{convert}"
    );
}

#[test]
fn toolkit_show_lists_tools_with_effective_tags() {
    let out = run(parse(&["upeg", "toolkit", "show", "convert"])).expect("toolkit show convert");
    assert!(out.contains("toolkit      convert"), "got:\n{out}");
    assert!(
        out.contains("convert.html_encode") && out.contains("[convert") && out.contains("pure"),
        "toolkit show must list tools with effective tags, got:\n{out}"
    );
}

#[test]
fn tag_and_board_commands_expose_first_class_resources() {
    let tags = run(parse(&["upeg", "tag", "list"])).expect("tag list");
    assert!(
        tags.lines().any(|line| line.starts_with("pure\t")),
        "{tags}"
    );

    let pure = run(parse(&["upeg", "tag", "show", "pure"])).expect("tag show pure");
    assert!(
        pure.contains("tag          pure") && pure.contains("num.hex_to_decimal"),
        "got:\n{pure}"
    );

    let boards = run(parse(&["upeg", "board", "list"])).expect("board list");
    assert!(
        boards.lines().any(|line| line.starts_with("dev\t")),
        "{boards}"
    );

    let dev = run(parse(&["upeg", "board", "show", "dev"])).expect("board show dev");
    assert!(
        dev.contains("board        dev") && dev.contains("num.hex_to_decimal"),
        "got:\n{dev}"
    );
}

#[test]
fn tool_list_tag_filter_narrows_to_a_single_toolkit_tag() {
    let out = run(parse(&["upeg", "tool", "list", "--tag", "convert"])).unwrap();
    // All output lines must have second column == "convert".
    for line in out.lines() {
        let toolkit = line.split('\t').nth(1).unwrap_or("");
        assert_eq!(
            toolkit, "convert",
            "non-convert row leaked through filter:\n{line}"
        );
    }
    // Spot-check expected convert tools are present. `num.hex_to_decimal`
    // used to live here before the `num` toolkit split it out — it no
    // longer carries the `convert` tag, so `convert.html_encode` replaces
    // it as the third example.
    for expected in [
        "convert.html_encode",
        "convert.base64_encode",
        "convert.base32_encode",
    ] {
        assert!(
            out.contains(expected),
            "missing `{expected}` in --toolkit=convert output"
        );
    }
    // Spot-check non-convert tools are absent. `num.hex_to_decimal` moved
    // to the `num` toolkit, so it must no longer show under `--tag convert`.
    assert!(!out.contains("hash.sha256"));
    assert!(!out.contains("id.uuid_v7"));
    assert!(!out.contains("num.hex_to_decimal"));
}

#[test]
fn tool_list_returns_empty_results_for_an_unknown_tag() {
    let out = run(parse(&["upeg", "tool", "list", "--tag", "no_such_tag_xyz"])).unwrap();
    assert!(
        out.is_empty(),
        "unknown tag should produce no rows, got: {out:?}"
    );
}

#[test]
fn tool_list_board_typo_keeps_stdout_empty_and_exit_zero() {
    // Same pipe-friendly contract as --tag applies to --board.
    // Typo'd board name produces a stderr hint (verified live), but
    // stdout stays empty and exit is 0 so scripts piping
    // `tool list --board <maybe-empty> | grep` keep working.
    let out = run(parse(&["upeg", "tool", "list", "--board", "der"])).unwrap();
    assert!(
        out.is_empty(),
        "typo on --board must keep stdout empty (pipe-friendly); got {out:?}"
    );
}

#[test]
fn tool_list_tag_typo_keeps_stdout_empty_and_exit_zero() {
    // a typo on `--tag` (e.g. `encod`) emits a stderr hint
    // but stdout stays empty and exit code is still 0 so pipe-friendly
    // scripts (`tool list --tag <maybe-empty> | grep`) keep working
    // even when the tag happens to be misspelled today.
    //
    // We can't capture stderr from `cargo test` here, so this test
    // just pins the *no-side-effect-on-stdout* contract — the hint
    // itself is verified by the live demo + the helper's own tests.
    let out = run(parse(&["upeg", "tool", "list", "--tag", "encod"])).unwrap();
    assert!(
        out.is_empty(),
        "typo on --tag must keep stdout empty (pipe-friendly); got {out:?}"
    );
}

// ─── tool list --surface / --json  ────────────────

#[test]
fn tool_list_surface_filter_shows_other_surface_views() {
    // Default (no --surface) implicitly filters by `cli`. With
    // `--surface mcp`, all built-ins (which have ALL_SURFACES) still
    // appear because mcp is in the surfaces list.
    let cli_view = run(parse(&["upeg", "tool", "list"])).unwrap();
    let mcp_view = run(parse(&["upeg", "tool", "list", "--surface", "mcp"])).unwrap();
    assert!(mcp_view.contains("num.hex_to_decimal"));
    // Same set for ALL_SURFACES tools — the surface filter only excludes
    // when a tool restricts itself to specific surfaces.
    for line in cli_view.lines() {
        let id = line.split('\t').next().unwrap();
        // ALL_SURFACES tools should still be listed under any surface.
        // Skip non-ALL_SURFACES entries (registered by other tests).
        if let Some(meta) = upeg_runtime::toolbox_tool(id)
            && meta.surfaces.len() == 7
        {
            assert!(
                mcp_view.contains(id),
                "ALL_SURFACES tool `{id}` missing from --surface mcp view"
            );
        }
    }
}

#[test]
fn tool_list_surface_filter_excludes_non_matching_tools() {
    // Register a tool only on http; verify it shows under --surface http
    // but not under --surface cli.
    let id = "test.iter49.http_only_view";
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
        invoker: upeg_core::Invoker::External,
        surfaces: &[upeg_core::Surface::Http],
        boards: &[],
    });
    let cli_out = run(parse(&["upeg", "tool", "list"])).unwrap();
    let http_out = run(parse(&["upeg", "tool", "list", "--surface", "http"])).unwrap();
    assert!(
        !cli_out.contains(id),
        "http-only tool leaked into default cli list"
    );
    assert!(
        http_out.contains(id),
        "http-only tool missing from --surface http view"
    );
}

#[test]
fn tool_list_unknown_surface_returns_a_clean_error() {
    let r = run(parse(&["upeg", "tool", "list", "--surface", "fax"]));
    match r {
        Err(CliError::ToolFailed(msg)) => {
            assert!(msg.contains("unknown surface"));
            assert!(msg.contains("fax"));
        }
        other => panic!("expected ToolFailed, got {other:?}"),
    }
}

#[test]
fn tool_list_json_output_is_a_valid_json_array() {
    // Pin every field the `to_json_object` helper emits so that
    // additions like `outputSchema`, `boards`, `embedUrl`, and `selectorBindings`
    // cannot silently drop here. Same shape pinned by MCP's parity
    // test and HTTP's `/v1/tools` test.
    let out = run(parse(&["upeg", "tool", "list", "--json"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("--json must emit valid JSON: {e}\n{out}"));
    let arr = v.as_array().expect("top level must be array");
    assert!(!arr.is_empty(), "expected at least one tool");
    let first = &arr[0];
    for required in [
        "id",
        "toolkit",
        "tool",
        "tags",
        "description",
        "inputSchema",
        "outputSchema",
        "pin",
        "pegboardUnits",
        "pegboardSpan",
        "invoker",
        "surfaces",
        "boards",
        "embedUrl",
        "selectorBindings",
    ] {
        assert!(
            first.get(required).is_some(),
            "json entry missing `{required}`: {first}"
        );
    }
}

#[test]
fn tag_filter_narrows_tool_list_json_results() {
    let out = run(parse(&["upeg", "tool", "list", "--json", "--tag", "hash"])).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let arr = v.as_array().unwrap();
    for entry in arr {
        assert_eq!(entry["toolkit"], "hash");
    }
    let ids: Vec<&str> = arr.iter().filter_map(|e| e["id"].as_str()).collect();
    assert!(ids.contains(&"hash.sha256"));
    assert!(ids.contains(&"hash.md5"));
}

#[test]
fn tool_list_format_is_three_tab_separated_columns() {
    let out = run(parse(&["upeg", "tool", "list"])).expect("tool list");
    for line in out.lines() {
        let cols: Vec<_> = line.split('\t').collect();
        assert_eq!(
            cols.len(),
            3,
            "expected 3 tab-separated columns, got: {line:?}"
        );
    }
}

#[test]
fn tool_show_prints_the_manifest() {
    let out = run(parse(&["upeg", "tool", "show", "num.hex_to_decimal"]))
        .expect("tool show should succeed");
    assert!(out.contains("id"));
    assert!(out.contains("num.hex_to_decimal"));
    assert!(out.contains("pin"));
    assert!(out.contains("Inline"));
    assert!(out.contains("invoker"));
    assert!(out.contains("function"));
    assert!(out.contains("surfaces"));
}

#[test]
fn num_hex_to_decimal_happy_path_works() {
    let out = run(parse(&["upeg", "num", "hex-to-decimal", "0xff"])).expect("happy path");
    assert_eq!(out, "255\n");
}

#[test]
fn num_hex_to_decimal_error_path_returns_tool_failure() {
    let r = run(parse(&["upeg", "num", "hex-to-decimal", "0xZZ"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

#[test]
fn num_hex_to_decimal_works_without_a_prefix() {
    let out = run(parse(&["upeg", "num", "hex-to-decimal", "ff"])).expect("no prefix");
    assert_eq!(out, "255\n");
}

#[test]
fn cli_error_messages_carry_upeg_prefix() {
    let e = CliError::UnknownTool("x".into());
    assert!(e.message().starts_with("upeg:"));
    let e = CliError::ToolFailed("invalid hex".into());
    assert!(e.message().starts_with("upeg:"));
}

#[test]
fn cli_error_exit_codes_are_user_errors() {
    // PRD §6.5 doesn't pin specific codes but 1 is the conventional user
    // error code (vs 2 = misuse, 0 = success). Keep them stable for scripts.
    assert_eq!(CliError::UnknownTool("x".into()).exit_code(), 1);
    assert_eq!(CliError::ToolFailed("err".into()).exit_code(), 1);
}

#[test]
fn id_uuid_v7_prints_a_canonical_36_char_string() {
    let out = run(parse(&["upeg", "id", "uuid-v7"])).expect("uuid-v7 dispatch");
    let s = out.trim_end_matches('\n');
    assert_eq!(s.len(), 36);
    assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
    // Position 14 is the version nibble — should be '7' for v7.
    assert_eq!(s.chars().nth(14), Some('7'));
}

#[test]
fn id_nanoid_prints_a_url_safe_21_char_string() {
    let out = run(parse(&["upeg", "id", "nanoid"])).expect("nanoid dispatch");
    let s = out.trim_end_matches('\n');
    assert_eq!(s.len(), 21);
    assert!(
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

#[test]
fn tool_list_includes_id_uuid_v7() {
    let out = run(parse(&["upeg", "tool", "list"])).expect("tool list");
    assert!(
        out.contains("id.uuid_v7"),
        "expected id.uuid_v7 in tool list, got:\n{out}"
    );
}

#[test]
fn convert_base64_encode_matches_a_known_vector() {
    let out = run(parse(&["upeg", "convert", "base64-encode", "foo"])).expect("encode");
    assert_eq!(out, "Zm9v\n");
}

#[test]
fn convert_base64_decode_matches_a_known_vector() {
    let out = run(parse(&["upeg", "convert", "base64-decode", "Zm9v"])).expect("decode");
    assert_eq!(out, "foo\n");
}

#[test]
fn convert_base64_decode_returns_tool_failure_for_invalid_input() {
    let r = run(parse(&["upeg", "convert", "base64-decode", "!!!"]));
    assert!(matches!(r, Err(CliError::ToolFailed(_))));
}

#[test]
fn tool_list_reflects_all_four_newly_registered_builtin_tools() {
    let out = run(parse(&["upeg", "tool", "list"])).expect("tool list"); // Each new #[upeg::tool] should appear automatically — no manual edit
    // to tool list code. PRD §5.1.
    for expected in [
        "num.hex_to_decimal",
        "id.uuid_v7",
        "convert.base64_encode",
        "convert.base64_decode",
    ] {
        assert!(out.contains(expected), "missing {expected} in:\n{out}");
    }
}
