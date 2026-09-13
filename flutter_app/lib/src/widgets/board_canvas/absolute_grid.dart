/// Stack-based absolute grid: the layer that turns a layout snapshot
/// into positioned drop cells, pins, and preview overlays.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/state/resize_mode_provider.dart';
import 'package:upeg/src/state/resize_preview_provider.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';
import 'package:upeg/src/widgets/board_canvas/drop_targets.dart';
import 'package:upeg/src/widgets/board_canvas/pin_drag.dart';
import 'package:upeg/src/widgets/board_canvas/pin_with_label.dart';
import 'package:upeg/src/widgets/board_canvas/push_preview_overlay.dart';
import 'package:upeg/src/widgets/board_canvas/resize_affordance.dart';
import 'package:upeg/src/widgets/pin.dart' show PinTapCallback;

/// Upper bound for rendered rows/columns. This keeps accidental corrupted
/// board metadata from creating an unbounded grid.
const int _maxGridCellsPerAxis = 1 << 16;

/// Maximum number of rows beyond the highest placement that can be rendered
/// during an active drag. Caps the dynamic expansion to prevent unbounded
/// re-rendering if the user drags far past the bottom.
const int _maxRenderedDragRows = 64;

/// Stack-based pegboard grid keyed on `(placement.x, placement.y)`.
///
/// Rendered as `BOARD_COLS * rows` `DragTarget<PinDragPayload>` cells
/// stacked under the pins; dropping a `Draggable` onto a target dispatches
/// the move to Rust via [onMovePin] using row-major push semantics.
class AbsoluteGrid extends ConsumerStatefulWidget {
  const AbsoluteGrid({
    required this.placements,
    required this.columns,
    required this.rows,
    required this.boardKey,
    required this.onPinTap,
    required this.onMovePin,
    this.onEditPinColor,
    this.onOpenModal,
    this.verticalScrollController,
    super.key,
  });

  final List<PlacementDto> placements;
  final int columns;
  final int rows;
  final BoardKey boardKey;
  final PinTapCallback onPinTap;
  final PinTapCallback? onEditPinColor;
  final PinTapCallback? onOpenModal;
  final MovePinCallback? onMovePin;

  /// Scroll controller for the vertical scroll view that wraps this grid.
  /// When provided, drag operations near the viewport edges trigger
  /// autoscroll so newly-expanded drop targets become reachable.
  final ScrollController? verticalScrollController;

  @override
  ConsumerState<AbsoluteGrid> createState() => AbsoluteGridState();
}

class AbsoluteGridState extends ConsumerState<AbsoluteGrid> {
  final GlobalKey _gridKey = GlobalKey();

  /// Imperative handle onto the drop-target highlight layer (see
  /// `DropHighlightLayer`). `PinWithLabel._onDragUpdate` updates it
  /// DIRECTLY through this key rather than lifting hover state up into
  /// `AbsoluteGridState` — an update here would otherwise trigger a
  /// full `AbsoluteGridState.build()` (every `DropCell` + pin)
  /// PLUS `_expandForHoveredRow`'s OWN rebuild on nearly every pointer
  /// move, which was empirically found to desync the row-expansion
  /// lookahead math (regression in `board_canvas_test.dart`'s
  /// drag-expansion cases). Isolating hover state to its own small
  /// widget keeps a hover-only update from touching anything else.
  final GlobalKey<DropHighlightLayerState> _highlightKey = GlobalKey();

  int _extraRenderedRows = 0;

  void _onZoneEnter() {
    // Called when a dragged pin enters the invisible expansion zone
    // below the rendered grid. Expands using the row just below the
    // last rendered row so subsequent pointer movement can land on
    // newly rendered `DropCell` widgets.
    final totalRows = widget.rows + _extraRenderedRows;
    _expandForHoveredRow(totalRows);
  }

  void _expandForHoveredRow(int row) {
    final baseRows = widget.rows;
    final maxRows = maxPlacementRow(widget.placements) + _maxRenderedDragRows;
    final requestedRows = row + 1 + dragExpansionBufferRows;
    final nextRows = BoardGridMetrics.clampCount(
      requestedRows,
      min: baseRows,
      max: maxRows,
    );
    final nextExtraRenderedRows = nextRows - baseRows;
    if (nextExtraRenderedRows <= _extraRenderedRows) return;

    setState(() {
      _extraRenderedRows = nextExtraRenderedRows;
    });
  }

  void _resetDragExpansion() {
    _highlightKey.currentState?.clearHover();
    if (_extraRenderedRows == 0) return;
    setState(() {
      _extraRenderedRows = 0;
    });
  }

  @override
  void didUpdateWidget(covariant AbsoluteGrid oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.rows != oldWidget.rows ||
        widget.boardKey != oldWidget.boardKey ||
        widget.placements != oldWidget.placements) {
      _extraRenderedRows = 0;
    }
  }

  @override
  Widget build(BuildContext context) {
    final totalRows = BoardGridMetrics.clampCount(
      widget.rows + _extraRenderedRows,
      min: 1,
      max: _maxGridCellsPerAxis,
    );
    final totalColumns = BoardGridMetrics.clampCount(
      widget.columns,
      min: 1,
      max: _maxGridCellsPerAxis,
    );
    final width = BoardGridMetrics.spanPixels(
      totalColumns,
      BoardGridMetrics.cellWidth,
    );
    final height = BoardGridMetrics.spanPixels(
      totalRows,
      BoardGridMetrics.cellHeight,
    );
    // F16 — push preview overlay: while move-mode is Active, ask the
    // FRB layer what the layout WOULD look like if the user committed
    // at currentX/currentY, and paint a ghost label per displaced
    // placement. The overlay mounts inside the same Stack so it lines
    // up with the existing absolute grid; the wrapper key
    // `board-canvas-push-preview` is the documented contract for the
    // F16 widget test.
    final moveMode = ref.watch(moveModeProvider);
    // Capture the promoted variant in a single typed local so the
    // overlay branch below can read `activeMove.toolId` without a
    // bare `as MoveModeActive` cast. The relationship "preview is
    // non-empty ⇒ move is active" is now compile-time enforced via
    // `PushPreviewOverlay.activeMove: MoveModeActive`.
    final MoveModeActive? activeMove = moveMode is MoveModeActive
        ? moveMode
        : null;
    final MoveModeActive? activeMoveForBoard =
        activeMove?.boardKey == widget.boardKey ? activeMove : null;
    final List<PlacementDto> previewPlacements;
    if (activeMoveForBoard != null) {
      final load = ref.watch(pushPreviewLoaderProvider);
      previewPlacements = load(
        widget.boardKey,
        activeMoveForBoard.toolId,
        activeMoveForBoard.currentX,
        activeMoveForBoard.currentY,
      );
    } else {
      previewPlacements = const <PlacementDto>[];
    }
    // Pin-resize preview: while resize-mode is Active (keyboard `e` or
    // the SE-corner drag handle — both drive the SAME state machine so
    // this is the single preview path), ask the FRB layer what the
    // layout WOULD look like if the span were committed. Empty preview
    // == invalid span → hide the overlay (same contract as F16).
    final resizeMode = ref.watch(resizeModeProvider);
    final ResizeModeActive? activeResize = resizeMode is ResizeModeActive
        ? resizeMode
        : null;
    final ResizeModeActive? activeResizeForBoard =
        activeResize?.boardKey == widget.boardKey ? activeResize : null;
    final List<PlacementDto> resizePreviewPlacements;
    if (activeResizeForBoard != null) {
      final load = ref.watch(resizePreviewLoaderProvider);
      resizePreviewPlacements = load(
        widget.boardKey,
        activeResizeForBoard.toolId,
        activeResizeForBoard.currentCols,
        activeResizeForBoard.currentRows,
      );
    } else {
      resizePreviewPlacements = const <PlacementDto>[];
    }
    final PlacementDto? resizeAnchor = activeResizeForBoard == null
        ? null
        : findPlacementByToolId(widget.placements, activeResizeForBoard.toolId);
    return KeyedSubtree(
      key: _gridKey,
      child: SizedBox(
        key: const Key('board-canvas-absolute-grid'),
        width: width,
        height: height,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            // Bottom layer: empty drop-target cells, one per (col, row).
            for (int row = 0; row < totalRows; row++)
              for (int col = 0; col < totalColumns; col++)
                Positioned(
                  left: BoardGridMetrics.pixelX(col),
                  top: BoardGridMetrics.pixelY(row),
                  width: BoardGridMetrics.cellWidth,
                  height: BoardGridMetrics.cellHeight,
                  child: DropCell(
                    col: col,
                    row: row,
                    boardKey: widget.boardKey,
                    onMovePin: widget.onMovePin,
                    onHoverRow: _expandForHoveredRow,
                    onDropFinished: _resetDragExpansion,
                  ),
                ),
            // Invisible drag-expansion zone below the rendered grid. When a
            // dragged pin enters this zone, the grid grows downward so that
            // subsequent pointer movement hits newly rendered drop cells.
            // Modeless: a pin is always draggable, so the zone is always
            // active.
            DragExpansionZone(
              gridTopRows: totalRows,
              columns: widget.columns,
              onEnterZone: _onZoneEnter,
            ),
            // Top layer: each placement positioned by `(x, y)` and sized
            // by `(w, h)` so multi-unit pins span correctly.
            for (final placement in widget.placements)
              Positioned(
                key: ValueKey<PinKey>((
                  widget.boardKey,
                  ToolId.parse(placement.toolId),
                )),
                left: BoardGridMetrics.pixelX(placement.x),
                top: BoardGridMetrics.pixelY(placement.y),
                width: BoardGridMetrics.pixelW(placement.w),
                height: BoardGridMetrics.pixelH(placement.h),
                child: PinWithLabel(
                  placement: placement,
                  boardKey: widget.boardKey,
                  columns: widget.columns,
                  onTap: widget.onPinTap,
                  onEditColor: widget.onEditPinColor,
                  onOpenModal: widget.onOpenModal,
                  gridKey: _gridKey,
                  onDragFinished: _resetDragExpansion,
                  onDragRowUpdate: _expandForHoveredRow,
                  highlightKey: _highlightKey,
                  verticalScrollController: widget.verticalScrollController,
                ),
              ),
            // Drop-target hover highlight — sits above the pins so it
            // stays visible even when hovering over an occupied cell.
            // `IgnorePointer` keeps it from stealing the drag's hit
            // testing from the `DropCell`s underneath. Its own
            // `ConsumerState` isolates hover-only rebuilds from the rest
            // of this grid — see the doc on `_highlightKey`.
            DropHighlightLayer(
              key: _highlightKey,
              boardKey: widget.boardKey,
              placements: widget.placements,
            ),
            // F16 — preview overlay layer. Only mounts when move-mode is
            // Active AND the FRB returned a non-empty projection. The
            // typed `activeMove` parameter makes the precondition
            // unforgeable at compile time.
            if (activeMoveForBoard != null && previewPlacements.isNotEmpty)
              Positioned.fill(
                key: const Key('board-canvas-push-preview'),
                child: PushPreviewOverlay(
                  previewPlacements: previewPlacements,
                  activeToolId: activeMoveForBoard.toolId,
                ),
              ),
            // Pin-resize preview overlay: the target span rect at the
            // pin's current anchor (accent, or warn when committing
            // would push a neighbour), plus the same displaced-pin
            // labels move-mode paints. Only mounts when the FRB
            // projection is non-empty (empty == invalid span).
            if (activeResizeForBoard != null &&
                resizeAnchor != null &&
                resizePreviewPlacements.isNotEmpty) ...[
              ResizeSpanHighlight(
                anchor: resizeAnchor,
                activeResize: activeResizeForBoard,
                previewPlacements: resizePreviewPlacements,
                currentPlacements: widget.placements,
              ),
              Positioned.fill(
                key: const Key('board-canvas-resize-preview'),
                child: PushPreviewOverlay(
                  previewPlacements: resizePreviewPlacements,
                  activeToolId: activeResizeForBoard.toolId,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
