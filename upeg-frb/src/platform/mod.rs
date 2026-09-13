//! Platform glue used by the FRB `api/boot.rs` lifecycle.
//!
//! `instance_lock` owns the OS-level desktop single-instance lock.
//! `host_bootstrap` runs the embed-or-attach HTTP host bootstrap.
//!
//! Both modules are compiled out on `wasm32` — the browser cannot host
//! a daemon and cannot take an OS file lock, so `boot::init_app` skips
//! them and returns `HostState::NoHost` instead.

#[cfg(not(target_arch = "wasm32"))]
pub mod host_bootstrap;
#[cfg(not(target_arch = "wasm32"))]
pub mod instance_lock;
