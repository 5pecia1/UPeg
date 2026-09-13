/// Tests for the popup board scoping
/// (lib/src/popup/popup_hits_provider.dart): the pure placement
/// intersection plus the provider-level scope resolution the popup
/// grid and keyboard cursor share.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/popup/popup_hits_provider.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto;
import 'package:upeg/src/state/app_state.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

PaletteHit _hit(String id) => PaletteHit(
  id: id,
  label: id,
  description: '',
  score: 1.0,
  pinKind: PinKindDto.inline,
);

PlacementDto _placement(String toolId, {required int x, required int y}) =>
    PlacementDto(toolId: toolId, x: x, y: y, w: 1, h: 1);

LayoutSnapshotDto _layout(String boardKey, List<PlacementDto> placements) =>
    LayoutSnapshotDto(boardKey: boardKey, boardCols: 6, placements: placements);

ProviderContainer _container({
  required List<PaletteHit> catalogue,
  required List<PlacementDto> placements,
  List<BoardDto> boards = const [BoardDto(key: 'dev', title: 'Dev')],
}) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(boardKey: 'dev'),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      paletteSearcherProvider.overrideWith(
        (ref) =>
            (String _) => catalogue,
      ),
      layoutLoaderProvider.overrideWith(
        (ref) =>
            (query) => _layout(query.boardKey.value, placements),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _settle(ProviderContainer container, BoardKey boardKey) async {
  await container.read(popupCatalogueHitsProvider.future);
  await container.read(layoutProvider(LayoutQuery.all(boardKey)).future);
  await container.read(boardsProvider.future);
}

void main() {
  group('scopedHitsForPlacements', () {
    test('배치_순서대로_핀된_hit만_남긴다', () {
      final catalogue = [_hit('a.one'), _hit('b.two'), _hit('c.three')];
      final placements = [
        _placement('c.three', x: 0, y: 1),
        _placement('a.one', x: 1, y: 0),
      ];

      final scoped = scopedHitsForPlacements(catalogue, placements);

      // Row-major placement order (y, then x), un-pinned tools dropped.
      expect(scoped.map((hit) => hit.id).toList(), ['a.one', 'c.three']);
    });

    test('카탈로그에_없는_핀은_건너뛴다', () {
      final scoped = scopedHitsForPlacements(
        [_hit('a.one')],
        [_placement('ghost.tool', x: 0, y: 0), _placement('a.one', x: 1, y: 0)],
      );

      expect(scoped.map((hit) => hit.id).toList(), ['a.one']);
    });
  });

  group('popupBoardScopeProvider', () {
    test('보드가_선택되면_그리드를_보드의_핀으로_스코프한다', () async {
      final boardKey = BoardKey.parse('dev');
      final container = _container(
        catalogue: [_hit('a.one'), _hit('b.two'), _hit('c.three')],
        placements: [_placement('b.two', x: 0, y: 0)],
      );
      container.read(currentBoardKeyProvider.notifier).select(boardKey);
      await _settle(container, boardKey);

      final scope = container.read(popupBoardScopeProvider);

      expect(scope, isA<PopupScopePinned>());
      final pinned = scope as PopupScopePinned;
      expect(pinned.boardKey, boardKey);
      expect(pinned.boardTitle, 'Dev');
      expect(pinned.hits.map((hit) => hit.id).toList(), ['b.two']);
      // The keyboard cursor list follows the same scope.
      expect(
        container
            .read(popupEffectiveHitsProvider)
            .map((hit) => hit.id)
            .toList(),
        ['b.two'],
      );
    });

    test('검색어가_있으면_보드_스코프를_해제하고_전체를_검색한다', () async {
      final boardKey = BoardKey.parse('dev');
      final container = _container(
        catalogue: [_hit('a.one'), _hit('b.two')],
        placements: [_placement('b.two', x: 0, y: 0)],
      );
      container.read(currentBoardKeyProvider.notifier).select(boardKey);
      await _settle(container, boardKey);

      container.read(paletteQueryProvider.notifier).state = 'one';

      expect(container.read(popupBoardScopeProvider), isA<PopupScopeAll>());
    });

    test('핀이_없는_보드는_전체_카탈로그로_폴백한다', () async {
      final boardKey = BoardKey.parse('dev');
      final container = _container(
        catalogue: [_hit('a.one'), _hit('b.two')],
        placements: const [],
      );
      container.read(currentBoardKeyProvider.notifier).select(boardKey);
      await _settle(container, boardKey);

      expect(container.read(popupBoardScopeProvider), isA<PopupScopeAll>());
      expect(container.read(popupEffectiveHitsProvider).length, 2);
    });

    test('보드_선택이_없으면_스코프하지_않는다', () async {
      final container = _container(
        catalogue: [_hit('a.one')],
        placements: [_placement('a.one', x: 0, y: 0)],
      );
      await container.read(popupCatalogueHitsProvider.future);

      expect(container.read(popupBoardScopeProvider), isA<PopupScopeAll>());
    });
  });
}
