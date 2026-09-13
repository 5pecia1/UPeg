/// Cmd+[/Cmd+] focused-pin slot movement seam.
///
/// Translates a `KeyboardCommandDto.MovePinPrev` / `.MovePinNext`
/// command into a call against the FRB `move_pin` write that
/// advances the focused pin one anchor in the requested direction.
///
/// `MovePinSlotFn` is a typedef so callers (and tests) can override
/// it without exposing the entire FRB surface. Production wraps
/// `move_pin` and reads the placement list from
/// `load_layout_snapshot` to compute the new anchor; the typedef
/// keeps that flow plumbed but private.
///
/// `dispatchMovePinSlot` is a tiny helper that pulls the focused
/// tool id from [`focusedPinProvider`] and dispatches the right
/// delta. It returns immediately when no pin is focused so the
/// shortcut is a quiet no-op outside the canvas.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_pin_commit_provider.dart';

/// `(toolId, delta)` — delta is `-1` for prev, `+1` for next.
typedef MovePinSlotFn = Future<void> Function(ToolId toolId, int delta);

/// Production binding. Reads the active board + current layout snapshot,
/// moves the focused pin one valid row-major anchor earlier/later, and
/// delegates the actual write to [`movePinCommitFnProvider`].
final movePinSlotFnProvider = Provider<MovePinSlotFn>((ref) {
  return (toolId, delta) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;

    final snapshot = ref.read(layoutLoaderProvider)(LayoutQuery.all(boardKey));
    final placement = _placementFor(snapshot.placements, toolId);
    if (placement == null) return;

    final anchor = _nextSlotAnchor(placement, snapshot.boardCols, delta);
    if (anchor == null) return;

    final commit = ref.read(movePinCommitFnProvider);
    await commit(toolId, anchor.x, anchor.y);
    bumpLayoutRevisionRef(ref, boardKey);
  };
});

PlacementDto? _placementFor(List<PlacementDto> placements, ToolId toolId) {
  for (final placement in placements) {
    if (placement.toolId == toolId.value) return placement;
  }
  return null;
}

({int x, int y})? _nextSlotAnchor(
  PlacementDto placement,
  int boardCols,
  int delta,
) {
  if (delta == 0 || boardCols <= 0) return null;

  final width = placement.w <= 0 ? 1 : placement.w;
  final slotsPerRow = boardCols - width + 1;
  if (slotsPerRow <= 0) return null;

  final maxX = slotsPerRow - 1;
  final currentX = placement.x > maxX ? maxX : placement.x;
  final currentY = placement.y < 0 ? 0 : placement.y;
  final currentIndex = currentY * slotsPerRow + currentX;
  final nextIndex = currentIndex + delta;
  if (nextIndex < 0) return null;

  return (x: nextIndex % slotsPerRow, y: nextIndex ~/ slotsPerRow);
}

/// Resolve [`focusedPinProvider`] and call [`movePinSlotFnProvider`]
/// with the matching delta. No-op when no pin is focused or when
/// [cmd] is not one of the MovePin* variants.
Future<void> dispatchMovePinSlot(
  ProviderContainer container, {
  required KeyboardCommandDto cmd,
}) async {
  final delta = switch (cmd) {
    KeyboardCommandDto_MovePinPrev() => -1,
    KeyboardCommandDto_MovePinNext() => 1,
    _ => null,
  };
  if (delta == null) return;
  final toolId = container.read(focusedPinProvider);
  if (toolId == null) return;
  final move = container.read(movePinSlotFnProvider);
  await move(toolId, delta);
}
