/// Per-(board, tool) pin status + FRB mutation seams.
///
/// `pinnedProvider((boardKey, toolId))` returns a boolean that the
/// palette rows render as a pill. Widgets should perform writes through
/// `pegboardMutationsProvider` so refresh rules remain centralized.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as frb;

/// Identity for the family — Dart's standard `Record` already
/// implements value equality, but a typedef documents the intent.
typedef PinKey = (BoardKey boardKey, ToolId toolId);

typedef IsPinnedLoader = bool Function(BoardKey boardKey, ToolId toolId);
typedef PinMutator = void Function(BoardKey boardKey, ToolId toolId);

final isPinnedLoaderProvider = Provider<IsPinnedLoader>(
  (ref) =>
      (boardKey, toolId) =>
          frb.isToolPinned(boardKey: boardKey.value, toolId: toolId.value),
);

final pinToolMutatorProvider = Provider<PinMutator>(
  (ref) =>
      (boardKey, toolId) =>
          frb.pinTool(boardKey: boardKey.value, toolId: toolId.value),
);

final unpinToolMutatorProvider = Provider<PinMutator>(
  (ref) =>
      (boardKey, toolId) =>
          frb.unpinTool(boardKey: boardKey.value, toolId: toolId.value),
);

final pinnedProvider = Provider.family<bool, PinKey>((ref, key) {
  final load = ref.watch(isPinnedLoaderProvider);
  return load(key.$1, key.$2);
});

/// Resolves the set of board keys that already contain `toolId` in their
/// persisted layout. Drives the palette's multi-board pin chip cluster.
///
/// Overrideable for widget tests via [pinnedBoardsLoaderProvider].
typedef PinnedBoardsLoader = Set<BoardKey> Function(ToolId toolId);

final pinnedBoardsLoaderProvider = Provider<PinnedBoardsLoader>(
  (ref) =>
      (toolId) => {
        for (final boardKey in frb.pinnedBoardsForTool(toolId: toolId.value))
          BoardKey.parse(boardKey),
      },
);

final pinnedBoardsForToolProvider = Provider.family<Set<BoardKey>, ToolId>((
  ref,
  toolId,
) {
  final load = ref.watch(pinnedBoardsLoaderProvider);
  return load(toolId);
});
