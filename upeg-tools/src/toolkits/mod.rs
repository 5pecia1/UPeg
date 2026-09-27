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

#[cfg(feature = "color")]
pub mod color;
#[cfg(feature = "convert")]
pub mod convert;
#[cfg(feature = "csv")]
pub mod csv;
// Like `eth`/`weather`, `devcontainer`'s `#[tool]`-annotated functions emit
// their own `StaticToolMeta` that must exist on every target (including
// `wasm32`) so the tools stay discoverable there for host-attach; only the
// filesystem/SQLite implementation is native-gated inside `devcontainer.rs`.
// See its module doc for the full native/wasm split.
#[cfg(feature = "devcontainer")]
pub mod devcontainer;
// Unlike `net` (a pure helper module whose meta lives in `gui_meta.rs`),
// `eth`'s `#[tool]`-annotated functions emit their own `StaticToolMeta` —
// that must exist on every target (including `wasm32`) so the tool stays
// discoverable there for host-attach; only the actual network-calling
// implementation is native-gated inside `eth.rs`. See `eth.rs`'s module
// doc for the full native/wasm split.
#[cfg(feature = "eth")]
pub mod eth;
#[cfg(feature = "hash")]
pub mod hash;
// Shared HTTPS plumbing for the network toolkits (`eth`, `weather`). Not a
// toolkit and not `#[tool]`-annotated, so — unlike them — it emits no
// `StaticToolMeta` and can be gated off wholesale on `wasm32`, which has no
// socket for it to use. See its module doc.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(any(feature = "eth", feature = "weather"))]
pub mod http;
#[cfg(feature = "id")]
pub mod id;
#[cfg(feature = "media")]
pub mod media;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "num")]
pub mod num;
#[cfg(feature = "qr")]
pub mod qr;
#[cfg(feature = "security")]
pub mod security;
#[cfg(feature = "text")]
pub mod text;
#[cfg(feature = "time")]
pub mod time;
// Like `eth`, `weather`'s `#[tool]`-annotated functions emit their own
// `StaticToolMeta` that must exist on every target (including `wasm32`) so the
// tool stays discoverable there for host-attach; only the network-calling
// implementation is native-gated inside `weather.rs`. See its module doc.
#[cfg(feature = "weather")]
pub mod weather;

/// Cryptographic OS RNG fill. Shared by `id::nanoid` and
/// `security::{password_generate, bytes_generate}` so the entry point is
/// exercised once.
#[cfg(any(feature = "id", feature = "security"))]
pub(super) fn fill_os_random(buf: &mut [u8]) -> Result<(), &'static str> {
    getrandom::fill(buf).map_err(|_| "OS RNG failed")
}
