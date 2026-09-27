/// F15 — FRB seam for the move-mode commit path.
///
/// `MovePinCommitFn(toolId, anchorX, anchorY)` is what the BoardPage
/// move-mode handler calls after the user presses Enter inside the
/// active move state. Production wraps the existing FRB `move_pin`
/// (already in `pegboard.dart`) so this seam stays a single line.
/// Tests `overrideWith` a recorder to assert anchor + tool id
/// without crossing the dylib.
///
/// SoC: this seam is intentionally separate from
/// `move_pin_slot_provider.dart` (Q6 — Cmd+[/] delta-based slot
/// shuffle) because the two callers feed `move_pin` from different
/// inputs: Q6 has a delta, Q7 has an absolute anchor.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';

/// Commit a pin move to `(anchorX, anchorY)`. Mirrors the FRB
/// `move_pin` signature so callers stay typed; `Future<void>` is
/// returned even though `move_pin` is sync FRB so callers can await
/// the commit before refreshing the canvas snapshot.
typedef MovePinCommitFn =
    Future<void> Function(PinId pinId, int anchorX, int anchorY);

/// Production binding. Reads `currentBoardKeyProvider` and forwards the anchor
/// pair through [pegboardMutationsProvider] so a successful commit refreshes the
/// current board layout. Mutation errors are logged by the controller and do
/// not invalidate the stale-but-known layout.
final movePinCommitFnProvider = Provider<MovePinCommitFn>((ref) {
  return (pinId, anchorX, anchorY) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    await ref
        .read(pegboardMutationsProvider)
        .move((boardKey, pinId), anchorX: anchorX, anchorY: anchorY);
  };
});
