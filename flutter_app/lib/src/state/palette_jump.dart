/// Pure jump-target policy for the palette ↔ board integration.
///
/// After the palette runs a tool (Enter / Cmd+Enter) or the user clicks
/// a pinned board chip, the palette reveals the tool's pin by switching
/// to its board and handing it keyboard focus. This function decides
/// *which* board that is, kept side-effect free so the priority rule is
/// unit-testable without widgets or providers.
library;

import 'package:upeg/src/identity.dart';

/// Decide which board the palette should jump to for a tool.
///
/// Priority:
///   1. [currentBoardKey] when the tool is pinned there (no switch),
///   2. otherwise the first board in [boardOrder] that carries a pin,
///   3. `null` when the tool is pinned nowhere (no jump).
///
/// Boards absent from [boardOrder] are never returned — the palette
/// cannot switch to a board the board list does not know about.
BoardKey? resolvePaletteJumpBoard({
  required BoardKey? currentBoardKey,
  required List<BoardKey> boardOrder,
  required Set<BoardKey> pinnedBoards,
}) {
  if (pinnedBoards.isEmpty) return null;
  if (currentBoardKey != null && pinnedBoards.contains(currentBoardKey)) {
    return currentBoardKey;
  }
  for (final boardKey in boardOrder) {
    if (pinnedBoards.contains(boardKey)) return boardKey;
  }
  return null;
}
