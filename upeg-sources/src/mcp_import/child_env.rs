//! Self-import recursion guard (E-6).
//!
//! `upeg` can import its own `mcp` Surface as an upstream MCP server
//! (`examples/mcp-imports/local.toml`: `command = "upeg", args =
//! ["mcp"]`). The spawned child is itself a full `upeg mcp` process, and
//! — before this guard existed — its own startup path eagerly loaded
//! `~/.upeg/mcp-imports` again, which spawned a grandchild `upeg mcp`,
//! which spawned a great-grandchild, and so on. Each level's
//! `initialize` request timed out waiting for a child that was itself
//! busy recursing, and the whole tree only stopped once it exhausted an
//! OS-level limit (open files / processes) — at which point every
//! inherited-stderr write from every still-running level could race a
//! torn-down pipe.
//!
//! The fix stamps every spawned upstream subprocess with a marker env
//! var. Long-lived server entry points (`upeg host start`, the
//! desktop-embedded host, in-process `upeg mcp`) check the marker
//! before eagerly loading imports, so a self-imported `upeg mcp` loads
//! imports exactly once — at the ORIGINAL host that spawned it — never
//! recursively at every spawned copy.

use tokio::process::Command;

/// Env var name. `pub` — documented in this module's docs and
/// read (indirectly, via [`is_mcp_import_child`]) by `upeg-cli`'s
/// long-lived host entry points.
pub const MCP_IMPORT_CHILD_ENV: &str = "UPEG_MCP_IMPORT_CHILD";

/// Marker value. Presence of the var is what [`is_mcp_import_child`]
/// checks — the value itself is never inspected — but a typed const
/// still keeps the one write site ([`stamp`]) honest instead of an ad
/// hoc `"1"` literal.
const MCP_IMPORT_CHILD_ENV_VALUE: &str = "1";

/// Stamp `cmd` as an MCP-import upstream child. Called at the single
/// spawn site (`spawn_child`) so every upstream subprocess — including
/// a self-imported `upeg mcp` — carries the marker.
pub(super) fn stamp(cmd: &mut Command) -> &mut Command {
    cmd.env(MCP_IMPORT_CHILD_ENV, MCP_IMPORT_CHILD_ENV_VALUE)
}

/// `true` when THIS process was itself spawned as somebody's
/// MCP-import upstream. Long-lived server entry points check this
/// before eagerly loading `~/.upeg/mcp-imports`, so the CLI never has
/// to hand-roll a `std::env::var` string compare.
pub fn is_mcp_import_child() -> bool {
    is_mcp_import_child_from(std::env::var_os(MCP_IMPORT_CHILD_ENV).as_deref())
}

/// Pure helper for [`is_mcp_import_child`]. Lets tests drive the
/// detection without mutating process env — process env is global and
/// shared across every test binary in the crate, so exercising the
/// public function directly would be flaky under parallel test
/// execution (same lens as `upeg-cli`'s `daemonize::is_detached_from`).
const fn is_mcp_import_child_from(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_true_when_marker_env_var_is_set() {
        assert!(is_mcp_import_child_from(Some(std::ffi::OsStr::new(
            MCP_IMPORT_CHILD_ENV_VALUE
        ))));
        // The value itself is not inspected — only presence matters
        // (an empty string is still Some).
        assert!(is_mcp_import_child_from(Some(std::ffi::OsStr::new(""))));
    }

    #[test]
    fn returns_false_when_marker_env_var_is_unset() {
        assert!(!is_mcp_import_child_from(None));
    }
}
