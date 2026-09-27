/// Resize-projection preview seam (pin-resize UX).
///
/// Bridges the FRB `preview_resize(board, tool, cols, rows)` (returning
/// `List<PlacementDto>`) into a typed, overridable Riverpod provider so
/// widget tests can paint the BoardCanvas resize overlay without
/// crossing the dylib boundary. Mirrors `push_preview_provider.dart`
/// (F16) — production calls the generated FRB shim, tests pass a
/// recording closure.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as frb;

/// Typed loader: `(boardKey, toolId, cols, rows) -> placements`.
/// Returns an empty list when the FRB layer rejects the span (unknown
/// tool, span outside `ColSpan`/`RowSpan`, …) — the canvas reads the
/// empty contract as "hide the overlay".
typedef ResizePreviewLoader =
    List<frb.PlacementDto> Function(
      BoardKey boardKey,
      PinId toolId,
      int cols,
      int rows,
    );

/// Provider seam. Production wraps the generated `previewResize` shim;
/// tests override with `overrideWithValue(recordingLoader)`.
final resizePreviewLoaderProvider = Provider<ResizePreviewLoader>(
  (ref) =>
      (boardKey, toolId, cols, rows) => frb.previewResize(
        boardKey: boardKey.value,
        pinId: toolId.value,
        cols: cols,
        rows: rows,
      ),
);
