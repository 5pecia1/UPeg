//! Bounded live tail of a still-running Tool's output.
//!
//! `upeg_runtime::ProgressEvent::chunk` is whatever the invoker had
//! ready — a whole line, several lines, or half of one. The TUI right
//! pane wants the opposite shape: the last few *lines*, ready to render
//! this frame. [`LiveTail`] is that adapter, and it is deliberately the
//! only place in the TUI that knows a chunk is not a line.
//!
//! Bounded by construction: a tool that prints a gigabyte still costs
//! [`TUI_LIVE_TAIL_MAX_LINES`] retained lines of at most
//! [`TUI_LIVE_TAIL_MAX_LINE_BYTES`] each, because both the line ring and
//! the unterminated remainder drop what does not fit instead of growing.
//! A tail is a preview, not a transcript — the final `ToolResult`
//! envelope is still the contract.

use std::collections::VecDeque;

/// How many trailing output lines the running pane keeps.
pub(crate) const TUI_LIVE_TAIL_MAX_LINES: usize = 8;

/// How many bytes of one line the running pane keeps. Anything past
/// this is dropped rather than buffered: the pane is one terminal row
/// wide per line, so the overflow could never be shown anyway.
pub(crate) const TUI_LIVE_TAIL_MAX_LINE_BYTES: usize = 512;

/// Carriage return — "redraw the current line", the shape progress bars
/// and spinners emit. Named so the `\r` / `\n` pair reads as protocol
/// rather than as punctuation.
const CARRIAGE_RETURN: char = '\r';
/// Line feed — commits the current line to the ring.
const LINE_FEED: char = '\n';

/// The last [`TUI_LIVE_TAIL_MAX_LINES`] lines a running Tool produced.
///
/// `pending_cr` is what makes a `\r\n` pair split across two chunks
/// behave like the single line terminator it is: a bare `\r` clears the
/// line being built (overwrite semantics), but a `\r` immediately
/// followed by `\n` commits it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveTail {
    /// Completed lines, oldest first.
    lines: VecDeque<String>,
    /// The line currently being built — visible before its terminator
    /// arrives, so a tool that prints a prompt without a newline is not
    /// invisible until it exits.
    partial: String,
    /// A `\r` was the last byte seen and its companion `\n` may still
    /// be in the next chunk.
    pending_cr: bool,
}

impl LiveTail {
    /// Absorb one raw progress chunk. Partial lines are expected.
    pub(crate) fn push_chunk(&mut self, chunk: &str) {
        for character in chunk.chars() {
            if self.pending_cr {
                self.pending_cr = false;
                if character == LINE_FEED {
                    self.commit_line();
                    continue;
                }
                // A lone `\r`: the tool is overwriting the line it just
                // drew, so the half-drawn text is not output the user
                // should keep.
                self.partial.clear();
            }
            match character {
                CARRIAGE_RETURN => self.pending_cr = true,
                LINE_FEED => self.commit_line(),
                _ => self.push_visible(character),
            }
        }
    }

    /// The lines to render, oldest first, newest last — at most
    /// [`TUI_LIVE_TAIL_MAX_LINES`] including the unterminated one.
    pub(crate) fn lines(&self) -> impl Iterator<Item = &str> {
        let partial = (!self.partial.is_empty()).then_some(self.partial.as_str());
        let shown = self.lines.len() + usize::from(partial.is_some());
        let dropped = shown.saturating_sub(TUI_LIVE_TAIL_MAX_LINES);
        self.lines
            .iter()
            .map(String::as_str)
            .skip(dropped)
            .chain(partial)
    }

    /// No output has arrived yet — the pane shows its placeholder.
    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.partial.is_empty()
    }

    fn commit_line(&mut self) {
        let line = std::mem::take(&mut self.partial);
        self.lines.push_back(line);
        while self.lines.len() > TUI_LIVE_TAIL_MAX_LINES {
            self.lines.pop_front();
        }
    }

    fn push_visible(&mut self, character: char) {
        if self.partial.len() + character.len_utf8() <= TUI_LIVE_TAIL_MAX_LINE_BYTES {
            self.partial.push(character);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tail_lines(tail: &LiveTail) -> Vec<&str> {
        tail.lines().collect()
    }

    #[test]
    fn new_tail_is_empty() {
        let tail = LiveTail::default();
        assert!(tail.is_empty());
        assert!(tail_lines(&tail).is_empty());
    }

    #[test]
    fn chunk_splits_into_lines_at_newlines() {
        let mut tail = LiveTail::default();
        tail.push_chunk("first\nsecond\n");
        assert_eq!(tail_lines(&tail), ["first", "second"]);
    }

    #[test]
    fn line_spanning_multiple_chunks_joins_into_one() {
        let mut tail = LiveTail::default();
        tail.push_chunk("he");
        tail.push_chunk("llo");
        tail.push_chunk(" world\n");
        assert_eq!(tail_lines(&tail), ["hello world"]);
    }

    #[test]
    fn partial_line_before_newline_is_visible() {
        let mut tail = LiveTail::default();
        tail.push_chunk("working");
        assert!(!tail.is_empty());
        assert_eq!(tail_lines(&tail), ["working"]);

        tail.push_chunk("\n");
        assert_eq!(tail_lines(&tail), ["working"]);
    }

    #[test]
    fn overflow_drops_oldest_lines_first() {
        let mut tail = LiveTail::default();
        for index in 0..(TUI_LIVE_TAIL_MAX_LINES + 3) {
            tail.push_chunk(&format!("line {index}\n"));
        }
        let lines = tail_lines(&tail);
        assert_eq!(lines.len(), TUI_LIVE_TAIL_MAX_LINES);
        assert_eq!(lines.first().copied(), Some("line 3"));
        assert_eq!(
            lines.last().copied(),
            Some(format!("line {}", TUI_LIVE_TAIL_MAX_LINES + 2).as_str())
        );
    }

    #[test]
    fn partial_line_fits_within_cap() {
        let mut tail = LiveTail::default();
        for index in 0..TUI_LIVE_TAIL_MAX_LINES {
            tail.push_chunk(&format!("line {index}\n"));
        }
        tail.push_chunk("tail");

        let lines = tail_lines(&tail);
        assert_eq!(lines.len(), TUI_LIVE_TAIL_MAX_LINES);
        assert_eq!(lines.last().copied(), Some("tail"));
        assert_eq!(lines.first().copied(), Some("line 1"));
    }

    #[test]
    fn lone_carriage_return_overwrites_current_line() {
        let mut tail = LiveTail::default();
        tail.push_chunk("50%\r100%");
        assert_eq!(tail_lines(&tail), ["100%"]);
    }

    #[test]
    fn crlf_treated_as_single_line_ending() {
        let mut tail = LiveTail::default();
        tail.push_chunk("done\r\n");
        assert_eq!(tail_lines(&tail), ["done"]);
    }

    #[test]
    fn crlf_spanning_chunk_boundary_is_one_line() {
        let mut tail = LiveTail::default();
        tail.push_chunk("done\r");
        tail.push_chunk("\nnext");
        assert_eq!(tail_lines(&tail), ["done", "next"]);
    }

    #[test]
    fn line_length_truncated_at_byte_cap() {
        let mut tail = LiveTail::default();
        let long = "x".repeat(TUI_LIVE_TAIL_MAX_LINE_BYTES * 4);
        tail.push_chunk(&long);

        let lines = tail_lines(&tail);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), TUI_LIVE_TAIL_MAX_LINE_BYTES);
    }

    #[test]
    fn multibyte_chars_not_split_at_byte_cap() {
        let mut tail = LiveTail::default();
        // Filling with only 3-byte characters can never land exactly on
        // the 512 cap, so the last character must be rejected whole.
        let long = "가".repeat(TUI_LIVE_TAIL_MAX_LINE_BYTES);
        tail.push_chunk(&long);

        let lines = tail_lines(&tail);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= TUI_LIVE_TAIL_MAX_LINE_BYTES);
        assert!(lines[0].chars().all(|c| c == '가'));
    }
}
