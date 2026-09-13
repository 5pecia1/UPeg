/// FRB-backed list of registered boards.
///
/// Wraps `pegboard.listBoards()` in a `FutureProvider` so the BoardPage can
/// render a loading state while the host enumerates persisted boards.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as pegboard;
import 'package:upeg/src/rust/api/pegboard.dart' show BoardDto;

/// Indirection so tests can override the underlying FRB call without
/// loading the native dylib.
typedef BoardsLoader = List<BoardDto> Function();

final boardsLoaderProvider = Provider<BoardsLoader>(
  (ref) => pegboard.listBoards,
);

/// Board creation seam used by BoardTabs dialogs and widget tests.
typedef BoardCreator = String Function(String title);

final boardCreatorProvider = Provider<BoardCreator>(
  (ref) =>
      (title) => pegboard.createBoard(title: title),
);

/// Board rename seam used by BoardTabs dialogs and widget tests.
typedef BoardRenamer = void Function(String boardKey, String newTitle);

final boardRenamerProvider = Provider<BoardRenamer>(
  (ref) =>
      (boardKey, newTitle) =>
          pegboard.renameBoard(boardKey: boardKey, newTitle: newTitle),
);

/// Board deletion seam used by BoardTabs dialogs and widget tests.
typedef BoardDeleter = void Function(String boardKey);

final boardDeleterProvider = Provider<BoardDeleter>(
  (ref) =>
      (boardKey) => pegboard.deleteBoard(boardKey: boardKey),
);

final boardsProvider = FutureProvider<List<BoardDto>>((ref) async {
  final load = ref.watch(boardsLoaderProvider);
  return load();
});
