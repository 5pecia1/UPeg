pub(crate) mod args;
pub(crate) mod completion;
pub(crate) mod doctor;
pub(crate) mod doctor_surfaces;
pub(crate) mod formatters;
pub(crate) mod readiness;
#[cfg(feature = "wasm-plugin")]
pub(crate) mod wasm_template;

use self::args::parse_kv_arg;
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::ffi::OsString;
use upeg_core::{
    Surface,
    interface_inventory::{
        Compatibility, ContractIo, ContractIoKind, ContractLocator, ContractShape, DocRef,
        InterfaceEntry, InterfaceKind, OwnerRef, SourceRef, SurfaceSet, TestMapping,
    },
};

use crate::inventory::command::InterfaceCommand;

const CLI_SURFACE_VERSION: &str = "v1";
const CLI_OWNER_PATH: &str = "upeg-cli/Cargo.toml";
const CLI_DOCS_PATH: &str = "README.md";
const CLI_TEST_PATH: &str = "upeg-cli/src/inventory/tests.rs";
/// The test inside [`CLI_TEST_PATH`] that actually asserts these
/// entries. Pinned by the inventory honesty check, which fails when
/// no `fn <name>` with this spelling exists in that file.
const CLI_TEST_NAME: &str = "interface_inventory_covers_cli_http_and_mcp";

struct CliCommandDeclaration {
    id: &'static str,
    command_path: &'static str,
}

const CLI_COMMANDS: &[CliCommandDeclaration] = &[
    CliCommandDeclaration {
        id: "cli.tool.list",
        command_path: "tool.list",
    },
    CliCommandDeclaration {
        id: "cli.call",
        command_path: "call",
    },
    CliCommandDeclaration {
        id: "cli.board.list",
        command_path: "board.list",
    },
    CliCommandDeclaration {
        id: "cli.board.call",
        command_path: "board.call",
    },
    CliCommandDeclaration {
        id: "cli.board.pin",
        command_path: "board.pin",
    },
    CliCommandDeclaration {
        id: "cli.board.unpin",
        command_path: "board.unpin",
    },
    CliCommandDeclaration {
        id: "cli.board.move",
        command_path: "board.move",
    },
    CliCommandDeclaration {
        id: "cli.board.context",
        command_path: "board.context",
    },
    CliCommandDeclaration {
        id: "cli.board.connect",
        command_path: "board.connect",
    },
    CliCommandDeclaration {
        id: "cli.board.describe",
        command_path: "board.describe",
    },
    CliCommandDeclaration {
        id: "cli.host.start",
        command_path: "host.start",
    },
    CliCommandDeclaration {
        id: "cli.host.status",
        command_path: "host.status",
    },
    CliCommandDeclaration {
        id: "cli.host.stop",
        command_path: "host.stop",
    },
];

pub(crate) fn interface_inventory_entries() -> Vec<InterfaceEntry> {
    CLI_COMMANDS
        .iter()
        .map(CliCommandDeclaration::interface_entry)
        .collect()
}

impl CliCommandDeclaration {
    fn interface_entry(&self) -> InterfaceEntry {
        InterfaceEntry {
            id: self.id.to_string(),
            surfaces: SurfaceSet::single(Surface::Cli),
            kind: InterfaceKind::CliCommand,
            contract: ContractShape::new(
                ContractLocator::command_path(self.command_path),
                ContractIo::declared(
                    ContractIoKind::CommandArgs,
                    None,
                    None,
                    "CLI command arguments",
                ),
                ContractIo::not_declared(
                    "CLI command output is command-specific; tool calls render canonical ToolResult values by output mode",
                ),
            ),
            version: CLI_SURFACE_VERSION.to_string(),
            compatibility: Compatibility::Stable,
            owner: OwnerRef {
                path: Some(CLI_OWNER_PATH.to_string()),
                url: None,
            },
            docs: DocRef {
                path: Some(CLI_DOCS_PATH.to_string()),
                url: None,
            },
            source: SourceRef {
                path: Some("upeg-cli/src/surfaces/cli/mod.rs".to_string()),
                url: None,
            },
            tests: TestMapping::covered(CLI_TEST_PATH, Some(CLI_TEST_NAME.to_string())),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "upeg",
    version,
    about = "Universal Pegboard — pin once, call anywhere."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Suppress auto-loader stderr summaries (`upeg: loaded N tool(s)...`).
    /// Useful when piping output in shell scripts.
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,
    /// Open or execute with a Board context. With no subcommand, starts the
    /// TUI filtered to this board (`upeg --board dev`). With `call` or a
    /// dynamic `{toolkit} {tool}` command, injects `_upeg` board/project
    /// context into the invocation args.
    #[arg(long)]
    pub board: Option<String>,
    /// Open the TUI with one effective Tag selected. This keeps `upeg`
    /// keyboard-first while still exposing the Board + Tag filter model
    /// without adding a separate compatibility command.
    #[arg(long = "tui-tag")]
    pub tui_tag: Option<String>,
    /// Resolve project manifests and relative tool paths from this directory.
    /// Applied once, before runtime sources are loaded.
    #[arg(long, global = true, value_name = "DIR")]
    pub working_directory: Option<std::path::PathBuf>,
    /// Select a `.upeg` project root without changing process cwd.
    #[arg(long, global = true, value_name = "DIR")]
    pub project: Option<std::path::PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect, validate, initialize, and resolve a `.upeg` project.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Interface inventory generation and drift checks.
    Interface {
        #[command(subcommand)]
        action: InterfaceCommand,
    },
    /// Inspect the Toolbox.
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
    /// Inspect Toolkit groups.
    Toolkit {
        #[command(subcommand)]
        action: ToolkitAction,
    },
    /// Inspect Tag discovery groups.
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Inspect Board tabs.
    Board {
        #[command(subcommand)]
        action: BoardAction,
    },
    /// Inspect metadata-only execution history.
    Log {
        /// Filter to one canonical Tool id.
        #[arg(long)]
        tool: Option<String>,
        /// Filter to one surface (`cli`/`tui`/`desktop`/`pwa`/`ext`/`mcp`/`http`).
        #[arg(long)]
        surface: Option<String>,
        /// Filter to one status (`ok`/`tool_error`/`not_found`).
        #[arg(long)]
        status: Option<String>,
        /// Filter to one trigger label (`source[:condition]`) captured in the dispatch context.
        #[arg(long)]
        trigger: Option<String>,
        /// Show records at or after this point. Accepts epoch milliseconds or
        /// relative windows such as `30s`, `15m`, `1h`, or `7d`.
        #[arg(long)]
        since: Option<String>,
        /// Maximum rows to print from the tail of the log.
        #[arg(long, default_value_t = 100)]
        limit: usize,
        /// Emit JSON instead of tab-separated rows.
        #[arg(long)]
        json: bool,
    },
    /// Inspect, copy, or export retained local failure diagnostics.
    Diagnostics {
        #[command(subcommand)]
        action: DiagnosticAction,
    },
    /// Manage credential references. Values stay in env/OS secret stores.
    Credential {
        #[command(subcommand)]
        action: CredentialAction,
    },
    /// Trigger discovery and manual firing.
    Trigger {
        #[command(subcommand)]
        action: TriggerAction,
    },
    /// Run as an MCP server over stdio (JSON-RPC). Wires every Tool registered
    /// via `#[upeg::tool]` into Claude Code / Cursor / Claude Desktop.
    /// Configure in `claude_desktop_config.json`:
    ///
    ///   { "command": "upeg", "args": ["mcp"] }
    Mcp {
        /// Scope the server to one Board: `tools/list` exposes only the
        /// tools pinned on this board (user pegboard state), and
        /// `tools/call` merges each pin's saved args preset. "board =
        /// server". Omit to expose every MCP-surface tool.
        #[arg(long)]
        board: Option<String>,
    },
    /// Run as an HTTP server.
    ///
    /// With no subcommand, starts a foreground server. With `--daemon`,
    /// detaches and writes to a log file. Use `status` / `stop` /
    /// `restart` / `logs` to manage a running daemon — all four read
    /// `~/.upeg/server.json` for state.
    Http {
        /// Lifecycle subcommand. Omit for default `start` behaviour.
        #[command(subcommand)]
        action: Option<HttpAction>,
        /// Bind address. Default `127.0.0.1:0` (ephemeral loopback).
        #[arg(long)]
        addr: Option<String>,
        /// Detach into a background daemon process.
        #[arg(long)]
        daemon: bool,
        /// Explicit bearer token. Otherwise read from `UPEG_HTTP_TOKEN`
        /// or auto-generated and published to `server.json`.
        #[arg(long, value_name = "TOKEN")]
        token: Option<String>,
        /// Read bearer token from this file (trimmed). Overrides
        /// `--token` if both are given.
        #[arg(long = "token-file", value_name = "PATH")]
        token_file: Option<std::path::PathBuf>,
        /// Override the daemon log file location. Default
        /// `~/.upeg/upeg-http.log`.
        #[arg(long, value_name = "PATH")]
        log_file: Option<std::path::PathBuf>,
        /// Extra web origin allowed to call the REST data plane via
        /// CORS (repeatable, e.g. `--cors-origin https://app.example.com`).
        /// Loopback (`http://127.0.0.1:*`, `http://localhost:*`) and
        /// `chrome-extension://` origins are always allowed; this adds
        /// to that set. No wildcard (`*`).
        #[arg(long = "cors-origin", value_name = "ORIGIN")]
        cors_origin: Vec<String>,
    },
    /// Manage the shared host supervisor process.
    Host {
        #[command(subcommand)]
        action: HostAction,
    },
    /// Print a shell completion script for the named shell. Pipe into
    /// the shell's completion dir, e.g. `upeg completions bash > ~/.bash_completion.d/upeg`.
    #[command(alias = "completion")]
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
    /// WASM plugin operations. Available only when the
    /// `wasm-plugin` cargo feature is enabled at build time.
    #[cfg(feature = "wasm-plugin")]
    Wasm {
        #[command(subcommand)]
        action: WasmAction,
    },
    /// Author, validate/install, and enumerate WASM guest plugins.
    /// `new` is pure scaffolding and always available; `install`/`list`
    /// load plugin manifests through `upeg-wasm` and need the
    /// `wasm-plugin` cargo feature, same as `upeg wasm`.
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Print install diagnostics: binary path/version, enabled features,
    /// status of each runtime source directory, toolbox counts.
    /// `--json` emits a machine-readable JSON object of the same data.
    Doctor {
        /// Emit a JSON object instead of the human-readable form.
        #[arg(long)]
        json: bool,
    },
    /// Enter the interactive TUI explicitly. Equivalent to
    /// running `upeg` with no subcommand on a terminal stdout — same
    /// entry point ([`crate::app::run_no_command`]), so it honours the
    /// global `--board` / `--tui-tag` flags identically. Exists for
    /// shell aliases/wrappers that always pass an explicit subcommand
    /// rather than relying on "no args on a terminal".
    Tui,
    /// Generic tool caller. Sends JSON-formatted args to the named Tool
    /// and prints its primary output value by default. If a host is
    /// already running, `call` automatically attaches to it via the
    /// discovery file so the run shows up in shared state; otherwise it
    /// dispatches in-process.
    ///
    /// Three ways to pass args:
    ///   1. Positional JSON object: `upeg call num.hex_to_decimal '{"input":"0xff"}'`
    ///   2. Repeatable `-a key=value`: `upeg call num.hex_to_decimal -a input=0xff`
    ///   3. Stdin: `cat args.json | upeg call num.hex_to_decimal -`
    // Without this, clap reflows the numbered list into a single
    // paragraph, which buries forms 2 and 3 mid-sentence — the very
    // forms a caller reaching for raw JSON has not found yet.
    #[command(verbatim_doc_comment)]
    Call {
        /// Tool id, e.g. `num.hex_to_decimal`.
        tool_id: String,
        /// JSON args object. Defaults to `{}`. Pass `-` to read from
        /// stdin. Ignored when `-a/--arg` is used.
        #[arg(default_value = "{}")]
        args: String,
        /// Build args from repeatable `key=value` pairs instead of raw JSON.
        #[arg(short = 'a', long = "arg", value_parser = parse_kv_arg)]
        arg: Vec<(String, serde_json::Value)>,
        /// Print the resolved JSON args object instead of dispatching.
        /// Useful for verifying `-a key=value` composition.
        #[arg(long)]
        dry_run: bool,
        /// Emit the canonical success/error JSON envelope.
        #[arg(long, conflicts_with_all = ["field", "pretty"])]
        json: bool,
        /// Print one output field value by id.
        #[arg(long, value_name = "ID", conflicts_with_all = ["json", "pretty"])]
        field: Option<String>,
        /// Print labeled output rows instead of only the primary value.
        #[arg(long, conflicts_with_all = ["json", "field"])]
        pretty: bool,
        /// Dispatch in-process even when a host is running (skip the
        /// discovery-file auto-attach).
        #[arg(long)]
        local: bool,
        /// Destination for a `File` output: a file path, or a directory to
        /// place the tool's named file into. Defaults to the current
        /// directory using the file's own name. Ignored by non-file tools.
        #[arg(long, value_name = "PATH")]
        out: Option<std::path::PathBuf>,
        /// Overwrite the destination when a `File` output would land on an
        /// existing file. Without this, the call refuses rather than clobber.
        #[arg(long)]
        force: bool,
    },
    /// Dynamic `{toolkit} {tool}` route for runtime/project Toolkits.
    /// Supports `--help`, `--json`/`--field <ID>`/`--pretty`, and
    /// `--local` with the same semantics as `call`.
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ProjectChoiceArg {
    Global,
    Project,
}

impl From<ProjectChoiceArg> for upeg_core::ProjectToolChoice {
    fn from(value: ProjectChoiceArg) -> Self {
        match value {
            ProjectChoiceArg::Global => Self::Global,
            ProjectChoiceArg::Project => Self::Project,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    /// Show the active or detected project and its conflicts.
    Show {
        #[arg(long)]
        json: bool,
        root: Option<std::path::PathBuf>,
    },
    /// Validate project.toml and every project Toolkit without running tools.
    Validate {
        #[arg(long)]
        json: bool,
        root: Option<std::path::PathBuf>,
    },
    /// Create a `.upeg` marker and a minimal project.toml.
    Init {
        root: Option<std::path::PathBuf>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Choose the global or project definition for a duplicate Tool id.
    Choose {
        tool_id: String,
        choice: ProjectChoiceArg,
        #[arg(long)]
        root: Option<std::path::PathBuf>,
    },
}

/// Failure-report operations. Reports are redacted and bounded before they
/// reach disk; `export --debug` merely includes the already-safe detail JSON.
#[derive(Subcommand, Debug)]
pub enum DiagnosticAction {
    /// List retained reports, newest first.
    List {
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show one report by id.
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Print a redacted support bundle suitable for copying or saving.
    Export {
        id: String,
        /// Include the redacted structured failure details.
        #[arg(long)]
        debug: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum HttpAction {
    /// Same as `upeg http` with no subcommand. Reads top-level
    /// `--addr`/`--daemon`/`--token`/etc. for its options.
    Start,
    /// Print the running host's endpoint, pid, and uptime.
    Status {
        /// Emit JSON instead of the human-readable block.
        #[arg(long)]
        json: bool,
        /// Also print a pairing block: the endpoint and bearer token as
        /// plain text, for browser/mobile pairing without digging the
        /// port + token out of `server.json`. Local operator only —
        /// this is CLI-side display, never served over HTTP.
        #[arg(long)]
        pairing: bool,
    },
    /// Stop the running host. Sends SIGTERM with 5s grace, then
    /// SIGKILL only with `--force`.
    Stop {
        /// Escalate to SIGKILL after the grace window expires.
        #[arg(long)]
        force: bool,
    },
    /// Stop then start. Inherits the top-level `--daemon` flag etc.
    Restart {
        /// Force-kill the existing host instead of waiting on grace.
        #[arg(long)]
        force: bool,
    },
    /// Tail the daemon log file.
    Logs {
        /// Maximum lines to print from the tail.
        #[arg(long, default_value_t = 100)]
        lines: usize,
    },
}

#[derive(Subcommand, Debug)]
pub enum HostAction {
    /// Start the host supervisor.
    Start {
        /// Detach into a background daemon process.
        #[arg(long)]
        daemon: bool,
        /// Bind address. Default `127.0.0.1:0` (ephemeral loopback).
        #[arg(long)]
        addr: Option<String>,
        /// Override the daemon log file location.
        #[arg(long, value_name = "PATH")]
        log_file: Option<std::path::PathBuf>,
        /// Extra web origin allowed to call the REST data plane via
        /// CORS (repeatable). See `upeg http --cors-origin`.
        #[arg(long = "cors-origin", value_name = "ORIGIN")]
        cors_origin: Vec<String>,
    },
    /// Print host status.
    Status {
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Stop the host supervisor.
    Stop {
        /// Escalate to SIGKILL after the grace window expires.
        #[arg(long)]
        force: bool,
    },
    /// Tail the host daemon log.
    Logs {
        /// Maximum lines to print from the tail.
        #[arg(long, default_value_t = 100)]
        lines: usize,
    },
}

#[cfg(feature = "wasm-plugin")]
#[derive(Subcommand, Debug)]
pub enum WasmAction {
    /// Load a `.wasm` plugin and register every tool it declares.
    /// Prints one id per line. Use as a sanity check that the plugin is
    /// well-formed; for persistent registration drop the file into
    /// `$UPEG_WASM_DIR` (or `~/.upeg/wasm/`) so every `upeg`
    /// invocation auto-loads it. Inside a running `upeg host` session
    /// the registration sticks across host (HTTP) calls.
    Load {
        /// Path to an extism-shaped `.wasm` plugin.
        path: String,
    },
    /// Print a starter `src/lib.rs` for an upeg WASM plugin. Pipe into
    /// a fresh cargo project that depends on `extism-pdk` and targets
    /// `wasm32-unknown-unknown`, then build with
    /// `cargo build --target wasm32-unknown-unknown --release`.
    Template,
}

#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// Scaffold a ready-to-build guest crate at `<dir>/<name>` (default
    /// `<dir>`: the current directory). Refuses to overwrite an
    /// existing non-empty directory. Works without `--features
    /// wasm-plugin` — it only writes files, it never loads a plugin.
    New {
        /// Plugin/Toolkit name. Must be a lowercase Rust-identifier-safe
        /// name (letters, digits, underscore, starting with a letter):
        /// it becomes the Toolkit id, the crate name, and a Rust
        /// function name in the scaffold.
        name: String,
        /// Parent directory to scaffold `<name>/` into. Defaults to the
        /// current directory.
        #[arg(long, value_name = "PATH")]
        dir: Option<std::path::PathBuf>,
        /// Emit `path = "..."` dependencies against a local UPeg
        /// checkout instead of `git` dependencies — for developing the
        /// plugin from inside the UPeg repo itself.
        #[arg(long, value_name = "UPEG_REPO_PATH")]
        local: Option<std::path::PathBuf>,
    },
    /// Validate a `.wasm` file (or a crate directory holding one under
    /// `target/wasm32-unknown-unknown/release/`) and, on success, copy
    /// it into `~/.upeg/wasm/` (or `$UPEG_WASM_DIR`). Validation never
    /// registers the plugin into the live toolbox; only a successful
    /// copy plus the next `upeg` invocation's auto-load does.
    #[cfg(feature = "wasm-plugin")]
    Install {
        /// Path to a `.wasm` file, or a crate directory containing a
        /// built `target/wasm32-unknown-unknown/release/*.wasm`.
        path: std::path::PathBuf,
        /// Overwrite an existing destination file even if its content
        /// differs from the one being installed.
        #[arg(long)]
        force: bool,
    },
    /// List every installed plugin file in `~/.upeg/wasm/` with the
    /// tool ids it declares. An empty or missing directory prints a
    /// friendly empty state rather than failing.
    #[cfg(feature = "wasm-plugin")]
    List,
}

#[derive(Subcommand, Debug)]
pub enum ToolAction {
    /// List every registered Tool.
    ///
    /// Default output: tab-separated `<id>\t<toolkit>\t<pin>` rows
    /// (pipe-friendly). With `--json`, emit one JSON array
    /// matching the `/v1/tools` HTTP shape so machine consumers don't
    /// have to parse tabs.
    List {
        /// Filter to a single effective tag (e.g. `pure`, `convert`, `hash`).
        #[arg(long)]
        tag: Option<String>,
        /// Filter to a single surface (`cli`/`tui`/`desktop`/`pwa`/`ext`/`mcp`/`http`).
        /// Defaults to `cli` (this binary's own surface). Useful for
        /// introspecting what other surfaces would expose.
        #[arg(long)]
        surface: Option<String>,
        /// Filter to tools assigned to a specific board.
        #[arg(long)]
        board: Option<String>,
        /// Filter to a single pin kind (`Inline`/`Launcher`/`Live`/`Action`/`Embed`).
        /// Useful for discovering Embed-shaped tools: `--pin Embed`.
        #[arg(long)]
        pin: Option<String>,
        /// Emit a JSON array instead of tab-separated rows.
        #[arg(long)]
        json: bool,
    },
    /// Show one Tool's manifest. Default output is a key-value block.
    /// `--json` emits a single JSON object matching the `/v1/tools` entry shape.
    Show {
        /// Canonical Tool id (e.g. `num.hex_to_decimal`).
        id: String,
        /// Emit a JSON object instead of the key-value block.
        #[arg(long)]
        json: bool,
    },
    /// Inspect External command prerequisites without executing the tool.
    Check {
        /// Canonical Tool id (e.g. `dev.verify`).
        id: String,
        /// Emit the readiness contract as JSON.
        #[arg(long)]
        json: bool,
        /// Inspect this process rather than an attachable host.
        #[arg(long)]
        local: bool,
    },
    /// Dry-run a Declarative TOML file: parse + validate without
    /// touching the registry. Exit 0 = ok, 1 = invalid.
    Validate {
        /// Path to a `*.toml` file describing one Tool.
        path: String,
        /// Also resolve chain step ids against the registry. Catches
        /// typos and dangling references at validation time. Built-ins
        /// always resolve; TOML/WASM/MCP-managed tools resolve only if
        /// they're already loaded in this process.
        #[arg(long = "resolve-chain")]
        resolve_chain: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ToolkitAction {
    /// List registered Toolkits.
    List {
        /// Emit a JSON array instead of one id per line.
        #[arg(long)]
        json: bool,
    },
    /// Show one Toolkit and its Tools.
    Show {
        /// Toolkit id (e.g. `convert`).
        id: String,
        /// Emit a JSON object instead of the text block.
        #[arg(long)]
        json: bool,
    },
    /// Dry-run every `*.toml` file in a directory: same parse-and-check
    /// routine as `tool validate`, batched. Nothing is registered — this
    /// stays read-only diagnostics like the single-file form. Prints one
    /// ok/error line per file plus a summary count; exits non-zero if any
    /// file fails.
    Validate {
        /// Directory to scan. Defaults to the runtime toolkits dir
        /// (`$UPEG_TOOLKITS_DIR` or `~/.upeg/toolkits`) — the same
        /// directory the auto-loader watches at startup.
        dir: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TagAction {
    /// List effective tags.
    List {
        /// Emit a JSON array instead of one row per tag.
        #[arg(long)]
        json: bool,
    },
    /// Show one tag and its Tools.
    Show {
        /// Tag label (e.g. `pure`, `chain`, `convert`).
        tag: String,
        /// Emit a JSON object instead of the text block.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BoardAction {
    /// List Boards with Tools pinned to them.
    List {
        /// Emit a JSON array instead of one row per board.
        #[arg(long)]
        json: bool,
    },
    /// Show one Board and its Tools.
    Show {
        /// Board key (e.g. `dev`).
        board: String,
        /// Emit a JSON object instead of the text block.
        #[arg(long)]
        json: bool,
    },
    /// Board-scoped route: `upeg board <board> list` shows that board's
    /// pins; `upeg board <board> call <tool> …` calls a pinned tool with
    /// its saved args preset merged as defaults. The first token is the
    /// board key; the rest parses as [`BoardScopedAction`].
    #[command(external_subcommand)]
    Scoped(Vec<String>),
}

/// Parser for the tokens after `upeg board <board>`.
#[derive(Parser, Debug)]
#[command(name = "upeg board <board>", no_binary_name = true)]
pub struct BoardScopedCli {
    #[command(subcommand)]
    pub action: BoardScopedAction,
}

#[derive(Subcommand, Debug)]
pub enum BoardScopedAction {
    /// List the tools pinned on this board (user pegboard state).
    List {
        /// Emit a JSON array instead of tab-separated rows.
        #[arg(long)]
        json: bool,
    },
    /// Call a tool pinned on this board. The pin's saved args preset
    /// merges as defaults; explicit args override preset keys.
    Call {
        /// Tool id, e.g. `num.hex_to_decimal`.
        tool_id: String,
        /// JSON args object. Defaults to `{}`. Pass `-` to read from
        /// stdin. Ignored when `-a/--arg` is used.
        #[arg(default_value = "{}")]
        args: String,
        /// Build args from repeatable `key=value` pairs instead of raw JSON.
        #[arg(short = 'a', long = "arg", value_parser = parse_kv_arg)]
        arg: Vec<(String, serde_json::Value)>,
        /// Emit the canonical success/error JSON envelope.
        #[arg(long, conflicts_with_all = ["field", "pretty"])]
        json: bool,
        /// Print one output field value by id.
        #[arg(long, value_name = "ID", conflicts_with_all = ["json", "pretty"])]
        field: Option<String>,
        /// Print labeled output rows instead of only the primary value.
        #[arg(long, conflicts_with_all = ["json", "field"])]
        pretty: bool,
        /// Dispatch in-process even when a host is running (skip the
        /// discovery-file auto-attach).
        #[arg(long)]
        local: bool,
    },
    /// Pin a tool onto this board. Writes the same shared pegboard store
    /// the Desktop/TUI pin gesture writes, so a CLI pin and a GUI pin are
    /// indistinguishable afterwards.
    Pin {
        /// Tool id, e.g. `num.hex_to_decimal`.
        tool_id: String,
        /// Per-pin size override (`U1` 1×1, `U2` 2×1, `U2T` 1×2).
        /// Omit to keep the tool manifest's own `pegboard_units`.
        #[arg(long, value_name = "UNITS")]
        units: Option<String>,
        /// Target cell as `<row>,<col>`, zero-based. Colliding pins are
        /// pushed forward row-major, exactly like a desktop drag-drop.
        /// Omit to append at the first free cell.
        #[arg(long, value_name = "ROW,COL")]
        at: Option<String>,
        /// Emit a JSON object instead of a one-line summary.
        #[arg(long)]
        json: bool,
    },
    /// Remove a tool's pin from this board.
    Unpin {
        /// Tool id, e.g. `num.hex_to_decimal`.
        tool_id: String,
        /// Emit a JSON object instead of a one-line summary.
        #[arg(long)]
        json: bool,
    },
    /// Move an already-pinned tool to another cell on this board.
    Move {
        /// Tool id, e.g. `num.hex_to_decimal`.
        tool_id: String,
        /// Target cell as `<row>,<col>`, zero-based.
        #[arg(long, value_name = "ROW,COL")]
        at: String,
        /// Emit a JSON object instead of a one-line summary.
        #[arg(long)]
        json: bool,
    },
    /// Show the guidance, pinned tools, defaults, and execution readiness
    /// an agent receives for this board.
    Context {
        /// Emit the complete machine-readable context.
        #[arg(long)]
        json: bool,
    },
    /// Emit a structured MCP stdio connection config for this board.
    Connect,
    /// Update guidance for a personal board. Project-board guidance is
    /// authored in its project manifest instead.
    Describe {
        /// Replace the board's short description.
        #[arg(long, conflicts_with = "clear_description")]
        description: Option<String>,
        /// Remove the board's short description.
        #[arg(long)]
        clear_description: bool,
        /// Replace the board's agent instructions.
        #[arg(long, conflicts_with = "clear_instructions")]
        instructions: Option<String>,
        /// Remove the board's agent instructions.
        #[arg(long)]
        clear_instructions: bool,
        /// Emit the updated board guidance as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum CredentialAction {
    /// Register or replace a credential reference without storing the secret.
    Add {
        /// Logical credential name used by manifests.
        name: String,
        /// Secret schema type (`secret`, `api_key`, `bearer`, ...).
        #[arg(long = "type")]
        value_type: Option<String>,
        /// Reference backend: `env` or `keychain`.
        #[arg(long)]
        store: Option<String>,
        /// Environment variable that contains the secret value.
        #[arg(long)]
        env: Option<String>,
        /// OS keychain service name when `--store keychain`.
        #[arg(long)]
        service: Option<String>,
        /// OS keychain account name when `--store keychain`.
        #[arg(long)]
        account: Option<String>,
        /// Adapter-specific target (for example HTTP header name).
        #[arg(long)]
        target: Option<String>,
    },
    /// List credential references and whether their env var is currently set.
    List {
        /// Emit JSON instead of tab-separated rows.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum TriggerAction {
    /// List manifest-declared trigger bindings.
    List {
        /// Emit JSON instead of tab-separated rows.
        #[arg(long)]
        json: bool,
    },
    /// Run trigger adapters once, or continuously with `--watch`.
    Run {
        /// Keep polling trigger adapters instead of performing one pass.
        #[arg(long)]
        watch: bool,
    },
    /// Fire a trigger by dispatching the matching Tool id with trigger context.
    ///
    /// `_upeg.trigger` is stamped with the Tool's first declared trigger
    /// binding (`source[:condition]`); a Tool with no binding is stamped with
    /// nothing.
    Fire {
        /// Trigger/Tool id — the canonical Tool id.
        id: String,
        /// JSON args object. Defaults to `{}`. Pass `-` to read from stdin.
        #[arg(default_value = "{}")]
        args: String,
        /// Build args from repeatable `key=value` pairs instead of raw JSON.
        #[arg(short = 'a', long = "arg", value_parser = parse_kv_arg)]
        arg: Vec<(String, serde_json::Value)>,
    },
}

#[cfg(test)]
mod tui_command_tests {
    use super::{Cli, Command};
    use clap::Parser as _;

    #[test]
    fn upeg_tui_parses_as_tui_command_variant() {
        let cli = Cli::parse_from(["upeg", "tui"]);
        assert!(
            matches!(cli.command, Some(Command::Tui)),
            "got {:?}",
            cli.command
        );
    }

    #[test]
    fn upeg_tui_accepts_global_board_and_tui_tag_flags() {
        let cli = Cli::parse_from(["upeg", "--board", "dev", "--tui-tag", "pure", "tui"]);
        assert!(matches!(cli.command, Some(Command::Tui)));
        assert_eq!(cli.board.as_deref(), Some("dev"));
        assert_eq!(cli.tui_tag.as_deref(), Some("pure"));
    }
}
