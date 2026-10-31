/// Unit tests for the tag filter providers.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart' show BoardTagCountDto;
import 'package:upeg/src/state/tag_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

ProviderContainer _makeContainer({List<Override> extra = const []}) {
  final c = ProviderContainer(
    overrides: [...pegboardSelectionOverrides(), ...extra],
  );
  addTearDown(c.dispose);
  return c;
}

void main() {
  group('tagOptionsProvider', () {
    test('tagOptionsProvider_exposes_loader_result_as_typed_variants', () {
      // Loader still returns the raw Rust strings ('all' first, then
      // specific tags). The provider wraps each in the sealed type.
      final container = _makeContainer(
        extra: [
          tagOptionsLoaderProvider.overrideWith(
            (ref) =>
                () => const <String>['all', 'convert', 'id'],
          ),
        ],
      );

      expect(container.read(tagOptionsProvider), const <TagSelection>[
        TagAll(),
        TagSpecific('convert'),
        TagSpecific('id'),
      ]);
    });
  });

  group('tagOptionsForBoardProvider', () {
    test('boardTagCountsProvider_reads_one_snapshot_and_preserves_labels', () {
      final boardKey = BoardKey.parse('dev');
      var calls = 0;
      final container = _makeContainer(
        extra: [
          boardTagCountsLoaderProvider.overrideWithValue((board) {
            expect(board, boardKey);
            calls += 1;
            return const [
              BoardTagCountDto(tag: 'all', count: 3),
              BoardTagCountDto(tag: 'pure', count: 2),
            ];
          }),
        ],
      );

      expect(container.read(boardTagCountsProvider(boardKey)), const [
        (tag: TagAll(), count: 3),
        (tag: TagSpecific('pure'), count: 2),
      ]);
      expect(container.read(boardTagCountsProvider(boardKey)).length, 2);
      expect(calls, 1);
    });

    test(
      'tagOptionsForBoardProvider_exposes_per_board_key_loader_result_as_typed_variants',
      () {
        final boardKey = BoardKey.parse('dev');
        BoardKey? observedBoard;
        final container = _makeContainer(
          extra: [
            boardTagOptionsLoaderProvider.overrideWithValue((board) {
              observedBoard = board;
              return const <String>['all', 'convert'];
            }),
          ],
        );

        expect(container.read(tagOptionsForBoardProvider(boardKey)), const [
          TagAll(),
          TagSpecific('convert'),
        ]);
        expect(observedBoard, boardKey);
      },
    );

    test('countForBoardTagProvider_passes_board_key_and_tag_to_loader', () {
      final boardKey = BoardKey.parse('dev');
      BoardKey? observedBoard;
      TagSelection? observedTag;
      final container = _makeContainer(
        extra: [
          boardTagCountLoaderProvider.overrideWithValue((board, tag) {
            observedBoard = board;
            observedTag = tag;
            return 3;
          }),
        ],
      );

      final count = container.read(
        countForBoardTagProvider((boardKey, const TagSpecific('convert'))),
      );

      expect(count, 3);
      expect(observedBoard, boardKey);
      expect(observedTag, const TagSpecific('convert'));
    });
  });

  group('selectedTagProvider', () {
    test('selectedTagProvider_initial_value_is_TagAll', () {
      final container = _makeContainer();

      expect(container.read(selectedTagProvider), const TagAll());
    });

    test('selectedTagProvider_switches_to_TagSpecific_via_select', () {
      final container = _makeContainer();

      container.read(selectedTagProvider.notifier).select('convert');
      expect(container.read(selectedTagProvider), const TagSpecific('convert'));
    });
  });
}
