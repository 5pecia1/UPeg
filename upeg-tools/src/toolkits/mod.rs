//! Per-toolkit Tool implementations.
//!
//! Each submodule below covers one logical toolkit (`convert`, `hash`,
//! `time`, `id`, `text`, `color`, `security`, `media`, `qr`,
//! `csv`, `num`, `eth`). The
//! `#[upeg::tool]`-annotated `pub fn`s are re-exported through
//! `crate::*` (via `lib.rs`) so external consumers keep using
//! `upeg_tools::sha256_hex(...)` style paths unchanged.
//!
//! `fill_os_random` is the only cross-toolkit helper — it lives here so
//! `id` and `security` can share one OS-RNG entry point.

pub mod color;
pub mod convert;
pub mod csv;
// Like `eth`/`weather`, `devcontainer`'s `#[tool]`-annotated functions emit
// their own `StaticToolMeta` that must exist on every target (including
// `wasm32`) so the tools stay discoverable there for host-attach; only the
// filesystem/SQLite implementation is native-gated inside `devcontainer.rs`.
// See its module doc for the full native/wasm split.
pub mod devcontainer;
// Unlike `net` (a pure helper module whose meta lives in `gui_meta.rs`),
// `eth`'s `#[tool]`-annotated functions emit their own `StaticToolMeta` —
// that must exist on every target (including `wasm32`) so the tool stays
// discoverable there for host-attach; only the actual network-calling
// implementation is native-gated inside `eth.rs`. See `eth.rs`'s module
// doc for the full native/wasm split.
pub mod eth;
pub mod hash;
// Shared HTTPS plumbing for the network toolkits (`eth`, `weather`). Not a
// toolkit and not `#[tool]`-annotated, so — unlike them — it emits no
// `StaticToolMeta` and can be gated off wholesale on `wasm32`, which has no
// socket for it to use. See its module doc.
#[cfg(not(target_arch = "wasm32"))]
pub mod http;
pub mod id;
pub mod media;
#[cfg(not(target_arch = "wasm32"))]
pub mod net;
pub mod num;
pub mod qr;
pub mod security;
pub mod text;
pub mod time;
// Like `eth`, `weather`'s `#[tool]`-annotated functions emit their own
// `StaticToolMeta` that must exist on every target (including `wasm32`) so the
// tool stays discoverable there for host-attach; only the network-calling
// implementation is native-gated inside `weather.rs`. See its module doc.
pub mod weather;

/// Cryptographic OS RNG fill. Shared by `id::nanoid` and
/// `security::{password_generate, bytes_generate}` so the entry point is
/// exercised once.
pub(super) fn fill_os_random(buf: &mut [u8]) -> Result<(), &'static str> {
    getrandom::fill(buf).map_err(|_| "OS RNG failed")
}
