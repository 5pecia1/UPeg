`flat-64.heic` and `checker-1024.heic` are Apple-encoded HEVC still images
from `heic-rs` 0.1.1's generated test fixtures (MIT OR Apache-2.0).
The former is a 64 × 64 reddish image (source RGB `[200, 30, 60]`).
The latter is a 2 × 2 grid of 512 × 512 tiles. Its reference decode has
white pixels at `(32, 32)` and `(992, 992)`, and black at `(96, 32)`;
see upstream `tests/fixtures/GROUND-TRUTH.md`.

Tests use the same bytes under both `.heic` and `.heif` names to confirm
that input decoding follows the file contents while the file picker accepts
both extensions.
