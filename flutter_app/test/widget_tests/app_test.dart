/// Widget tests for [UpegApp].
///
/// `appInitProvider` is overridden to drive each [UpegApp] routing branch
/// (loading / error / data) without crossing the FFI boundary. Its provider
/// logging branch is exercised with a synthetic [AppInitReport] and an
/// overridden desktop integration installer.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/app.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/platform/tray.dart' show TrayMenuSync;
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/suggestions_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';

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
    tagCountLoaderProvider.overrideWithValue((selection) => 0),
    boardTagCountLoaderProvider.overrideWithValue((_, selection) => 0),
    toolsLoaderProvider.overrideWithValue(() => const []),
    suggestionsLoaderProvider.overrideWithValue((boardKey) => const []),
  ];
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
          'upeg: appInitProvider — desktop integrations done, painting BoardPage',
        ),
      );
      expect(logs.any((line) => line.contains('ERROR')), isFalse);
    });

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
  });
}
