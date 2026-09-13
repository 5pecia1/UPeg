//! "The consumer went away" as a value.
//!
//! Both streaming lanes on this surface answer with a body that owns the
//! running dispatch's [`CancellationToken`]: the NDJSON route
//! (`stream.rs`) and the `/mcp` SSE route (`rpc.rs`'s `mcp_sse`). A
//! client hanging up drops the response body and nothing else — no
//! error, no chunk that fails to queue, no read that returns zero — so
//! the drop *is* the signal, and a value whose `Drop` cancels is the
//! whole mechanism.
//!
//! It lives in its own module rather than in either lane because both
//! own one, and a second copy would be a second place for "does hanging
//! up stop the tool?" to be answered differently.

use upeg_runtime::CancellationToken;

/// Cancels its token when dropped.
///
/// A field on the body rather than a `Drop` impl on the body itself: a
/// response body is a `Stream` whose `poll_next` takes
/// `Pin<&mut Self>`, and keeping "cancel on the way out" in a value of
/// its own leaves that impl exactly as simple as it was.
///
/// Cancelling after the dispatch has already finished — the ordinary end
/// of a stream, where the body is dropped once its last line is read —
/// is a no-op: the token is out of scope by then and nothing polls it.
pub(in crate::surfaces::http) struct CancelOnDrop(CancellationToken);

impl CancelOnDrop {
    pub(in crate::surfaces::http) const fn new(token: CancellationToken) -> Self {
        Self(token)
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
