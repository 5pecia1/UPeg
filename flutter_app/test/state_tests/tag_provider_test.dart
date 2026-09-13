/// Unit tests for the tag filter providers.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
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
    test('tagOptionsProvider는_loader_결과를_typed_변형으로_노출한다', () {
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
    test(
      'tagOptionsForBoardProvider는_board_key별_loader_결과를_typed_변형으로_노출한다',
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

    test('countForBoardTagProvider는_board_key와_tag를_loader에_전달한다', () {
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
    test('selectedTagProvider는_초기값이_TagAll이다', () {
      final container = _makeContainer();

      expect(container.read(selectedTagProvider), const TagAll());
    });

    test('selectedTagProvider는_select로_TagSpecific으로_변경한다', () {
      final container = _makeContainer();

      container.read(selectedTagProvider.notifier).select('convert');
      expect(container.read(selectedTagProvider), const TagSpecific('convert'));
    });
  });
}
