//! Identity of the TUI's one in-flight dispatch.
//!
//! `View::Running` used to *be* the run state: the pane was on screen
//! exactly while a worker was alive. It is not, and the gap is
//! observable — a double `Esc` opens the quit overlay, backing out of
//! that overlay lands on `View::List`, and the worker keeps running with
//! nothing on screen to say so. The model then believed nothing was
//! running while the event loop still held the dispatch, so the next Run
//! built a fresh `View::Running` whose `Effect::Dispatch` the loop
//! silently dropped and whose pane then filled with the *old* run's
//! output.
//!
//! So the run gets an identity of its own, held on `State` rather than
//! in the `View`: [`ActiveRun`] is what the model knows is in flight,
//! and [`RunToken`] is the value both sides stamp onto everything
//! belonging to that one dispatch. A progress event that names a token
//! the model no longer considers active is a straggler from a finished
//! run, not output to render.

/// Identity of one TUI dispatch.
///
/// Minted per run rather than derived from the tool id, because the same
/// tool can be run again the moment the previous run ends and the two
/// runs' straggling progress events must not be confusable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunToken(u64);

/// The value [`RunTokenMint`] hands out first. `0` is deliberately never
/// minted, so a defaulted [`RunToken`] can never collide with a live
/// run.
const FIRST_RUN_TOKEN: u64 = 1;

/// Source of [`RunToken`]s for one TUI session.
///
/// A per-`State` counter rather than a process-global one: the TUI's
/// update function is pure, and a global would make two tests running in
/// the same binary observe each other's run numbering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunTokenMint {
    last: u64,
}

impl RunTokenMint {
    /// The next unused token. Saturating rather than wrapping: reaching
    /// `u64::MAX` runs in one session is not reachable, and reusing a
    /// token is the one outcome that would resurrect this module's bug.
    pub(crate) fn mint(&mut self) -> RunToken {
        self.last = self.last.saturating_add(FIRST_RUN_TOKEN);
        RunToken(self.last)
    }
}

/// The dispatch the event loop has in flight, as the model sees it.
///
/// Independent of the current `View` on purpose: leaving the running
/// pane loses the tail, never the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveRun {
    /// Identity every message belonging to this run carries.
    pub run: RunToken,
    /// What is running, so the surface can say so when a second Run is
    /// refused.
    pub tool_id: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 새_mint의_첫_token은_기본값과_다르다() {
        let mut mint = RunTokenMint::default();
        assert_ne!(mint.mint(), RunToken::default());
    }

    #[test]
    fn 연속으로_민팅한_token은_서로_다르다() {
        let mut mint = RunTokenMint::default();
        let first = mint.mint();
        let second = mint.mint();
        assert_ne!(first, second);
    }
}
