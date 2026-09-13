/// Unit tests for shared native state version polling.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/shared_state.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/shared_state_versions.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

void main() {
  group('sharedStateSyncProvider', () {
    test('외부_shared_settings_version_변경은_tweaks를_다시_로드한다', () async {
      var version = BigInt.one;
      var loads = 0;
      final container = ProviderContainer(
        overrides: [
          sharedStateVersionLoaderProvider.overrideWithValue(
            () => SharedStateVersionsDto(tweaks: version),
          ),
          tweaksLoaderProvider.overrideWithValue(() {
            loads += 1;
            return TweaksDto(
              theme: 'Dark',
              accent: loads == 1 ? 'Cyan' : 'Pink',
              showHoles: true,
              locale: 'Ko',
              localHttpHost: false,
            );
          }),
        ],
      );
      addTearDown(container.dispose);

      await container.read(sharedStateSyncProvider.future);
      expect((await container.read(tweaksProvider.future)).accent, 'Cyan');

      version = BigInt.two;
      await container.read(sharedStateRefreshProvider)();

      expect((await container.read(tweaksProvider.future)).accent, 'Pink');
    });

    test('외부_pegboard_version_변경은_board와_tag_selection을_복원한다', () async {
      var version = BigInt.one;
      var boardKey = 'dev';
      var tag = 'all';
      var boards = const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
      final container = ProviderContainer(
        overrides: [
          sharedStateVersionLoaderProvider.overrideWithValue(
            () => SharedStateVersionsDto(pegboard: version),
          ),
          boardsLoaderProvider.overrideWithValue(() => boards),
          pegboardSelectionLoaderProvider.overrideWithValue(
            () => PegboardSelectionDto(boardKey: boardKey, tag: tag),
          ),
          pegboardSelectionSaverProvider.overrideWithValue((_) {}),
          pegboardSelectionTagOptionsLoaderProvider.overrideWithValue(
            (_) => const <String>['all', 'pure'],
          ),
        ],
      );
      addTearDown(container.dispose);

      await container.read(sharedStateSyncProvider.future);

      version = BigInt.two;
      boardKey = 'ops';
      tag = 'pure';
      boards = const <BoardDto>[BoardDto(key: 'ops', title: 'Ops')];
      await container.read(sharedStateRefreshProvider)();

      expect(container.read(currentBoardKeyProvider), BoardKey.parse('ops'));
      expect(container.read(selectedTagProvider), const TagSpecific('pure'));
    });
  });
}
