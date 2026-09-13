/// Drop-target layer of the board canvas: the invisible cells a dragged
/// pin can land on, the downward expansion zone below the rendered grid,
/// and the accept/push/reject hover highlight painted above the pins.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';
import 'package:upeg/src/widgets/board_canvas/pin_drag.dart';

/// Height, in pixel rows, of the invisible drag-expansion zone rendered
/// below the bottom of the currently visible grid. When a dragged pin
/// enters this zone, the grid expands downward so that subsequent pointer
/// movement lands on newly rendered `DropCell` widgets.
const int _dragExpansionTriggerRows = 3;

class DragExpansionZone extends StatelessWidget {
  const DragExpansionZone({
    required this.gridTopRows,
    required this.columns,
    required this.onEnterZone,
    super.key,
  });

  /// Number of rows already rendered above this zone.
  final int gridTopRows;
  final int columns;
  final VoidCallback onEnterZone;

  @override
  Widget build(BuildContext context) {
    final cellH = BoardGridMetrics.cellHeight;
    final gap = UpegSizing.pinGap;
    final rowSize = cellH + gap;
    final top = BoardGridMetrics.spanPixels(gridTopRows, cellH);
    final height = _dragExpansionTriggerRows * rowSize;

    return Positioned(
      left: 0,
      top: top,
      width: BoardGridMetrics.spanPixels(columns, BoardGridMetrics.cellWidth),
      height: height,
      child: DragTarget<PinDragPayload>(
        onWillAcceptWithDetails: (_) {
          onEnterZone();
          return true;
        },
        onAcceptWithDetails: (_) {
          // Zone is hover-trigger only; actual drop commit happens via
          // DropCell. If the user drops while still on the zone, expand
          // one more row so the commit has a cell to land on.
          onEnterZone();
        },
        builder: (context, candidateData, _) {
          return const SizedBox.expand();
        },
      ),
    );
  }
}

/// Fallback span (in cells) for the drop-target hover highlight when the
/// dragged tool isn't found among the current placements (shouldn't
/// happen in practice — the payload always originates from a rendered
/// pin — but the spec calls for an explicit 1×1 fallback rather than a
/// crash).
const int _dropHighlightFallbackSpan = 1;

/// Fill alpha for the drop-target hover highlight (accent/warn tint).
const double dropHighlightFillAlpha = 0.18;

/// Border width for the drop-target hover highlight.
const double dropHighlightBorderWidth = 2.0;

/// (w, h) span, in cells, that the drop-target hover highlight should
/// cover — the dragged pin's real footprint, or [_dropHighlightFallbackSpan]
/// if the dragged tool can't be found among the current placements.
({int w, int h}) _dropHighlightSpan(PlacementDto? draggedPlacement) => (
  w: draggedPlacement?.w ?? _dropHighlightFallbackSpan,
  h: draggedPlacement?.h ?? _dropHighlightFallbackSpan,
);

/// Drop-target verdict for the hover highlight: [reject] when the FRB
/// preview came back empty (anchor out of range / unknown tool — see
/// `pushPreviewLoaderProvider` doc), [push] when committing here would
/// displace some OTHER placement's (x, y), [accept] otherwise.
enum DropVerdict { accept, push, reject }

DropVerdict dropVerdict(
  List<PlacementDto> preview,
  ToolId draggedToolId,
  List<PlacementDto> current,
) {
  if (preview.isEmpty) return DropVerdict.reject;
  final currentByToolId = {for (final p in current) p.toolId: p};
  for (final previewPlacement in preview) {
    if (previewPlacement.toolId == draggedToolId.value) continue;
    final original = currentByToolId[previewPlacement.toolId];
    if (original == null ||
        original.x != previewPlacement.x ||
        original.y != previewPlacement.y) {
      return DropVerdict.push;
    }
  }
  return DropVerdict.accept;
}

/// Drop-target hover highlight layer. Owns its OWN small piece of state
/// (the hovered cell + memoized push-preview projection) instead of
/// living on `AbsoluteGridState`, and is updated IMPERATIVELY via
/// [AbsoluteGridState._highlightKey] from `PinWithLabel._onDragUpdate`
/// rather than through a prop passed down and lifted back up. This
/// isolation is load-bearing, not a style choice: an update here only
/// rebuilds this one small widget, instead of `AbsoluteGridState`'s
/// entire `Stack` (every `DropCell` + pin) on nearly every pointer
/// move — which was empirically found to desync the row-expansion
/// lookahead math driven by the SAME `_onDragUpdate` dispatch
/// (regression across three `board_canvas_test.dart` drag-expansion
/// cases before this was split out).
class DropHighlightLayer extends ConsumerStatefulWidget {
  const DropHighlightLayer({
    required this.boardKey,
    required this.placements,
    super.key,
  });

  final BoardKey boardKey;
  final List<PlacementDto> placements;

  @override
  ConsumerState<DropHighlightLayer> createState() => DropHighlightLayerState();
}

class DropHighlightLayerState extends ConsumerState<DropHighlightLayer> {
  int? _hoverCol;
  int? _hoverRow;
  PinDragPayload? _hoverData;
  List<PlacementDto>? _hoverPreview;

  /// Called by `PinWithLabel._onDragUpdate` with the cell currently
  /// under the drag pointer. Memoized: recomputes the push-preview
  /// projection only when the (col, row, tool) anchor actually changes,
  /// per the drag-race note in feedback_dragend_race.md — [data] is a
  /// snapshot built at the call site, never a re-read of ambient drag
  /// state.
  void updateHover(int col, int row, PinDragPayload data) {
    if (_hoverCol == col &&
        _hoverRow == row &&
        _hoverData?.toolId == data.toolId) {
      return;
    }
    final load = ref.read(pushPreviewLoaderProvider);
    final preview = load(widget.boardKey, data.toolId, col, row);
    setState(() {
      _hoverCol = col;
      _hoverRow = row;
      _hoverData = data;
      _hoverPreview = preview;
    });
  }

  /// Called when the pointer moves outside the addressable grid during
  /// drag, or the drag ends/cancels — there's no cell to highlight.
  void clearHover() {
    if (_hoverCol == null && _hoverData == null) return;
    setState(() {
      _hoverCol = null;
      _hoverRow = null;
      _hoverData = null;
      _hoverPreview = null;
    });
  }

  @override
  void didUpdateWidget(covariant DropHighlightLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    // A fresh placements/board snapshot invalidates any memoized
    // preview computed against the old layout.
    if (widget.boardKey != oldWidget.boardKey ||
        widget.placements != oldWidget.placements) {
      _hoverCol = null;
      _hoverRow = null;
      _hoverData = null;
      _hoverPreview = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final hoverCol = _hoverCol;
    final hoverRow = _hoverRow;
    final hoverData = _hoverData;
    final hoverPreview = _hoverPreview;
    if (hoverCol == null ||
        hoverRow == null ||
        hoverData == null ||
        hoverPreview == null) {
      return const SizedBox.shrink();
    }
    final tokens = context.upeg;
    final draggedPlacement = findPlacementByToolId(
      widget.placements,
      hoverData.toolId,
    );
    final span = _dropHighlightSpan(draggedPlacement);
    final verdict = dropVerdict(
      hoverPreview,
      hoverData.toolId,
      widget.placements,
    );
    final color = verdict == DropVerdict.accept ? tokens.accent : tokens.warn;
    return Positioned(
      left: BoardGridMetrics.pixelX(hoverCol),
      top: BoardGridMetrics.pixelY(hoverRow),
      width: BoardGridMetrics.pixelW(span.w),
      height: BoardGridMetrics.pixelH(span.h),
      child: IgnorePointer(
        key: const Key('board-canvas-drop-highlight'),
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

/// Border width for the empty-cell mouse-hover affordance.
const double _cellHoverBorderWidth = 1.0;

/// One absolute-positioned drop target cell. Accepts the payload of any
/// dragged pin and, when [AbsoluteGrid] supplies an [onMovePin], commits
/// the move to Rust on drop. Also shows a soft border on plain mouse
/// hover (no active drag) so an empty cell's boundary reads as a target
/// even before a drag starts.
///
/// The drop-target HIGHLIGHT overlay (accept/push/reject) is NOT driven
/// from this widget's `DragTarget` callbacks — see
/// `PinWithLabel._onDragUpdate` for why: `onWillAcceptWithDetails`/
/// `onLeave` fire synchronously from the SAME dispatch as the dragged
/// pin's own `onDragUpdate` (used for row-expansion lookahead), and
/// doing Riverpod/`setState` work from inside that dispatch — even
/// deferred to a microtask or post-frame callback — was empirically
/// found to desync the lookahead row math (regression across three
/// `board_canvas_test.dart` drag-expansion cases). `_onDragUpdate`
/// already computes the pointer's cell synchronously and reliably (it's
/// the same call the lookahead expansion relies on), so the highlight
/// reuses THAT single dispatch point instead of racing a second one.
class DropCell extends StatefulWidget {
  const DropCell({
    required this.col,
    required this.row,
    required this.boardKey,
    required this.onMovePin,
    required this.onHoverRow,
    required this.onDropFinished,
    super.key,
  });

  final int col;
  final int row;
  final BoardKey boardKey;
  final MovePinCallback? onMovePin;
  final ValueChanged<int> onHoverRow;
  final VoidCallback onDropFinished;

  @override
  State<DropCell> createState() => _DropCellState();
}

class _DropCellState extends State<DropCell> {
  bool _mouseHover = false;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return MouseRegion(
      onEnter: (_) => setState(() => _mouseHover = true),
      onExit: (_) => setState(() => _mouseHover = false),
      child: DragTarget<PinDragPayload>(
        onWillAcceptWithDetails: (_) {
          widget.onHoverRow(widget.row);
          return true;
        },
        onAcceptWithDetails: (details) {
          final data = details.data;
          // Capture tool id locally BEFORE any async hop. Mirrors the
          // dragend-race snapshot pattern documented in
          // feedback_dragend_race.md — the original payload reference can
          // be torn down by the Draggable host as soon as the gesture
          // arena resolves.
          final toolId = data.toolId;
          final movePin = widget.onMovePin;
          if (movePin != null) {
            movePin(
              boardKey: widget.boardKey,
              toolId: toolId,
              anchorX: widget.col,
              anchorY: widget.row,
            );
          }
          widget.onDropFinished();
        },
        builder: (context, candidateData, _) {
          // The drag-candidate case is already covered by the
          // drop-highlight overlay in AbsoluteGrid; only show the plain
          // hover border when the mouse is over the cell WITHOUT an
          // active drag candidate, so the two affordances never overlap.
          if (_mouseHover && candidateData.isEmpty) {
            return DecoratedBox(
              key: const Key('board-canvas-cell-hover'),
              decoration: BoxDecoration(
                border: Border.all(
                  color: tokens.lineSoft,
                  width: _cellHoverBorderWidth,
                ),
                borderRadius: BorderRadius.circular(UpegSizing.radius2),
              ),
            );
          }
          return const SizedBox.expand();
        },
      ),
    );
  }
}
