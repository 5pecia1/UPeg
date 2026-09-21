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
        painter: PegboardDotsPainter(
          hole: tokens.hole,
          holeDeep: tokens.holeDeep,
          showHoles: showHoles,
        ),
        child: child,
      ),
    );
  }
}

/// Wraps [AbsoluteGrid] (the scrollable, padded grid content) with the
/// peg-hole dots painter so the holes scroll with the cells instead of
/// staying pinned to the viewport. Nested inside the same padded scroll
/// child as `AbsoluteGrid` so its local coordinate frame IS the grid's
/// frame — cell (0, 0)'s top-left corner is local `(0, 0)` here too, so
/// [PegboardDotsPainter.holeX]/[PegboardDotsPainter.holeY] need no extra
/// origin correction.
class PegboardGridDots extends ConsumerWidget {
  const PegboardGridDots({
    required this.tokens,
    required this.child,
    super.key,
  });

  final UpegTokens tokens;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final showHoles = ref.watch(showHolesProvider);
    return CustomPaint(
      key: const Key('board-canvas-grid-dots'),
      painter: PegboardDotsPainter(
        hole: tokens.hole,
        holeDeep: tokens.holeDeep,
        showHoles: showHoles,
      ),
      child: child,
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
/// Pegboard contract — "holes are the coordinate system": the dot pitch is derived from the
/// SAME cell constants [AbsoluteGrid] uses to place pins, so a hole
/// always sits at the gap-centred intersection between two adjacent
/// cell slots. No independent magic-number spacing.
class PegboardDotsPainter extends CustomPainter {
  PegboardDotsPainter({
    required this.hole,
    required this.holeDeep,
    required this.showHoles,
  });

  final Color hole;
  final Color holeDeep;
  final bool showHoles;

  /// Horizontal dot pitch — one pin cell width plus one gap. Derived
  /// (not hardcoded) from [BoardGridMetrics] so the hole grid always
  /// lines up with the columns the grid positions pins into.
  static const double pitchX =
      BoardGridMetrics.cellWidth + BoardGridMetrics.gap;

  /// Vertical dot pitch — one pin cell height plus one gap. Derived from
  /// [BoardGridMetrics] so the hole grid always lines up with the rows
  /// the grid positions pins into.
  static const double pitchY =
      BoardGridMetrics.cellHeight + BoardGridMetrics.gap;

  /// Half a cell-gap — the offset from a cell-slot boundary to the hole
  /// that sits centred on it.
  static const double _holeInset = BoardGridMetrics.gap / 2;

  static const double _highlightRadius = 1.0;
  static const double _shadowRadius = 0.5;
  static const Offset _shadowOffset = Offset(1.0, 1.0);

  /// x-position of the k-th vertical hole line (k = 0..cols). Sits at
  /// the gap-centred intersection between cell `k-1` and cell `k`, so
  /// `holeX(0)` lands half a gap to the left of the first cell's left
  /// edge — the outer corner of the (0, *) cell slots.
  static double holeX(int k) => k * pitchX - _holeInset;

  /// y-position of the k-th horizontal hole line (k = 0..rows). See
  /// [holeX].
  static double holeY(int k) => k * pitchY - _holeInset;

  @override
  void paint(Canvas canvas, Size size) {
    if (!showHoles) return;
    final highlightPaint = Paint()..color = hole;
    final shadowPaint = Paint()..color = holeDeep;
    for (double y = -_holeInset; y <= size.height + _holeInset; y += pitchY) {
      for (double x = -_holeInset; x <= size.width + _holeInset; x += pitchX) {
        // Mirrors the two stacked radial-gradients in input.css:
        // a light dot on the top-left, a darker dot offset by 1px so
        // the hole looks recessed.
        canvas.drawCircle(Offset(x, y), _highlightRadius, highlightPaint);
        canvas.drawCircle(
          Offset(x, y) + _shadowOffset,
          _shadowRadius,
          shadowPaint,
        );
      }
    }
  }

  @override
  bool shouldRepaint(covariant PegboardDotsPainter old) =>
      old.hole != hole ||
      old.holeDeep != holeDeep ||
      old.showHoles != showHoles;
}
