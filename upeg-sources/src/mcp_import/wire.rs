//! JSON-RPC 2.0 wire plumbing for an `UpstreamServer`'s stdio pipes:
//! request/response framing, the notification write, the capped line
//! reader, subprocess teardown, and the sync/async bridge every call
//! site needs. Pulled out of the parent module purely to stay under
//! the workspace's per-file line budget — every item here operates on
//! explicit parameters (no access to `UpstreamServer`'s private
//! fields), so the split adds no coupling.

use std::future::Future;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::runtime::RuntimeFlavor;

use super::{ImportError, MCP_SHUTDOWN_TIMEOUT};

pub(super) struct PendingRequest {
    pub(super) server: String,
    pub(super) id: u64,
    pub(super) method: &'static str,
    pub(super) params: Value,
    pub(super) timeout: Duration,
}

pub(super) async fn request_async(
    child: &mut Child,
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: PendingRequest,
) -> Result<Value, ImportError> {
    let PendingRequest {
        server,
        id,
        method,
        params,
        timeout,
    } = request;
    let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
    let result = tokio::time::timeout(timeout, async {
        let req = build_request_frame(id, method, params);
        stdin.write_all(req.to_string().as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        let mut line = String::new();
        match read_line_capped_async(stdout, &mut line).await? {
            crate::protocol::LineReadOutcome::Eof => {
                return Err(ImportError::Protocol(format!(
                    "subprocess closed stdout before responding to `{method}`"
                )));
            }
            crate::protocol::LineReadOutcome::CapHit => {
                return Err(ImportError::Protocol(format!(
                    "subprocess response exceeds {} bytes for `{method}`",
                    crate::protocol::MAX_LINE_BYTES,
                )));
            }
            crate::protocol::LineReadOutcome::Line { .. } => {}
        }
        let resp: Value = serde_json::from_str(line.trim()).map_err(|e| {
            ImportError::Protocol(format!(
                "invalid JSON line: {e}: {}",
                super::display_value(&Value::String(line))
            ))
        })?;
        if resp["id"] != json!(id) {
            return Err(ImportError::Protocol(format!(
                "id mismatch: sent {id}, got {}",
                resp["id"]
            )));
        }
        if let Some(err) = resp.get("error") {
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            return Err(ImportError::Rpc {
                method,
                code,
                message,
            });
        }
        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    })
    .await;

    if let Ok(result) = result {
        result
    } else {
        let _ = kill_and_wait(child).await;
        Err(ImportError::Timeout {
            server,
            method,
            timeout_ms,
        })
    }
}

/// Build a JSON-RPC 2.0 request frame, omitting the `params` member
/// entirely when `params` is `Value::Null` (E-5). A `json!` object
/// literal always inserts the key it names, so `"params": null` was
/// always serialized before this helper existed — official
/// `@modelcontextprotocol` TypeScript SDK servers validate the frame
/// strictly and drop `params: null` requests silently, which made
/// every `tools/list` (sent with no params) time out against real
/// servers. Per JSON-RPC 2.0, an absent `params` and a `null` params
/// are not equivalent, so this is a spec-compliance fix, not a
/// cosmetic one.
fn build_request_frame(id: u64, method: &str, params: Value) -> Value {
    let mut frame = serde_json::Map::with_capacity(4);
    frame.insert("jsonrpc".to_string(), json!("2.0"));
    frame.insert("id".to_string(), json!(id));
    frame.insert("method".to_string(), json!(method));
    if !params.is_null() {
        frame.insert("params".to_string(), params);
    }
    Value::Object(frame)
}

/// Write a JSON-RPC 2.0 notification (no `id` member — the frame's
/// absence of `id` is exactly what tells the peer no response is
/// owed, per spec). Does not wait for or expect a reply.
pub(super) async fn write_notification_async(
    stdin: &mut ChildStdin,
    method: &'static str,
) -> std::io::Result<()> {
    let note = json!({ "jsonrpc": "2.0", "method": method });
    stdin.write_all(note.to_string().as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await
}

async fn read_line_capped_async<R>(
    reader: &mut R,
    line: &mut String,
) -> std::io::Result<crate::protocol::LineReadOutcome>
where
    R: AsyncBufRead + Unpin,
{
    let n = (&mut *reader)
        .take(crate::protocol::MAX_LINE_BYTES as u64)
        .read_line(line)
        .await?;
    Ok(match (n, line.ends_with('\n')) {
        (0, _) => crate::protocol::LineReadOutcome::Eof,
        (n, false) if n >= crate::protocol::MAX_LINE_BYTES => {
            crate::protocol::LineReadOutcome::CapHit
        }
        (n, _) => crate::protocol::LineReadOutcome::Line { bytes: n },
    })
}

pub(super) async fn kill_and_wait(child: &mut Child) -> std::io::Result<()> {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(MCP_SHUTDOWN_TIMEOUT, child.wait()).await;
    Ok(())
}

pub(super) fn block_on_runtime<F>(runtime: &tokio::runtime::Runtime, future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    match tokio::runtime::Handle::try_current().map(|handle| handle.runtime_flavor()) {
        Ok(RuntimeFlavor::MultiThread) => tokio::task::block_in_place(|| runtime.block_on(future)),
        Ok(RuntimeFlavor::CurrentThread) => {
            std::thread::scope(
                |scope| match scope.spawn(|| runtime.block_on(future)).join() {
                    Ok(output) => output,
                    Err(payload) => std::panic::resume_unwind(payload),
                },
            )
        }
        Err(_) => runtime.block_on(future),
        #[allow(
            unreachable_patterns,
            reason = "RuntimeFlavor is non-exhaustive across Tokio releases"
        )]
        Ok(_) => runtime.block_on(future),
    }
}
