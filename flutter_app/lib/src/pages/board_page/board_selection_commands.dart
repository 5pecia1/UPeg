/// Board / tag selection commands driven by the keyboard.
///
/// Free functions over a [WidgetRef] rather than methods on the page
/// state: none of them touch the widget tree, so keeping them out of
/// `BoardPage` leaves the page with only the parts that genuinely need
/// a `BuildContext`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto, PlacementDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/pegboard_mutations_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

/// Highest board slot addressable by the number-row shortcuts (`1`..`9`).
const int maxBoardSlot = 9;

/// Cycle board selection forward / backward. Wraps around at the
/// list ends so the user can press `[` / `]` repeatedly without
/// landing on an invalid index.
void cycleBoard(WidgetRef ref, {required bool forward}) {
  final boards = ref.read(boardsProvider).value;
  if (boards == null || boards.isEmpty) return;
  final current = ref.read(currentBoardKeyProvider);
  final idx = current == null
      ? 0
      : boards
            .indexWhere((b) => b.key == current.value)
            .clamp(0, boards.length - 1);
  final delta = forward ? 1 : -1;
  final next = boards[(idx + delta + boards.length) % boards.length];
  ref.read(currentBoardKeyProvider.notifier).select(BoardKey.parse(next.key));
}

void clearBoardFilter(WidgetRef ref) {
  final boards = ref.read(boardsProvider).value;
  if (boards == null || boards.isEmpty) return;
  ref
      .read(currentBoardKeyProvider.notifier)
      .select(BoardKey.parse(boards.first.key));
}

void cycleTag(WidgetRef ref) {
  final boardKey = ref.read(currentBoardKeyProvider);
  final tags = ref.read(tagOptionsForBoardProvider(boardKey));
  if (tags.isEmpty) return;
  final current = ref.read(selectedTagProvider);
  final idx = tags.indexOf(current).clamp(0, tags.length - 1);
  final next = tags[(idx + 1) % tags.length];
  ref.read(selectedTagProvider.notifier).setSelection(next);
}

void switchBoardSlot(WidgetRef ref, int slot) {
  if (slot < 1 || slot > maxBoardSlot) return;
  final boards = ref.read(boardsProvider).value;
  if (boards == null) return;
  final idx = slot - 1;
  if (idx >= boards.length) return;
  ref
      .read(currentBoardKeyProvider.notifier)
      .select(BoardKey.parse(boards[idx].key));
}

BoardDto? currentBoard(WidgetRef ref) {
  final current = ref.read(currentBoardKeyProvider);
  if (current == null) return null;
  final boards = ref.read(boardsProvider).value;
  if (boards == null) return null;
  for (final board in boards) {
    if (board.key == current.value) return board;
  }
  return null;
}

Future<void> toggleFocusedPin(WidgetRef ref) async {
  final boardKey = ref.read(currentBoardKeyProvider);
  final pinId = ref.read(focusedPinProvider);
  if (boardKey == null || pinId == null) return;
  await ref.read(pegboardMutationsProvider).remove((boardKey, pinId));
}

List<PlacementDto> visiblePlacementsForKeyboard(WidgetRef ref) {
  final boardKey = ref.read(currentBoardKeyProvider);
  if (boardKey == null) return const <PlacementDto>[];
  final selectedTag = ref.read(selectedTagProvider);
  final snapshot = ref.read(layoutLoaderProvider)(
    LayoutQuery(boardKey: boardKey, tag: selectedTag),
  );
  final placements = [...snapshot.placements]
    ..sort((a, b) {
      final row = a.y.compareTo(b.y);
      if (row != 0) return row;
      return a.x.compareTo(b.x);
    });
  return placements;
}
