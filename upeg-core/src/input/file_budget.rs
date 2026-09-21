//! Canonical `File` value budget policy.
//!
//! Single declaration of every resource limit enforced on `File` input and
//! output trees. Node count, metadata bytes, and nesting depth are the same
//! recursive `name`/`content` walk on both sides of the boundary, so they
//! are declared once here and aliased by [`super::file_resource_limits`]
//! (input) and [`super::file_output_preflight`] (output) under their
//! direction-specific names. The two raw-byte ceilings stay distinct on
//! purpose — input trees accept up to 50 MiB, output trees up to 64 MiB —
//! see README.md ("this input budget is separate from the output root
//! budget above").
//!
//! Dart (`flutter_app/lib/src/rust/canonical_file_value_budget.dart`,
//! `flutter_app/lib/src/platform/file_input_resource_limits.dart`,
//! `flutter_app/lib/src/platform/bounded_file_reader.dart`) and JS
//! (`chrome-ext/wire.js`, `chrome-ext/file_input.js`) declare the same
//! numbers independently, with no shared build-time source. They are pinned
//! against these constants by
//! `upeg-core/tests/file_budget_cross_language_pin.rs`; any edit here must
//! keep that test green.

const KIBIBYTE_BYTES: u64 = 1024;
const MEBIBYTE_BYTES: u64 = KIBIBYTE_BYTES * 1024;

/// Maximum `FileValue` nodes in one tree.
///
/// Shared by input and output: both walk the same recursive shape and must
/// agree, so this is the one place the number is written down.
pub const MAX_FILE_VALUE_NODES: u64 = 128;

/// Maximum aggregate UTF-8 bytes of names and MIME values in one tree.
///
/// Shared by input and output for the same reason as [`MAX_FILE_VALUE_NODES`].
pub const MAX_FILE_VALUE_METADATA_BYTES: u64 = 16 * KIBIBYTE_BYTES;

/// Maximum nodes allowed on one root-to-leaf path (the root is depth one).
///
/// Shared by input and output. A budget of 64 accommodates practical
/// directory trees while keeping the recursive decode safely below the
/// stack limit; without it a hostile payload of nested `directory` entries
/// recurses once per level and aborts the process.
pub const MAX_FILE_NESTING_DEPTH: usize = 64;

/// Maximum aggregate decoded bytes accepted by Core `File` **input**
/// validation. Deliberately distinct from [`MAX_FILE_OUTPUT_RAW_BYTES`].
pub const MAX_FILE_INPUT_RAW_BYTES: u64 = 50 * MEBIBYTE_BYTES;

/// Maximum recursive regular-file count accepted by a `File` input policy.
pub const MAX_FILE_INPUT_COUNT: u32 = 100;

/// Maximum aggregate decoded bytes accepted in one `FileValue` **output**
/// tree. Deliberately distinct from [`MAX_FILE_INPUT_RAW_BYTES`].
pub const MAX_FILE_OUTPUT_RAW_BYTES: u64 = 64 * MEBIBYTE_BYTES;
