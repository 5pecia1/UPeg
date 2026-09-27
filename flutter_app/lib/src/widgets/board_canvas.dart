/// Renders the placements for one board.
///
/// Pegboard contract:
///   * peg-hole grid background,
///   * canonical [`UpegSizing.pinCellWidth`] columns laid out via an
///     absolute-position `Stack` so a stored `(x, y)` maps to the same
///     cell across Desktop / PWA / TUI surfaces,
///   * `EmptyBoardCard` shown when no placements exist,
///   * `Draggable<PinDragPayload>` + `DragTarget` cells over the grid
///     so pin reorder happens by drag, with row-major push commit
///     persisted via the FRB `move_pin` call.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas/absolute_grid.dart';
import 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart';
import 'package:upeg/src/widgets/board_canvas/pegboard_dots.dart';
import 'package:upeg/src/widgets/board_canvas/pin_drag.dart';
import 'package:upeg/src/widgets/empty_board.dart';
import 'package:upeg/src/widgets/pin.dart';

/// The canvas is assembled from `widgets/board_canvas/`; these are the
/// parts callers of `BoardCanvas` legitimately name (geometry constants
/// for layout assertions, the drag payload, the peg-hole painter).
export 'package:upeg/src/widgets/board_canvas/board_grid_metrics.dart'
    show BoardGridMetrics, boardCanvasPinCellHeight, scrollOffsetToReveal;
export 'package:upeg/src/widgets/board_canvas/pegboard_dots.dart'
    show BoardCanvasPegboardForTesting, PegboardDotsPainter;
export 'package:upeg/src/widgets/board_canvas/pin_drag.dart';

/// Empty-state copy. Extracted as a constant so widget tests can assert
/// against it without coupling to the wording.
const String emptyBoardHint = 'no pins on this board yet';

class BoardCanvas extends ConsumerWidget {
  const BoardCanvas({
    required this.boardKey,
    required this.onPinTap,
    this.onOpenPalette,
    this.onEditPinColor,
    this.onOpenModal,
    super.key,
  });

  final BoardKey boardKey;
  final PinTapCallback onPinTap;
  final PinTapCallback? onEditPinColor;

  /// Fired from a pin's "open" context-menu entry (right-click / long-press)
  /// to open the expanded modal — the mouse/touch mirror of keyboard `o`.
  final PinTapCallback? onOpenModal;

  /// Optional CTA invoked from the empty-board card. When `null` the
  /// empty-board card falls back to the lightweight [emptyBoardHint]
  /// text so tests that don't care about onboarding can omit it.
  final OpenPaletteCallback? onOpenPalette;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selectedTag = ref.watch(selectedTagProvider);
    final layout = ref.watch(
      layoutProvider(LayoutQuery(boardKey: boardKey, tag: selectedTag)),
    );
    return layout.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (err, _) => Center(
        child: Text(
          'failed to load layout: $err',
          style: TextStyle(color: Theme.of(context).colorScheme.error),
        ),
      ),
      data: (snapshot) => _BoardCanvasGrid(
        snapshot: snapshot,
        boardKey: boardKey,
        onPinTap: onPinTap,
        onEditPinColor: onEditPinColor,
        onOpenModal: onOpenModal,
        onOpenPalette: onOpenPalette,
        onMovePin:
            ({
              required boardKey,
              required pinId,
              required anchorX,
              required anchorY,
            }) {
              unawaited(
                ref
                    .read(pegboardMutationsProvider)
                    .move(
                      (boardKey, pinId),
                      anchorX: anchorX,
                      anchorY: anchorY,
                    ),
              );
            },
      ),
    );
  }
}

/// Pure render-from-snapshot, factored out so widget tests can drive it
/// directly with a fixture without poking the Rust dylib.
class _BoardCanvasGrid extends ConsumerStatefulWidget {
  const _BoardCanvasGrid({
    required this.snapshot,
    required this.onPinTap,
    this.onEditPinColor,
    this.onOpenModal,
    this.boardKey,
    this.onOpenPalette,
    this.onMovePin,
  });

  final LayoutSnapshotDto snapshot;
  final PinTapCallback onPinTap;
  final PinTapCallback? onEditPinColor;

  /// Forwarded to each [Pin] as the "open modal" context-menu handler.
  final PinTapCallback? onOpenModal;

  /// Forwarded to [EmptyBoard] so the suggestion list can derive
  /// per-board picks. Optional — `null` collapses to the simple
  /// "no pins on this board yet" text path.
  final BoardKey? boardKey;

  final OpenPaletteCallback? onOpenPalette;

  /// Optional move callback. When `null`, drag-targets still accept
  /// the gesture but commit nothing — tests use this to verify the
  /// drop path fires the right `(anchor_x, anchor_y)` without depending
  /// on the Rust dylib.
  final MovePinCallback? onMovePin;

  @override
  ConsumerState<_BoardCanvasGrid> createState() => _BoardCanvasGridState();
}

class _BoardCanvasGridState extends ConsumerState<_BoardCanvasGrid> {
  late final ScrollController _verticalScrollController;
  late final ScrollController _horizontalScrollController;

  @override
  void initState() {
    super.initState();
    _verticalScrollController = ScrollController();
    _horizontalScrollController = ScrollController();
    // A focused pin may already exist when the grid mounts (palette
    // closed, board switched back) — restore its visibility too.
    _scheduleRevealFocusedPin();
  }

  @override
  void didUpdateWidget(covariant _BoardCanvasGrid oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Board/tag switch delivers a new snapshot while the focused pin id
    // is restored out-of-band; re-reveal against the fresh layout.
    if (widget.snapshot != oldWidget.snapshot) _scheduleRevealFocusedPin();
  }

  @override
  void dispose() {
    _verticalScrollController.dispose();
    _horizontalScrollController.dispose();
    super.dispose();
  }

  /// Keyboard focus follow (arrows / hjkl / Tab): after the frame that
  /// applied the focus change, scroll both axes just enough that the
  /// focused pin's cell rect is fully inside the viewport. Fully visible
  /// pins cause no movement (no jump), clipped pins move minimally.
  void _scheduleRevealFocusedPin() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _revealFocusedPin();
    });
  }

  void _revealFocusedPin() {
    final focused = ref.read(focusedPinProvider);
    if (focused == null) return;
    PlacementDto? placement;
    for (final candidate in widget.snapshot.placements) {
      if (candidate.pinId == focused.value) {
        placement = candidate;
        break;
      }
    }
    if (placement == null) return;
    // Scroll-content coordinates: the padded grid sits `pegboardPadding`
    // in from the scroll origin on both axes.
    const inset = UpegSizing.pegboardPadding;
    final left = inset + BoardGridMetrics.pixelX(placement.x);
    final top = inset + BoardGridMetrics.pixelY(placement.y);
    _revealAxis(
      _horizontalScrollController,
      targetStart: left,
      targetEnd: left + BoardGridMetrics.pixelW(placement.w),
    );
    _revealAxis(
      _verticalScrollController,
      targetStart: top,
      targetEnd: top + BoardGridMetrics.pixelH(placement.h),
    );
  }

  void _revealAxis(
    ScrollController controller, {
    required double targetStart,
    required double targetEnd,
  }) {
    if (!controller.hasClients) return;
    final position = controller.position;
    final target = scrollOffsetToReveal(
      currentOffset: position.pixels,
      viewportExtent: position.viewportDimension,
      targetStart: targetStart,
      targetEnd: targetEnd,
    ).clamp(position.minScrollExtent, position.maxScrollExtent);
    if (target == position.pixels) return;
    controller.jumpTo(target);
  }

  @override
  Widget build(BuildContext context) {
    ref.listen<PinId?>(focusedPinProvider, (previous, next) {
      if (next != null && next != previous) _scheduleRevealFocusedPin();
    });
    final tokens = context.upeg;
    final placements = widget.snapshot.placements;
    if (placements.isEmpty) {
      if (widget.onOpenPalette != null) {
        return BoardCanvasPegboardForTesting(
          tokens: tokens,
          child: Center(
            child: EmptyBoard(
              onOpenPalette: widget.onOpenPalette!,
              boardKey:
                  widget.boardKey ?? BoardKey.parse(widget.snapshot.boardKey),
            ),
          ),
        );
      }
      return BoardCanvasPegboardForTesting(
        tokens: tokens,
        child: Center(
          key: const Key('board-canvas-empty'),
          child: Text(
            t(ref, 'empty.pegboard.no_pins'),
            style: TextStyle(
              color: tokens.fg3,
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
            ),
          ),
        ),
      );
    }
    final effectiveBoardKey =
        widget.boardKey ?? BoardKey.parse(widget.snapshot.boardKey);
    final rowsForLayout = maxPlacementRow(placements) + dragExpansionBufferRows;
    return BoardCanvasPegboardForTesting(
      tokens: tokens,
      // The grid path paints its own dots nested inside the scrolled
      // content (via PegboardGridDots below) so holes scroll with the
      // cells; the outer wallpaper layer would otherwise stay pinned to
      // the viewport and drift out of alignment as the user scrolls.
      paintDots: false,
      child: SingleChildScrollView(
        key: const Key('board-canvas-grid'),
        scrollDirection: Axis.horizontal,
        controller: _horizontalScrollController,
        child: SingleChildScrollView(
          key: const Key('board-canvas-vertical-scroll'),
          controller: _verticalScrollController,
          padding: const EdgeInsets.all(UpegSizing.pegboardPadding),
          child: PegboardGridDots(
            tokens: tokens,
            child: AbsoluteGrid(
              placements: placements,
              columns: widget.snapshot.boardCols,
              rows: rowsForLayout,
              boardKey: effectiveBoardKey,
              onPinTap: widget.onPinTap,
              onEditPinColor: widget.onEditPinColor,
              onOpenModal: widget.onOpenModal,
              onMovePin: widget.onMovePin,
              verticalScrollController: _verticalScrollController,
            ),
          ),
        ),
      ),
    );
  }
}

/// Test-only constructor for the pure grid renderer. Accepts an optional
/// [onMovePin] injection so widget tests can assert drag-drop commits
/// without touching the Rust dylib.
@visibleForTesting
Widget debugBoardCanvasGrid({
  required LayoutSnapshotDto snapshot,
  required PinTapCallback onPinTap,
  BoardKey? boardKey,
  OpenPaletteCallback? onOpenPalette,
  MovePinCallback? onMovePin,
  PinTapCallback? onEditPinColor,
  PinTapCallback? onOpenModal,
}) {
  return _BoardCanvasGrid(
    snapshot: snapshot,
    boardKey: boardKey,
    onPinTap: onPinTap,
    onEditPinColor: onEditPinColor,
    onOpenModal: onOpenModal,
    onOpenPalette: onOpenPalette,
    onMovePin: onMovePin,
  );
}
