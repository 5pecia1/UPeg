//! Dynamic `upeg {toolkit} {tool} <args...>` route (PRD §6.5).
//!
//! clap's `external_subcommand` hands this route a raw argv tail, so
//! everything clap would normally own — `--help`, output-mode flags,
//! `--local` — must be intercepted here BEFORE the kebab→snake token
//! normalization and schema binding. Without the intercept, `upeg num
//! --help` used to normalize `--help` into `__help` and die with
//! `unknown tool num/__help`.
//!
//! Flag semantics mirror `upeg call` (same output modes, same host
//! auto-attach) so the two invocation surfaces stay in parity.

use std::ffi::OsString;

use upeg_runtime::toolbox_tool_in_toolkit;

use super::args;
use super::call_output::{CallOutputMode, run_dispatch_outcome};
use super::{HostAttachPolicy, dispatch_local_or_attached, normalize_cli_tool_token};
use crate::error::CliError;
use crate::surfaces::cli::formatters::{format_tool_show, format_toolkit_help};

/// Option tokens the dynamic route understands. Matched literally —
/// clap never parses past `external_subcommand`, so the route's flag
/// surface is defined by these constants instead of a derive.
const HELP_FLAG_LONG: &str = "--help";
const HELP_FLAG_SHORT: &str = "-h";
const JSON_FLAG: &str = "--json";
const FIELD_FLAG: &str = "--field";
const PRETTY_FLAG: &str = "--pretty";
const LOCAL_FLAG: &str = "--local";
/// Conventional end-of-options marker: every later token stays
/// positional, so literal values like `--json` remain expressible.
const END_OF_OPTIONS: &str = "--";

/// Parsed shape of the argv tail behind `upeg {toolkit} ...`. The first
/// non-flag token is the tool; later non-flag tokens are schema-bound
/// positionals (`-` keeps its stdin-placeholder meaning).
#[derive(Debug, Default, PartialEq, Eq)]
struct DynamicInvocation {
    tool: Option<String>,
    positionals: Vec<String>,
    help: bool,
    json: bool,
    field: Option<String>,
    pretty: bool,
    local: bool,
}

impl DynamicInvocation {
    fn parse(tokens: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        let mut invocation = Self::default();
        let mut options_done = false;
        let mut tokens = tokens.into_iter();
        while let Some(token) = tokens.next() {
            if options_done {
                invocation.push_positional(token);
                continue;
            }
            match token.as_str() {
                END_OF_OPTIONS => options_done = true,
                HELP_FLAG_LONG | HELP_FLAG_SHORT => invocation.help = true,
                JSON_FLAG => invocation.json = true,
                PRETTY_FLAG => invocation.pretty = true,
                LOCAL_FLAG => invocation.local = true,
                FIELD_FLAG => {
                    invocation.field = Some(tokens.next().ok_or_else(|| {
                        CliError::tool_failed(format!(
                            "a value is required for `{FIELD_FLAG} <ID>` but none was supplied"
                        ))
                    })?);
                }
                other if is_unknown_long_flag(other) => {
                    return Err(CliError::tool_failed(format!(
                        "unknown flag `{other}` (supported: {HELP_FLAG_LONG}, {JSON_FLAG}, \
                         {FIELD_FLAG} <ID>, {PRETTY_FLAG}, {LOCAL_FLAG}); use `{END_OF_OPTIONS}` \
                         to pass literal `--` values as positional args"
                    )));
                }
                _ => invocation.push_positional(token),
            }
        }
        Ok(invocation)
    }

    fn push_positional(&mut self, token: String) {
        if self.tool.is_none() {
            self.tool = Some(token);
        } else {
            self.positionals.push(token);
        }
    }

    /// Same mutual exclusion `upeg call` enforces via clap's
    /// `conflicts_with_all`, reproduced manually for the raw tail.
    fn output_mode(&self) -> Result<CallOutputMode, CliError> {
        let picked =
            usize::from(self.json) + usize::from(self.field.is_some()) + usize::from(self.pretty);
        if picked > 1 {
            return Err(CliError::tool_failed(format!(
                "{JSON_FLAG}, {FIELD_FLAG}, and {PRETTY_FLAG} cannot be combined; pick one output mode"
            )));
        }
        Ok(CallOutputMode::from_flags(
            self.json,
            self.field.clone(),
            self.pretty,
        ))
    }

    const fn attach_policy(&self) -> HostAttachPolicy {
        HostAttachPolicy::from_local_flag(self.local)
    }
}

/// A token is rejected as an unknown option only when it uses the
/// long-flag shape. Single-dash tokens stay positional: `-` is the
/// stdin placeholder and values like `-5` are legitimate inputs.
fn is_unknown_long_flag(token: &str) -> bool {
    token.starts_with(END_OF_OPTIONS)
}

pub(super) fn run_dynamic_tool_command(
    argv: Vec<OsString>,
    active_board: Option<&str>,
) -> Result<String, CliError> {
    let mut parts = argv.into_iter().map(|s| s.to_string_lossy().into_owned());
    let toolkit_token = parts.next().unwrap_or_default();
    let invocation = DynamicInvocation::parse(parts)?;
    let toolkit = normalize_cli_tool_token(&toolkit_token);

    // `--help` short-circuits before any registry dispatch: toolkit
    // help without a tool token, schema-based usage (the `tool show`
    // block) with one.
    if invocation.help {
        return match &invocation.tool {
            None => format_toolkit_help(&toolkit),
            Some(tool) => format_tool_show(resolve_tool(&toolkit, tool)?.id),
        };
    }

    let Some(tool_token) = &invocation.tool else {
        return Err(CliError::UnknownTool(toolkit_token));
    };
    let meta = resolve_tool(&toolkit, tool_token)?;
    let output_mode = invocation.output_mode()?;
    let attach_policy = invocation.attach_policy();

    let args = args::schema_bound_args_from_cli(&meta.input_spec, invocation.positionals)?;
    let outcome = dispatch_local_or_attached(
        meta.id,
        args,
        active_board,
        attach_policy,
        output_mode.live_output(),
    )?;
    run_dispatch_outcome(meta.id.to_string(), outcome, output_mode)
}

fn resolve_tool(toolkit: &str, tool_token: &str) -> Result<&'static upeg_core::ToolMeta, CliError> {
    let tool = normalize_cli_tool_token(tool_token);
    toolbox_tool_in_toolkit(toolkit, &tool)
        .ok_or_else(|| CliError::UnknownTool(format!("{toolkit}/{tool}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_tokens(tokens: &[&str]) -> Result<DynamicInvocation, CliError> {
        DynamicInvocation::parse(tokens.iter().map(ToString::to_string))
    }

    #[test]
    fn 첫_비플래그_토큰은_도구이고_나머지는_위치인자다() {
        let inv = parse_tokens(&["hex-to-decimal", "0xff", "extra"]).unwrap();
        assert_eq!(inv.tool.as_deref(), Some("hex-to-decimal"));
        assert_eq!(inv.positionals, vec!["0xff", "extra"]);
        assert!(!inv.help && !inv.json && !inv.pretty && !inv.local);
    }

    #[test]
    fn help_플래그는_긴형과_짧은형_모두_어디서든_인식된다() {
        for tokens in [
            &["--help"][..],
            &["-h"][..],
            &["hex-to-decimal", "--help"][..],
            &["--help", "hex-to-decimal"][..],
        ] {
            let inv = parse_tokens(tokens).unwrap();
            assert!(inv.help, "help must be set for {tokens:?}");
        }
    }

    #[test]
    fn field_플래그는_다음_토큰을_값으로_소비한다() {
        let inv = parse_tokens(&["hex-to-decimal", "--field", "result", "0xff"]).unwrap();
        assert_eq!(inv.field.as_deref(), Some("result"));
        assert_eq!(inv.positionals, vec!["0xff"]);
    }

    #[test]
    fn field_플래그는_값이_없으면_오류다() {
        let err = parse_tokens(&["hex-to-decimal", "--field"]).unwrap_err();
        assert!(err.message().contains("--field"), "got: {}", err.message());
    }

    #[test]
    fn 출력_플래그_조합은_거부된다() {
        let inv = parse_tokens(&["t", "--json", "--pretty"]).unwrap();
        let err = inv.output_mode().unwrap_err();
        assert!(
            err.message().contains("cannot be combined"),
            "got: {}",
            err.message()
        );
    }

    #[test]
    fn 이중대시_이후_토큰은_플래그처럼_보여도_위치인자다() {
        let inv = parse_tokens(&["echo", "--", "--json", "--help"]).unwrap();
        assert_eq!(inv.positionals, vec!["--json", "--help"]);
        assert!(!inv.json && !inv.help);
    }

    #[test]
    fn 알수없는_긴_플래그는_오류이고_단일대시_토큰은_위치인자다() {
        let err = parse_tokens(&["echo", "--verbose"]).unwrap_err();
        assert!(
            err.message().contains("--verbose"),
            "got: {}",
            err.message()
        );

        let inv = parse_tokens(&["echo", "-5", "-"]).unwrap();
        assert_eq!(inv.positionals, vec!["-5", "-"]);
    }

    #[test]
    fn local_플래그는_attach_정책을_local_only로_바꾼다() {
        let auto = parse_tokens(&["echo", "hi"]).unwrap();
        assert_eq!(auto.attach_policy(), HostAttachPolicy::Auto);

        let local = parse_tokens(&["echo", "hi", "--local"]).unwrap();
        assert_eq!(local.attach_policy(), HostAttachPolicy::LocalOnly);
    }
}
