/// Focused-pin layout reorder seam.
///
/// `Reorder` is intentionally separate from Cmd/Ctrl+[/] slot movement:
/// reorder swaps the focused pin with its row-major neighbour, while
/// MovePinPrev/Next moves a pin one coordinate slot.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';

typedef ReorderPinFn =
    Future<void> Function(PinId pinId, OrderDirectionDto direction);

final reorderPinFnProvider = Provider<ReorderPinFn>((ref) {
  return (pinId, direction) async {
    final boardKey = ref.read(currentBoardKeyProvider);
    if (boardKey == null) return;
    await ref.read(pegboardMutationsProvider).reorder((
      boardKey,
      pinId,
    ), direction);
  };
});

Future<void> dispatchReorderPin(
  ProviderContainer container,
  KeyboardCommandDto cmd,
) async {
  final direction = switch (cmd) {
    KeyboardCommandDto_Reorder(:final direction) => direction,
    _ => null,
  };
  if (direction == null) return;
  final toolId = container.read(focusedPinProvider);
  if (toolId == null) return;
  final reorder = container.read(reorderPinFnProvider);
  await reorder(toolId, direction);
}
