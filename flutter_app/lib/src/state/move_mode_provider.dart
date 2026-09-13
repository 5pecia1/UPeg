/// F15 — keyboard move-mode state machine (inventory row F15).
///
/// Sealed [`MoveModeState`] discriminates two states:
///   * [MoveModeIdle] — no move in progress (the default).
///   * [MoveModeActive] — a pin is being nudged with arrow keys
///     before the user commits with Enter / cancels with Esc.
///
/// SoC: the state machine itself is pure data + transitions (Q7a).
/// UI integration (canvas preview overlay + arrow-key plumbing into
/// `BoardPage._dispatchCommand`) lives in Q7b.
///
/// Type system: a Dart 3 sealed class so callers `switch` on the
/// variant and the compiler enforces exhaustiveness — no
/// stringly-typed `mode == 'active'` checks.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_pin_commit_provider.dart';

/// Sealed root of the move-mode state. Subclasses are `final` so
/// downstream packages can't extend them and break the exhaustive
/// switch.
sealed class MoveModeState {
  const MoveModeState();
}

/// No move-mode operation in progress. Singleton-ish via the `const`
/// constructor.
final class MoveModeIdle extends MoveModeState {
  const MoveModeIdle();
}

/// A pin is being nudged. `originX` / `originY` snapshot the start
/// position; `currentX` / `currentY` track the user's nudges before
/// commit. The notifier clamps `currentX` to [0, maxX], where `maxX`
/// is derived from the current board columns minus the pin span. This
/// matches the FRB `move_pin` clamp so preview stays consistent with
/// commit for both U1 and wider pins.
final class MoveModeActive extends MoveModeState {
  const MoveModeActive({
    required this.boardKey,
    required this.toolId,
    required this.originX,
    required this.originY,
    required this.currentX,
    required this.currentY,
    required this.maxX,
  });

  final BoardKey boardKey;
  final ToolId toolId;
  final int originX;
  final int originY;
  final int currentX;
  final int currentY;
  final int maxX;
}

/// Notifier for [`moveModeProvider`].
class MoveModeNotifier extends Notifier<MoveModeState> {
  @override
  MoveModeState build() => const MoveModeIdle();

  /// Transition Idle → Active, snapshotting the pin's origin.
  void start({
    required BoardKey boardKey,
    required ToolId toolId,
    required int originX,
    required int originY,
    required int maxX,
  }) {
    final normalizedMaxX = maxX < 0 ? 0 : maxX;
    final clampedOriginX = _clampX(originX, normalizedMaxX);
    state = MoveModeActive(
      boardKey: boardKey,
      toolId: toolId,
      originX: clampedOriginX,
      originY: originY,
      currentX: clampedOriginX,
      currentY: originY,
      maxX: normalizedMaxX,
    );
  }

  /// Increment `currentX` / `currentY` while Active; no-op otherwise.
  /// Negative values clamp to 0; `currentX` clamps to the active pin's
  /// span-aware rightmost anchor. When the pin reaches that anchor and the
  /// user nudges right, the downstream FRB `place_tool_with_push` performs
  /// the row-major reflow (pushes into the next row) — Flutter should NOT
  /// independently compute geometry beyond this upper bound.
  void nudge({required int dx, required int dy}) {
    final active = state;
    if (active is! MoveModeActive) return;
    final nx = active.currentX + dx;
    final ny = active.currentY + dy;
    state = MoveModeActive(
      boardKey: active.boardKey,
      toolId: active.toolId,
      originX: active.originX,
      originY: active.originY,
      currentX: _clampX(nx, active.maxX),
      currentY: ny < 0 ? 0 : ny,
      maxX: active.maxX,
    );
  }

  /// Revert to Idle without surfacing the in-flight position.
  void cancel() {
    state = const MoveModeIdle();
  }

  /// Snapshot the current Active state, reset to Idle, return the
  /// snapshot. Returns `null` when Idle. Callers (BoardPage / canvas)
  /// use the snapshot to fire the FRB `move_pin` write.
  MoveModeActive? commit() {
    final active = state;
    if (active is! MoveModeActive) return null;
    state = const MoveModeIdle();
    return active;
  }
}

final moveModeProvider = NotifierProvider<MoveModeNotifier, MoveModeState>(
  MoveModeNotifier.new,
);

/// Dispatcher that translates a [KeyboardCommandDto] into a
/// [`moveModeProvider`] transition. Returns `true` when the command
/// was consumed by the move-mode handler so the caller (BoardPage)
/// can stop propagating the event.
///
/// Inventory row F15 — keyboard `m / M / arrows / Enter / Esc`.
///
/// Rules:
///   * `StartMove` flips Idle → Active when a pin is focused (via
///     [`focusedPinProvider`]) and the current board snapshot still
///     contains that pin. The origin is the persisted placement
///     anchor, so arrow nudges are relative to where the pin actually
///     is. Returns `false` (not consumed) when no pin is focused or
///     the active layout cannot resolve it.
///   * `Move(direction)` is only consumed while Active — Idle falls
///     through so global arrow-based navigation keeps working.
///   * `Commit` snapshots the active state, flips back to Idle, and
///     fires the FRB commit through [`movePinCommitFnProvider`].
///   * `Cancel` reverts to Idle without writing.
///   * Other variants return `false` so the BoardPage dispatcher
///     keeps flowing.
bool handleMoveModeCommand(
  ProviderContainer container, {
  required KeyboardCommandDto cmd,
}) {
  final notifier = container.read(moveModeProvider.notifier);
  final state = container.read(moveModeProvider);
  switch (cmd) {
    case KeyboardCommandDto_StartMove():
      final toolId = container.read(focusedPinProvider);
      if (toolId == null) return false;
      final origin = _focusedPlacementOrigin(container, toolId);
      if (origin == null) return false;
      notifier.start(
        boardKey: origin.boardKey,
        toolId: toolId,
        originX: origin.x,
        originY: origin.y,
        maxX: origin.maxX,
      );
      return true;
    case KeyboardCommandDto_Move(:final direction):
      if (state is! MoveModeActive) return false;
      final (dx, dy) = switch (direction) {
        DirectionDto.left => (-1, 0),
        DirectionDto.right => (1, 0),
        DirectionDto.up => (0, -1),
        DirectionDto.down => (0, 1),
      };
      notifier.nudge(dx: dx, dy: dy);
      return true;
    case KeyboardCommandDto_Commit():
      if (state is! MoveModeActive) return false;
      final committed = notifier.commit();
      if (committed == null) return true;
      if (container.read(currentBoardKeyProvider) != committed.boardKey) {
        return true;
      }
      // Fire-and-forget the FRB write; the canvas rebuild picks up
      // the new placement on the next snapshot.
      final commit = container.read(movePinCommitFnProvider);
      // ignore: discarded_futures — fire-and-forget by design.
      commit(committed.toolId, committed.currentX, committed.currentY);
      return true;
    case KeyboardCommandDto_Cancel():
      if (state is! MoveModeActive) return false;
      notifier.cancel();
      return true;
    default:
      return false;
  }
}

({BoardKey boardKey, int x, int y, int maxX})? _focusedPlacementOrigin(
  ProviderContainer container,
  ToolId toolId,
) {
  final boardKey = container.read(currentBoardKeyProvider);
  if (boardKey == null) return null;
  final snapshot = container.read(layoutLoaderProvider)(
    LayoutQuery.all(boardKey),
  );
  final placement = _placementFor(snapshot.placements, toolId);
  if (placement == null) return null;
  return (
    boardKey: boardKey,
    x: placement.x,
    y: placement.y,
    maxX: _maxStartX(boardCols: snapshot.boardCols, spanW: placement.w),
  );
}

int _maxStartX({required int boardCols, required int spanW}) {
  if (boardCols <= 0 || spanW <= 0 || spanW >= boardCols) return 0;
  return boardCols - spanW;
}

int _clampX(int x, int maxX) {
  if (x < 0) return 0;
  if (x > maxX) return maxX;
  return x;
}

PlacementDto? _placementFor(List<PlacementDto> placements, ToolId toolId) {
  for (final placement in placements) {
    if (placement.toolId == toolId.value) return placement;
  }
  return null;
}
