/// Keyboard/mouse resize-mode state machine (pin-resize UX).
///
/// Sealed [`ResizeModeState`] discriminates two states:
///   * [ResizeModeIdle] — no resize in progress (the default).
///   * [ResizeModeActive] — a pin's span is being adjusted (arrow keys
///     or the SE-corner drag handle) before the user commits with
///     Enter / pan-end, or cancels with Esc.
///
/// Clone of the move-mode pattern (`move_mode_provider.dart`, F15):
/// the state machine is pure data + transitions; UI integration
/// (canvas preview overlay + key plumbing into
/// `BoardPage._dispatchCommand` + the drag handle) lives in the
/// widgets. Both the keyboard path and the mouse drag drive THIS one
/// state, so there is a single preview + commit path.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pegboard_units.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/move_mode_provider.dart';
import 'package:upeg/src/state/resize_pin_commit_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';

/// Minimum span on either axis — a pin always covers at least one cell.
const int minPinSpan = 1;

/// Sealed root of the resize-mode state.
sealed class ResizeModeState {
  const ResizeModeState();
}

/// No resize in progress.
final class ResizeModeIdle extends ResizeModeState {
  const ResizeModeIdle();
}

/// A pin's span is being adjusted. `baseCols`/`baseRows` snapshot the
/// committed EFFECTIVE size at entry (span override or manifest);
/// `currentCols`/`currentRows` track the in-flight target.
/// `manifestCols`/`manifestRows` are the tool's declared footprint —
/// what `0` (reset) returns to and what makes a commit collapse to
/// `clearSpan`. `maxCols` is the board width (spans never exceed it).
final class ResizeModeActive extends ResizeModeState {
  const ResizeModeActive({
    required this.boardKey,
    required this.toolId,
    required this.baseCols,
    required this.baseRows,
    required this.currentCols,
    required this.currentRows,
    required this.maxCols,
    required this.manifestCols,
    required this.manifestRows,
  });

  final BoardKey boardKey;
  final ToolId toolId;
  final int baseCols;
  final int baseRows;
  final int currentCols;
  final int currentRows;
  final int maxCols;
  final int manifestCols;
  final int manifestRows;
}

/// Notifier for [`resizeModeProvider`].
class ResizeModeNotifier extends Notifier<ResizeModeState> {
  @override
  ResizeModeState build() => const ResizeModeIdle();

  /// Transition Idle → Active, snapshotting the pin's committed
  /// effective size as both base and current.
  void start({
    required BoardKey boardKey,
    required ToolId toolId,
    required int baseCols,
    required int baseRows,
    required int manifestCols,
    required int manifestRows,
    required int maxCols,
  }) {
    final normalizedMaxCols = maxCols < minPinSpan ? minPinSpan : maxCols;
    final cols = _clampCols(baseCols, normalizedMaxCols);
    final rows = _clampRows(baseRows);
    state = ResizeModeActive(
      boardKey: boardKey,
      toolId: toolId,
      baseCols: cols,
      baseRows: rows,
      currentCols: cols,
      currentRows: rows,
      maxCols: normalizedMaxCols,
      manifestCols: _clampCols(manifestCols, normalizedMaxCols),
      manifestRows: _clampRows(manifestRows),
    );
  }

  /// Apply `(dCols, dRows)` to the current span while Active; no-op
  /// otherwise. The result clamps to `1..maxCols` columns and `>= 1`
  /// rows — matching the FRB `ColSpan`/`RowSpan` newtypes so the
  /// preview can never request a span the commit would reject.
  void resizeBy({required int dCols, required int dRows}) {
    final active = state;
    if (active is! ResizeModeActive) return;
    state = _withCurrent(
      active,
      cols: active.currentCols + dCols,
      rows: active.currentRows + dRows,
    );
  }

  /// Snap the current span back to the manifest footprint (`0` key).
  void reset() {
    final active = state;
    if (active is! ResizeModeActive) return;
    state = _withCurrent(
      active,
      cols: active.manifestCols,
      rows: active.manifestRows,
    );
  }

  /// Revert to Idle without surfacing the in-flight span.
  void cancel() {
    state = const ResizeModeIdle();
  }

  /// Snapshot the current Active state, reset to Idle, return the
  /// snapshot. Returns `null` when Idle. Callers use the snapshot to
  /// decide between `setSpan` / `clearSpan` / no-op.
  ResizeModeActive? commit() {
    final active = state;
    if (active is! ResizeModeActive) return null;
    state = const ResizeModeIdle();
    return active;
  }

  ResizeModeActive _withCurrent(
    ResizeModeActive active, {
    required int cols,
    required int rows,
  }) {
    return ResizeModeActive(
      boardKey: active.boardKey,
      toolId: active.toolId,
      baseCols: active.baseCols,
      baseRows: active.baseRows,
      currentCols: _clampCols(cols, active.maxCols),
      currentRows: _clampRows(rows),
      maxCols: active.maxCols,
      manifestCols: active.manifestCols,
      manifestRows: active.manifestRows,
    );
  }

  int _clampCols(int cols, int maxCols) {
    if (cols < minPinSpan) return minPinSpan;
    if (cols > maxCols) return maxCols;
    return cols;
  }

  int _clampRows(int rows) => rows < minPinSpan ? minPinSpan : rows;
}

final resizeModeProvider =
    NotifierProvider<ResizeModeNotifier, ResizeModeState>(
      ResizeModeNotifier.new,
    );

/// Dispatcher that translates a [KeyboardCommandDto] into a
/// [`resizeModeProvider`] transition. Returns `true` when the command
/// was consumed so the caller (BoardPage) can stop propagating.
///
/// Rules (mirroring [handleMoveModeCommand]):
///   * `StartResize` flips Idle → Active when a pin is focused and the
///     active layout still contains it. Seeds base/current from the
///     placement's EFFECTIVE size and the manifest footprint from the
///     tool catalog. Cancels a stale move-mode (mutual exclusion).
///   * `StartMove` is NOT consumed here, but cancels a stale resize so
///     the downstream move handler starts from a clean slate.
///   * `ResizeBy` / `ResetSpan` are only consumed while Active.
///   * `Commit` snapshots the active state, flips back to Idle, and
///     fires `setSpan` / `clearSpan` through
///     [`resizePinCommitFnProvider`] — or nothing when the span did
///     not change from the base.
///   * `Cancel` reverts to Idle without writing (nothing was
///     persisted, so no FRB call is needed).
bool handleResizeModeCommand(
  ProviderContainer container, {
  required KeyboardCommandDto cmd,
}) {
  final notifier = container.read(resizeModeProvider.notifier);
  final state = container.read(resizeModeProvider);
  switch (cmd) {
    case KeyboardCommandDto_StartResize():
      final toolId = container.read(focusedPinProvider);
      if (toolId == null) return false;
      final seed = _focusedResizeSeed(container, toolId);
      if (seed == null) return false;
      // Mutual exclusion: move and resize can't both be active —
      // entering resize cancels a stale move.
      container.read(moveModeProvider.notifier).cancel();
      notifier.start(
        boardKey: seed.boardKey,
        toolId: toolId,
        baseCols: seed.baseCols,
        baseRows: seed.baseRows,
        manifestCols: seed.manifestCols,
        manifestRows: seed.manifestRows,
        maxCols: seed.maxCols,
      );
      return true;
    case KeyboardCommandDto_StartMove():
      // Symmetric exclusion: entering move cancels a stale resize.
      // NOT consumed — the move handler downstream starts the move.
      if (state is ResizeModeActive) notifier.cancel();
      return false;
    case KeyboardCommandDto_ResizeBy(:final cols, :final rows):
      if (state is! ResizeModeActive) return false;
      notifier.resizeBy(dCols: cols, dRows: rows);
      return true;
    case KeyboardCommandDto_ResetSpan():
      if (state is! ResizeModeActive) return false;
      notifier.reset();
      return true;
    case KeyboardCommandDto_Commit():
      if (state is! ResizeModeActive) return false;
      commitResizeMode(container);
      return true;
    case KeyboardCommandDto_Cancel():
      if (state is! ResizeModeActive) return false;
      notifier.cancel();
      return true;
    default:
      return false;
  }
}

/// Shared commit path for keyboard Enter AND the drag handle's pan-end.
/// Snapshots the active state, then decides:
///   * current == base            → nothing changed, no FRB write,
///   * current == manifest        → `clearSpan` (no redundant override),
///   * otherwise                  → `setSpan(current)`.
/// The write goes through [`resizePinCommitFnProvider`] so tests can
/// fake it; a board switched mid-resize commits nothing (mirrors the
/// move-mode guard).
void commitResizeMode(ProviderContainer container) {
  final committed = container.read(resizeModeProvider.notifier).commit();
  if (committed == null) return;
  if (container.read(currentBoardKeyProvider) != committed.boardKey) return;
  if (committed.currentCols == committed.baseCols &&
      committed.currentRows == committed.baseRows) {
    return;
  }
  final ResizeCommitAction action;
  if (committed.currentCols == committed.manifestCols &&
      committed.currentRows == committed.manifestRows) {
    action = const ResizeCommitClearSpan();
  } else {
    action = ResizeCommitSetSpan(
      cols: committed.currentCols,
      rows: committed.currentRows,
    );
  }
  final commit = container.read(resizePinCommitFnProvider);
  // Fire-and-forget the FRB write; the canvas rebuild picks up the new
  // placement on the next snapshot (same contract as move-mode).
  // ignore: discarded_futures — fire-and-forget by design.
  commit(committed.toolId, action);
}

({
  BoardKey boardKey,
  int baseCols,
  int baseRows,
  int manifestCols,
  int manifestRows,
  int maxCols,
})?
_focusedResizeSeed(ProviderContainer container, ToolId toolId) {
  final boardKey = container.read(currentBoardKeyProvider);
  if (boardKey == null) return null;
  final snapshot = container.read(layoutLoaderProvider)(
    LayoutQuery.all(boardKey),
  );
  PlacementDto? placement;
  for (final candidate in snapshot.placements) {
    if (candidate.toolId == toolId.value) {
      placement = candidate;
      break;
    }
  }
  if (placement == null) return null;
  // Manifest footprint from the tool catalog; while the catalog is
  // still loading, fall back to the placement's effective size (best
  // effort — reset/clearSpan detection then treats "as rendered" as
  // the manifest).
  final units = container.read(toolByIdProvider(toolId))?.pegboardUnits;
  final manifest = units?.footprint ?? (cols: placement.w, rows: placement.h);
  return (
    boardKey: boardKey,
    baseCols: placement.w,
    baseRows: placement.h,
    manifestCols: manifest.cols,
    manifestRows: manifest.rows,
    maxCols: snapshot.boardCols,
  );
}
