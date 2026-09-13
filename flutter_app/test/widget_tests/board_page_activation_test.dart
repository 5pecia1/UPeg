/// Embed pin activation awaits catalog before opening.
///
/// When a user taps an embed pin on the board, we must ensure the tool
/// catalog has loaded before resolving the tool via `toolByIdProvider`.
/// On cold boot, `toolsProvider` (a FutureProvider) may still be loading
/// when the activation runs, causing a valid embed tool to be silently
/// dropped. The activation handler now awaits `toolsProvider.future`
/// before resolving the tool, mirroring the pattern in
/// `LaunchIntentApplier._openEmbedForToolId` and
/// `PendingActivationBridge._openModalForToolId`.
///
/// Regression test for PR#24 review comment:
/// https://github.com/5pecia1/UPeg/pull/24#discussion_r3317956953
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/pages/board_page.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/boot.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pause.dart';
import 'package:upeg/src/rust/api/status.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/focused_pin_provider.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/pegboard_selection_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/state/status_provider.dart' show statusReaderProvider;
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';
import 'package:upeg/src/widgets/pin.dart';

import '../test_helpers/tool_fixture.dart';

const _testBoardKey = 'dev';
const _testBoardCols = 6;

final _testBoard = BoardDto(key: _testBoardKey, title: 'Dev');

String _fakeTranslate(String key, LocaleDto locale) => key;

String _fakeTranslateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) => key;

const _testStatus = StatusSnapshotDto(
  network: NetworkStatusDto(
    reachability: NetworkReachabilityDto.offline,
    label: 'offline',
  ),
  paused: PausedStateDto.running,
  mcpImportCount: 0,
  mcpImportPhase: McpImportPhaseDto.notStarted,
  buildVersion: 'test',
);

ToolDto _embedTool(String id) => fixtureToolDto(
  id: id,
  toolkit: 'embed',
  label: 'Test Embed',
  description: 'Test embed tool',
  pinKind: PinKindDto.embed,
  invoker: InvokerDto.embed,
  pegboardUnits: PegboardUnitsDto.u2,
);

LayoutSnapshotDto _singlePlacement(String toolId) => LayoutSnapshotDto(
  boardKey: _testBoardKey,
  boardCols: _testBoardCols,
  placements: [PlacementDto(toolId: toolId, x: 0, y: 0, w: 2, h: 1)],
);

Widget _boardHarness({
  required ToolsLoader toolsLoader,
  required LayoutSnapshotDto snapshot,
  PinActivationFn? activation,
  InlineDraftStore? inlineDraftStore,
  LiveDispatchFn? liveDispatch,
}) {
  return ProviderScope(
    overrides: [
      i18nTranslateOverride.overrideWithValue(_fakeTranslate),
      i18nTranslateArgsOverride.overrideWithValue(_fakeTranslateArgs),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => [_testBoard],
      ),
      pegboardSelectionLoaderProvider.overrideWith(
        (ref) =>
            () =>
                const PegboardSelectionDto(boardKey: _testBoardKey, tag: 'all'),
      ),
      pegboardSelectionSaverProvider.overrideWith((ref) => (selection) {}),
      pegboardSelectionTagOptionsLoaderProvider.overrideWith(
        (ref) =>
            (boardKey) => const ['all'],
      ),
      tagOptionsLoaderProvider.overrideWith(
        (ref) =>
            () => const ['all'],
      ),
      tagCountLoaderProvider.overrideWith(
        (ref) =>
            (selection) => 0,
      ),
      boardTagOptionsLoaderProvider.overrideWith(
        (ref) =>
            (boardKey) => const ['all'],
      ),
      boardTagCountLoaderProvider.overrideWith(
        (ref) =>
            (boardKey, selection) => snapshot.placements.length,
      ),
      layoutLoaderProvider.overrideWith(
        (ref) =>
            (query) => snapshot,
      ),
      toolsLoaderProvider.overrideWith((ref) => toolsLoader),
      pinActivationProvider.overrideWith(
        (ref) =>
            activation ??
            ({required toolId, required argsJson}) =>
                PinActivationDto_OpenEmbed(toolId: toolId.value),
      ),
      if (liveDispatch != null)
        liveDispatchToolFnProvider.overrideWithValue(liveDispatch),
      resolveEmbedFnProvider.overrideWith(
        (ref) =>
            ({required toolId, required args}) => null,
      ),
      statusReaderProvider.overrideWith(
        (ref) =>
            () => _testStatus,
      ),
      if (inlineDraftStore != null)
        inlineDraftProvider.overrideWithValue(inlineDraftStore),
    ],
    child: MaterialApp(
      home: BoardPage(
        report: const AppInitReport(
          launch: DesktopLaunchDto(board: _testBoardKey),
          hostState: HostStateDto.noHost(),
          version: 'test',
        ),
      ),
    ),
  );
}

void _useWideSurface(WidgetTester tester) {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

void main() {
  group('Embed pin activation catalog await (PR#24)', () {
    test('카탈로그가_로드되지_않았을_때_toolByIdProvider는_null을_반환한다', () async {
      // When tools haven't loaded yet, toolByIdProvider returns null.
      final container = ProviderContainer(
        overrides: [
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => [
                  fixtureToolDto(id: 'num.hex_to_decimal', toolkit: 'convert'),
                ],
          ),
        ],
      );
      addTearDown(container.dispose);

      // First, verify tool is not found synchronously (catalog not loaded)
      final toolBefore = container.read(
        toolByIdProvider(ToolId.parse('num.hex_to_decimal')),
      );
      expect(toolBefore, isNull);
    });

    test(
      'toolsProvider_future를_await하면_이후_toolByIdProvider가_tool을_찾는다',
      () async {
        // After awaiting toolsProvider.future, toolByIdProvider finds the tool.
        final container = ProviderContainer(
          overrides: [
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [
                    fixtureToolDto(
                      id: 'num.hex_to_decimal',
                      toolkit: 'convert',
                    ),
                  ],
            ),
          ],
        );
        addTearDown(container.dispose);

        // First, verify tool is not found synchronously
        var tool = container.read(
          toolByIdProvider(ToolId.parse('num.hex_to_decimal')),
        );
        expect(tool, isNull);

        // Await the catalog to load
        await container.read(toolsProvider.future);

        // Now the tool should be found
        tool = container.read(
          toolByIdProvider(ToolId.parse('num.hex_to_decimal')),
        );
        expect(tool, isNotNull);
        expect(tool!.id, 'num.hex_to_decimal');
      },
    );

    test('빈_카탈로그에서는_존재하지_않는_tool이_await후에도_null을_반환한다', () async {
      // Even after loading an empty catalog, unknown tools remain null.
      final container = ProviderContainer(
        overrides: [
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => [],
          ),
        ],
      );
      addTearDown(container.dispose);

      // Await the catalog to load (empty)
      await container.read(toolsProvider.future);

      // Unknown tool is still null
      final tool = container.read(
        toolByIdProvider(ToolId.parse('nonexistent.tool')),
      );
      expect(tool, isNull);
    });
  });

  group('BoardPage embed activation (widget tests)', () {
    testWidgets('보드에_핀된_embed는_인라인_핀으로_포커스된다', (tester) async {
      // E3: an embed tool pinned on the visible board is already rendered
      // inline. Activation focuses that pin instead of pushing a
      // full-screen EmbedPage (inline-canonical, no activation step).
      _useWideSurface(tester);
      final tool = _embedTool('embed.on_board');

      await tester.pumpWidget(
        _boardHarness(
          toolsLoader: () => [tool],
          snapshot: _singlePlacement(tool.id),
          activation: ({required toolId, required argsJson}) =>
              PinActivationDto_OpenEmbed(toolId: toolId.value),
        ),
      );
      await tester.pump();
      await tester.pump();

      expect(find.byType(Pin), findsOneWidget);

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      // Inline surface — no full-screen EmbedPage was pushed.
      expect(find.byType(EmbedPage), findsNothing);

      // The inline pin is focused.
      final container = ProviderScope.containerOf(
        tester.element(find.byType(BoardPage)),
      );
      expect(container.read(focusedPinProvider), equals(ToolId.parse(tool.id)));
    });

    testWidgets('핀되지_않은_embed는_전체화면으로_열린다', (tester) async {
      // E3: a tool NOT pinned on the visible board (off-board entry) still
      // opens the full-screen EmbedPage after the catalog loads. The
      // activation targets the off-board tool regardless of which pin was
      // tapped, modelling a palette / deep-link hit.
      _useWideSurface(tester);
      final onBoard = _embedTool('embed.on_board');
      final offBoard = _embedTool('embed.off_board');

      await tester.pumpWidget(
        _boardHarness(
          toolsLoader: () => [onBoard, offBoard],
          snapshot: _singlePlacement(onBoard.id),
          activation: ({required toolId, required argsJson}) =>
              PinActivationDto_OpenEmbed(toolId: offBoard.id),
        ),
      );
      await tester.pump();
      await tester.pump();

      expect(find.byType(Pin), findsOneWidget);
      expect(find.byType(EmbedPage), findsNothing);

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      // Off-board tool → full-screen EmbedPage (catalog-await path intact).
      expect(find.byType(EmbedPage), findsOneWidget);
    });

    testWidgets('핀되지_않은_미지_도구는_스낵바만_표시한다', (tester) async {
      // Off-board activation of a tool missing from the catalog surfaces a
      // "not found" snackbar (catalog-await regression coverage) and does
      // NOT open EmbedPage.
      _useWideSurface(tester);
      const missingToolId = 'missing.tool';
      final onBoard = _embedTool('embed.on_board');

      await tester.pumpWidget(
        _boardHarness(
          toolsLoader: () => [onBoard],
          snapshot: _singlePlacement(onBoard.id),
          activation: ({required toolId, required argsJson}) =>
              PinActivationDto_OpenEmbed(toolId: missingToolId),
        ),
      );
      await tester.pump();
      await tester.pump();

      expect(find.byType(Pin), findsOneWidget);

      await tester.tap(find.byType(Pin));
      await tester.pumpAndSettle();

      expect(find.byType(EmbedPage), findsNothing);
      expect(find.text('Tool not found: $missingToolId'), findsOneWidget);
    });

    testWidgets('현재 보드의 inline draft만 expanded modal 입력으로 넘긴다', (tester) async {
      _useWideSurface(tester);
      final tool = fixtureToolDto(
        id: 'demo.modal_seed',
        label: 'Modal seed',
        source: const SourceDto.manual(),
        inputFields: const <InputFieldDto>[
          InputFieldDto(
            key: 'value',
            label: 'Value',
            fieldType: InputFieldType.text(),
            required_: true,
          ),
        ],
      );
      final toolId = ToolId.parse(tool.id);
      final drafts = InlineDraftStore()
        ..set(
          (BoardKey.parse(_testBoardKey), toolId),
          ToolArgs.fromJsonObject(const <String, Object?>{
            'value': 'current board value',
          }),
        )
        ..set(
          (BoardKey.parse('other'), toolId),
          ToolArgs.fromJsonObject(const <String, Object?>{
            'value': 'other board value',
          }),
        );

      await tester.pumpWidget(
        _boardHarness(
          toolsLoader: () => <ToolDto>[tool],
          snapshot: _singlePlacement(tool.id),
          inlineDraftStore: drafts,
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(GenericInlinePinBody), findsOneWidget);
      final pin = tester.widget<Pin>(find.byType(Pin));
      pin.onTap!.call(pin.placement);
      await tester.pumpAndSettle();

      final modal = find.byType(ExpandedModalPage);
      expect(modal, findsOneWidget);
      final initialInput = tester
          .widget<ExpandedModalPage>(modal)
          .initialInput!
          .toJsonObject();
      expect(initialInput['value'], 'current board value');
      expect(initialInput['value'], isNot('other board value'));
    });

    testWidgets('즉시 실행이 동기 예외를 던져도 실행 중 상태를 해제한다', (tester) async {
      _useWideSurface(tester);
      // `PinKindDto.action` (not `.inline`) deliberately keeps this pin
      // OUTSIDE the generic-inline-body gate in board_canvas.dart: an
      // Inline+Function tool's tap now opens the expanded modal (the
      // inline body owns its own Run affordance), so a plain
      // tap-to-dispatch-immediately pin — which is what this test
      // exercises — needs a pin kind the generic-inline path never claims.
      final tool = fixtureToolDto(
        id: 'fixture.sync_failure',
        pinKind: PinKindDto.action,
      );
      final toolId = ToolId.parse(tool.id);

      await tester.pumpWidget(
        _boardHarness(
          toolsLoader: () => <ToolDto>[tool],
          snapshot: _singlePlacement(tool.id),
          activation: ({required toolId, required argsJson}) =>
              PinActivationDto_DispatchImmediate(toolId: toolId.value),
          liveDispatch: ({required toolId, required args}) {
            throw StateError('synchronous dispatch failure');
          },
        ),
      );
      await tester.pumpAndSettle();
      final container = ProviderScope.containerOf(
        tester.element(find.byType(BoardPage)),
      );
      final pin = tester.widget<Pin>(find.byType(Pin));
      final uncaughtError = Completer<Object>();

      runZonedGuarded(
        () => pin.onTap!.call(pin.placement),
        (error, _) => uncaughtError.complete(error),
      );

      expect(await uncaughtError.future, isA<StateError>());
      expect(container.read(runningToolsProvider), isNot(contains(toolId)));
    });
  });
}
