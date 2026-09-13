/// Shared selection behaviour of [`currentBoardKeyProvider`].
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

List<BoardDto> _boards(List<String> keys) => [
  for (final key in keys) BoardDto(key: key, title: key),
];

void main() {
  group('currentBoardKeyProvider shared selection', () {
    test('currentBoardKeyProvider_select은_shared_selection에_쓴다', () {
      final saved = <PegboardSelectionDto>[];
      final container = ProviderContainer(
        overrides: [
          ...pegboardSelectionOverrides(onSave: saved.add),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => _boards(['dev', 'side']),
          ),
        ],
      );
      addTearDown(container.dispose);

      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('side'));

      expect(saved.single.boardKey, 'side');
      expect(container.read(currentBoardKeyProvider), BoardKey.parse('side'));
    });

    test(
      'currentBoardKeyProvider_restore는_shared_selection의_board로_초기화한다',
      () async {
        final container = ProviderContainer(
          overrides: [
            ...pegboardSelectionOverrides(boardKey: 'side'),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => _boards(['dev', 'side']),
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(currentBoardKeyProvider.notifier).restore();

        expect(container.read(currentBoardKeyProvider), BoardKey.parse('side'));
      },
    );

    test(
      'currentBoardKeyProvider_restore는_저장된_보드가_사라지면_첫번째_보드로_폴백한다',
      () async {
        final container = ProviderContainer(
          overrides: [
            ...pegboardSelectionOverrides(boardKey: 'ghost'),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => _boards(['dev', 'side']),
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(currentBoardKeyProvider.notifier).restore();

        expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));
      },
    );

    test(
      'currentBoardKeyProvider_restore는_저장된_보드가_없으면_첫번째_보드로_초기화한다',
      () async {
        final container = ProviderContainer(
          overrides: [
            ...pegboardSelectionOverrides(),
            boardsLoaderProvider.overrideWith(
              (ref) =>
                  () => _boards(['dev', 'side']),
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(currentBoardKeyProvider.notifier).restore();

        expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));
      },
    );
  });
}
