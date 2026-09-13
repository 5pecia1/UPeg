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
    test('selectedTagProvider_select은_shared_selection에_쓴다', () {
      final saved = <PegboardSelectionDto>[];
      final c = _container(onSave: saved.add);

      c.read(selectedTagProvider.notifier).select('convert');

      expect(saved.single.tag, 'convert');
      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });

    test('selectedTagProvider_clear는_shared_selection에_all을_쓴다', () {
      final saved = <PegboardSelectionDto>[];
      final c = _container(tag: 'convert', onSave: saved.add);

      c.read(selectedTagProvider.notifier).clear();

      expect(saved.single.tag, const TagAll().frbValue);
      expect(c.read(selectedTagProvider), const TagAll());
    });

    test('selectedTagProvider_restore는_저장된_tag로_초기화한다', () async {
      final c = _container(tag: 'convert');

      await c.read(selectedTagProvider.notifier).restore();

      expect(c.read(selectedTagProvider), const TagSpecific('convert'));
    });

    test('selectedTagProvider_restore는_저장된_값이_없으면_all로_초기화한다', () async {
      final c = _container();

      await c.read(selectedTagProvider.notifier).restore();

      expect(c.read(selectedTagProvider), const TagAll());
    });
  });
}
