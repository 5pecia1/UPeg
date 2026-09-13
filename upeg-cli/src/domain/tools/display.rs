//! Tool-id rendering primitives — `display_id` for the 64-char cap that
//! every surface uses when echoing an id back in an error message, and
//! the shared `truncate_for_display` primitive it builds on.
//!
//! Multibyte-safe : truncates by char count, never byte slice,
//! so ids containing emoji or non-ASCII don't panic the daemon thread
//! that serves the request.

/// truncate excessively-long tool ids when echoing them back
/// in error messages. 64 chars is enough to identify a typo against any
/// realistic tool id (longest built-in is ~25 chars; MCP-namespaced is
/// ~50 with a long server prefix).
pub fn display_id(id: &str) -> String {
    truncate_for_display(id, 64)
}

/// shared truncate-with-marker primitive used by both
/// `display_id` (MAX=64) and `display_value` (MAX=200, in `mcp_import`).
///
/// Contract:
///   1. Byte-length pre-filter (`s.len() <= max_chars`): O(1) early-out
///      for typical short ASCII inputs that don't need truncation at all.
///   2. Char-count check (`s.chars().count() <= max_chars`): multibyte
///      safety. The byte threshold can over-trigger (e.g., 17 emojis =
///      68 bytes but only 17 chars).
///   3. `chars().take + collect`: UTF-8-safe truncation. Suffix reports
///      `chars` to match the unit used in the gate.
pub(crate) fn truncate_for_display(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let prefix: String = s.chars().take(max_chars).collect();
    format!("{prefix}…(truncated, {total_chars} chars total)")
}
