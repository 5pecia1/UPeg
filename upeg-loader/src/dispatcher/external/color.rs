//! What an External child is told about color support.
//!
//! upeg captures both child streams through pipes, so the child sees a
//! non-TTY stdout and every well-behaved CLI turns its color (and often
//! its whole human-oriented rendering) off. That default is deliberate:
//! captured output stays deterministic and free of escape sequences.
//!
//! `color = "force"` is the per-tool opt-out. It is *pure environment*:
//! the conventional `CLICOLOR_FORCE` / `FORCE_COLOR` variables that the
//! ecosystem agreed on, the removal of an inherited `NO_COLOR` (which
//! the same convention says outranks them), plus a `TERM` fallback for
//! programs that consult terminfo before deciding. No pty, no
//! platform-specific code — it behaves identically on Unix and Windows.
//!
//! What it cannot do: a program that decides on `isatty(3)` alone —
//! `git`, `ls`, `grep` — never reads these variables, and needs its own
//! flag (`git -c color.ui=always`, `ls --color=always`). `force` is the
//! ecosystem convention, not a pty.

use std::process::Command;
use std::str::FromStr;

/// TOML spelling of [`ColorPolicy::Inherit`].
pub(crate) const COLOR_INHERIT: &str = "inherit";
/// TOML spelling of [`ColorPolicy::Force`].
pub(crate) const COLOR_FORCE: &str = "force";

/// Every accepted `color` value, in declaration order — the one list
/// the parser's error message and the docs both read.
pub(crate) const COLOR_POLICIES: &[&str] = &[COLOR_INHERIT, COLOR_FORCE];

/// `CLICOLOR_FORCE` (the `clicolors` convention: any non-empty value
/// other than `0` means "emit color even when not a TTY").
pub(super) const CLICOLOR_FORCE_ENV: &str = "CLICOLOR_FORCE";
/// `FORCE_COLOR` (the Node/`chalk` convention, also honored by many Go
/// and Rust CLIs).
pub(super) const FORCE_COLOR_ENV: &str = "FORCE_COLOR";
/// Value both of the above are set to.
pub(super) const COLOR_ENABLED_VALUE: &str = "1";
/// `NO_COLOR` (no-color.org): *any* value, empty included, means "do not
/// emit color", and the convention ranks it above the two force
/// variables. Inheriting it would make `color = "force"` a no-op for
/// every well-behaved CLI, so `force` unsets it for the child.
pub(super) const NO_COLOR_ENV: &str = "NO_COLOR";
/// `TERM` — consulted by terminfo-based programs before they emit any
/// escape sequence at all.
const TERM_ENV: &str = "TERM";
/// `TERM` value used only when the surface running upeg has none (a
/// daemon, a GUI-launched host). A capable, universally present entry.
const DEFAULT_TERM_VALUE: &str = "xterm-256color";

/// Per-tool color declaration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ColorPolicy {
    /// Inherit upeg's own environment. The child sees a pipe and turns
    /// color off by itself.
    #[default]
    Inherit,
    /// Tell the child color is supported.
    Force,
}

/// A `color = "…"` value that is not one of [`COLOR_POLICIES`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown color policy `{value}`")]
pub(crate) struct UnknownColorPolicy {
    pub(crate) value: String,
}

impl FromStr for ColorPolicy {
    type Err = UnknownColorPolicy;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            COLOR_INHERIT => Ok(Self::Inherit),
            COLOR_FORCE => Ok(Self::Force),
            other => Err(UnknownColorPolicy {
                value: other.to_string(),
            }),
        }
    }
}

impl ColorPolicy {
    /// Read a manifest's declaration. An absent field is
    /// [`Self::Inherit`], so nothing changes for the tools that never
    /// opt in.
    pub(crate) fn parse_declaration(declared: Option<&str>) -> Result<Self, UnknownColorPolicy> {
        declared.map_or(Ok(Self::Inherit), str::parse)
    }

    /// Apply the policy to a command that has not had its declared
    /// `env` applied yet.
    ///
    /// Ordering is the point: these are *defaults*, so a tool that also
    /// declares `env = [{ name = "FORCE_COLOR", value = "0" }]` — or
    /// re-declares `NO_COLOR` — wins. `TERM` is only supplied when the
    /// surface running upeg has none — a real terminal's `TERM` is more
    /// accurate than any guess.
    pub(crate) fn apply(self, command: &mut Command) {
        if self == Self::Inherit {
            return;
        }
        command.env(CLICOLOR_FORCE_ENV, COLOR_ENABLED_VALUE);
        command.env(FORCE_COLOR_ENV, COLOR_ENABLED_VALUE);
        command.env_remove(NO_COLOR_ENV);
        if std::env::var_os(TERM_ENV).is_none() {
            command.env(TERM_ENV, DEFAULT_TERM_VALUE);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::process::Command;

    use super::{
        CLICOLOR_FORCE_ENV, COLOR_ENABLED_VALUE, COLOR_POLICIES, ColorPolicy, FORCE_COLOR_ENV,
        NO_COLOR_ENV, UnknownColorPolicy,
    };

    /// What `apply` left in the command's environment plan for one
    /// variable. Three outcomes, so a bare `Option` cannot say them.
    #[derive(Debug, PartialEq, Eq)]
    enum EnvDirective {
        /// The policy never mentioned it — the child inherits.
        Untouched,
        Set(String),
        /// Explicitly unset for the child, whatever the parent has.
        Removed,
    }

    fn 환경_지시(command: &Command, name: &str) -> EnvDirective {
        command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(name))
            .map_or(EnvDirective::Untouched, |(_, value)| match value {
                Some(value) => EnvDirective::Set(value.to_string_lossy().into_owned()),
                None => EnvDirective::Removed,
            })
    }

    #[test]
    fn 선언이_없으면_상속이다() {
        assert_eq!(
            ColorPolicy::parse_declaration(None),
            Ok(ColorPolicy::Inherit)
        );
    }

    #[test]
    fn force_선언은_강제_정책이_된다() {
        assert_eq!(
            ColorPolicy::parse_declaration(Some("force")),
            Ok(ColorPolicy::Force)
        );
    }

    #[test]
    fn 앞뒤_공백은_다듬어진다() {
        assert_eq!(
            ColorPolicy::parse_declaration(Some("  force ")),
            Ok(ColorPolicy::Force)
        );
    }

    #[test]
    fn 알_수_없는_값은_타입_오류가_된다() {
        assert_eq!(
            ColorPolicy::parse_declaration(Some("always")),
            Err(UnknownColorPolicy {
                value: "always".to_string(),
            })
        );
    }

    #[test]
    fn 허용값_목록은_두_가지다() {
        assert_eq!(COLOR_POLICIES, &["inherit", "force"]);
    }

    #[test]
    fn force는_상속된_no_color를_자식에게서_지운다() {
        // no-color.org ranks NO_COLOR above CLICOLOR_FORCE/FORCE_COLOR,
        // so leaving an inherited one in place would make `force` a
        // no-op for every CLI that honors the convention.
        let mut command = Command::new("true");
        ColorPolicy::Force.apply(&mut command);

        assert_eq!(
            환경_지시(&command, NO_COLOR_ENV),
            EnvDirective::Removed,
            "force는 NO_COLOR를 제거 지시로 남겨야 한다"
        );
        assert_eq!(
            환경_지시(&command, CLICOLOR_FORCE_ENV),
            EnvDirective::Set(COLOR_ENABLED_VALUE.to_string())
        );
        assert_eq!(
            환경_지시(&command, FORCE_COLOR_ENV),
            EnvDirective::Set(COLOR_ENABLED_VALUE.to_string())
        );
    }

    #[test]
    fn inherit은_no_color에_손대지_않는다() {
        let mut command = Command::new("true");
        ColorPolicy::Inherit.apply(&mut command);

        assert_eq!(환경_지시(&command, NO_COLOR_ENV), EnvDirective::Untouched);
        assert_eq!(
            환경_지시(&command, CLICOLOR_FORCE_ENV),
            EnvDirective::Untouched
        );
    }
}
