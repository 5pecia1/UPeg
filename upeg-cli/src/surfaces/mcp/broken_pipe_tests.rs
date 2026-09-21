//! Regression coverage: `upeg mcp`'s serve loops must exit
//! cleanly when stdout is a broken pipe, never panic.
//!
//! Every write in this module already goes through `let _ =
//! writeln!(...)` / `let _ = out.flush();`, which cannot panic (the
//! `Result` is discarded, not `.unwrap()`ed) — an earlier audit
//! found no panicking write site *here*. The actual panic came from
//! `eprint!` in `infrastructure::mcp_imports` on a closed inherited
//! stderr (see that module's `write_to_broken_pipe_writer_does_not_panic`
//! test for the fix). These tests pin the "no panic on a broken
//! stdout" contract for `surfaces::mcp`'s own loops against
//! regression, since a future edit swapping `writeln!` for `println!`
//! (which panics on write failure) would reintroduce exactly this
//! bug.

use super::*;

/// A writer that always fails with `BrokenPipe` on both `write` and
/// `flush`, simulating a stdout pipe whose reading end has closed.
struct AlwaysBrokenPipe;

impl std::io::Write for AlwaysBrokenPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = buf;
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

#[test]
fn serve_loop_with_board_exits_without_panic_when_stdout_is_broken_pipe() {
    let mut reader = std::io::Cursor::new(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n".to_vec(),
    );
    let mut writer = AlwaysBrokenPipe;

    // Every write inside the loop fails with BrokenPipe; the loop
    // must still run to reader EOF and return without panicking.
    serve_loop_with_board(&mut reader, &mut writer, None);
}

#[test]
fn invalid_json_error_response_write_does_not_panic_on_broken_pipe() {
    let mut reader = std::io::Cursor::new(b"not json\n".to_vec());
    let mut writer = AlwaysBrokenPipe;

    serve_loop_with_board(&mut reader, &mut writer, None);
}

#[test]
fn serve_loop_once_returns_without_panic_when_stdout_is_broken_pipe() {
    let mut writer = AlwaysBrokenPipe;

    serve_loop_once(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
        &mut writer,
        None,
    );
}
