/// Currently-selected board key for the BoardPage.
///
/// The public provider stays board-focused for existing call sites,
/// but durable state now lives in `pegboardSelectionProvider` so
/// Flutter and TUI read/write the same native pegboard selection.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';

class CurrentBoardNotifier extends Notifier<BoardKey?> {
  @override
  BoardKey? build() {
    return ref.watch(
      pegboardSelectionProvider.select((selection) => selection.boardKey),
    );
  }

  void select(BoardKey boardKey) {
    ref.read(pegboardSelectionProvider.notifier).selectBoard(boardKey);
  }

  void clear() {
    ref.read(pegboardSelectionProvider.notifier).clearBoard();
  }

  Future<void> restore() async {
    final boards = await ref.read(boardsProvider.future);
    await ref.read(pegboardSelectionProvider.notifier).restore(boards);
  }
}

final currentBoardKeyProvider =
    NotifierProvider<CurrentBoardNotifier, BoardKey?>(CurrentBoardNotifier.new);
