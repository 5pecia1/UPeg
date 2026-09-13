//! Interface inventory — contract declarations and the `upeg interface`
//! subcommand.
//!
//! Every public surface (CLI command, HTTP route, MCP method, TUI
//! view, desktop/PWA/extension entry point) is mirrored into a
//! deterministic JSON inventory that gates drift in CI. This module
//! both **declares** the entries (CLI surface declarations live next
//! to the CLI module; HTTP/MCP declarations live next to their
//! surfaces; the surfaceless desktop/PWA/extension entries live in
//! [`desktop_pwa_ext`]) and **runs** the generate/check/comment
//! command (in [`command`]).

pub mod command;
pub(crate) mod desktop_pwa_ext;

#[cfg(test)]
mod tests;
