/// Unit tests for [`emptyBoardSuggestionsProvider`].
///
/// The provider is a `Provider.family` over the per-board suggestion
/// list — load-bearing contract: the loader receives the board key
/// verbatim and the same key reuses the cached value.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/suggestions_provider.dart';

import '../test_helpers/tool_fixture.dart';

final ToolDto _hex = fixtureToolDto(
  id: 'num.hex_to_decimal',
  toolkit: 'convert',
  label: 'hex → dec',
  tags: const <String>['convert'],
);

void main() {
  group('emptyBoardSuggestionsProvider', () {
    test('emptyBoardSuggestionsProvider_calls_loader_per_board_key', () {
      final receivedKeys = <BoardKey>[];
      final container = ProviderContainer(
        overrides: [
          suggestionsLoaderProvider.overrideWith(
            (ref) => (boardKey) {
              receivedKeys.add(boardKey);
              return <ToolDto>[_hex];
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      final dev = container.read(
        emptyBoardSuggestionsProvider(BoardKey.parse('dev')),
      );
      final prod = container.read(
        emptyBoardSuggestionsProvider(BoardKey.parse('prod')),
      );

      expect(dev.single.id, 'num.hex_to_decimal');
      expect(prod.single.id, 'num.hex_to_decimal');
      expect(receivedKeys, [BoardKey.parse('dev'), BoardKey.parse('prod')]);
    });

    test('emptyBoardSuggestionsProvider_caches_for_the_same_key', () {
      var callCount = 0;
      final container = ProviderContainer(
        overrides: [
          suggestionsLoaderProvider.overrideWith(
            (ref) => (boardKey) {
              callCount++;
              return <ToolDto>[_hex];
            },
          ),
        ],
      );
      addTearDown(container.dispose);

      container.read(emptyBoardSuggestionsProvider(BoardKey.parse('dev')));
      container.read(emptyBoardSuggestionsProvider(BoardKey.parse('dev')));

      expect(callCount, 1);
    });
  });
}
