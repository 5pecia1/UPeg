/// Unit tests for the per-board [`layoutProvider`] family.
///
/// `loadLayoutSnapshot` is swapped via [layoutLoaderProvider]. We
/// verify:
///   1) each board key receives its own cached snapshot,
///   2) the loader is invoked exactly once per key (no leak across
///      family entries).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

void main() {
  group('layoutProvider', () {
    test('layoutProvider_returns_per_board_key_snapshots', () async {
      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWith(
            (ref) =>
                (query) => LayoutSnapshotDto(
                  boardKey: query.boardKey.value,
                  boardCols: 6,
                  placements: const <PlacementDto>[],
                ),
          ),
        ],
      );
      addTearDown(container.dispose);

      final dev = await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );
      final prod = await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('prod'))).future,
      );

      expect(dev.boardKey, 'dev');
      expect(prod.boardKey, 'prod');
    });

    test('layoutProvider_calls_loader_once_for_the_same_key', () async {
      final calls = <LayoutQuery>[];
      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWith(
            (ref) => (query) {
              calls.add(query);
              return LayoutSnapshotDto(
                boardKey: query.boardKey.value,
                boardCols: 6,
                placements: const <PlacementDto>[],
              );
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      final query = LayoutQuery.all(BoardKey.parse('dev'));
      await container.read(layoutProvider(query).future);
      // Second read on the same family key hits the cache.
      await container.read(layoutProvider(query).future);

      expect(calls, [query]);
    });

    test('layoutProvider_passes_tag_filter_to_loader', () async {
      late LayoutQuery observed;
      final container = ProviderContainer(
        overrides: [
          layoutLoaderProvider.overrideWithValue((query) {
            observed = query;
            return const LayoutSnapshotDto(
              boardKey: 'dev',
              boardCols: 6,
              placements: [],
            );
          }),
        ],
      );
      addTearDown(container.dispose);

      await container.read(
        layoutProvider(
          LayoutQuery(
            boardKey: BoardKey.parse('dev'),
            tag: const TagSpecific('pure'),
          ),
        ).future,
      );

      expect(observed.boardKey, BoardKey.parse('dev'));
      expect(observed.tag, const TagSpecific('pure'));
      expect(observed.tagForFrb, 'pure');
    });
  });
}
