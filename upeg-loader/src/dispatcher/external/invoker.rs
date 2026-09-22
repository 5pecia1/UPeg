//! Builds the runtime dispatcher closure for an `invoker = "External"`
//! tool.
//!
//! Everything a manifest can say about *how* the child runs is resolved
//! once, when the dispatcher is built, into an [`ExternalInvocation`];
//! each dispatch then only has to render the argument templates and
//! pick a working directory. Failures are typed all the way out (see
//! [`ExternalFailure`]) so the canonical envelope carries the exit code
//! and both captured streams instead of one lossy string.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use upeg_core::{
    EXECUTION_CONTEXT_ARG, EXECUTION_CONTEXT_CWD, FieldConstraints, InputFieldSpec, ToolResult,
};
use upeg_runtime::{
    DispatchArgs, INVALID_ARGS_CODE, ProgressReporter, active_cancellation, tool_failure,
    tool_failure_with_details,
};

use super::capture::RunControls;
use super::color::ColorPolicy;
use super::error::ExternalProcessError;
use super::outcome::{ExternalExit, ExternalTermination};
use super::pty::TerminalMode;
use super::template::{ArgTemplate, Substitution};
use super::{CaptureCompletion, run_command};
use crate::ToolToml;
use crate::dispatcher::chain::OutputAdapter;
use crate::dispatcher::credentials;
use crate::dispatcher::{BOARD_ENV, CANCELLED_ERROR_CODE, PROJECT_MANIFEST_ENV, TOOL_ERROR_CODE};
use crate::execution_requirements::external_execution_requirements;
use crate::manifest_origin::ManifestOrigin;
use crate::model::{CredentialRefToml, KeyValueToml, format_number_default};

/// Execution-context keys the invoker reads out of the `_upeg`
/// envelope. Named here so no dispatch path spells them inline.
const CONTEXT_BOARD_KEY: &str = "board";
const CONTEXT_PROJECT_MANIFEST_KEY: &str = "projectManifest";
const CONTEXT_BOARD_ENV_KEY: &str = "boardEnv";

/// Everything an External tool declares about running its command,
/// resolved once at load time.
struct ExternalInvocation {
    command: String,
    template: Vec<ArgTemplate>,
    /// Declared inputs, keyed by name — everything argument rendering
    /// needs to know about a placeholder that has no caller value.
    inputs: BTreeMap<String, DeclaredInput>,
    /// Plain `env = [...]` pairs. Applied before credentials, which
    /// therefore win on a name collision.
    env: Vec<(String, String)>,
    /// `color = "…"`. Applied *before* `env`, so a declared variable
    /// overrides what the policy would have set.
    color: ColorPolicy,
    /// `pty = …`. Decides whether the child gets two pipes or one
    /// pseudoterminal — see [`super::pty`].
    terminal: TerminalMode,
    credentials: Vec<CredentialRefToml>,
    working_directory: WorkingDirectoryPolicy,
    timeout: Option<Duration>,
    setup: Option<upeg_runtime::execution_requirements::ToolSetupMetadata>,
}

/// What one declared `[[tools.inputs]]` entry contributes to argument
/// rendering when the caller supplies no value.
struct DeclaredInput {
    /// The declared `default`, pre-rendered as substitution text.
    default: Option<String>,
    /// `required = true`. An optional input is the only one whose lone
    /// `{key}` token may be dropped instead of rendered empty.
    required: bool,
}

/// Where the child runs, and in what order the candidates are tried.
struct WorkingDirectoryPolicy {
    /// `cwd = "…"`, already made absolute against the manifest's own
    /// directory at load time.
    declared: Option<PathBuf>,
    /// A Project Manifest's directory: both the fallback working
    /// directory and the boundary a caller-supplied `_upeg.cwd` may not
    /// escape. `None` for toolkit-directory manifests, whose tools keep
    /// inheriting the process cwd and let the caller point anywhere.
    project_root: Option<PathBuf>,
}

/// Every way one External invocation can fail.
enum ExternalFailure {
    /// The child ran (or was terminated) and brought diagnostics with it.
    Process(ExternalExit),
    /// The caller's own arguments were unusable — today only a bad
    /// `_upeg.cwd`.
    InvalidArgs(String),
    /// Setup or capture broke before the child could report anything.
    Setup(String),
    MissingExecutable {
        readiness: Box<upeg_runtime::readiness::ToolReadiness>,
        spawn_error: String,
    },
}

impl ExternalFailure {
    fn into_tool_result(self) -> ToolResult {
        match self {
            // A cancelled run gets its own code so a consumer can tell
            // "I hung up / I asked it to stop" apart from "the command
            // failed", which are different events with the same shape.
            Self::Process(exit) => {
                let code = if exit.termination() == ExternalTermination::Cancelled {
                    CANCELLED_ERROR_CODE
                } else {
                    TOOL_ERROR_CODE
                };
                tool_failure_with_details(code, exit.message(), exit.details())
            }
            Self::InvalidArgs(message) => tool_failure(INVALID_ARGS_CODE, message),
            Self::Setup(message) => tool_failure(TOOL_ERROR_CODE, message),
            Self::MissingExecutable {
                readiness,
                spawn_error,
            } => tool_failure_with_details(
                TOOL_ERROR_CODE,
                missing_executable_message(&readiness, &spawn_error),
                json!({
                    "readiness": readiness,
                    "spawn_error": spawn_error,
                }),
            ),
        }
    }
}

fn missing_executable_message(
    readiness: &upeg_runtime::readiness::ToolReadiness,
    spawn_error: &str,
) -> String {
    let command = readiness.command.as_deref().unwrap_or("external command");
    let guidance = readiness
        .setup
        .as_ref()
        .and_then(|setup| setup.instructions.as_deref())
        .or_else(|| {
            readiness
                .setup
                .as_ref()
                .and_then(|setup| setup.guide_url.as_deref())
        });
    match guidance {
        Some(guidance) => {
            format!("external command `{command}` could not be started: {spawn_error}. {guidance}")
        }
        None => format!("external command `{command}` could not be started: {spawn_error}"),
    }
}

fn is_path_name(name: &OsStr) -> bool {
    if cfg!(windows) {
        name.to_string_lossy().eq_ignore_ascii_case("PATH")
    } else {
        name == OsStr::new("PATH")
    }
}

/// Build a runtime dispatcher closure for an `Invoker::External` Tool.
/// Returns `None` when the toml didn't declare External + a command.
///
/// `origin` is the manifest file this tool came from. It anchors a
/// relative `cwd` declaration and, for a Project Manifest, supplies the
/// implicit working directory — see [`ManifestOrigin`].
///
/// Argument substitution rules live in [`super::template`]; value
/// resolution for one placeholder is:
///
///   - explicit `Value::String(s)` → `s` verbatim;
///   - explicit non-null, non-string JSON → its `serde_json` string
///     form (`5`, `true`, `[1,2]`), so every arg is a string the way a
///     shell would see it;
///   - missing key or explicit `Value::Null` → the input's declared
///     `default`, or nothing at all (which drops the token only when it
///     is a lone `{key}` for an optional input — see [`super::template`]).
pub(crate) fn external_dispatcher_for(
    parsed: &ToolToml,
    origin: Option<&ManifestOrigin>,
) -> Option<impl for<'a> Fn(DispatchArgs<'a>) -> ToolResult + Send + Sync + 'static> {
    // Iter 242: trim invoker before the equality check. Pre-iter-242
    // `invoker = "External "` (trailing space) skipped this match
    // → no dispatcher built → tool registered with meta only →
    // "dispatch not implemented" at runtime. validate_toml's iter-242
    // check now also catches the missing-command case at load time, so
    // by the time we get here the External invoker definitely has a
    // non-empty command — but we still trim it so a `command = "git "`
    // doesn't try to spawn an executable literally named `git `.
    let requirements = external_execution_requirements(parsed, origin)?;
    let setup = requirements.setup;
    let command = requirements.command?;

    let invocation = ExternalInvocation {
        command,
        template: parsed
            .args_template
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|token| ArgTemplate::parse(token))
            .collect(),
        inputs: declared_inputs(parsed),
        env: declared_env(parsed.env.as_deref()),
        color: color_policy(parsed),
        terminal: TerminalMode::from_declaration(pty_declaration(parsed)),
        credentials: parsed.credentials.clone().unwrap_or_default(),
        working_directory: WorkingDirectoryPolicy {
            declared: requirements.declared_working_directory,
            project_root: requirements.project_root,
        },
        timeout: parsed.timeout_ms.map(Duration::from_millis),
        setup,
    };
    let output_adapter = OutputAdapter::from_tool(parsed);

    Some(move |args: DispatchArgs<'_>| -> ToolResult {
        match invocation.run(args) {
            Ok(captured) => {
                output_adapter.text_result_with_stderr(captured.stdout, &captured.stderr)
            }
            Err(failure) => failure.into_tool_result(),
        }
    })
}

/// One successful invocation's decoded streams.
struct SuccessfulRun {
    stdout: String,
    stderr: String,
}

impl ExternalInvocation {
    fn run(&self, args: DispatchArgs<'_>) -> Result<SuccessfulRun, ExternalFailure> {
        let mut process = Command::new(&self.command);
        process.args(self.rendered_args(args));
        if let Some(directory) = self.working_directory.resolve(&args)? {
            process.current_dir(directory);
        }
        self.color.apply(&mut process);
        for (name, value) in &self.env {
            process.env(name, value);
        }
        apply_execution_context(&mut process, &args);
        for (target, value) in
            credentials::resolve_credentials(&self.credentials).map_err(ExternalFailure::Setup)?
        {
            process.env(target, value);
        }

        // Both ambient capabilities are captured on the dispatching
        // thread: each lives in a thread-local installed by whichever
        // surface wanted it, and the capture layer's own threads cannot
        // see them. `None` — nobody asked — costs the child nothing.
        let controls = RunControls::new(
            ProgressReporter::capture(),
            active_cancellation(),
            self.terminal,
        );
        let output = run_command(&mut process, self.timeout, controls)
            .map_err(|error| self.setup_failure(error, &process))?;
        let succeeded = matches!(
            output.completion,
            CaptureCompletion::Exited(status) if status.success()
        );
        if !succeeded {
            return Err(ExternalFailure::Process(ExternalExit::from_capture(
                &self.command,
                &output,
            )));
        }
        Ok(SuccessfulRun {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn setup_failure(&self, error: ExternalProcessError, process: &Command) -> ExternalFailure {
        if error.is_spawn_not_found() && self.command_is_not_on_process_path(process) {
            let working_directory = process
                .get_current_dir()
                .map(PathBuf::from)
                .or_else(|| std::env::current_dir().ok());
            let Some(working_directory) = working_directory.filter(|path| path.is_dir()) else {
                return ExternalFailure::Setup(format!("`{}`: {error}", self.command));
            };
            return ExternalFailure::MissingExecutable {
                readiness: Box::new(upeg_runtime::readiness::ToolReadiness {
                    status: upeg_runtime::readiness::ToolReadinessStatus::MissingExecutable,
                    platform: upeg_runtime::readiness::current_tool_platform(),
                    command: Some(self.command.clone()),
                    working_directory: Some(working_directory),
                    executable: None,
                    setup: self.setup.as_ref().map(|setup| {
                        setup.selected(upeg_runtime::readiness::current_tool_platform())
                    }),
                }),
                spawn_error: error.to_string(),
            };
        }
        ExternalFailure::Setup(format!("`{}`: {error}", self.command))
    }

    fn command_is_not_on_process_path(&self, process: &Command) -> bool {
        let directory = process
            .get_current_dir()
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok());
        let Some(directory) = directory else {
            return false;
        };
        let inherited_path = std::env::var_os("PATH");
        let mut child_path = None;
        for (name, value) in process.get_envs() {
            if is_path_name(name) {
                child_path = Some(value);
            }
        }
        upeg_runtime::readiness::find_executable(
            &self.command,
            &directory,
            child_path.flatten(),
            inherited_path.as_deref(),
        )
        .is_none()
    }

    fn rendered_args(&self, args: DispatchArgs<'_>) -> Vec<String> {
        let resolve = |key: &str| -> Substitution {
            match args.get(key) {
                Some(Value::String(text)) => Substitution::Text(text.clone()),
                None | Some(Value::Null) => self.absent_substitution(key),
                Some(other) => Substitution::Text(other.to_string()),
            }
        };
        self.template
            .iter()
            .filter_map(|token| token.render(&resolve))
            .collect()
    }

    /// Substitution for a placeholder the caller left out.
    ///
    /// Load-time validation rejects a template naming an undeclared
    /// input, so the unknown-key arm is only reachable for manifests
    /// assembled in memory without the loader; treating those as
    /// optional preserves the pre-existing drop behaviour for them.
    fn absent_substitution(&self, key: &str) -> Substitution {
        let Some(input) = self.inputs.get(key) else {
            return Substitution::AbsentOptional;
        };
        match &input.default {
            Some(text) => Substitution::Text(text.clone()),
            None if input.required => Substitution::AbsentRequired,
            None => Substitution::AbsentOptional,
        }
    }
}

/// Board key, project manifest path, and board env from the `_upeg`
/// envelope. These stay separate from the declared `env` list: they are
/// surface-supplied context, not manifest content.
fn apply_execution_context(process: &mut Command, args: &DispatchArgs<'_>) {
    let Some(context) = args.get(EXECUTION_CONTEXT_ARG) else {
        return;
    };
    if let Some(board) = context.get(CONTEXT_BOARD_KEY).and_then(Value::as_str) {
        process.env(BOARD_ENV, board);
    }
    if let Some(manifest) = context
        .get(CONTEXT_PROJECT_MANIFEST_KEY)
        .and_then(Value::as_str)
    {
        process.env(PROJECT_MANIFEST_ENV, manifest);
    }
    if let Some(env) = context
        .get(CONTEXT_BOARD_ENV_KEY)
        .and_then(Value::as_object)
    {
        for (key, value) in env {
            if let Some(value) = value.as_str() {
                process.env(key, value);
            }
        }
    }
}

impl WorkingDirectoryPolicy {
    /// Pick the working directory for one invocation.
    ///
    /// A declared `cwd` always wins. After that the two manifest
    /// origins deliberately differ:
    ///
    ///   * a toolkit-directory tool has no root of its own, so the
    ///     caller's `_upeg.cwd` wins and otherwise the child inherits
    ///     the process working directory (`None`);
    ///   * a Project Manifest tool belongs to its project. The manifest
    ///     directory is the root *and* the boundary: a caller `cwd`
    ///     inside the project is honored (running `cargo check` from a
    ///     workspace member is legitimate), one pointing outside it is
    ///     ignored in favour of the manifest directory rather than
    ///     silently relocating the project's own tool.
    ///
    /// Ignoring — not rejecting — an outside `cwd` is the deliberate
    /// choice: surfaces send the user's shell directory as ambient
    /// context, so `invalid_args` would turn "you happened to be in
    /// `/tmp`" into a hard failure. Structurally unusable values
    /// (relative, nonexistent) stay `invalid_args`; see
    /// [`caller_working_directory`].
    fn resolve(&self, args: &DispatchArgs<'_>) -> Result<Option<PathBuf>, ExternalFailure> {
        if let Some(declared) = &self.declared {
            if !declared.is_dir() {
                return Err(ExternalFailure::Setup(format!(
                    "declared cwd `{}` is not an existing directory",
                    declared.display()
                )));
            }
            return Ok(Some(declared.clone()));
        }
        let caller = caller_working_directory(args)?;
        let Some(root) = self.project_root.as_ref() else {
            return Ok(caller);
        };
        Ok(Some(
            caller
                .filter(|candidate| is_within(candidate, root))
                .unwrap_or_else(|| root.clone()),
        ))
    }
}

/// Whether `candidate` is `root` itself or a directory under it.
///
/// Both sides are canonicalized so `..` segments and symlinked project
/// roots cannot dress an outside directory up as an inside one. A path
/// that fails to canonicalize is treated as outside: the containment
/// question is unanswerable, and the manifest root is the safe answer.
fn is_within(candidate: &Path, root: &Path) -> bool {
    let (Ok(candidate), Ok(root)) = (candidate.canonicalize(), root.canonicalize()) else {
        return false;
    };
    candidate.starts_with(root)
}

/// Read and validate `_upeg.cwd`.
///
/// The caller is another process (a shell, the daemon, an MCP client),
/// so a relative path here would be relative to *its* notion of "here",
/// which upeg cannot reconstruct — hence absolute-only, and rejected as
/// `invalid_args` rather than silently ignored. Whether an accepted
/// value is actually *used* is [`WorkingDirectoryPolicy::resolve`]'s
/// call: a Project Manifest tool ignores one pointing outside its
/// project.
fn caller_working_directory(args: &DispatchArgs<'_>) -> Result<Option<PathBuf>, ExternalFailure> {
    let Some(value) = args
        .get(EXECUTION_CONTEXT_ARG)
        .and_then(|context| context.get(EXECUTION_CONTEXT_CWD))
    else {
        return Ok(None);
    };
    let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return Err(ExternalFailure::InvalidArgs(format!(
            "`{EXECUTION_CONTEXT_ARG}.{EXECUTION_CONTEXT_CWD}` must be a non-empty string"
        )));
    };
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(ExternalFailure::InvalidArgs(format!(
            "`{EXECUTION_CONTEXT_ARG}.{EXECUTION_CONTEXT_CWD}` must be an absolute path, got `{text}`"
        )));
    }
    if !path.is_dir() {
        return Err(ExternalFailure::InvalidArgs(format!(
            "`{EXECUTION_CONTEXT_ARG}.{EXECUTION_CONTEXT_CWD}` `{text}` is not an existing directory"
        )));
    }
    Ok(Some(path.to_path_buf()))
}

/// Index the declared inputs, pre-rendering each `default` as
/// substitution text.
///
/// Reading them back out of the lowered [`InputFieldSpec`] keeps one
/// source of truth: whatever reached the JSON Schema and the GUI forms
/// is exactly what the command line sees.
fn declared_inputs(parsed: &ToolToml) -> BTreeMap<String, DeclaredInput> {
    parsed
        .inputs
        .iter()
        .filter_map(|field| {
            let spec = InputFieldSpec::try_from(field.clone()).ok()?;
            Some((
                spec.name.as_str().to_string(),
                DeclaredInput {
                    default: constraint_default_text(&spec.constraints),
                    required: spec.required,
                },
            ))
        })
        .collect()
}

fn constraint_default_text(constraints: &FieldConstraints) -> Option<String> {
    if let Some(number) = constraints
        .number
        .as_ref()
        .and_then(|number| number.default)
    {
        return Some(format_number_default(number));
    }
    constraints
        .string
        .as_ref()
        .and_then(|string| string.default.clone())
}

/// Whether this tool asked for a real terminal. Absent is `false`, and
/// a declaration the host cannot honour never reaches here — the loader
/// rejects `pty = true` on a non-Unix host.
fn pty_declaration(parsed: &ToolToml) -> bool {
    parsed.pty.unwrap_or_default()
}

/// What the child is told about color support.
///
/// `pty = true` implies `color = "force"`: a manifest that went to the
/// trouble of asking for a real terminal wants color out of the programs
/// that read the *environment* too, not only out of the ones that call
/// `isatty`.
///
/// It is an implication, not an override. A tool that spells `color` out
/// wins — the same rule that lets a declared `env` entry beat the policy
/// it would otherwise have set, so "what I wrote is what happens" holds
/// for every one of these fields.
///
/// An unparseable value is rejected at load time
/// (`validate_external_process_fields`), so by the time a dispatcher is
/// built the declaration is known-good and the fallback below is
/// unreachable in practice.
fn color_policy(parsed: &ToolToml) -> ColorPolicy {
    match parsed.color.as_deref() {
        Some(declared) => ColorPolicy::parse_declaration(Some(declared)).unwrap_or_default(),
        None if pty_declaration(parsed) => ColorPolicy::Force,
        None => ColorPolicy::Inherit,
    }
}

fn declared_env(env: Option<&[KeyValueToml]>) -> Vec<(String, String)> {
    env.unwrap_or_default()
        .iter()
        .map(|KeyValueToml { name, value }| (name.trim().to_string(), value.clone()))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}
