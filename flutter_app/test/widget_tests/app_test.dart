/// Widget tests for [UpegApp].
///
/// `appInitProvider` is overridden to drive each [UpegApp] routing branch
/// (loading / error / data) without crossing the FFI boundary. Its provider
/// logging branch is exercised with a synthetic [AppInitReport] and an
/// overridden desktop integration installer. The boot-order contracts
/// (first-frame theme, initial selection restore, shared-state baseline and
/// polling start) run against fake loaders.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/app.dart';
import 'package:upeg/src/identity.dart' show BoardKey;
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/platform/tray.dart' show TrayMenuSync;
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
import 'package:upeg/src/rust/api/shared_state.dart'
    show SharedStateVersionsDto;
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/rust/api/tweaks.dart' show TweaksDto;
import 'package:upeg/src/state/accent.dart' show Accent;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/shared_state_versions.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/suggestions_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart' show UpegTheme;

import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/i18n_test_catalog.dart';

const _fakeReport = AppInitReport(
  launch: DesktopLaunchDto(board: 'dev'),
  hostState: HostStateDto.noHost(),
  version: 'test',
);

const _fakeBoard = BoardDto(key: 'dev', title: 'Dev');

const _fakeStatus = StatusSnapshotDto(
  network: NetworkStatusDto(
    reachability: NetworkReachabilityDto.offline,
    label: 'offline',
  ),
  paused: PausedStateDto.running,
  mcpImportCount: 0,
  mcpImportPhase: McpImportPhaseDto.notStarted,
  buildVersion: 'test',
);

class _FixedStatusNotifier extends StatusNotifier {
  _FixedStatusNotifier(this._snapshot);

  final StatusSnapshotDto _snapshot;

  @override
  StatusSnapshotDto build() => _snapshot;
}

Widget _harness({
  required AsyncValue<AppInitReport> initState,
  List<Override> extraOverrides = const [],
}) {
  return ProviderScope(
    overrides: [
      appInitProvider.overrideWith((ref) async {
        // Mirror the requested AsyncValue. `AsyncLoading` => never
        // completes; `AsyncError` => throws so Riverpod surfaces the
        // error branch.
        return initState.when(
          loading: () => Completer<AppInitReport>().future,
          error: (err, _) => throw err,
          data: (d) => d,
        );
      }),
      ...extraOverrides,
    ],
    child: const UpegApp(),
  );
}

List<Override> _boardPageOverrides() {
  return [
    ...pegboardSelectionOverrides(boardKey: 'dev'),
    localeProvider.overrideWithValue(LocaleDto.en),
    ...i18nTestOverrides,
    statusSnapshotProvider.overrideWith(
      () => _FixedStatusNotifier(_fakeStatus),
    ),
    boardsLoaderProvider.overrideWith(
      (ref) =>
          () => const <BoardDto>[_fakeBoard],
    ),
    layoutLoaderProvider.overrideWithValue(
      (query) => LayoutSnapshotDto(
        boardKey: query.boardKey.value,
        boardCols: 6,
        placements: const <PlacementDto>[],
      ),
    ),
    tagOptionsLoaderProvider.overrideWithValue(() => const <String>['all']),
    boardTagOptionsLoaderProvider.overrideWithValue(
      (_) => const <String>['all'],
    ),
    boardTagCountsLoaderProvider.overrideWithValue(
      (_) => const [BoardTagCountDto(tag: 'all', count: 0)],
    ),
    tagCountLoaderProvider.overrideWithValue((selection) => 0),
    boardTagCountLoaderProvider.overrideWithValue((_, selection) => 0),
    toolsLoaderProvider.overrideWithValue(() => const []),
    suggestionsLoaderProvider.overrideWithValue((boardKey) => const []),
  ];
}

TweaksDto _tweaks({required String theme, String accent = 'Green'}) {
  return TweaksDto(
    theme: theme,
    accent: accent,
    showHoles: true,
    locale: 'En',
    localHttpHost: false,
  );
}

/// [UpegApp] over the real wiring except `appInitProvider`, which stays
/// pending until [initGate] completes, and the shared-state loaders, which
/// are the caller's recording fakes.
Widget _bootHarness({
  required Completer<AppInitReport> initGate,
  required SharedStateVersionLoader versionLoader,
  required TweaksLoader tweaksLoader,
}) {
  return ProviderScope(
    overrides: [
      appInitProvider.overrideWith((ref) => initGate.future),
      sharedStateVersionLoaderProvider.overrideWithValue(versionLoader),
      tweaksLoaderProvider.overrideWithValue(tweaksLoader),
      ..._boardPageOverrides(),
    ],
    child: const UpegApp(),
  );
}

void main() {
  group('UpegApp', () {
    test('appInitProvider_logs_the_AppInitReport_and_writes_no_ERROR', () async {
      final logs = <String>[];
      var integrationsInstalled = false;
      final container = ProviderContainer(
        overrides: [
          appBootLogProvider.overrideWithValue(logs.add),
          appInitRunnerProvider.overrideWithValue(
            () async => const AppInitReport(
              launch: DesktopLaunchDto(board: 'dev'),
              hostState: HostStateDto.noHost(),
              version: 'test-version',
            ),
          ),
          desktopIntegrationsInstallerProvider.overrideWithValue((ref) async {
            integrationsInstalled = true;
          }),
          boardsLoaderProvider.overrideWithValue(
            () => const <BoardDto>[_fakeBoard],
          ),
          ...pegboardSelectionOverrides(boardKey: 'dev'),
        ],
      );
      addTearDown(container.dispose);

      final report = await container.read(appInitProvider.future);

      expect(report.version, equals('test-version'));
      expect(integrationsInstalled, isTrue);
      expect(logs, contains('upeg: appInitProvider — calling initApp()'));
      expect(
        logs,
        contains(
          'upeg: appInitProvider — initApp returned, version=test-version',
        ),
      );
      expect(
        logs,
        contains(
          'upeg: appInitProvider — routing prerequisites ready, painting BoardPage',
        ),
      );
      expect(logs.any((line) => line.contains('ERROR')), isFalse);
      expect(
        logs.any((line) => line.contains('initial selection restore failed')),
        isFalse,
      );
    });

    test(
      'appInitProvider_restores_the_saved_selection_before_it_completes',
      () async {
        BoardKey? boardWhenIntegrationsInstall;
        final container = ProviderContainer(
          overrides: [
            appBootLogProvider.overrideWithValue((_) {}),
            appInitRunnerProvider.overrideWithValue(() async => _fakeReport),
            desktopIntegrationsInstallerProvider.overrideWithValue((ref) async {
              // The cold-boot launch intent is seeded inside the installer, so
              // the restored Board must already be there for a link to win.
              boardWhenIntegrationsInstall = ref.read(currentBoardKeyProvider);
            }),
            boardsLoaderProvider.overrideWithValue(
              () => const <BoardDto>[_fakeBoard],
            ),
            ...pegboardSelectionOverrides(boardKey: 'dev'),
          ],
        );
        addTearDown(container.dispose);
        expect(container.read(currentBoardKeyProvider), isNull);

        await container.read(appInitProvider.future);

        expect(boardWhenIntegrationsInstall, BoardKey.parse('dev'));
        expect(container.read(currentBoardKeyProvider), BoardKey.parse('dev'));
      },
    );

    testWidgets(
      'appInitProvider_stops_waiting_for_a_failing_Boards_load_after_2s',
      (tester) async {
        final logs = <String>[];
        final container = ProviderContainer(
          overrides: [
            appBootLogProvider.overrideWithValue(logs.add),
            appInitRunnerProvider.overrideWithValue(() async => _fakeReport),
            desktopIntegrationsInstallerProvider.overrideWithValue(
              (ref) async {},
            ),
            // An Exception (not an Error) is retried by Riverpod's default
            // retry for up to ~38 s, which keeps `boardsProvider.future`
            // pending; the 2-second cap must end the wait first.
            boardsLoaderProvider.overrideWithValue(
              () => throw Exception('boards unavailable'),
            ),
            ...pegboardSelectionOverrides(boardKey: 'dev'),
          ],
        );
        addTearDown(container.dispose);
        final init = container.listen(appInitProvider, (_, _) {});

        await tester.pump(const Duration(seconds: 1));
        expect(init.read().hasValue, isFalse);

        await tester.pump(const Duration(seconds: 1));
        expect(init.read().hasValue, isTrue);
        expect(
          logs.any((line) => line.contains('initial selection restore failed')),
          isTrue,
        );
        expect(logs.any((line) => line.contains('ERROR')), isFalse);

        // Cancel the pending Boards retry timer before the test's timer check.
        init.close();
        container.dispose();
      },
    );

    testWidgets('UpegApp_shows_the_SplashPage_while_loading', (tester) async {
      await tester.pumpWidget(
        _harness(initState: const AsyncValue<AppInitReport>.loading()),
      );
      // Don't pumpAndSettle — the loading future never completes by
      // design. A single pump is enough for the initial frame.
      await tester.pump();

      expect(find.byType(SplashPage), findsOneWidget);
      expect(find.byKey(splashWordmarkKey), findsOneWidget);
      // TrayMenuSync must be mounted OUTSIDE the appInit gate — it has to
      // subscribe to `pauseControllableProvider` before `UpegTray.install`
      // reads it, even while `appInitProvider` is still loading. Mounting
      // it only under `init.when(data:)` would (re-)introduce the "Pause
      // never re-enables on the embedded desktop" regression.
      expect(find.byType(TrayMenuSync), findsOneWidget);
    });

    testWidgets('UpegApp_shows_guidance_text_in_the_error_state', (
      tester,
    ) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            appInitProvider.overrideWith((ref) => throw Exception('boot fail')),
          ],
          child: const UpegApp(),
        ),
      );
      // Pump enough frames for FutureProvider to settle into AsyncError.
      await tester.pumpAndSettle();

      expect(find.textContaining('boot failed'), findsOneWidget);
      expect(find.textContaining('boot fail'), findsOneWidget);
    });

    testWidgets('UpegApp_surfaces_the_AlreadyRunning_boot_error_clearly', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          initState: const AsyncValue<AppInitReport>.error(
            FrbError.alreadyRunning(pid: 4242),
            StackTrace.empty,
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.textContaining('upeg is already running'), findsOneWidget);
      expect(find.textContaining('PID 4242'), findsOneWidget);
      expect(find.textContaining('Use the existing window'), findsOneWidget);
      expect(find.textContaining('FrbError'), findsNothing);
    });

    testWidgets('should show AlreadyRunning without retrying native init', (
      tester,
    ) async {
      var attempts = 0;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            appBootLogProvider.overrideWithValue((_) {}),
            appInitRunnerProvider.overrideWithValue(() async {
              attempts += 1;
              throw const FrbError.alreadyRunning(pid: 4242);
            }),
            sharedStateVersionLoaderProvider.overrideWithValue(
              () => const SharedStateVersionsDto(),
            ),
            tweaksLoaderProvider.overrideWithValue(
              () => _tweaks(theme: 'Light'),
            ),
          ],
          child: const UpegApp(),
        ),
      );
      await tester.pump();

      expect(attempts, 1);
      expect(find.textContaining('upeg is already running'), findsOneWidget);
      expect(find.textContaining('PID 4242'), findsOneWidget);
      await tester.pump(const Duration(seconds: 1));
      expect(attempts, 1);
    });

    testWidgets('UpegApp_transitions_to_BoardPage_on_AppInitReport_data', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          initState: const AsyncValue<AppInitReport>.data(_fakeReport),
          extraOverrides: _boardPageOverrides(),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(SplashPage), findsNothing);
      expect(find.textContaining('boot failed'), findsNothing);
      expect(find.byType(BoardPage), findsOneWidget);
    });

    testWidgets('UpegApp_first_frame_paints_the_saved_Dark_theme_and_accent', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          initState: const AsyncValue<AppInitReport>.loading(),
          extraOverrides: [
            tweaksLoaderProvider.overrideWithValue(
              () => _tweaks(theme: 'Dark', accent: 'Amber'),
            ),
          ],
        ),
      );

      // No further pump: init is still pending and the first frame has
      // already used the saved values (no loading frame with the defaults).
      expect(find.byType(SplashPage), findsOneWidget);
      final app = tester.widget<MaterialApp>(find.byType(MaterialApp));
      expect(app.themeMode, ThemeMode.dark);
      expect(
        app.theme!.colorScheme.primary,
        UpegTheme.forAccent(
          Accent.amber,
          brightness: Brightness.light,
        ).colorScheme.primary,
      );
    });

    testWidgets('UpegApp_first_frame_paints_the_saved_Light_theme', (
      tester,
    ) async {
      await tester.pumpWidget(
        _harness(
          initState: const AsyncValue<AppInitReport>.loading(),
          extraOverrides: [
            tweaksLoaderProvider.overrideWithValue(
              () => _tweaks(theme: 'Light'),
            ),
          ],
        ),
      );

      expect(
        tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
        ThemeMode.light,
      );
    });

    testWidgets('UpegApp_takes_the_version_baseline_before_any_other_read', (
      tester,
    ) async {
      final calls = <String>[];
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            appBootLogProvider.overrideWithValue((_) {}),
            appInitRunnerProvider.overrideWithValue(() {
              calls.add('initApp');
              return Completer<AppInitReport>().future;
            }),
            sharedStateVersionLoaderProvider.overrideWithValue(() {
              calls.add('versions');
              return const SharedStateVersionsDto();
            }),
            tweaksLoaderProvider.overrideWithValue(() {
              calls.add('tweaks');
              return _tweaks(theme: 'Light');
            }),
          ],
          child: const UpegApp(),
        ),
      );

      // A change another surface makes while this app boots must not be
      // absorbed into the baseline, so the probe precedes initApp and the
      // first shared-state read.
      expect(calls.first, 'versions');
      expect(calls, containsAll(<String>['initApp', 'tweaks']));
    });

    testWidgets('UpegApp_starts_the_polling_timer_only_after_init_succeeds', (
      tester,
    ) async {
      var versionLoads = 0;
      final initGate = Completer<AppInitReport>();
      await tester.pumpWidget(
        _bootHarness(
          initGate: initGate,
          versionLoader: () {
            versionLoads += 1;
            return const SharedStateVersionsDto();
          },
          tweaksLoader: () => _tweaks(theme: 'Light'),
        ),
      );
      await tester.pump(kSharedStatePollInterval * 3);
      expect(
        versionLoads,
        1,
        reason: 'only the baseline while init is pending',
      );

      initGate.complete(_fakeReport);
      await tester.pump();
      final loadsAtStart = versionLoads;
      await tester.pump(kSharedStatePollInterval);

      expect(versionLoads, greaterThan(loadsAtStart));
    });

    testWidgets(
      'UpegApp_applies_a_tweaks_change_made_during_boot_once_polling_starts',
      (tester) async {
        var tweaksVersion = BigInt.one;
        var tweaksLoads = 0;
        final initGate = Completer<AppInitReport>();
        await tester.pumpWidget(
          _bootHarness(
            initGate: initGate,
            versionLoader: () => SharedStateVersionsDto(tweaks: tweaksVersion),
            tweaksLoader: () {
              tweaksLoads += 1;
              return _tweaks(theme: 'Light');
            },
          ),
        );
        await tester.pump();
        expect(tweaksLoads, 1);

        // Another surface saves tweaks while initApp is still running.
        tweaksVersion = BigInt.two;
        await tester.pump(kSharedStatePollInterval * 3);
        expect(tweaksLoads, 1, reason: 'nothing polls before init succeeds');

        initGate.complete(_fakeReport);
        await tester.pump();
        await tester.pump(kSharedStatePollInterval);
        // One more frame: the invalidated provider rebuilds when it is read.
        await tester.pump();

        expect(tweaksLoads, 2);
      },
    );
  });

  group('installDesktopIntegrations', () {
    testWidgets(
      'should finish boot while summon registrations are pending and preserve deep-link order',
      (tester) async {
        final tray = Completer<void>();
        final hotkey = Completer<void>();
        final embed = Completer<void>();
        final seed = Completer<void>();
        final listener = Completer<void>();
        final steps = <String>[];
        var ready = false;

        final boot = installDesktopIntegrations(
          prepareControlledEmbed: () {
            steps.add('embed');
            return embed.future;
          },
          installTray: () {
            steps.add('tray');
            return tray.future;
          },
          installGlobalHotkey: () {
            steps.add('hotkey');
            return hotkey.future;
          },
          seedLaunchIntent: () {
            steps.add('seed');
            return seed.future;
          },
          installDeepLinkListener: () {
            steps.add('listener');
            return listener.future;
          },
          log: (_) {},
        ).then((_) => ready = true);

        expect(steps, ['tray', 'hotkey', 'embed']);
        expect(ready, isFalse);
        embed.complete();
        await tester.pump();
        expect(steps.last, 'seed');
        expect(ready, isFalse);
        seed.complete();
        await tester.pump();
        expect(steps.last, 'listener');
        expect(ready, isFalse);
        listener.complete();
        await boot;

        expect(ready, isTrue);
        expect(tray.isCompleted, isFalse);
        expect(hotkey.isCompleted, isFalse);
        tray.complete();
        hotkey.complete();
        await tester.pump();
      },
    );

    test('should contain background registration errors after boot', () async {
      final tray = Completer<void>();
      final hotkey = Completer<void>();
      final logs = <String>[];
      await installDesktopIntegrations(
        prepareControlledEmbed: null,
        installTray: () => tray.future,
        installGlobalHotkey: () => hotkey.future,
        seedLaunchIntent: () async {},
        installDeepLinkListener: () async {},
        log: logs.add,
      );

      tray.completeError(StateError('tray unavailable'));
      hotkey.completeError(StateError('hotkey unavailable'));
      await Future<void>.delayed(Duration.zero);

      expect(
        logs,
        contains('upeg: UpegTray.install failed: Bad state: tray unavailable'),
      );
      expect(
        logs,
        contains(
          'upeg: UpegGlobalHotkey.install failed: Bad state: hotkey unavailable',
        ),
      );
    });

    testWidgets('should cap both background waits without delaying boot', (
      tester,
    ) async {
      final logs = <String>[];
      await installDesktopIntegrations(
        prepareControlledEmbed: null,
        installTray: () => Completer<void>().future,
        installGlobalHotkey: () => Completer<void>().future,
        seedLaunchIntent: () async {},
        installDeepLinkListener: () async {},
        log: logs.add,
      );
      expect(logs.where((line) => line.contains('timed out')), isEmpty);

      await tester.pump(const Duration(seconds: 2));

      expect(logs.where((line) => line.contains('timed out')), hasLength(2));
      expect(logs.any((line) => line.contains('UpegTray.install ok')), isFalse);
      expect(logs.any((line) => line.contains('requested')), isFalse);
    });
  });

  group('runBestEffortIntegration', () {
    test('a_call_that_finishes_in_time_logs_ok', () async {
      final logs = <String>[];

      await runBestEffortIntegration(
        'Thing.install',
        () async {},
        log: logs.add,
      );

      expect(logs, ['upeg: Thing.install ok']);
    });

    test('a_call_that_finishes_in_time_logs_the_given_success_label', () async {
      final logs = <String>[];

      await runBestEffortIntegration(
        'Thing.install',
        () async {},
        successLabel: 'requested',
        log: logs.add,
      );

      expect(logs, ['upeg: Thing.install requested']);
    });

    test(
      'a_call_that_outlives_the_cap_logs_timed_out_and_no_success_label',
      () async {
        final logs = <String>[];
        final neverCompletes = Completer<void>();

        await runBestEffortIntegration(
          'Thing.install',
          () => neverCompletes.future,
          successLabel: 'requested',
          timeout: const Duration(milliseconds: 10),
          log: logs.add,
        );

        expect(logs, hasLength(1));
        expect(logs.single, contains('Thing.install timed out'));
        expect(logs.single, isNot(contains(' ok')));
        expect(logs.single, isNot(contains('requested')));
      },
    );

    test(
      'a_call_that_throws_logs_failed_and_the_stack_but_no_success',
      () async {
        final logs = <String>[];

        await runBestEffortIntegration(
          'Thing.install',
          () async => throw StateError('boom'),
          log: logs.add,
        );

        expect(logs, hasLength(2), reason: 'the failure line, then the stack');
        expect(logs.first, 'upeg: Thing.install failed: Bad state: boom');
        expect(logs.any((line) => line.endsWith(' ok')), isFalse);
      },
    );
  });
}
