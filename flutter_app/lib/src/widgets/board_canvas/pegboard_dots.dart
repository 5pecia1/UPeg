/// Peg-hole wallpaper: the dot grid that makes a board read as a
/// physical pegboard, plus the painter both the empty-board and the
/// populated-grid paths share.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';

/// `.pegboard-bg` equivalent — paints the dot-grid background behind
/// the pin grid. Tokens come from the ambient [UpegTokens] so a theme
/// switch repaints automatically; the boolean `showHoles` flag comes
/// from [showHolesProvider] so a Tweaks toggle live-removes the holes
/// without re-mounting the canvas.
///
/// Public-for-testing so widget tests can locate the painter via
/// `find.byType(BoardCanvasPegboardForTesting)` and read its painter
/// field for assertions.
@visibleForTesting
class BoardCanvasPegboardForTesting extends ConsumerWidget {
  const BoardCanvasPegboardForTesting({
    required this.tokens,
    required this.child,
    this.paintDots = true,
    super.key,
  });

  final UpegTokens tokens;
  final Widget child;

  /// When `true` (default), this widget paints the peg-hole wallpaper
  /// itself, sized to whatever viewport it's given — the right choice
  /// for the empty-board path, which has no scrollable grid to align
  /// dots against. The populated-grid path instead sets this `false`
  /// and paints its dots via [PegboardGridDots], nested INSIDE the
  /// scroll content so the holes scroll together with the cells they
  /// mark (see class doc on [PegboardDotsPainter]).
  final bool paintDots;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (!paintDots) {
      return DecoratedBox(
        decoration: BoxDecoration(color: tokens.bg),
        child: child,
      );
    }
    final showHoles = ref.watch(showHolesProvider);
    return DecoratedBox(
      decoration: BoxDecoration(color: tokens.bg),
      child: CustomPaint(
        painter: PegboardDotsPainter(hole: tokens.hole, showHoles: showHoles),
        child: child,
      ),
    );
  }
}

/// Paints a viewport-sized board inside the scroll content, so the holes
/// follow the cells while covering space around a small grid. The pin grid
/// stays top-centred inside its original padding; the painter receives that
/// grid origin to keep each hole on a cell-gap intersection.
class PegboardGridDots extends ConsumerWidget {
  const PegboardGridDots({
    required this.tokens,
    required this.viewportSize,
    required this.gridSize,
    required this.child,
    super.key,
  });

  final UpegTokens tokens;
  final Size viewportSize;
  final Size gridSize;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final showHoles = ref.watch(showHolesProvider);
    const padding = UpegSizing.pegboardPadding;
    final paintSize = Size(
      gridSize.width + 2 * padding > viewportSize.width
          ? gridSize.width + 2 * padding
          : viewportSize.width,
      gridSize.height + 2 * padding > viewportSize.height
          ? gridSize.height + 2 * padding
          : viewportSize.height,
    );
    return ConstrainedBox(
      constraints: BoxConstraints(
        minWidth: paintSize.width,
        minHeight: paintSize.height,
      ),
      child: CustomPaint(
        key: const Key('board-canvas-grid-dots'),
        painter: PegboardDotsPainter(
          hole: tokens.hole,
          showHoles: showHoles,
          gridOriginX: (paintSize.width - gridSize.width) / 2,
          gridOriginY: padding,
        ),
        child: Padding(
          padding: const EdgeInsets.all(padding),
          child: Align(alignment: Alignment.topCenter, child: child),
        ),
      ),
    );
  }
}

/// `.pegboard-bg` peg-hole dot painter. `showHoles=false` early-returns
/// from `paint()` so toggling the flag at the provider level live-
/// removes the dots without re-mounting the widget. Lives at module
/// scope (not private) so unit tests can assert `paint(Canvas, Size)`
/// behaviour directly and widget tests can cast `CustomPaint.painter`
/// without `as dynamic`.
///
/// The pattern places one small hole at each grid-slot intersection,
/// while the persisted pin coordinates still use the original cell geometry.
class PegboardDotsPainter extends CustomPainter {
  PegboardDotsPainter({
    required this.hole,
    required this.showHoles,
    this.gridOriginX = 0,
    this.gridOriginY = 0,
  });

  final Color hole;
  final bool showHoles;

  /// Horizontal location of the grid's first cell within the painted board.
  final double gridOriginX;
  final double gridOriginY;

  /// Full horizontal cell-slot pitch, derived from [BoardGridMetrics].
  static const double slotPitchX =
      BoardGridMetrics.cellWidth + BoardGridMetrics.gap;

  /// Full vertical cell-slot pitch, derived from [BoardGridMetrics].
  static const double slotPitchY =
      BoardGridMetrics.cellHeight + BoardGridMetrics.gap;

  static const double pitchX = slotPitchX;
  static const double pitchY = slotPitchY;

  /// Half a cell-gap — the offset from a cell-slot boundary to the hole
  /// that sits centred on it.
  static const double _holeInset = BoardGridMetrics.gap / 2;

  static const double holeRadius = 1.25;

  /// x-position of the k-th vertical hole line (k = 0..cols). Sits at
  /// the gap-centred intersection between cell `k-1` and cell `k`, so
  /// `holeX(0)` lands half a gap to the left of the first cell's left
  /// edge — the outer corner of the (0, *) cell slots.
  static double holeX(int k) => k * slotPitchX - _holeInset;

  /// y-position of the k-th horizontal hole line (k = 0..rows). See
  /// [holeX].
  static double holeY(int k) => k * slotPitchY - _holeInset;

  @override
  void paint(Canvas canvas, Size size) {
    if (!showHoles) return;
    final paint = Paint()..color = hole;
    final minX = ((_holeInset - gridOriginX) / pitchX).ceil();
    final minY = ((_holeInset - gridOriginY) / pitchY).ceil();
    final maxX = ((size.width + _holeInset - gridOriginX) / pitchX).floor();
    final maxY = ((size.height + _holeInset - gridOriginY) / pitchY).floor();
    for (int row = minY; row <= maxY; row++) {
      for (int col = minX; col <= maxX; col++) {
        canvas.drawCircle(
          Offset(
            gridOriginX + col * pitchX - _holeInset,
            gridOriginY + row * pitchY - _holeInset,
          ),
          holeRadius,
          paint,
        );
      }
    }
  }

  @override
  bool shouldRepaint(covariant PegboardDotsPainter old) =>
      old.hole != hole ||
      old.showHoles != showHoles ||
      old.gridOriginX != gridOriginX ||
      old.gridOriginY != gridOriginY;
}
