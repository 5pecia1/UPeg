/// Widget tests for [StatusBar].
///
/// Drives the bar against a synthetic Riverpod scope so the FRB calls
/// are never hit. We verify that the board title + pinned-count text
/// reflects the currently-selected board.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/status_bar.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

import '../test_helpers/i18n_test_catalog.dart';

const BoardDto _dev = BoardDto(key: 'dev', title: 'Dev');
const BoardDto _prod = BoardDto(key: 'prod', title: 'Prod');

const PlacementDto _placement = PlacementDto(
  toolId: 'num.hex_to_decimal',
  x: 0,
  y: 0,
  w: 1,
  h: 1,
);

const StatusSnapshotDto _defaultStatus = StatusSnapshotDto(
  network: NetworkStatusDto(
    reachability: NetworkReachabilityDto.loopbackOnly,
    label: 'loopback-only',
  ),
  paused: PausedStateDto.running,
  mcpImportCount: 0,
  mcpImportPhase: McpImportPhaseDto.done,
  buildVersion: '0.0.0',
);

TweaksDto _tweaks(String locale) => TweaksDto(
  theme: 'Dark',
  accent: 'Green',
  showHoles: true,
  locale: locale,
  localHttpHost: false,
);

ProviderContainer _scope({
  required List<BoardDto> boards,
  required Map<String, List<PlacementDto>> layouts,
  String? selectedKey,
  StatusSnapshotDto status = _defaultStatus,
}) {
  final container = ProviderContainer(
    overrides: [
      ...i18nTestOverrides,
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => _tweaks('En'),
      ),
      tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
      ...pegboardSelectionOverrides(boardKey: selectedKey),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      layoutLoaderProvider.overrideWith(
        (ref) =>
            (query) => LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements:
                  layouts[query.boardKey.value] ?? const <PlacementDto>[],
            ),
      ),
      statusSnapshotProvider.overrideWith(() => _FakeStatusNotifier(status)),
    ],
  );
  if (selectedKey != null) {
    container
        .read(currentBoardKeyProvider.notifier)
        .select(BoardKey.parse(selectedKey));
  }
  return container;
}

class _FakeStatusNotifier extends StatusNotifier {
  _FakeStatusNotifier(this._initial);

  final StatusSnapshotDto _initial;

  @override
  StatusSnapshotDto build() => _initial;
}

Widget _harness(ProviderContainer container) {
  return UncontrolledProviderScope(
    container: container,
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: const Scaffold(body: StatusBar()),
    ),
  );
}

void main() {
  group('StatusBar', () {
    testWidgets('StatusBar는_선택된_보드_제목과_pinned_개수를_표시한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev, _prod],
        layouts: const <String, List<PlacementDto>>{
          'dev': <PlacementDto>[_placement, _placement],
        },
        selectedKey: 'dev',
      );
      addTearDown(container.dispose);

      // Pre-resolve futures so the bar reads `.value`.
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('Dev'), findsOneWidget);
      expect(find.text(' · 2 pinned'), findsOneWidget);
    });

    testWidgets('StatusBar는_선택이_없으면_제목_placeholder를_표시한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: null,
      );
      addTearDown(container.dispose);

      await container.read(boardsProvider.future);

      await tester.pumpWidget(_harness(container));
      await tester.pump(); // initial frame
      await tester.pump();

      // Em-dash sentinel from status_bar.dart's `boardTitle = '—'`.
      expect(find.text('—'), findsOneWidget);
      expect(find.text(' · 0 pinned'), findsOneWidget);
    });

    testWidgets('StatusBar는_network_라벨을_provider값으로_렌더한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.remoteBindAllowed,
            label: 'remote-bind allowed',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('remote-bind allowed'), findsOneWidget);
      // The stale hardcoded label must be GONE.
      expect(find.text('loopback-only'), findsNothing);
    });

    testWidgets('상시_상태_텍스트는_3대1_이상_대비_토큰인_fg3를_사용한다', (tester) async {
      // paused 등 상시 노출 상태정보가 fg4(~2.4:1)로 그려지던 회귀 가드 —
      // 대비 하한(minNonTextContrast)을 만족하는 fg3 로 고정한다.
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.paused,
          mcpImportCount: 3,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      final tokens = UpegTheme.darkTheme().extension<UpegTokens>()!;
      for (final label in ['paused', 'imports 3', 'loopback-only', 'v0.0.0']) {
        final text = tester.widget<Text>(find.text(label));
        expect(
          text.style?.color,
          tokens.fg3,
          reason: '"$label" 상태 텍스트는 fg3 여야 한다',
        );
        expect(text.style?.color, isNot(tokens.fg4));
      }
    });

    testWidgets('StatusBar는_paused일때_paused_뱃지를_표시한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.paused,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('paused'), findsOneWidget);
    });

    testWidgets('StatusBar는_running일때_paused_뱃지를_숨긴다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('paused'), findsNothing);
    });

    testWidgets('StatusBar는_MCP_로드_개수를_provider값으로_렌더한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 3,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('imports 3'), findsOneWidget);
    });

    testWidgets('StatusBar는_MCP_로드_개수가_0이면_뱃지를_숨긴다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.textContaining('imports '), findsNothing);
    });

    testWidgets('StatusBar는_임포트_로딩_중이면_로딩_칩을_표시한다', (tester) async {
      // 내장 host는 서빙을 시작한 뒤에 임포트를 로드한다. 그 창 동안
      // 개수 0은 "임포트가 없다"가 아니라 "아직 안 들어왔다"이므로
      // 칩이 그 사실을 말해야 한다 (docs/architecture/mcp.md).
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.loading,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(
        find.text(i18nEn('desktop.status.imports_loading')),
        findsOneWidget,
      );
    });

    testWidgets('StatusBar는_로딩_중이면_개수_대신_로딩_칩을_보여준다', (tester) async {
      // 로딩 중에 보이는 개수는 중간값이다 — 최종 개수인 척하면 안 된다.
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 3,
          mcpImportPhase: McpImportPhaseDto.loading,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(
        find.text(i18nEn('desktop.status.imports_loading')),
        findsOneWidget,
      );
      expect(find.text('imports 3'), findsNothing);
    });

    testWidgets('StatusBar는_로딩이_끝나면_로딩_칩을_치운다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 3,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        ),
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text(i18nEn('desktop.status.imports_loading')), findsNothing);
      expect(find.text('imports 3'), findsOneWidget);
    });

    testWidgets('StatusBar는_2초마다_statusSnapshot을_재호출한다', (tester) async {
      // Counter-backed reader: every Notifier tick bumps the counter
      // and renders `imports $calls` so the assertion is a pure DOM read.
      var calls = 0;
      StatusSnapshotDto reader() {
        calls++;
        return StatusSnapshotDto(
          network: const NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: calls,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.0.0',
        );
      }

      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          boardsLoaderProvider.overrideWith(
            (ref) =>
                () => const <BoardDto>[_dev],
          ),
          layoutLoaderProvider.overrideWith(
            (ref) =>
                (query) => LayoutSnapshotDto(
                  boardKey: query.boardKey.value,
                  boardCols: 6,
                  placements: const <PlacementDto>[],
                ),
          ),
          statusReaderProvider.overrideWithValue(reader),
        ],
      );
      // We dispose the container explicitly at the end of the test
      // so the timer cancellation runs INSIDE the test body, before
      // the binding's `!timersPending` invariant fires.
      container
          .read(currentBoardKeyProvider.notifier)
          .select(BoardKey.parse('dev'));
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pump(); // initial frame
      expect(calls, 1);
      expect(find.text('imports 1'), findsOneWidget);

      // Advance fake time past one refresh interval.
      await tester.pump(kStatusRefreshInterval);
      expect(calls, 2);
      expect(find.text('imports 2'), findsOneWidget);

      // Dispose the container so the Notifier's `onDispose` fires
      // and cancels the timer — Flutter's testing harness fails the
      // test otherwise on the `!timersPending` invariant. We replace
      // `addTearDown` here so the dispose runs INSIDE the test body
      // (before the binding's invariant check), not after it.
      container.dispose();
      await tester.pump();
    });

    testWidgets('StatusBar는_buildVersion을_provider값으로_렌더한다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{},
        selectedKey: 'dev',
        status: const StatusSnapshotDto(
          network: NetworkStatusDto(
            reachability: NetworkReachabilityDto.loopbackOnly,
            label: 'loopback-only',
          ),
          paused: PausedStateDto.running,
          mcpImportCount: 0,
          mcpImportPhase: McpImportPhaseDto.done,
          buildVersion: '0.42.1',
        ),
      );
      addTearDown(container.dispose);

      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      expect(find.text('v0.42.1'), findsOneWidget);
    });

    testWidgets('로케일을_한국어로_바꾸면_상태바_문구가_실시간으로_한국어가_된다', (tester) async {
      final container = _scope(
        boards: const <BoardDto>[_dev],
        layouts: const <String, List<PlacementDto>>{
          'dev': <PlacementDto>[_placement, _placement],
        },
        selectedKey: 'dev',
      );
      addTearDown(container.dispose);
      await container.read(boardsProvider.future);
      await container.read(
        layoutProvider(LayoutQuery.all(BoardKey.parse('dev'))).future,
      );
      await container.read(tweaksProvider.future);

      await tester.pumpWidget(_harness(container));
      await tester.pumpAndSettle();

      // English first — the catalog row, not a re-typed literal.
      expect(find.text(i18nEn('desktop.status.board_prefix')), findsOneWidget);
      expect(
        find.text(i18nEn('desktop.status.pinned_count', {'count': '2'})),
        findsOneWidget,
      );

      // Live flip: saving Ko tweaks re-renders without a remount (K05).
      await container.read(tweaksProvider.notifier).save(_tweaks('Ko'));
      await tester.pumpAndSettle();

      expect(find.text(i18nEn('desktop.status.board_prefix')), findsNothing);
      expect(find.text(i18nKo('desktop.status.board_prefix')), findsOneWidget);
      expect(
        find.text(i18nKo('desktop.status.pinned_count', {'count': '2'})),
        findsOneWidget,
      );
    });
  });
}
