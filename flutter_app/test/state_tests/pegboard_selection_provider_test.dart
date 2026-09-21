import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/tag_selection.dart';

ProviderContainer _makeContainer({List<Override> overrides = const []}) {
  final container = ProviderContainer(overrides: overrides);
  addTearDown(container.dispose);
  return container;
}

const _boards = [
  BoardDto(key: 'dev', title: 'Dev'),
  BoardDto(key: 'ops', title: 'Ops'),
];

void main() {
  group('pegboardSelectionProvider', () {
    test(
      'pegboardSelectionProvider_reads_shared_selection_on_restore',
      () async {
        final container = _makeContainer(
          overrides: [
            pegboardSelectionLoaderProvider.overrideWithValue(
              () => const PegboardSelectionDto(boardKey: 'dev', tag: 'pure'),
            ),
            pegboardSelectionSaverProvider.overrideWithValue((_) {}),
            pegboardSelectionTagOptionsLoaderProvider.overrideWithValue(
              (_) => const ['all', 'pure'],
            ),
          ],
        );

        await container
            .read(pegboardSelectionProvider.notifier)
            .restore(_boards);

        final state = container.read(pegboardSelectionProvider);
        expect(state.boardKey, BoardKey.parse('dev'));
        expect(state.tag, const TagSpecific('pure'));
      },
    );

    test('pegboardSelectionProvider_keeps_valid_tag_on_board_change', () async {
      final saved = <PegboardSelectionDto>[];
      final container = _makeContainer(
        overrides: [
          pegboardSelectionLoaderProvider.overrideWithValue(
            () => const PegboardSelectionDto(boardKey: 'dev', tag: 'pure'),
          ),
          pegboardSelectionSaverProvider.overrideWithValue(saved.add),
          pegboardSelectionTagOptionsLoaderProvider.overrideWithValue(
            (_) => const ['all', 'pure'],
          ),
        ],
      );
      await container.read(pegboardSelectionProvider.notifier).restore(_boards);

      container
          .read(pegboardSelectionProvider.notifier)
          .selectBoard(BoardKey.parse('ops'));

      final state = container.read(pegboardSelectionProvider);
      expect(state.boardKey, BoardKey.parse('ops'));
      expect(state.tag, const TagSpecific('pure'));
      expect(
        saved.single,
        const PegboardSelectionDto(boardKey: 'ops', tag: 'pure'),
      );
    });

    test(
      'pegboardSelectionProvider_normalizes_only_missing_tag_to_all_on_board_change',
      () async {
        final saved = <PegboardSelectionDto>[];
        final container = _makeContainer(
          overrides: [
            pegboardSelectionLoaderProvider.overrideWithValue(
              () => const PegboardSelectionDto(boardKey: 'dev', tag: 'pure'),
            ),
            pegboardSelectionSaverProvider.overrideWithValue(saved.add),
            pegboardSelectionTagOptionsLoaderProvider.overrideWithValue(
              (boardKey) => boardKey == BoardKey.parse('ops')
                  ? const ['all']
                  : const ['all', 'pure'],
            ),
          ],
        );
        await container
            .read(pegboardSelectionProvider.notifier)
            .restore(_boards);

        container
            .read(pegboardSelectionProvider.notifier)
            .selectBoard(BoardKey.parse('ops'));

        final state = container.read(pegboardSelectionProvider);
        expect(state.boardKey, BoardKey.parse('ops'));
        expect(state.tag, const TagAll());
        expect(
          saved.single,
          const PegboardSelectionDto(boardKey: 'ops', tag: 'all'),
        );
      },
    );
  });
}
