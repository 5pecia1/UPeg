//! Regression coverage for E-6(c): `upeg mcp`'s serve loops must exit
//! cleanly when stdout is a broken pipe, never panic.
//!
//! Every write in this module already goes through `let _ =
//! writeln!(...)` / `let _ = out.flush();`, which cannot panic (the
//! `Result` is discarded, not `.unwrap()`ed) — the audit for E-6(c)
//! found no panicking write site *here*. The actual panic came from
//! `eprint!` in `infrastructure::mcp_imports` on a closed inherited
//! stderr (see that module's `broken_pipe_라이터에_써도_패닉하지_않는다`
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
fn stdout가_broken_pipe여도_serve_loop_with_board는_패닉없이_종료한다() {
    let mut reader = std::io::Cursor::new(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n".to_vec(),
    );
    let mut writer = AlwaysBrokenPipe;

    // Every write inside the loop fails with BrokenPipe; the loop
    // must still run to reader EOF and return without panicking.
    serve_loop_with_board(&mut reader, &mut writer, None);
}

#[test]
fn 잘못된_json_오류_응답_기록도_broken_pipe에서_패닉하지_않는다() {
    let mut reader = std::io::Cursor::new(b"not json\n".to_vec());
    let mut writer = AlwaysBrokenPipe;

    serve_loop_with_board(&mut reader, &mut writer, None);
}

#[test]
fn stdout가_broken_pipe여도_serve_loop_once는_패닉없이_반환한다() {
    let mut writer = AlwaysBrokenPipe;

    serve_loop_once(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
        &mut writer,
        None,
    );
}
