/// Push-placement preview seam (inventory row F16).
///
/// Bridges the FRB `preview_push(board, tool, anchor_x, anchor_y)`
/// (returning `List&lt;PlacementDto&gt;`) into a typed, overridable Riverpod provider
/// so widget tests can paint the BoardCanvas overlay without crossing
/// the dylib boundary. Production calls the generated FRB shim;
/// tests pass a recording closure.
///
/// Type system / SoC: the loader is a top-level `typedef` (concrete
/// shape), NOT a `dynamic`-typed Map — the canvas consumer pattern-
/// matches on `PlacementDto` directly.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as frb;

/// Typed loader: `(boardKey, toolId, anchorX, anchorY) -> placements`.
/// Returns an empty list when the FRB layer rejects the anchor (unknown
/// tool, BOARD_COLS overflow, …) — the canvas reads the empty contract
/// as "hide the overlay".
typedef PushPreviewLoader =
    List<frb.PlacementDto> Function(
      BoardKey boardKey,
      ToolId toolId,
      int anchorX,
      int anchorY,
    );

/// Provider seam. Production wraps the generated `previewPush` shim;
/// tests override with `overrideWithValue(recordingLoader)`.
final pushPreviewLoaderProvider = Provider<PushPreviewLoader>(
  (ref) =>
      (boardKey, toolId, anchorX, anchorY) => frb.previewPush(
        boardKey: boardKey.value,
        toolId: toolId.value,
        anchorX: anchorX,
        anchorY: anchorY,
      ),
);
