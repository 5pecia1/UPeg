/// Widget tests for the popup quick-launcher behaviour: inline
/// execution of immediate-dispatch tools (result rendered in place,
/// popup stays open), the full-dashboard handoff for form tools, the
/// F2/copy affordances, and the board-tab grid scoping.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/popup/popup_inline_outcome_provider.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/pause.dart' show PausedStateDto;
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/last_outcome_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart';
import 'package:upeg/src/state/pending_activation_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';

import '../test_helpers/fake_keyboard_resolver.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pegboard_selection_overrides.dart';

class _FixedStatusNotifier extends StatusNotifier {
  @override
  StatusSnapshotDto build() => const StatusSnapshotDto(
    network: NetworkStatusDto(
      reachability: NetworkReachabilityDto.loopbackOnly,
      label: 'loopback-only',
    ),
    paused: PausedStateDto.running,
    mcpImportCount: 0,
    mcpImportPhase: McpImportPhaseDto.notStarted,
    buildVersion: '0.0.0',
  );
}

class _RecordingClipboardWriter extends ClipboardWriter {
  final List<String> written = [];

  @override
  Future<void> write(String text) async {
    written.add(text);
  }
}

class _RecordingWindowHider extends WindowHider {
  int hideCount = 0;

  @override
  Future<void> hide() async {
    hideCount += 1;
  }
}

PaletteHit _hit(String id, [String? label]) => PaletteHit(
  id: id,
  label: label ?? id,
  description: '',
  score: 1.0,
  pinKind: PinKindDto.inline,
);

CanonicalToolResult _okResult(String text) => CanonicalToolResult(
  ok: true,
  primaryOutputId: 'out',
  outputs: [
    CanonicalOutputEntry(
      id: 'out',
      kind: 'string',
      value: CanonicalOutputValue.string(value: text),
    ),
  ],
);

const CanonicalToolResult _errorResult = CanonicalToolResult(
  ok: false,
  outputs: [],
  error: CanonicalToolError(code: 'boom', message: 'it broke'),
);

/// Immediate-dispatch verdict for every tool — the inline-run harness
/// default. Tests that need the form path override per-tool.
Override _dispatchImmediateVerdict() => pinActivationProvider.overrideWith(
  (ref) =>
      ({required toolId, required argsJson}) =>
          PinActivationDto.dispatchImmediate(toolId: toolId.value),
);

Override _fakeDispatch(
  Future<CanonicalToolResult> Function(ToolId toolId) dispatch, {
  List<String>? calls,
}) => liveDispatchToolFnProvider.overrideWith(
  (ref) => ({required toolId, required args}) {
    calls?.add(toolId.value);
    return dispatch(toolId);
  },
);

ProviderContainer _container({
  List<PaletteHit> catalogue = const <PaletteHit>[],
  List<PaletteHit> Function(String query)? searcher,
  List<BoardDto> boards = const <BoardDto>[],
  List<Override> extraOverrides = const <Override>[],
}) {
  final container = ProviderContainer(
    overrides: [
      ...i18nTestOverrides,
      ...pegboardSelectionOverrides(boardKey: 'dev'),
      statusSnapshotProvider.overrideWith(_FixedStatusNotifier.new),
      fakeKeyboardResolverOverride,
      paletteSearcherProvider.overrideWith(
        (ref) =>
            searcher ??
            (String query) => query.trim().isEmpty ? catalogue : const [],
      ),
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
      windowModeProvider.overrideWith(
        () => WindowModeNotifier(initial: WindowMode.popup),
      ),
      ...extraOverrides,
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _pumpPopup(WidgetTester tester, ProviderContainer container) {
  return tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: PopupPage()),
    ),
  );
}

void main() {
  group('Popup 인라인 실행', () {
    testWidgets('즉시_dispatch_도구는_popup_안에서_실행되고_인라인_결과를_렌더한다', (tester) async {
      final container = _container(
        catalogue: [_hit('id.uuid_v7', 'UUID v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('0198-uuid')),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('popup-hit-id.uuid_v7')));
      await tester.pumpAndSettle();

      // The popup stays open — no dashboard transition, no pending id.
      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(container.read(pendingActivationProvider), isNull);
      // Inline result rendered in place: OK badge + output preview.
      expect(
        find.byKey(const ValueKey('popup-inline-result-id.uuid_v7')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('popup-inline-ok-id.uuid_v7')),
        findsOneWidget,
      );
      expect(find.text('0198-uuid'), findsOneWidget);
      // Shared cache record — back on the board the pin shows the same
      // result.
      expect(
        container.read(lastOutcomeProvider)[ToolId.parse('id.uuid_v7')],
        isNotNull,
      );
    });

    testWidgets('인라인_실행_실패는_ERROR_배지와_에러_메시지를_렌더한다', (tester) async {
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _errorResult),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('popup-hit-id.uuid_v7')));
      await tester.pumpAndSettle();

      expect(
        find.byKey(const ValueKey('popup-inline-error-id.uuid_v7')),
        findsOneWidget,
      );
      expect(find.text('it broke'), findsOneWidget);
      // Errors stay popup-local: the board cache keeps its ok-only
      // contract.
      expect(container.read(lastOutcomeProvider), isEmpty);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    testWidgets('폼_필요_도구는_기존대로_full_대시보드로_전환한다', (tester) async {
      final dispatched = <String>[];
      final container = _container(
        catalogue: [_hit('num.hex_to_decimal')],
        extraOverrides: [
          // Base harness verdict: OpenModal (form needed).
          pinActivationProvider.overrideWith(
            (ref) =>
                ({required toolId, required argsJson}) =>
                    PinActivationDto.openModal(toolId: toolId.value),
          ),
          _fakeDispatch((_) async => _errorResult, calls: dispatched),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.tap(
        find.byKey(const ValueKey('popup-hit-num.hex_to_decimal')),
      );
      await tester.pumpAndSettle();

      expect(container.read(windowModeProvider), WindowMode.full);
      expect(
        container.read(pendingActivationProvider),
        ToolId.parse('num.hex_to_decimal'),
      );
      expect(dispatched, isEmpty);
      expect(
        find.byKey(const ValueKey('popup-inline-result-num.hex_to_decimal')),
        findsNothing,
      );
    });

    testWidgets('결과_표시_상태에서_같은_툴을_다시_실행하면_결과가_갱신된다', (tester) async {
      var run = 0;
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('run-${++run}')),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      final cell = find.byKey(const ValueKey('popup-hit-id.uuid_v7'));
      await tester.tap(cell);
      await tester.pumpAndSettle();
      expect(find.text('run-1'), findsOneWidget);

      await tester.tap(cell);
      await tester.pumpAndSettle();

      expect(find.text('run-2'), findsOneWidget);
      expect(run, 2);
    });

    testWidgets('copy_버튼은_인라인_결과를_클립보드에_복사한다', (tester) async {
      final writer = _RecordingClipboardWriter();
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('copy-me')),
          clipboardWriterProvider.overrideWithValue(writer),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('popup-hit-id.uuid_v7')));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('popup-inline-copy-id.uuid_v7')),
      );
      await tester.pumpAndSettle();

      expect(writer.written, ['copy-me']);
    });

    testWidgets('F2는_최근_인라인_결과를_클립보드에_복사한다', (tester) async {
      final writer = _RecordingClipboardWriter();
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('f2-copy')),
          clipboardWriterProvider.overrideWithValue(writer),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      // No result yet — F2 must stay unhandled (nothing to copy).
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pump();
      expect(writer.written, isEmpty);

      await tester.tap(find.byKey(const ValueKey('popup-hit-id.uuid_v7')));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.f2);
      await tester.pumpAndSettle();

      expect(writer.written, ['f2-copy']);
      // The popup-local cache marks the run as latest for F2.
      expect(
        container.read(popupInlineOutcomeProvider).lastRun,
        ToolId.parse('id.uuid_v7'),
      );
    });

    testWidgets('Enter_활성화도_즉시_dispatch_도구를_인라인으로_실행한다', (tester) async {
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('enter-run')),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();

      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(find.text('enter-run'), findsOneWidget);
    });
  });

  group('Popup 보드 스코프', () {
    List<Override> scopedOverrides() => [
      _dispatchImmediateVerdict(),
      _fakeDispatch((_) async => _okResult('ok')),
      layoutLoaderProvider.overrideWith(
        (ref) =>
            (query) => LayoutSnapshotDto(
              boardKey: query.boardKey.value,
              boardCols: 6,
              placements: query.boardKey.value == 'dev'
                  ? const [
                      PlacementDto(toolId: 'a.one', x: 0, y: 0, w: 1, h: 1),
                    ]
                  : const [
                      PlacementDto(toolId: 'b.two', x: 0, y: 0, w: 1, h: 1),
                    ],
            ),
      ),
    ];

    const boards = <BoardDto>[
      BoardDto(key: 'dev', title: 'Dev'),
      BoardDto(key: 'media', title: 'Media'),
    ];

    testWidgets('보드탭은_그리드를_선택된_보드의_핀으로_스코프하고_헤더를_노출한다', (tester) async {
      final container = _container(
        catalogue: [_hit('a.one'), _hit('b.two'), _hit('c.three')],
        boards: boards,
        extraOverrides: scopedOverrides(),
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      // Board restore lands on 'dev' → only its pin renders, under the
      // PINNED · Dev header.
      expect(find.byKey(const Key('popup-pinned-header')), findsOneWidget);
      expect(
        find.text(i18nEn('popup.pinned_section', {'board': 'Dev'})),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('popup-hit-a.one')), findsOneWidget);
      expect(find.byKey(const ValueKey('popup-hit-b.two')), findsNothing);
      expect(find.byKey(const ValueKey('popup-hit-c.three')), findsNothing);
    });

    testWidgets('다른_보드탭을_누르면_그리드가_그_보드의_핀으로_바뀐다', (tester) async {
      final container = _container(
        catalogue: [_hit('a.one'), _hit('b.two')],
        boards: boards,
        extraOverrides: scopedOverrides(),
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Media'));
      await tester.pumpAndSettle();

      expect(
        find.text(i18nEn('popup.pinned_section', {'board': 'Media'})),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('popup-hit-b.two')), findsOneWidget);
      expect(find.byKey(const ValueKey('popup-hit-a.one')), findsNothing);
    });

    testWidgets('검색어를_입력하면_보드_스코프_대신_전체_검색_결과를_보여준다', (tester) async {
      final container = _container(
        // Typed queries flow through paletteResultsProvider; use a
        // query-aware searcher so a non-empty query returns a global
        // (non-board) match.
        searcher: (query) => query.trim().isEmpty
            ? [_hit('a.one'), _hit('b.two')]
            : [_hit('b.two')],
        boards: boards,
        extraOverrides: scopedOverrides(),
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      await tester.enterText(
        find.byKey(const Key('popup-search-field')),
        'two',
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('popup-pinned-header')), findsNothing);
      expect(find.byKey(const ValueKey('popup-hit-b.two')), findsOneWidget);
    });
  });

  group('Popup 실행 후 흐름', () {
    testWidgets('인라인_결과_표시_상태에서도_esc는_popup을_닫는다', (tester) async {
      final hider = _RecordingWindowHider();
      final container = _container(
        catalogue: [_hit('id.uuid_v7')],
        extraOverrides: [
          _dispatchImmediateVerdict(),
          _fakeDispatch((_) async => _okResult('done')),
          popupWindowHiderProvider.overrideWithValue(hider),
        ],
      );
      await _pumpPopup(tester, container);
      await tester.pumpAndSettle();

      // Run inline — the popup stays open for follow-up launches.
      await tester.tap(find.byKey(const ValueKey('popup-hit-id.uuid_v7')));
      await tester.pumpAndSettle();
      expect(container.read(windowModeProvider), WindowMode.popup);
      expect(hider.hideCount, 0);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();

      // Esc keeps its close contract: popup-mode close = hide to tray.
      expect(hider.hideCount, 1);
      expect(container.read(windowModeProvider), WindowMode.popup);
    });
  });
}
