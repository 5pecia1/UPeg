/// Board changes normalize the selected tag against the target board.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

ProviderContainer _container({
  required List<String> Function(BoardKey? boardKey) tagOptions,
}) {
  final c = ProviderContainer(
    overrides: [...pegboardSelectionOverrides(tagOptions: tagOptions)],
  );
  addTearDown(c.dispose);
  return c;
}

void main() {
  group('selectedTagProvider board change normalization', () {
    test(
      'selectedTagProvider_resets_only_tag_missing_from_new_board_to_all',
      () {
        final c = _container(
          tagOptions: (boardKey) => boardKey == BoardKey.parse('side')
              ? const ['all']
              : const ['all', 'convert'],
        );

        c.read(selectedTagProvider.notifier).select('convert');
        expect(c.read(selectedTagProvider), const TagSpecific('convert'));

        c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('side'));

        expect(c.read(selectedTagProvider), const TagAll());
      },
    );

    test('selectedTagProvider_keeps_tag_still_valid_on_new_board', () {
      final c = _container(tagOptions: (_) => const ['all', 'convert']);

      c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('main'));
      c.read(selectedTagProvider.notifier).select('convert');
      c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('side'));

      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });
  });
}
