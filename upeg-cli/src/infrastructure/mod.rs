//! Infrastructure layer — OS- and protocol-level glue beneath the
//! domain and surfaces.
//!
//! These modules own state that lives outside the process (filesystem
//! paths, OS notifications, child-process lifecycle, TCP discovery
//! files) and the conventions for talking to an already-running host
//! (`attach`, `clients`). They sit one layer below the surfaces — a
//! surface decides "I want to start an HTTP server"; this layer knows
//! how to daemonize, where to drop a `server.json` discovery file,
//! and how to resolve a bearer token from disk or env.

pub(crate) mod attach;
pub(crate) mod auth;
pub(crate) mod clients;
pub(crate) mod daemonize;
pub(crate) mod discovery;
pub(crate) mod lifecycle;
pub(crate) mod log_file;
pub(crate) mod mcp_imports;
pub(crate) mod notify;
pub(crate) mod paths;
pub(crate) mod pause;
pub(crate) mod process;
