#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use expect and panic to preserve failure context"
)]

//! End-to-end proof that a generated Board connection is directly runnable.

#[cfg(unix)]
mod unix {
    use std::collections::BTreeMap;
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::path::{Path, PathBuf};
    use std::process::{Child, ChildStdin, Command, Output, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::thread::JoinHandle;
    use std::time::Duration;

    use serde::Deserialize;
    use serde_json::{Value, json};

    const PERSONAL_BOARD: &str = "personal";
    const PROJECT_BOARD: &str = "workflow-project";
    const TOOL_ID: &str = "workflow.where";
    const UNPINNED_TOOL_ID: &str = "num.hex_to_decimal";
    const CONTEXT_TOOL_ID: &str = "upeg.board_context";
    const EXECUTION_LOG: &str = "execution.log";
    const EXPECTED_EXECUTABLE: &str = "sh";
    const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

    #[derive(Debug, Deserialize)]
    struct ConnectionSpec {
        command: PathBuf,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    }

    struct Fixture {
        _temporary: tempfile::TempDir,
        repository: PathBuf,
        other_directory: PathBuf,
        home: PathBuf,
        empty_toolkits: PathBuf,
        empty_wasm: PathBuf,
        empty_imports: PathBuf,
        manifest: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().expect("isolated temp directory");
            let repository = temporary.path().join("repository");
            let other_directory = temporary.path().join("client-cwd");
            let home = temporary.path().join("home");
            let empty_toolkits = temporary.path().join("empty-toolkits");
            let empty_wasm = temporary.path().join("empty-wasm");
            let empty_imports = temporary.path().join("empty-imports");
            for directory in [
                &repository,
                &other_directory,
                &home,
                &empty_toolkits,
                &empty_wasm,
                &empty_imports,
            ] {
                std::fs::create_dir_all(directory).expect("create fixture directory");
            }
            let manifest = repository.join("upeg.toml");
            let fixture = Self {
                _temporary: temporary,
                repository,
                other_directory,
                home,
                empty_toolkits,
                empty_wasm,
                empty_imports,
                manifest,
            };
            fixture.write_manifest("원본 프로젝트 안내", "원본 지침을 따른다.");
            let registered = upeg_loader::load_and_register_file_verbose(&fixture.manifest);
            assert_eq!(
                registered.loaded.len(),
                1,
                "the test process needs the external Tool metadata to edit presets: {:?}",
                registered.failed,
            );
            fixture
        }

        fn write_manifest(&self, description: &str, instructions: &str) {
            std::fs::write(
                &self.manifest,
                format!(
                    r#"id = "workflow"

[[boards]]
id = "{PROJECT_BOARD}"
label = "Workflow project"
description = {description:?}
instructions = {instructions:?}

[[tools]]
id = "where"
display_label = "Working directory echo"
description = "Print the execution directory and selected message"
pegboard_units = "U1"
invoker = "External"
command = "{EXPECTED_EXECUTABLE}"
args_template = ["-c", 'printf "%s|%s\n" "$PWD" "$1"; printf "%s\n" "$1" >> {EXECUTION_LOG}', "upeg-workflow", "{{message}}"]
surfaces = ["cli", "desktop", "mcp"]
boards = ["{PROJECT_BOARD}"]

[[tools.inputs]]
name = "message"
type = "string"
required = true
"#,
                ),
            )
            .expect("write project manifest");
        }

        fn command(&self, args: &[&str]) -> Command {
            let mut command = Command::new(env!("CARGO_BIN_EXE_upeg"));
            command
                .current_dir(&self.repository)
                .env("UPEG_HOME", &self.home)
                .env("UPEG_PROJECT_MANIFEST_PATH", &self.manifest)
                .env("UPEG_TOOLKITS_DIR", &self.empty_toolkits)
                .env("UPEG_WASM_DIR", &self.empty_wasm)
                .env("UPEG_MCP_IMPORTS_DIR", &self.empty_imports)
                .args(args);
            command
        }

        fn success(&self, args: &[&str]) -> String {
            let output = self.command(args).output().expect("run upeg");
            assert_success(&output, args);
            String::from_utf8(output.stdout).expect("upeg stdout UTF-8")
        }

        fn connection(&self, board: &str) -> ConnectionSpec {
            let output = self.success(&["board", board, "connect"]);
            let config: Value = serde_json::from_str(&output).expect("connection JSON");
            serde_json::from_value(config["mcpServers"][format!("upeg-{board}")].clone())
                .expect("MCP server config")
        }

        fn set_personal_preset(&self, message: &str) {
            let path = upeg_sources::pegboard::state_path_from_root(&self.home);
            let visibility = upeg_sources::pegboard::BoardVisibility::global_only();
            let mut state = upeg_sources::pegboard::load_state_from_path_in(&path, &visibility)
                .expect("read personal Board state");
            let placement = state
                .layouts
                .get_mut(PERSONAL_BOARD)
                .and_then(|placements| {
                    placements
                        .iter_mut()
                        .find(|placement| placement.tool_id == TOOL_ID)
                })
                .expect("workflow tool pin on the personal Board");
            placement.args_preset = Some(
                upeg_core::ArgsPreset::parse(&json!({"message": message}).to_string())
                    .expect("preset"),
            );
            upeg_sources::pegboard::save_state_to_path_in(&path, &state, &visibility)
                .expect("save preset");
        }
    }

    struct McpSession {
        child: Child,
        stdin: Option<ChildStdin>,
        responses: Receiver<Result<Value, String>>,
        reader: Option<JoinHandle<()>>,
    }

    impl McpSession {
        fn spawn(spec: &ConnectionSpec, client_directory: &Path) -> Self {
            let mut child = Command::new(&spec.command)
                .args(&spec.args)
                .envs(&spec.env)
                .current_dir(client_directory)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("spawn server with the generated MCP config");
            let stdin = child.stdin.take().expect("MCP stdin");
            let stdout = child.stdout.take().expect("MCP stdout");
            let (sender, responses) = mpsc::channel();
            let reader = std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let value = line.map_err(|error| error.to_string()).and_then(|line| {
                        serde_json::from_str(&line)
                            .map_err(|error| format!("invalid MCP JSON `{line}`: {error}"))
                    });
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            });
            Self {
                child,
                stdin: Some(stdin),
                responses,
                reader: Some(reader),
            }
        }

        fn notify(&mut self, method: &str) {
            self.write(&json!({"jsonrpc":"2.0", "method":method}));
        }

        fn request(&mut self, id: u64, method: &str, params: Option<Value>) -> Value {
            let mut request = json!({"jsonrpc":"2.0", "id":id, "method":method});
            if let Some(params) = params {
                request["params"] = params;
            }
            self.write(&request);
            loop {
                let response = self
                    .responses
                    .recv_timeout(RESPONSE_TIMEOUT)
                    .unwrap_or_else(|error| panic!("timed out waiting for MCP id {id}: {error}"))
                    .unwrap_or_else(|error| panic!("failed to read MCP stdout: {error}"));
                if response["id"] == id {
                    return response;
                }
            }
        }

        fn write(&mut self, request: &Value) {
            let stdin = self.stdin.as_mut().expect("open MCP stdin");
            serde_json::to_writer(&mut *stdin, request).expect("write MCP request");
            stdin.write_all(b"\n").expect("write MCP request separator");
            stdin.flush().expect("flush MCP request");
        }
    }

    impl Drop for McpSession {
        fn drop(&mut self) {
            self.stdin.take();
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
        }
    }

    fn assert_success(output: &Output, args: &[&str]) {
        assert!(
            output.status.success(),
            "upeg {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn tool<'a>(listing: &'a Value, name: &str) -> &'a Value {
        listing["result"]["tools"]
            .as_array()
            .expect("tools/list array")
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("{name} missing from tools/list: {listing}"))
    }

    fn call(session: &mut McpSession, id: u64, name: &str, arguments: Option<Value>) -> Value {
        let mut params = json!({"name":name});
        if let Some(arguments) = arguments {
            params["arguments"] = arguments;
        }
        session.request(id, "tools/call", Some(params))
    }

    #[test]
    fn a_generated_board_connection_honours_the_guidance_and_tool_execution_contract_end_to_end() {
        let fixture = Fixture::new();
        fixture.success(&[
            "board",
            PERSONAL_BOARD,
            "describe",
            "--description",
            "개인 릴리스 작업",
            "--instructions",
            "preset을 먼저 검토한다.",
        ]);
        fixture.success(&["board", PERSONAL_BOARD, "pin", TOOL_ID]);
        fixture.set_personal_preset("preset-message");

        let context: Value =
            serde_json::from_str(&fixture.success(&["board", PERSONAL_BOARD, "context", "--json"]))
                .expect("personal Board context JSON");
        assert_eq!(context["description"], "개인 릴리스 작업");
        assert_eq!(context["instructions"], "preset을 먼저 검토한다.");
        let configured_tool = context["tools"]
            .as_array()
            .expect("context tools")
            .iter()
            .find(|tool| tool["id"] == TOOL_ID)
            .expect("workflow tool in context");
        assert_eq!(configured_tool["defaults"]["message"], "preset-message");
        assert_eq!(configured_tool["readiness"]["status"], "ready");
        let repository = fixture
            .repository
            .canonicalize()
            .expect("repository canonical path");
        assert_eq!(
            configured_tool["working_directory"].as_str(),
            repository.to_str(),
        );
        assert_eq!(
            configured_tool["readiness"]["reasons"],
            json!([format!(
                "Executable checked: {EXPECTED_EXECUTABLE}. The command has not been run."
            )]),
        );

        let personal_connection = fixture.connection(PERSONAL_BOARD);
        assert_eq!(
            personal_connection
                .command
                .canonicalize()
                .expect("connection command"),
            Path::new(env!("CARGO_BIN_EXE_upeg"))
                .canonicalize()
                .expect("Cargo upeg binary"),
        );
        assert_eq!(
            personal_connection.args,
            [
                "--working-directory",
                fixture.repository.to_str().expect("repository path"),
                "mcp",
                "--board",
                PERSONAL_BOARD,
            ],
        );
        assert_eq!(
            Path::new(&personal_connection.env["UPEG_PROJECT_MANIFEST_PATH"]),
            fixture.manifest,
        );

        let mut personal = McpSession::spawn(&personal_connection, &fixture.other_directory);
        let initialized = personal.request(1, "initialize", Some(json!({})));
        assert!(
            initialized["result"]["instructions"]
                .as_str()
                .expect("initialize instructions")
                .contains(CONTEXT_TOOL_ID),
        );
        personal.notify("notifications/initialized");

        let listing = personal.request(2, "tools/list", None);
        let listed_tool = tool(&listing, TOOL_ID);
        assert_eq!(
            listed_tool["inputSchema"]["properties"]["message"]["default"],
            "preset-message",
        );
        assert!(
            !listed_tool["inputSchema"]["required"]
                .as_array()
                .expect("required array")
                .contains(&json!("message")),
        );
        tool(&listing, CONTEXT_TOOL_ID);

        let agent_context = call(&mut personal, 3, CONTEXT_TOOL_ID, None);
        assert_eq!(
            agent_context["result"]["structuredContent"]["working_directory"].as_str(),
            Some(
                fixture
                    .repository
                    .canonicalize()
                    .expect("repository canonical")
                    .to_str()
                    .expect("repository UTF-8"),
            ),
        );
        assert_eq!(
            agent_context["result"]["structuredContent"]["description"],
            "개인 릴리스 작업",
        );

        let preset_call = call(&mut personal, 4, TOOL_ID, None);
        let preset_text = preset_call["result"]["content"][0]["text"]
            .as_str()
            .expect("preset call result");
        assert!(
            preset_text.contains(&fixture.repository.to_string_lossy().to_string()),
            "the execution path is not the repository: {preset_text}",
        );
        assert!(preset_text.contains("preset-message"), "{preset_text}");

        let override_call = call(
            &mut personal,
            5,
            TOOL_ID,
            Some(json!({"message":"explicit-message"})),
        );
        let override_text = override_call["result"]["content"][0]["text"]
            .as_str()
            .expect("override call result");
        assert!(
            override_text.contains("explicit-message"),
            "{override_text}"
        );
        assert!(!override_text.contains("preset-message"), "{override_text}");
        assert_eq!(
            std::fs::read_to_string(fixture.repository.join(EXECUTION_LOG))
                .expect("execution log")
                .lines()
                .count(),
            2,
        );

        let unpinned = call(
            &mut personal,
            6,
            UNPINNED_TOOL_ID,
            Some(json!({"input":"0xff"})),
        );
        assert_eq!(unpinned["error"]["code"], -32601);
        assert!(
            unpinned["error"]["message"]
                .as_str()
                .expect("unpinned error")
                .contains(TOOL_ID),
        );
        drop(personal);

        let project_context: Value =
            serde_json::from_str(&fixture.success(&["board", PROJECT_BOARD, "context", "--json"]))
                .expect("project Board context JSON");
        assert_eq!(project_context["description"], "원본 프로젝트 안내");
        assert_eq!(project_context["instructions"], "원본 지침을 따른다.");
        assert_eq!(
            project_context["guidance_manifest"].as_str(),
            fixture.manifest.to_str(),
        );

        let rejected_edit = fixture
            .command(&[
                "board",
                PROJECT_BOARD,
                "describe",
                "--description",
                "저장소에서 덮어쓸 안내",
            ])
            .output()
            .expect("run project Board describe");
        assert!(!rejected_edit.status.success());
        assert!(
            String::from_utf8_lossy(&rejected_edit.stderr)
                .contains(&fixture.manifest.to_string_lossy().to_string()),
        );

        let project_connection = fixture.connection(PROJECT_BOARD);
        let mut project = McpSession::spawn(&project_connection, &fixture.other_directory);
        let initialized = project.request(10, "initialize", Some(json!({})));
        assert!(
            initialized["result"]["instructions"]
                .as_str()
                .expect("project initialize instructions")
                .contains("원본 프로젝트 안내"),
        );
        let original = call(&mut project, 11, CONTEXT_TOOL_ID, None);
        assert_eq!(
            original["result"]["structuredContent"]["instructions"],
            "원본 지침을 따른다.",
        );

        fixture.write_manifest("갱신된 프로젝트 안내", "새 지침을 따른다.");
        project.write(&json!({
            "jsonrpc":"2.0",
            "method":"tools/call",
            "params":{"name":TOOL_ID, "arguments":{"message":"must-not-run"}}
        }));
        let stale = call(&mut project, 12, CONTEXT_TOOL_ID, None);
        assert_eq!(stale["error"]["code"], -32001);
        assert_eq!(stale["error"]["data"]["reconnectRequired"], true);
        assert_eq!(
            std::fs::read_to_string(fixture.repository.join(EXECUTION_LOG))
                .expect("execution log after the stale notification")
                .lines()
                .count(),
            2,
            "even a tools/call left without a response must not run once the session changed",
        );
        drop(project);

        let mut reconnected = McpSession::spawn(&project_connection, &fixture.other_directory);
        let fresh = call(&mut reconnected, 13, CONTEXT_TOOL_ID, None);
        assert_eq!(
            fresh["result"]["structuredContent"]["description"],
            "갱신된 프로젝트 안내",
        );
        assert_eq!(
            fresh["result"]["structuredContent"]["instructions"],
            "새 지침을 따른다.",
        );
    }
}
