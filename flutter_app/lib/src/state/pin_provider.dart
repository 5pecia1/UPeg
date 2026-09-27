/// Per-placement identity and legacy pin query seams.
///
/// The active palette adds a fresh placement through [addPinMutatorProvider].
/// Legacy tool-level query seams remain temporarily for non-canvas callers.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' as frb;

/// Identity for the family — Dart's standard `Record` already
/// implements value equality, but a typedef documents the intent.
typedef PinKey = (BoardKey boardKey, PinId pinId);

typedef IsPinnedLoader = bool Function(BoardKey boardKey, ToolId toolId);
typedef AddPinMutator = String Function(BoardKey boardKey, ToolId toolId);
typedef PinMutator = void Function(BoardKey boardKey, ToolId toolId);

final addPinMutatorProvider = Provider<AddPinMutator>(
  (ref) =>
      (boardKey, toolId) =>
          frb.addPin(boardKey: boardKey.value, toolId: toolId.value),
);

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

/// Resolves boards that contain a tool for legacy consumers.
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
