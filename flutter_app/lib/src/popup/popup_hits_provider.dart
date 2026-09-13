/// Popup hit-list providers: board scoping + the single effective list.
///
/// The popup board-tab strip writes `currentBoardKeyProvider`; these
/// providers make that selection actually scope the grid. With an empty
/// query the grid shows the selected board's pinned tools (in placement
/// order) under a "PINNED · {board}" header; typing a query always
/// searches the full desktop catalogue, and a board with no resolvable
/// pins falls back to the full catalogue so the popup never renders an
/// empty launcher.
///
/// `popupEffectiveHitsProvider` stays the single source of truth for
/// both the rendered grid and the keyboard cursor — no chance the
/// cursor activates a different element from the one it highlights.
library;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show PlacementDto;
import 'package:upeg/src/state/app_state.dart';

/// Desktop-searchable tools for the empty-query popup.
///
/// Uses the same palette search path as typed queries so surface
/// filtering stays consistent while still avoiding the old 6-item
/// visual cap.
final popupCatalogueHitsProvider = FutureProvider<List<PaletteHit>>((
  ref,
) async {
  final search = ref.watch(paletteSearcherProvider);
  return search('');
});

/// What the empty-query popup grid is scoped to.
sealed class PopupBoardScope {
  const PopupBoardScope();
}

/// No board scoping — render the full catalogue / search results.
final class PopupScopeAll extends PopupBoardScope {
  const PopupScopeAll();
}

/// The grid is scoped to the selected board's pinned tools.
final class PopupScopePinned extends PopupBoardScope {
  const PopupScopePinned({
    required this.boardKey,
    required this.boardTitle,
    required this.hits,
  });

  final BoardKey boardKey;
  final String boardTitle;

  /// Catalogue hits for the board's placements, in placement order.
  final List<PaletteHit> hits;
}

/// Pure scoping: intersect the catalogue with the board placements,
/// keeping placement order (row-major: y, then x) so the popup mirrors
/// the board's visual arrangement.
@visibleForTesting
List<PaletteHit> scopedHitsForPlacements(
  List<PaletteHit> catalogue,
  List<PlacementDto> placements,
) {
  final byId = <String, PaletteHit>{for (final hit in catalogue) hit.id: hit};
  final ordered = [...placements]
    ..sort((a, b) => a.y != b.y ? a.y.compareTo(b.y) : a.x.compareTo(b.x));
  return <PaletteHit>[
    for (final placement in ordered)
      if (byId[placement.toolId] != null) byId[placement.toolId]!,
  ];
}

/// Resolve the current board scope for the popup grid.
///
/// Falls back to [PopupScopeAll] whenever scoping cannot apply: a
/// non-empty query, no board selection, the layout still loading (or
/// failing — widget tests without a layout seam land here), or a board
/// whose placements resolve to zero catalogue hits.
final popupBoardScopeProvider = Provider<PopupBoardScope>((ref) {
  final query = ref.watch(paletteQueryProvider).trim();
  if (query.isNotEmpty) return const PopupScopeAll();
  final boardKey = ref.watch(currentBoardKeyProvider);
  if (boardKey == null) return const PopupScopeAll();
  final layout = ref.watch(layoutProvider(LayoutQuery.all(boardKey))).value;
  if (layout == null) return const PopupScopeAll();
  final catalogue =
      ref.watch(popupCatalogueHitsProvider).value ?? const <PaletteHit>[];
  final hits = scopedHitsForPlacements(catalogue, layout.placements);
  if (hits.isEmpty) return const PopupScopeAll();
  return PopupScopePinned(
    boardKey: boardKey,
    boardTitle: _boardTitleFor(ref, boardKey),
    hits: hits,
  );
});

/// Board title for the header, falling back to the raw key while the
/// board list is still loading.
String _boardTitleFor(Ref ref, BoardKey boardKey) {
  final boards = ref.watch(boardsProvider).value ?? const [];
  for (final board in boards) {
    if (board.key == boardKey.value) return board.title;
  }
  return boardKey.value;
}

/// Effective hit list for the popup: the board-scoped pins, the full
/// registered catalogue (empty query), or the search results.
/// Centralising this here keeps the keyboard handler and the grid
/// widget reading from the same source.
final popupEffectiveHitsProvider = Provider<List<PaletteHit>>((ref) {
  final scope = ref.watch(popupBoardScopeProvider);
  if (scope is PopupScopePinned) return scope.hits;
  final String query = ref.watch(paletteQueryProvider).trim();
  if (query.isEmpty) {
    return ref.watch(popupCatalogueHitsProvider).value ?? const <PaletteHit>[];
  }
  return ref.watch(paletteResultsProvider).value ?? const <PaletteHit>[];
});
