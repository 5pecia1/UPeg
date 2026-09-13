/// Pin resize affordances: the SE-corner drag handle and the target-span
/// rect it previews. Both drive the SAME resize-mode state machine as the
/// keyboard `e` flow, so clamping and the commit decision exist once.
library;

import 'package:flutter/gestures.dart'
    show GestureDisposition, PanGestureRecognizer;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';
import 'package:upeg/src/widgets/board_canvas/drop_targets.dart';
import 'package:upeg/src/widgets/pin.dart' show pinResizeHandleKey;

/// Target-span rect painted while resize-mode is Active. Reuses the
/// drop-highlight visual language (accent border+fill, warn when the
/// projected commit displaces a neighbour) and the same `dropVerdict`
/// displacement check, so drag-drop and resize read as one system.
class ResizeSpanHighlight extends StatelessWidget {
  const ResizeSpanHighlight({
    required this.anchor,
    required this.activeResize,
    required this.previewPlacements,
    required this.currentPlacements,
    super.key,
  });

  /// The resized pin's CURRENT placement — the span grows from its
  /// anchor cell, mirroring the FRB commit semantics.
  final PlacementDto anchor;
  final ResizeModeActive activeResize;
  final List<PlacementDto> previewPlacements;
  final List<PlacementDto> currentPlacements;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final verdict = dropVerdict(
      previewPlacements,
      activeResize.toolId,
      currentPlacements,
    );
    final color = verdict == DropVerdict.push ? tokens.warn : tokens.accent;
    return Positioned(
      left: BoardGridMetrics.pixelX(anchor.x),
      top: BoardGridMetrics.pixelY(anchor.y),
      width: BoardGridMetrics.pixelW(activeResize.currentCols),
      height: BoardGridMetrics.pixelH(activeResize.currentRows),
      child: IgnorePointer(
        key: const Key('board-canvas-resize-highlight'),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: color.withValues(alpha: dropHighlightFillAlpha),
            border: Border.all(color: color, width: dropHighlightBorderWidth),
            borderRadius: BorderRadius.circular(UpegSizing.radius2),
          ),
        ),
      ),
    );
  }
}

/// Inset of the SE-corner resize handle from the pin's corner.
const double resizeHandleInset = 3.0;

/// Icon size of the SE-corner resize handle (matches the move handle).
const double _resizeHandleIconSize = 13.0;

/// SE-corner resize drag affordance. Dragging snaps the pin's span to
/// whole cells by driving the SAME resize-mode state machine as the
/// keyboard `e` flow — so the canvas preview overlay, the clamping and
/// the commit decision (`setSpan` vs `clearSpan` vs no-op) exist
/// exactly once, and Esc cancels an in-flight drag for free.
///
/// Positioned ABOVE the pin (a later Stack sibling), so it receives
/// pointer events even when the pin body owns gestures (embed webview /
/// memo field) — same reasoning as the injected move handle.
class ResizeHandle extends ConsumerStatefulWidget {
  const ResizeHandle({
    required this.placement,
    required this.boardKey,
    required this.maxCols,
    required this.manifestCols,
    required this.manifestRows,
    super.key,
  });

  final PlacementDto placement;
  final BoardKey boardKey;

  /// Board column count — the span's horizontal clamp bound.
  final int maxCols;

  /// Manifest footprint (from the tool's `pegboardUnits`) — what a
  /// commit at this size collapses to (`clearSpan`).
  final int manifestCols;
  final int manifestRows;

  @override
  ConsumerState<ResizeHandle> createState() => _ResizeHandleState();
}

class _ResizeHandleState extends ConsumerState<ResizeHandle> {
  /// Accumulated pan delta since pan-start. Snapshot-based (never a
  /// re-read of ambient drag state — feedback_dragend_race.md).
  Offset _panAccum = Offset.zero;

  /// Cell step per axis — one cell plus one gap, derived from the same
  /// constants `AbsoluteGrid` positions pins with (no re-derived
  /// magic numbers).
  static const double _colStep =
      BoardGridMetrics.cellWidth + BoardGridMetrics.gap;
  static const double _rowStep =
      BoardGridMetrics.cellHeight + BoardGridMetrics.gap;

  void _onPanStart(DragStartDetails details) {
    _panAccum = Offset.zero;
    final container = ProviderScope.containerOf(context);
    // Mutual exclusion, mirroring keyboard StartResize: a drag-resize
    // cancels a stale keyboard move-mode.
    container.read(moveModeProvider.notifier).cancel();
    container
        .read(resizeModeProvider.notifier)
        .start(
          boardKey: widget.boardKey,
          toolId: ToolId.parse(widget.placement.toolId),
          baseCols: widget.placement.w,
          baseRows: widget.placement.h,
          manifestCols: widget.manifestCols,
          manifestRows: widget.manifestRows,
          maxCols: widget.maxCols,
        );
  }

  void _onPanUpdate(DragUpdateDetails details) {
    _panAccum += details.delta;
    final state = ref.read(resizeModeProvider);
    // Idle here means the drag was cancelled out-of-band (Esc) — stop
    // driving the preview; pan-end will find nothing to commit.
    if (state is! ResizeModeActive) return;
    // Whole-cell snap: target span = span at pan-start + rounded cell
    // delta; the notifier clamps to 1..maxCols / >= 1.
    final targetCols = widget.placement.w + (_panAccum.dx / _colStep).round();
    final targetRows = widget.placement.h + (_panAccum.dy / _rowStep).round();
    if (targetCols == state.currentCols && targetRows == state.currentRows) {
      return;
    }
    ref
        .read(resizeModeProvider.notifier)
        .resizeBy(
          dCols: targetCols - state.currentCols,
          dRows: targetRows - state.currentRows,
        );
  }

  void _onPanEnd(DragEndDetails details) {
    // Same commit seam as keyboard Enter — one decision path.
    commitResizeMode(ProviderScope.containerOf(context));
  }

  void _onPanCancel() {
    ref.read(resizeModeProvider.notifier).cancel();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return MouseRegion(
      cursor: SystemMouseCursors.resizeDownRight,
      child: RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          // Eager pan: the handle sits inside the board's scroll views,
          // whose drag recognizers would otherwise win the gesture
          // arena (they have a smaller slop than pan). The handle is a
          // dedicated affordance, so claiming the pointer on DOWN is
          // correct — the same reason `Draggable` uses an immediate
          // recognizer for pin moves.
          _EagerPanGestureRecognizer:
              GestureRecognizerFactoryWithHandlers<_EagerPanGestureRecognizer>(
                _EagerPanGestureRecognizer.new,
                (recognizer) => recognizer
                  ..onStart = _onPanStart
                  ..onUpdate = _onPanUpdate
                  ..onEnd = _onPanEnd
                  ..onCancel = _onPanCancel,
              ),
        },
        child: Tooltip(
          message: t(ref, 'pin.resize.handle_tooltip'),
          child: Icon(
            Icons.south_east,
            key: pinResizeHandleKey,
            size: _resizeHandleIconSize,
            color: tokens.accent,
          ),
        ),
      ),
    );
  }
}

/// Pan recognizer that claims the pointer on DOWN instead of waiting
/// for the pan slop — see the `ResizeHandle` gesture doc.
class _EagerPanGestureRecognizer extends PanGestureRecognizer {
  @override
  void addAllowedPointer(PointerDownEvent event) {
    super.addAllowedPointer(event);
    resolve(GestureDisposition.accepted);
  }
}
