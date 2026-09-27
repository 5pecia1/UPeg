/// FRB seam for the resize-mode commit path.
///
/// `ResizePinCommitFn(toolId, action)` is what the resize-mode commit
/// (keyboard Enter AND the SE-corner drag handle's pan-end — one shared
/// path) fires after the state machine snapshot decided WHAT to
/// persist. The decision itself is typed as the sealed
/// [ResizeCommitAction] so tests can assert "setSpan vs clearSpan"
/// without stringly-typed flags:
///   * [ResizeCommitSetSpan] — persist a user span override,
///   * [ResizeCommitClearSpan] — drop the override so the manifest
///     footprint applies again.
///
/// Mirrors `move_pin_commit_provider.dart` (F15): production forwards
/// through [pegboardMutationsProvider]; tests `overrideWith` a recorder.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';

/// Sealed commit decision produced by the resize state machine.
sealed class ResizeCommitAction {
  const ResizeCommitAction();
}

/// Persist `(cols, rows)` as the pin's span override.
final class ResizeCommitSetSpan extends ResizeCommitAction {
  const ResizeCommitSetSpan({required this.cols, required this.rows});

  final int cols;
  final int rows;
}

/// Drop the span override — the manifest footprint applies again.
final class ResizeCommitClearSpan extends ResizeCommitAction {
  const ResizeCommitClearSpan();
}

/// Commit a resize decision for `toolId` on the current board.
/// `Future<void>` so callers can await the write before refreshing.
typedef ResizePinCommitFn =
    Future<void> Function(PinId pinId, ResizeCommitAction action);

/// Production binding. Reads `currentBoardKeyProvider` and forwards the
/// decision through [pegboardMutationsProvider] so a successful commit
/// refreshes the current board layout. Mutation errors are logged by the
/// controller and do not invalidate the stale-but-known layout.
final resizePinCommitFnProvider = Provider<ResizePinCommitFn>((ref) {
  return (pinId, action) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    final mutations = ref.read(pegboardMutationsProvider);
    switch (action) {
      case ResizeCommitSetSpan(:final cols, :final rows):
        await mutations.setSpan((boardKey, pinId), cols: cols, rows: rows);
      case ResizeCommitClearSpan():
        await mutations.clearSpan((boardKey, pinId));
    }
  };
});
