use crate::{StaticToolMeta, ToolkitMeta};

// ─── Static inventory declarations ──────────────────────────────
// Re-exported so proc-macro expansion can reference
// `::upeg_core::inventory::submit!` without requiring user crates to add
// `inventory` as a direct dependency. Mutable runtime registries live in
// `upeg-runtime`; core only declares the inventory collection points for its
// domain metadata types.
pub use inventory;

inventory::collect!(StaticToolMeta);
inventory::collect!(ToolkitMeta);
