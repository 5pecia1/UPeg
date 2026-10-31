/// Unit tests for shared native state version polling.
library;

import 'package:fake_async/fake_async.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/platform/window_visibility.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/shared_state.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/shared_state_versions.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

import '../test_helpers/fake_window_visibility_source.dart';

void main() {
  group('sharedStateSyncProvider', () {
    test('external_shared_settings_version_change_reloads_tweaks', () async {
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

    test(
      'external_pegboard_version_change_restores_board_and_tag_selection',
      () async {
        var version = BigInt.one;
        var boardKey = 'dev';
        var tag = 'all';
        var boards = const <BoardDto>[BoardDto(key: 'dev', title: 'Dev')];
        var tagCount = 1;
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
            boardTagCountsLoaderProvider.overrideWithValue(
              (_) => [BoardTagCountDto(tag: 'all', count: tagCount)],
            ),
          ],
        );
        addTearDown(container.dispose);

        await container.read(sharedStateSyncProvider.future);
        expect(container.read(boardTagCountsProvider(null)).single.count, 1);

        version = BigInt.two;
        tagCount = 2;
        boardKey = 'ops';
        tag = 'pure';
        boards = const <BoardDto>[BoardDto(key: 'ops', title: 'Ops')];
        await container.read(sharedStateRefreshProvider)();

        expect(container.read(currentBoardKeyProvider), BoardKey.parse('ops'));
        expect(container.read(selectedTagProvider), const TagSpecific('pure'));
        expect(container.read(boardTagCountsProvider(null)).single.count, 2);
      },
    );
  });

  group('sharedStatePollingProvider', () {
    // Plain `test()`s without a binding: the window seam is always faked, so
    // `windowManager` is never reached.
    ProviderContainer pollingContainer({
      required FakeWindowVisibilitySource source,
      required SharedStateVersionLoader loader,
    }) {
      final container = ProviderContainer(
        overrides: [
          windowVisibilitySourceProvider.overrideWithValue(source),
          sharedStateVersionLoaderProvider.overrideWithValue(loader),
        ],
      );
      addTearDown(container.dispose);
      return container;
    }

    test('starts_with_a_single_probe', () {
      fakeAsync((async) {
        var loads = 0;
        final container = pollingContainer(
          source: FakeWindowVisibilitySource(),
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );

        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();

        expect(loads, 1);
        expect(
          container.read(sharedStateSyncProvider),
          isA<AsyncData<SharedStateVersionsDto>>(),
          reason: 'the baseline is captured synchronously, never loading',
        );
      });
    });

    test('a_provider_built_earlier_keeps_its_baseline_probe', () {
      fakeAsync((async) {
        var loads = 0;
        final container = pollingContainer(
          source: FakeWindowVisibilitySource(),
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );

        // The app builds the sync provider before polling starts.
        container.read(sharedStateSyncProvider);
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();

        expect(loads, 1);
      });
    });

    test('shown_window_refreshes_on_every_tick', () {
      fakeAsync((async) {
        var loads = 0;
        final container = pollingContainer(
          source: FakeWindowVisibilitySource(),
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );
        container.read(sharedStatePollingProvider);

        async.elapse(kSharedStatePollInterval * 2);
        async.flushMicrotasks();

        expect(loads, 3, reason: 'baseline + two ticks');
      });
    });

    test('hidden_window_skips_ticks', () {
      fakeAsync((async) {
        var loads = 0;
        final source = FakeWindowVisibilitySource();
        final container = pollingContainer(
          source: source,
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();
        expect(loads, 1);

        source.hide();
        async.elapse(const Duration(seconds: 3));
        async.flushMicrotasks();

        expect(loads, 1, reason: 'no loader call while hidden');
      });
    });

    test('minimized_window_skips_ticks', () {
      fakeAsync((async) {
        var loads = 0;
        final source = FakeWindowVisibilitySource();
        final container = pollingContainer(
          source: source,
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();

        source.minimize();
        async.elapse(const Duration(seconds: 3));
        async.flushMicrotasks();

        expect(loads, 1);
      });
    });

    test('reshow_refreshes_immediately_and_picks_up_a_missed_change', () {
      fakeAsync((async) {
        var loads = 0;
        var tweaksVersion = BigInt.one;
        final source = FakeWindowVisibilitySource();
        final container = pollingContainer(
          source: source,
          loader: () {
            loads += 1;
            return SharedStateVersionsDto(tweaks: tweaksVersion);
          },
        );
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();
        source.hide();
        async.elapse(const Duration(seconds: 3));
        expect(loads, 1);

        tweaksVersion = BigInt.two;
        source.show();
        async.flushMicrotasks();

        expect(loads, 2, reason: 'one refresh at show, before any tick');
        expect(
          container.read(sharedStateSyncProvider).requireValue.tweaks,
          BigInt.two,
        );

        async.elapse(kSharedStatePollInterval);
        async.flushMicrotasks();
        expect(loads, 3, reason: 'ticks resume after the catch-up');
      });
    });

    test('maximize_after_minimize_refreshes_immediately', () {
      // Windows brings a minimized, maximized window back as `maximize`, not
      // `restore`; polling must not stay paused for it.
      fakeAsync((async) {
        var loads = 0;
        final source = FakeWindowVisibilitySource();
        final container = pollingContainer(
          source: source,
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();

        source.minimize();
        async.elapse(const Duration(seconds: 3));
        async.flushMicrotasks();
        expect(loads, 1, reason: 'no loader call while minimized');

        source.maximize();
        async.flushMicrotasks();
        expect(loads, 2, reason: 'one refresh when the window is back');

        async.elapse(kSharedStatePollInterval);
        async.flushMicrotasks();
        expect(loads, 3, reason: 'ticks resume after the catch-up');
      });
    });

    test('disposing_the_container_stops_the_ticks_and_the_listener', () {
      fakeAsync((async) {
        var loads = 0;
        final source = FakeWindowVisibilitySource();
        final container = pollingContainer(
          source: source,
          loader: () {
            loads += 1;
            return const SharedStateVersionsDto();
          },
        );
        container.read(sharedStatePollingProvider);
        async.flushMicrotasks();
        expect(source.listeners, hasLength(1));

        container.dispose();
        async.elapse(const Duration(seconds: 3));

        expect(loads, 1);
        expect(source.listeners, isEmpty);
      });
    });
  });
}
