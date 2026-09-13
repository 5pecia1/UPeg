//! Pure (I/O-free) business logic behind `upeg plugin new`: candidate
//! name validation and guest-crate scaffold templates.
//!
//! Filesystem side effects (creating directories, writing files, copying
//! `.wasm` artifacts) live in `crate::app::plugin_command` — this module
//! only decides "is this name valid" and "what should the scaffold
//! contain", both of which are unit-testable without touching disk.

pub(crate) mod name;
pub(crate) mod scaffold;
