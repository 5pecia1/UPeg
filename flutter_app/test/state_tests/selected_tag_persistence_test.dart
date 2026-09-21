/// Shared selection behaviour of [`selectedTagProvider`].
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

const _boards = [BoardDto(key: 'dev', title: 'Dev')];

ProviderContainer _container({
  String? tag,
  void Function(PegboardSelectionDto selection)? onSave,
}) {
  final c = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(
        boardKey: 'dev',
        tag: tag,
        onSave: onSave,
        tagOptions: (_) => const ['all', 'convert'],
      ),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => _boards,
      ),
    ],
  );
  addTearDown(c.dispose);
  return c;
}

void main() {
  group('selectedTagProvider shared selection', () {
    test('selectedTagProvider_select_writes_to_shared_selection', () {
      final saved = <PegboardSelectionDto>[];
      final c = _container(onSave: saved.add);

      c.read(selectedTagProvider.notifier).select('convert');

      expect(saved.single.tag, 'convert');
      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });

    test('selectedTagProvider_clear_writes_all_to_shared_selection', () {
      final saved = <PegboardSelectionDto>[];
      final c = _container(tag: 'convert', onSave: saved.add);

      c.read(selectedTagProvider.notifier).clear();

      expect(saved.single.tag, const TagAll().frbValue);
      expect(c.read(selectedTagProvider), const TagAll());
    });

    test('selectedTagProvider_restore_initializes_to_saved_tag', () async {
      final c = _container(tag: 'convert');

      await c.read(selectedTagProvider.notifier).restore();

      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });

    test(
      'selectedTagProvider_restore_initializes_to_all_when_no_saved_value',
      () async {
        final c = _container();

        await c.read(selectedTagProvider.notifier).restore();

        expect(c.read(selectedTagProvider), const TagAll());
      },
    );
  });
}
