//! Shell completion script generation — CLI-surface specific because
//! it's clap-tied. Pure (no I/O); the caller decides where to write
//! the output.
//!
//! `upeg completions <shell>` (alias `completion`) writes a script for
//! bash/zsh/fish/elvish/powershell to stdout; installation redirects it
//! into the shell's completion directory.
//!
//! The dynamic route has no subcommands clap knows about, so the
//! generator reads the toolbox *at generation time* and bakes candidates
//! into the script: canonical Tool ids for `upeg call <TAB>`, and one
//! synthesized subcommand per Toolkit holding kebab-case Tool names for
//! `upeg <toolkit> <TAB>` (a name colliding with a built-in subcommand
//! is never synthesized — a built-in is never shadowed). The script is
//! a **generation-time snapshot**: Tools installed or changed later do
//! not complete until it is regenerated. Synthesis is generation-only —
//! real parsing still goes through `Cli::parse`, so
//! `external_subcommand` semantics do not change.

use clap::CommandFactory;
use clap_complete::Shell;
use upeg_core::Surface;
use upeg_runtime::{toolkits_for_surface, tools_for_toolkit_on_surface};

use crate::surfaces::cli::Cli;
use crate::surfaces::cli::formatters::kebab_token;

/// Render a shell completion script for `shell`. Backed by
/// `clap_complete::generate`.
pub fn generate_completion(shell: Shell) -> String {
    let mut buf = Vec::<u8>::new();
    let mut cmd = with_dynamic_tool_candidates(Cli::command());
    let bin = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin, &mut buf);
    String::from_utf8(buf).unwrap_or_default()
}

/// Static clap metadata only knows the built-in subcommands; Tool ids
/// live in the runtime registry. Inject them at generation time so
/// `upeg <TAB>` offers toolkits, `upeg num <TAB>` offers tools, and
/// `upeg call <TAB>` offers canonical ids. The mutated `Command` is
/// used ONLY for script generation — real parsing still goes through
/// `Cli::parse`, so the dynamic route's `external_subcommand`
/// semantics are untouched.
fn with_dynamic_tool_candidates(mut cmd: clap::Command) -> clap::Command {
    let tool_ids: Vec<&'static str> = upeg_runtime::toolbox_tools()
        .filter(|t| t.is_on_surface(Surface::Cli))
        .map(|t| t.id)
        .collect();
    if !tool_ids.is_empty() {
        cmd = cmd.mut_subcommand("call", |call| {
            // `mut_arg` re-appends the Arg, which would demote the
            // `tool_id` positional behind `args`; pin both slots
            // explicitly so the generation-only Command stays valid.
            call.mut_arg("tool_id", |arg| {
                arg.index(1)
                    .value_parser(clap::builder::PossibleValuesParser::new(tool_ids))
            })
            .mut_arg("args", |arg| arg.index(2))
        });
    }

    for toolkit in toolkits_for_surface(Surface::Cli) {
        // Never shadow a built-in command (`tool`, `call`, ...): at
        // parse time clap resolves built-ins first, so a same-named
        // completion entry would only mislead.
        if cmd.find_subcommand(toolkit).is_some() {
            continue;
        }
        let mut toolkit_cmd = clap::Command::new(toolkit.to_string());
        for tool in tools_for_toolkit_on_surface(toolkit, Surface::Cli) {
            toolkit_cmd = toolkit_cmd.subcommand(
                clap::Command::new(kebab_token(tool.tool_id())).about(tool.description),
            );
        }
        cmd = cmd.subcommand(toolkit_cmd);
    }
    cmd
}
