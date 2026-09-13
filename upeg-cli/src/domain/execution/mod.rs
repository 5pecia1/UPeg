//! Tool execution and dispatch — the core "run a tool, get an outcome" loop.
//!
//! Surface-agnostic: any caller (CLI, HTTP, MCP, TUI, attach) that has a
//! `(tool_id, args)` pair routes through here.
//!
//! - [`dispatch`] — registry lookup + `Outcome` sentinel.
//! - [`context`] — surface/board/trigger annotations applied to args
//!   before dispatch.

pub(crate) mod context;
pub(crate) mod dispatch;
