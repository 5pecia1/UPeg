/// Pegboard cell geometry — the single source for "where does cell
/// `(x, y)` land in pixels".
///
/// Every layer of the canvas (drop cells, pins, hover highlights, resize
/// previews, the peg-hole dots) derives its rectangles from this one
/// class, so a stored `(x, y, w, h)` can never mean two different pixel
/// rects within the same surface.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Lift height for `PinWithLabel`. A 1-unit cell is taller than the
/// canonical column width (`pinCellWidth=168`) because pin chrome
/// (header + body + footer + 18px description line-clamp) stacks to
/// ~150 px at the smallest dot count.
@visibleForTesting
const double boardCanvasPinCellHeight = 150.0;

/// Number of empty rows kept below the currently hovered drag row.
const int dragExpansionBufferRows = 2;

/// Cell-slot geometry of the pegboard grid.
///
/// A "cell slot" is one canonical pin footprint plus the gap that
/// separates it from the next slot; a pin spanning `n` slots also
/// covers the `n - 1` gaps between them.
abstract final class BoardGridMetrics {
  static const double gap = UpegSizing.pinGap;
  static const double cellWidth = UpegSizing.pinCellWidth;
  static const double cellHeight = boardCanvasPinCellHeight;

  static double pixelX(int col) => col * (cellWidth + gap);
  static double pixelY(int row) => row * (cellHeight + gap);

  /// Pixel size of a pin spanning [span] cells, including the gaps that sit
  /// between the cells it covers. Single source for the rendered pin size so
  /// the board layer and the drag-feedback copy can never drift apart.
  static double pixelW(int span) => cellWidth * span + gap * (span - 1);
  static double pixelH(int span) => cellHeight * span + gap * (span - 1);

  /// Pixel extent of [cells] consecutive slots of [cellSize], gaps included.
  static double spanPixels(int cells, double cellSize) =>
      cells * cellSize + (cells - 1) * gap;

  /// Clamps a rendered row/column count into `[min, max]`.
  static int clampCount(int value, {required int min, required int max}) {
    if (value < min) return min;
    if (value > max) return max;
    return value;
  }
}

/// Index of the last row occupied by any of [placements] (0 when empty).
int maxPlacementRow(List<PlacementDto> placements) {
  int max = 0;
  for (final p in placements) {
    final last = p.y + p.h - 1;
    if (last > max) max = last;
  }
  return max;
}

/// dragged tool isn't currently placed on this board.
PlacementDto? findPlacementByToolId(
  List<PlacementDto> placements,
  ToolId toolId,
) {
  for (final p in placements) {
    if (p.toolId == toolId.value) return p;
  }
  return null;
}

/// Minimal scroll offset that fully reveals the `[targetStart, targetEnd]`
/// span inside a viewport of [viewportExtent] currently scrolled to
/// [currentOffset]. Pure — shared by both scroll axes and unit-testable
/// without a widget tree.
///
///   * already fully visible → returns [currentOffset] unchanged (no jump),
///   * clipped at the near edge (left/top) → aligns the span's start,
///   * clipped at the far edge (right/bottom) → moves just enough to
///     expose the span's end,
///   * span larger than the viewport → aligns the start edge so the pin
///     header stays visible.
double scrollOffsetToReveal({
  required double currentOffset,
  required double viewportExtent,
  required double targetStart,
  required double targetEnd,
}) {
  if (targetEnd - targetStart >= viewportExtent) return targetStart;
  if (targetStart < currentOffset) return targetStart;
  final viewportEnd = currentOffset + viewportExtent;
  if (targetEnd > viewportEnd) return currentOffset + (targetEnd - viewportEnd);
  return currentOffset;
}
