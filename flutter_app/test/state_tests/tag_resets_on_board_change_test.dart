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
    test('selectedTagProvider는_새_board에_없는_tag만_all로_리셋한다', () {
      final c = _container(
        tagOptions: (boardKey) => boardKey == BoardKey.parse('side')
            ? const ['all']
            : const ['all', 'convert'],
      );

      c.read(selectedTagProvider.notifier).select('convert');
      expect(c.read(selectedTagProvider), const TagSpecific('convert'));

      c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('side'));

      expect(c.read(selectedTagProvider), const TagAll());
    });

    test('selectedTagProvider는_새_board에도_유효한_tag를_유지한다', () {
      final c = _container(tagOptions: (_) => const ['all', 'convert']);

      c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('main'));
      c.read(selectedTagProvider.notifier).select('convert');
      c.read(currentBoardKeyProvider.notifier).select(BoardKey.parse('side'));

      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });
  });
}
